use std::path::Path;

use anyhow::Result;
use tracing::{info, warn};

use crate::{config::Config, wayland, wine::launcher::WineProcessHandle};

pub fn run(config_path: Option<&Path>) -> Result<()> {
    let cfg = Config::load(config_path)?;
    info!(?cfg, "starting we-layerd run mode");
    let _wine = WineProcessHandle::spawn(&cfg.wine)?;
    _wine.install_ctrlc_handler()?;
    info!(pid = _wine.pid(), "wine launcher enabled");
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
