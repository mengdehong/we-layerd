use std::time::{Duration, Instant};

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

use crate::{wayland::render_wgpu::WgpuRenderer, x11::capture_xcomposite};

#[derive(Debug, Clone, Copy)]
pub struct LayerRunConfig {
    pub capture_window: Option<u32>,
    pub fps_limit: u32,
}

#[derive(Default)]
struct AppState {
    running: bool,
    base_surface: Option<WlSurface>,
    renderer: Option<WgpuRenderer>,
    frame_size: (u32, u32),
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for AppState {
    fn event(
        state: &mut Self,
        layer_surface: &ZwlrLayerSurfaceV1,
        event: LayerSurfaceEvent,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            LayerSurfaceEvent::Configure {
                serial,
                width,
                height,
            } => {
                info!(serial, width, height, "layer surface configured");
                layer_surface.ack_configure(serial);
                if let Some(surface) = &state.base_surface {
                    surface.commit();
                }

                let width = width.max(1);
                let height = height.max(1);
                state.frame_size = (width, height);

                if let Some(renderer) = &mut state.renderer {
                    renderer.resize(width, height);
                }
            }
            LayerSurfaceEvent::Closed => {
                warn!("layer surface closed by compositor");
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

    let surface = compositor.create_surface(&qh, ());
    let layer_surface = layer_shell.get_layer_surface(
        &surface,
        None::<&WlOutput>,
        Layer::Background,
        "we-layerd".to_string(),
        &qh,
        (),
    );

    let anchor_all = Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right;
    layer_surface.set_anchor(anchor_all);
    layer_surface.set_exclusive_zone(-1);
    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer_surface.set_size(0, 0);

    let region = compositor.create_region(&qh, ());
    // Empty input region enables click-through background behavior.
    surface.set_input_region(Some(&region));
    region.destroy();

    surface.commit();

    let mut state = AppState {
        running: true,
        base_surface: Some(surface),
        renderer: None,
        frame_size: (1920, 1080),
    };

    if let Some(surface) = &state.base_surface {
        match WgpuRenderer::new(&conn, surface, 1920, 1080) {
            Ok(renderer) => state.renderer = Some(renderer),
            Err(err) => {
                warn!(error = %err, "wgpu initialization failed; continuing without renderer");
            }
        }
    }

    let _ = event_queue.roundtrip(&mut state);

    let fps = run_cfg.fps_limit.max(1);
    let frame_interval = Duration::from_secs_f64(1.0 / fps as f64);

    info!(fps, "wayland layer-shell render loop started");
    while state.running {
        let start = Instant::now();

        if let Err(err) = event_queue.dispatch_pending(&mut state) {
            error!(error = %err, "wayland event dispatch failed");
        }

        if let Some(renderer) = &mut state.renderer {
            if let Some(window) = run_cfg.capture_window {
                match capture_xcomposite::capture_single_frame(window) {
                    Ok(frame) => {
                        if let Err(err) = renderer.upload_rgba(frame.width, frame.height, &frame.rgba) {
                            warn!(error = %err, "failed to upload captured frame");
                        }
                    }
                    Err(err) => {
                        warn!(error = %err, window, "XComposite capture failed for frame");
                    }
                }
            }

            if let Err(err) = renderer.render() {
                warn!(error = %err, "frame rendering failed");
            }
        }

        if let Some(remaining) = frame_interval.checked_sub(start.elapsed()) {
            std::thread::sleep(remaining);
        }
    }

    Ok(())
}
