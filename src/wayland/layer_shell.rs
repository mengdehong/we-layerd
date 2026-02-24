use std::{
    collections::{BTreeMap, HashMap},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use tracing::{error, info, warn};
use wayland_client::{
    delegate_noop,
    globals::GlobalListContents,
    globals::registry_queue_init,
    protocol::{
        wl_compositor::WlCompositor, wl_output::WlOutput, wl_region::WlRegion, wl_registry,
        wl_surface::WlSurface,
    },
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{Layer, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{Anchor, Event as LayerSurfaceEvent, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};

use crate::{
    config::{CaptureConfig, ScaleMode},
    wayland::{outputs, render_wgpu::WgpuRenderer},
    x11::{capture_xcomposite, window_finder},
};

#[derive(Debug, Clone)]
pub struct LayerRunConfig {
    pub capture_window: Option<u32>,
    pub output_window_map: BTreeMap<String, u32>,
    pub fps_limit: u32,
    pub show_fps: bool,
    pub fps_report_interval_secs: u64,
    pub scale_mode: ScaleMode,
    pub auto_refind_window: bool,
    pub capture_match: CaptureConfig,
    pub wine_pid: Option<u32>,
}

struct OutputSurface {
    name: String,
    surface: WlSurface,
    renderer: Option<WgpuRenderer>,
    capture_window: Option<u32>,
    capturer: Option<capture_xcomposite::XCompositeCapturer>,
    last_refind_attempt: Option<Instant>,
    configured_once: bool,
    render_fail_streak: u64,
}

#[derive(Default)]
struct AppState {
    running: bool,
    outputs: Vec<OutputSurface>,
}

impl Dispatch<ZwlrLayerSurfaceV1, usize> for AppState {
    fn event(
        state: &mut Self,
        layer_surface: &ZwlrLayerSurfaceV1,
        event: LayerSurfaceEvent,
        index: &usize,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            LayerSurfaceEvent::Configure {
                serial,
                width,
                height,
            } => {
                layer_surface.ack_configure(serial);
                if let Some(output) = state.outputs.get_mut(*index) {
                    if !output.configured_once {
                        output.configured_once = true;
                        info!(
                            output = %output.name,
                            serial,
                            width,
                            height,
                            "received initial layer-surface configure"
                        );
                    } else {
                        info!(
                            output = %output.name,
                            serial,
                            width,
                            height,
                            "received layer-surface configure"
                        );
                    }
                    output.surface.commit();
                    if let Some(renderer) = &mut output.renderer {
                        renderer.resize(width.max(1), height.max(1));
                    }
                }
            }
            LayerSurfaceEvent::Closed => {
                warn!(index = *index, "layer surface closed by compositor");
                state.running = false;
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => info!(name, interface, version, "wayland global announced"),
            wl_registry::Event::GlobalRemove { name } => {
                warn!(name, "wayland global removed")
            }
            _ => {}
        }
    }
}

delegate_noop!(AppState: ignore WlCompositor);
delegate_noop!(AppState: ignore WlSurface);
delegate_noop!(AppState: ignore WlOutput);
delegate_noop!(AppState: ignore WlRegion);
delegate_noop!(AppState: ignore ZwlrLayerShellV1);

pub fn run_single_background_surface(run_cfg: LayerRunConfig) -> Result<()> {
    let conn = Connection::connect_to_env().context("failed to connect to Wayland display")?;
    let (globals, mut event_queue) = registry_queue_init::<AppState>(&conn)
        .context("failed to initialize Wayland registry")?;
    let qh = event_queue.handle();

    let compositor: WlCompositor = globals
        .bind(&qh, 4..=6, ())
        .context("failed to bind wl_compositor")?;
    let layer_shell: ZwlrLayerShellV1 = globals
        .bind(&qh, 1..=5, ())
        .context("failed to bind zwlr_layer_shell_v1")?;

    let globals_snapshot = globals.contents().clone_list();
    let output_globals = outputs::output_globals(&globals_snapshot);

    let mut state = AppState {
        running: true,
        outputs: Vec::new(),
    };

    if output_globals.is_empty() {
        warn!("no wl_output globals reported, creating fallback layer surface");
        create_output_surface(
            &mut state,
            &conn,
            &qh,
            &compositor,
            &layer_shell,
            "output-fallback".to_string(),
            None,
            run_cfg.capture_window,
        )?;
    } else {
        for (index, global) in output_globals.iter().enumerate() {
            let output = outputs::bind_output::<AppState>(globals.registry(), &qh, global)?;
            let name = format!("output-{}", global.name);

            let capture_window = if index == 0 {
                run_cfg.capture_window
            } else {
                run_cfg
                    .output_window_map
                    .get(&name)
                    .copied()
                    .or(run_cfg.capture_window)
            };

            create_output_surface(
                &mut state,
                &conn,
                &qh,
                &compositor,
                &layer_shell,
                name,
                Some(&output),
                capture_window,
            )?;
        }
    }

    let _ = event_queue.roundtrip(&mut state);

    let fps = run_cfg.fps_limit.max(1);
    let fps_report_interval = Duration::from_secs(run_cfg.fps_report_interval_secs.max(1));
    let mut fps_window_start = Instant::now();
    let mut fps_frame_count: u64 = 0;
    let mut measured_fps = fps as f32;
    info!(outputs = state.outputs.len(), fps, "wayland multi-output loop started");

    while state.running {
        let mut frame_cache: HashMap<u32, capture_xcomposite::CapturedFrame> = HashMap::new();

        if let Err(err) = event_queue.dispatch_pending(&mut state) {
            error!(error = %err, "wayland event dispatch failed");
        }

        for output in &mut state.outputs {
            if let Some(renderer) = &mut output.renderer {
                renderer.set_scale_mode(run_cfg.scale_mode);
                renderer.set_fps_overlay(measured_fps, run_cfg.show_fps);
                if let Some(window) = output.capture_window {
                    if !frame_cache.contains_key(&window) {
                        if output.capturer.is_none() {
                            match capture_xcomposite::XCompositeCapturer::new(window) {
                                Ok(capturer) => output.capturer = Some(capturer),
                                Err(err) => {
                                    warn!(error = %err, output = %output.name, window, "failed to initialize capturer");
                                }
                            }
                        }

                        if let Some(capturer) = output.capturer.as_mut() {
                            match capturer.capture_frame() {
                                Ok(frame) => {
                                    frame_cache.insert(window, frame);
                                }
                                Err(err) => {
                                    warn!(error = %err, output = %output.name, window, "capture failed for output");
                                    output.capturer = None;

                                    if run_cfg.auto_refind_window {
                                        let now = Instant::now();
                                        let can_refind = output
                                            .last_refind_attempt
                                            .map(|t| now.duration_since(t) >= Duration::from_secs(2))
                                            .unwrap_or(true);
                                        if !can_refind {
                                            continue;
                                        }
                                        output.last_refind_attempt = Some(now);

                                        if let Ok(Some(found)) = window_finder::find_window_for_process(
                                            &run_cfg.capture_match,
                                            run_cfg.wine_pid,
                                        ) {
                                            if output.capture_window != Some(found.window) {
                                                output.capture_window = Some(found.window);
                                                output.capturer = None;
                                                info!(output = %output.name, window = found.window, "rebound output to rediscovered X11 window");
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some(frame) = frame_cache.get(&window) {
                        if let Err(err) = renderer.upload_bgra(
                            frame.width,
                            frame.height,
                            frame.stride,
                            &frame.bgra,
                        ) {
                            warn!(error = %err, output = %output.name, "failed to upload frame");
                        }
                    }
                }

                if let Err(err) = renderer.render() {
                    output.render_fail_streak += 1;
                    if output.render_fail_streak <= 5 || output.render_fail_streak % 120 == 0 {
                        warn!(
                            error = %err,
                            output = %output.name,
                            streak = output.render_fail_streak,
                            configured_once = output.configured_once,
                            "render failed for output"
                        );
                    }
                } else if output.render_fail_streak > 0 {
                    info!(
                        output = %output.name,
                        recovered_after = output.render_fail_streak,
                        "render path recovered"
                    );
                    output.render_fail_streak = 0;
                }
            }
        }

        if run_cfg.show_fps {
            fps_frame_count += 1;
            let elapsed = fps_window_start.elapsed();
            if elapsed >= fps_report_interval {
                let measured = fps_frame_count as f64 / elapsed.as_secs_f64();
                measured_fps = measured as f32;
                info!(
                    measured_fps = format_args!("{measured:.1}"),
                    sample_window_ms = elapsed.as_millis(),
                    "runtime fps"
                );
                fps_window_start = Instant::now();
                fps_frame_count = 0;
            }
        }
    }

    Ok(())
}

pub fn probe_layer_shell_support() -> Result<bool> {
    let conn = Connection::connect_to_env().context("failed to connect to Wayland display")?;
    let (globals, _event_queue) = registry_queue_init::<AppState>(&conn)
        .context("failed to initialize Wayland registry")?;
    let present = globals
        .contents()
        .clone_list()
        .iter()
        .any(|g| g.interface == "zwlr_layer_shell_v1");
    Ok(present)
}

fn create_output_surface(
    state: &mut AppState,
    conn: &Connection,
    qh: &QueueHandle<AppState>,
    compositor: &WlCompositor,
    layer_shell: &ZwlrLayerShellV1,
    name: String,
    wl_output: Option<&WlOutput>,
    capture_window: Option<u32>,
) -> Result<()> {
    let index = state.outputs.len();
    let surface = compositor.create_surface(qh, ());
    let layer_surface = layer_shell.get_layer_surface(
        &surface,
        wl_output,
        Layer::Background,
        format!("we-layerd-{}", index),
        qh,
        index,
    );

    let anchor_all = Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right;
    layer_surface.set_anchor(anchor_all);
    layer_surface.set_exclusive_zone(-1);
    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer_surface.set_size(0, 0);

    let region = compositor.create_region(qh, ());
    surface.set_input_region(Some(&region));
    region.destroy();
    surface.commit();

    let renderer = match WgpuRenderer::new(conn, &surface, 1920, 1080) {
        Ok(renderer) => Some(renderer),
        Err(err) => {
            warn!(error = %err, output = %name, "wgpu initialization failed for output");
            None
        }
    };

    info!(output = %name, ?capture_window, "created layer surface for output");
    state.outputs.push(OutputSurface {
        name,
        surface,
        renderer,
        capture_window,
        capturer: None,
        last_refind_attempt: None,
        configured_once: false,
        render_fail_streak: 0,
    });

    Ok(())
}
