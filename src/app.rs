use std::path::Path;

use anyhow::Result;
use tracing::{info, warn};

use crate::{
    config::Config,
    wayland,
    wine::launcher::WineProcessHandle,
    x11::{capture_xcomposite, window_finder},
};

pub fn run(config_path: Option<&Path>) -> Result<()> {
    let cfg = Config::load(config_path)?;
    let mut capture_match = cfg.capture.clone();
    if capture_match.title_contains.is_none() {
        if let Some(title_hint) = extract_play_in_window_hint(&cfg.wine.args) {
            info!(title = %title_hint, "auto-derived capture.title_contains from wine args");
            capture_match.title_contains = Some(title_hint);
        }
    }
    info!(?cfg, ?capture_match, "starting we-layerd run mode");
    let wine = WineProcessHandle::spawn(&cfg.wine)?;
    wine.install_ctrlc_handler()?;
    wine.install_exit_monitor(cfg.wine.clone(), cfg.general.restart_wine_on_exit);
    let wine_pid = wine.pid();
    info!(pid = wine_pid, "wine launcher enabled");

    let capture_window = match window_finder::find_window_for_process(&capture_match, wine_pid)? {
        Some(found) => {
            info!(window = found.window, scanned = found.scanned_windows, "using X11 window");
            if let Some(path) = cfg.capture.debug_save_frame_png.as_deref() {
                let frame = capture_xcomposite::capture_single_frame(found.window)?;
                capture_xcomposite::save_frame_png(&frame, Path::new(path))?;
                info!(path, "saved debug XComposite frame");
            }
            Some(found.window)
        }
        None => {
            warn!("no X11 window found yet, continuing with Wayland layer loop");
            None
        }
    };

    wayland::layer_shell::run_single_background_surface(wayland::layer_shell::LayerRunConfig {
        capture_window,
        output_window_map: cfg.capture.output_window_map.clone(),
        fps_limit: cfg.general.fps_limit,
        show_fps: cfg.general.show_fps,
        fps_report_interval_secs: cfg.general.fps_report_interval_secs,
        auto_refind_window: cfg.general.refind_window_on_capture_error,
        capture_match,
        wine_pid,
    })
}

pub fn doctor() -> Result<()> {
    for key in ["WAYLAND_DISPLAY", "DISPLAY"] {
        match std::env::var(key) {
            Ok(value) => info!(%key, %value, "environment variable set"),
            Err(_) => warn!(%key, "environment variable not set"),
        }
    }

    match capture_xcomposite::probe_xcomposite_support() {
        Ok(()) => info!("XComposite extension probe: OK"),
        Err(err) => warn!(error = %err, "XComposite extension probe failed"),
    }

    match wayland::layer_shell::probe_layer_shell_support() {
        Ok(true) => info!("zwlr_layer_shell_v1 global is available"),
        Ok(false) => warn!("zwlr_layer_shell_v1 global not found on this compositor"),
        Err(err) => warn!(error = %err, "failed to query Wayland layer-shell support"),
    }

    Ok(())
}

fn extract_play_in_window_hint(args: &[String]) -> Option<String> {
    let idx = args.iter().position(|arg| arg == "-playInWindow")?;
    args.get(idx + 1).cloned().filter(|s| !s.trim().is_empty())
}
