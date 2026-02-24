use std::path::Path;

use anyhow::Result;
use tracing::{info, warn};

use crate::{config::Config, wayland, wine::launcher::WineProcessHandle, x11::window_finder};

pub fn run(config_path: Option<&Path>) -> Result<()> {
    let cfg = Config::load(config_path)?;
    info!(?cfg, "starting we-layerd run mode");
    let _wine = WineProcessHandle::spawn(&cfg.wine)?;
    _wine.install_ctrlc_handler()?;
    let wine_pid = _wine.pid();
    info!(pid = wine_pid, "wine launcher enabled");

    match window_finder::find_window_for_process(&cfg.capture, wine_pid)? {
        Some(found) => info!(window = found.window, scanned = found.scanned_windows, "using X11 window"),
        None => warn!("no X11 window found yet, continuing with Wayland layer loop"),
    }

    wayland::layer_shell::run_single_background_surface()
}

pub fn doctor() {
    for key in ["WAYLAND_DISPLAY", "DISPLAY"] {
        match std::env::var(key) {
            Ok(value) => info!(%key, %value, "environment variable set"),
            Err(_) => warn!(%key, "environment variable not set"),
        }
    }
}
