use std::{
    env,
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Result};
use tracing::{info, warn};

use crate::{
    cgroup::RuntimeCgroup,
    config::{Config, RuntimeMode, RuntimeWallpaperType},
    gnome::{self, ResolvedBackend},
    ipc::{self, ControlCommand},
    wayland,
    wine::launcher::{spawn_transient_command, WineProcessHandle},
    wm_visibility::DebugWindowVisibility,
    x11::{capture_xcomposite, window_finder, window_input},
};
use std::sync::mpsc;

pub fn run(config_path: Option<&Path>) -> Result<()> {
    let cfg = Config::load(config_path)?;
    let runtime_cfg = effective_runtime_config(&cfg);
    let capture_match = runtime_cfg.capture.clone();

    let runtime_cgroup = RuntimeCgroup::new(cfg.cgroup.clone());
    let runtime_cfg_toml = Arc::new(Mutex::new(runtime_cfg.to_toml_pretty()?));
    let current_cfg = Arc::new(Mutex::new(cfg.clone()));
    let (control_tx, control_rx) = mpsc::channel::<ControlCommand>();
    let status_cgroup = runtime_cgroup.clone();
    let debug_visibility = DebugWindowVisibility::new(
        cfg.general.hide_debug_window,
        cfg.general.hidden_workspace_name.clone(),
        &capture_match,
    );
    let handler_visibility = debug_visibility.clone();
    let _control_server = ipc::ControlServer::start(
        control_tx,
        {
            let runtime_cfg_toml = runtime_cfg_toml.clone();
            move || {
                let mut status = runtime_cfg_toml
                    .lock()
                    .map(|guard| guard.clone())
                    .unwrap_or_else(|_| "<status unavailable>".to_string());
                status.push_str("\n\n");
                status.push_str(&status_cgroup.render_status_toml());
                status
            }
        },
        move |cmd| match cmd {
            ControlCommand::HideWindow => {
                handler_visibility.hide()?;
                Ok(true)
            }
            ControlCommand::ShowWindow => {
                handler_visibility.show()?;
                Ok(true)
            }
            _ => Ok(false),
        },
        {
            let runtime_cfg_toml = runtime_cfg_toml.clone();
            let current_cfg = current_cfg.clone();
            move |config_path| {
                let next_cfg = Config::load(Some(config_path))?;
                let current_cfg_value = current_cfg
                    .lock()
                    .map(|guard| guard.clone())
                    .map_err(|_| anyhow!("failed to read current runtime config"))?;
                ensure_switchable_runtime(&current_cfg_value)?;
                switch_wallpaper(&next_cfg)?;
                let next_runtime_cfg = effective_runtime_config(&next_cfg);
                if let Ok(mut guard) = runtime_cfg_toml.lock() {
                    *guard = next_runtime_cfg.to_toml_pretty()?;
                }
                if let Ok(mut guard) = current_cfg.lock() {
                    *guard = next_cfg;
                }
                Ok(())
            }
        },
    )?;

    if let Some(runtime) = &runtime_cfg.runtime {
        if runtime.mode == RuntimeMode::VideoNative {
            return run_video_native(runtime.video_file.as_deref(), &runtime_cfg, &control_rx);
        }
    }

    info!(cfg = ?runtime_cfg, ?capture_match, "starting we-layerd run mode");
    ensure_runtime_access()?;
    let wine = WineProcessHandle::spawn(&runtime_cfg.wine)?;
    wine.install_ctrlc_handler()?;
    let cgroup_on_spawn = runtime_cgroup.clone();
    let on_spawn: Arc<dyn Fn(u32) + Send + Sync> = Arc::new(move |pid| {
        cgroup_on_spawn.on_wine_spawn(pid);
    });
    wine.install_exit_monitor(
        runtime_cfg.wine.clone(),
        runtime_cfg.general.restart_wine_on_exit,
        Some(on_spawn.clone()),
    );
    let wine_pid = wine.pid();
    if let Some(pid) = wine_pid {
        on_spawn(pid);
    }
    info!(pid = wine_pid, "wine launcher enabled");

    let capture_window = match window_finder::find_window_for_process(&capture_match, wine_pid)? {
        Some(found) => {
            info!(window = found.window, scanned = found.scanned_windows, "using X11 window");
            apply_debug_window_setup(&runtime_cfg, &debug_visibility, found.window);
            if let Some(path) = runtime_cfg.capture.debug_save_frame_png.as_deref() {
                let frame = capture_xcomposite::capture_single_frame(found.window)?;
                capture_xcomposite::save_frame_png(&frame, Path::new(path))?;
                info!(path, "saved debug XComposite frame");
            }
            Some(found)
        }
        None => {
            warn!("no X11 window found yet, continuing with Wayland layer loop");
            None
        }
    };

    if matches!(gnome::resolve_backend(&runtime_cfg), ResolvedBackend::GnomeShell) {
        return gnome::run_window_bridge(
            &runtime_cfg,
            &capture_match,
            capture_window,
            wine_pid,
            &control_rx,
        );
    }

    wayland::layer_shell::run_single_background_surface(
        wayland::layer_shell::LayerRunConfig {
            capture_window: capture_window.as_ref().map(|found| found.window),
            output_window_map: runtime_cfg.capture.output_window_map.clone(),
            fps_limit: runtime_cfg.general.fps_limit,
            show_fps: runtime_cfg.general.show_fps,
            fps_report_interval_secs: runtime_cfg.general.fps_report_interval_secs,
            scale_mode: runtime_cfg.general.scale_mode,
            auto_refind_window: runtime_cfg.general.refind_window_on_capture_error,
            capture_match,
            disable_debug_window_input: runtime_cfg.general.disable_debug_window_input,
            wine_pid,
        },
        Some(&control_rx),
    )
}

