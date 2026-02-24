use std::path::Path;

use anyhow::Result;
use tracing::{info, warn};

use crate::config::Config;

pub fn run(config_path: Option<&Path>) -> Result<()> {
    let cfg = Config::load(config_path)?;
    info!(?cfg, "starting we-layerd run mode");
    warn!("runtime loop not implemented yet");
    Ok(())
}

pub fn doctor() {
    for key in ["WAYLAND_DISPLAY", "DISPLAY"] {
        match std::env::var(key) {
            Ok(value) => info!(%key, %value, "environment variable set"),
            Err(_) => warn!(%key, "environment variable not set"),
        }
    }
}
