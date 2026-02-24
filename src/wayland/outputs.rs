use anyhow::Result;
use wayland_client::{globals::Global, protocol::wl_output::WlOutput, QueueHandle};

pub fn output_globals(globals: &[Global]) -> Vec<Global> {
    globals
        .iter()
        .filter(|g| g.interface == "wl_output")
        .cloned()
        .collect()
}

pub fn bind_output<State>(
    registry: &wayland_client::protocol::wl_registry::WlRegistry,
    qh: &QueueHandle<State>,
    global: &Global,
) -> Result<WlOutput>
where
    State: wayland_client::Dispatch<WlOutput, ()> + 'static,
{
    let version = global.version.min(4);
    Ok(registry.bind(global.name, version, qh, ()))
}