fn effective_runtime_config(cfg: &Config) -> Config {
    let mut runtime_cfg = cfg.clone();
    let mut capture_match = cfg.capture.clone();
    if capture_match.title_contains.is_none() {
        if let Some(title_hint) = extract_play_in_window_hint(&cfg.wine.args) {
            info!(title = %title_hint, "auto-derived capture.title_contains from wine args");
            capture_match.title_contains = Some(title_hint);
        }
    }
    runtime_cfg.capture = capture_match;
    if runtime_cfg.wine.args.iter().any(|arg| arg == "-playInWindow") {
        apply_xwayland_root_size(&mut runtime_cfg);
    }
    runtime_cfg
}

fn apply_xwayland_root_size(cfg: &mut Config) {
    if !cfg.wine.args.iter().any(|arg| arg == "-playInWindow") {
        return;
    }

    let Ok((root_width, root_height)) = window_input::current_root_size() else {
        return;
    };

    let mut adjusted = Vec::new();
    if let Some((before, after)) = replace_numeric_wine_arg(&mut cfg.wine.args, "-width", root_width) {
        adjusted.push(format!("-width: {before} -> {after}"));
    }
    if let Some((before, after)) = replace_numeric_wine_arg(&mut cfg.wine.args, "-height", root_height) {
        adjusted.push(format!("-height: {before} -> {after}"));
    }

    if !adjusted.is_empty() {
        info!(
            root_width,
            root_height,
            changes = adjusted.join(", "),
            "applied XWayland root-size wallpaper dimensions"
        );
    }
}

fn replace_numeric_wine_arg(args: &mut [String], flag: &str, replacement: u32) -> Option<(u32, u32)> {
    let index = args.iter().position(|arg| arg == flag)?;
    let value = args.get(index + 1)?.parse::<u32>().ok()?;
    let slot = args.get_mut(index + 1)?;
    if replacement == value {
        return None;
    }
    *slot = replacement.to_string();
    Some((value, replacement))
}

fn switch_wallpaper(cfg: &Config) -> Result<()> {
    ensure_switchable_runtime(cfg)?;
    let pid = spawn_transient_command(&cfg.wine)?;
    info!(pid, "sent openWallpaper command to running Wine wallpaper process");
    Ok(())
}

