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

#[derive(Default)]
struct AppState {
    running: bool,
    base_surface: Option<WlSurface>,
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

pub fn run_single_background_surface() -> Result<()> {
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
    };

    info!("wayland layer-shell loop started");
    while state.running {
        if let Err(err) = event_queue.blocking_dispatch(&mut state) {
            error!(error = %err, "wayland event dispatch failed");
            break;
        }
    }

    Ok(())
}