fn ensure_switchable_runtime(cfg: &Config) -> Result<()> {
    let runtime = cfg
        .runtime
        .as_ref()
        .ok_or_else(|| anyhow!("runtime block is required for wallpaper switching"))?;
    if runtime.mode != RuntimeMode::WineLayerd {
        return Err(anyhow!("only scene/web wallpapers can be switched without restarting Wine"));
    }
    if !matches!(runtime.wallpaper_type, RuntimeWallpaperType::Scene | RuntimeWallpaperType::Web) {
        return Err(anyhow!("only scene/web wallpapers can be switched without restarting Wine"));
    }
    Ok(())
}

fn run_video_native(
    video_file: Option<&str>,
    cfg: &Config,
    control_rx: &mpsc::Receiver<ControlCommand>,
) -> Result<()> {
    let video = video_file
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("runtime.video_file is required when runtime.mode=video_native"))?;

    info!(video, "starting native video mode via ffmpeg + wgpu");
    wayland::layer_shell::run_video_background_surface(
        Path::new(video),
        cfg.general.fps_limit,
        cfg.general.show_fps,
        cfg.general.fps_report_interval_secs,
        cfg.general.scale_mode,
        Some(control_rx),
    )
}

pub fn doctor() -> Result<()> {
    let cfg = Config::default();
    for key in ["WAYLAND_DISPLAY", "DISPLAY", "XAUTHORITY"] {
        match std::env::var(key) {
            Ok(value) => info!(%key, %value, "environment variable set"),
            Err(_) => warn!(%key, "environment variable not set"),
        }
    }

    match ensure_runtime_access() {
        Ok(()) => info!("X11 runtime access probe: OK"),
        Err(err) => warn!(error = %err, "X11 runtime access probe failed"),
    }

    match wayland::layer_shell::probe_layer_shell_support() {
        Ok(true) => info!("zwlr_layer_shell_v1 global is available"),
        Ok(false) => warn!("zwlr_layer_shell_v1 global not found on this compositor"),
        Err(err) => warn!(error = %err, "failed to query Wayland layer-shell support"),
    }

    if gnome::is_gnome_session() {
        match gnome::doctor(&cfg) {
            Ok(()) => {}
            Err(err) => warn!(error = %err, "GNOME extension D-Bus probe failed"),
        }
    }

    Ok(())
}

fn extract_play_in_window_hint(args: &[String]) -> Option<String> {
    let idx = args.iter().position(|arg| arg == "-playInWindow")?;
    args.get(idx + 1).cloned().filter(|s| !s.trim().is_empty())
}

fn ensure_runtime_access() -> Result<()> {
    capture_xcomposite::probe_xcomposite_support().map_err(|err| {
        let display = env::var("DISPLAY").ok();
        let xauthority = env::var("XAUTHORITY").ok();
        let hint = if err
            .to_string()
            .to_ascii_lowercase()
            .contains("invalid mit-magic-cookie-1 key")
        {
            "X11 authentication failed. Start we-layerd from the same graphical user session as XWayland, or export the matching X11 cookie file via XAUTHORITY before launching."
        } else {
            "X11 setup failed. Check DISPLAY, XAUTHORITY, and whether this process can access the active XWayland/X11 session."
        };

        err.context(format!(
            "{hint} DISPLAY={} XAUTHORITY={}",
            display.as_deref().unwrap_or("<unset>"),
            xauthority.as_deref().unwrap_or("<unset>")
        ))
    })
}

fn apply_debug_window_setup(cfg: &Config, debug_visibility: &DebugWindowVisibility, window: u32) {
    if let Err(err) = window_input::apply_wallpaper_window_hints(window) {
        warn!(error = %err, window, "failed to apply wallpaper window hints");
    }
    if cfg.general.disable_debug_window_input {
        if let Err(err) = window_input::set_mouse_passthrough(window) {
            warn!(error = %err, window, "failed to set debug window mouse passthrough");
        }
    }
    if debug_visibility.auto_hide {
        if let Err(err) = debug_visibility.hide() {
            warn!(error = %err, "failed to auto-hide debug window");
        }
    }
}
