use std::{env, path::Path, sync::mpsc, time::Duration};

use anyhow::{anyhow, Context, Result};
use tracing::{info, warn};
use zbus::blocking::{Connection, Proxy};

use crate::{
    config::{Backend, CaptureConfig, Config},
    ipc::ControlCommand,
    x11::{window_finder, window_input},
};

const DBUS_PATH: &str = "/io/github/weLayerd/Gnome";
const DBUS_INTERFACE: &str = "io.github.weLayerd.Gnome";
const REFIND_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredWindow {
    pub xid: u32,
    pub pid: u32,
    pub title: String,
    pub wm_class: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedBackend {
    LayerShell,
    GnomeShell,
}

pub fn resolve_backend(cfg: &Config) -> ResolvedBackend {
    match cfg.general.backend {
        Backend::LayerShell => ResolvedBackend::LayerShell,
        Backend::GnomeShell => ResolvedBackend::GnomeShell,
        Backend::Auto => {
            if is_gnome_session() {
                ResolvedBackend::GnomeShell
            } else {
                ResolvedBackend::LayerShell
            }
        }
    }
}

pub fn is_gnome_session() -> bool {
    [
        env::var("XDG_CURRENT_DESKTOP").ok(),
        env::var("XDG_SESSION_DESKTOP").ok(),
        env::var("DESKTOP_SESSION").ok(),
    ]
    .into_iter()
    .flatten()
    .any(|value| value.to_ascii_lowercase().contains("gnome"))
}

pub fn run_window_bridge(
    cfg: &Config,
    capture_match: &CaptureConfig,
    initial_window: Option<window_finder::WindowFinderResult>,
    wine_pid: Option<u32>,
    control_rx: &mpsc::Receiver<ControlCommand>,
) -> Result<()> {
    let client = GnomeShellClient::connect(&cfg.gnome.extension_dbus_name)?;
    let version = client.ping()?;
    info!(version, "connected to GNOME wallpaper extension");

    let mut active = None;
    if let Some(found) = initial_window {
        active = register_found_window(cfg, &client, found)?;
    }

    loop {
        match control_rx.try_recv() {
            Ok(ControlCommand::Stop) => break,
            Ok(ControlCommand::Reload) => {
                if let Some(window) = &active {
                    let _ = client.unregister_window(window.xid);
                    client.register_window(window)?;
                }
            }
            Ok(_) => {}
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => break,
        }

        match window_finder::find_window_for_process_once(capture_match, wine_pid) {
            Ok(Some(found)) => {
                let next = window_from_found(&found);
                if active.as_ref() != Some(&next) {
                    active = register_found_window(cfg, &client, found)?;
                }
            }
            Ok(None) => {
                if let Some(current) = active.take() {
                    client.unregister_window(current.xid)?;
                }
            }
            Err(err) => warn!(error = %err, "failed to rescan X11 window for GNOME backend"),
        }

        std::thread::sleep(REFIND_INTERVAL);
    }

    if let Some(current) = active {
        client.unregister_window(current.xid)?;
    }

    Ok(())
}

pub fn run_video_bridge(
    cfg: &Config,
    video_file: &Path,
    control_rx: &mpsc::Receiver<ControlCommand>,
) -> Result<()> {
    let client = GnomeShellClient::connect(&cfg.gnome.extension_dbus_name)?;
    let version = client.ping()?;
    info!(version, "connected to GNOME wallpaper extension");

    client.start_video(video_file)?;
    info!(file = %video_file.display(), "started GNOME video wallpaper");

    loop {
        match control_rx.try_recv() {
            Ok(ControlCommand::Stop) => break,
            Ok(ControlCommand::Pause) => {
                client.pause_video()?;
            }
            Ok(ControlCommand::Resume) => {
                client.resume_video()?;
            }
            Ok(ControlCommand::Reload) => {
                client.start_video(video_file)?;
            }
            Ok(ControlCommand::HideWindow) | Ok(ControlCommand::ShowWindow) => {}
            Err(mpsc::TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(mpsc::TryRecvError::Disconnected) => break,
        }
    }

    client.stop_video()?;
    Ok(())
}

pub fn doctor(cfg: &Config) -> Result<()> {
    if !is_gnome_session() {
        return Ok(());
    }

    let client = GnomeShellClient::connect(&cfg.gnome.extension_dbus_name)
        .context("GNOME extension D-Bus service is not reachable")?;
    let version = client.ping()?;
    info!(version, "GNOME extension D-Bus probe: OK");
    Ok(())
}

fn register_found_window(
    cfg: &Config,
    client: &GnomeShellClient,
    found: window_finder::WindowFinderResult,
) -> Result<Option<RegisteredWindow>> {
    if let Err(err) = window_input::apply_wallpaper_window_hints(found.window) {
        warn!(error = %err, window = found.window, "failed to apply wallpaper window hints");
    }
    if cfg.general.disable_debug_window_input {
        if let Err(err) = window_input::set_mouse_passthrough(found.window) {
            warn!(error = %err, window = found.window, "failed to set debug window mouse passthrough");
        }
    }

    let registered = window_from_found(&found);
    client.register_window(&registered)?;
    info!(
        xid = registered.xid,
        pid = registered.pid,
        title = %registered.title,
        wm_class = %registered.wm_class,
        "registered XWayland window with GNOME extension"
    );
    Ok(Some(registered))
}

fn window_from_found(found: &window_finder::WindowFinderResult) -> RegisteredWindow {
    RegisteredWindow {
        xid: found.window,
        pid: found.metadata.pid.unwrap_or_default(),
        title: found.metadata.title.clone(),
        wm_class: found.metadata.wm_class.clone(),
    }
}

struct GnomeShellClient {
    connection: Connection,
    bus_name: String,
}

impl GnomeShellClient {
    fn connect(bus_name: &str) -> Result<Self> {
        let connection = Connection::session().context("failed to connect to the session D-Bus")?;
        let client = Self { connection, bus_name: bus_name.to_string() };
        let _ = client.proxy().context("failed to create GNOME extension proxy")?;
        Ok(client)
    }

    fn ping(&self) -> Result<String> {
        self.proxy()?.call("Ping", &()).context("failed to ping GNOME extension")
    }

    fn register_window(&self, window: &RegisteredWindow) -> Result<()> {
        let ok: bool = self
            .proxy()?
            .call("RegisterWindow", &(window.xid, window.pid, &window.title, &window.wm_class))
            .context("failed to register window with GNOME extension")?;
        if ok {
            Ok(())
        } else {
            Err(anyhow!("GNOME extension rejected the requested window registration"))
        }
    }

    fn unregister_window(&self, xid: u32) -> Result<()> {
        let _: bool = self
            .proxy()?
            .call("UnregisterWindow", &(xid,))
            .context("failed to unregister window from GNOME extension")?;
        Ok(())
    }

    fn start_video(&self, video_file: &Path) -> Result<()> {
        let accepted: bool = self
            .proxy()?
            .call("StartVideo", &(video_file.display().to_string(),))
            .context("failed to start video wallpaper via GNOME extension")?;
        if accepted {
            Ok(())
        } else {
            Err(anyhow!("GNOME extension rejected the requested video wallpaper"))
        }
    }

    fn stop_video(&self) -> Result<()> {
        let _: bool = self
            .proxy()?
            .call("StopVideo", &())
            .context("failed to stop video wallpaper via GNOME extension")?;
        Ok(())
    }

    fn pause_video(&self) -> Result<()> {
        let _: bool = self
            .proxy()?
            .call("PauseVideo", &())
            .context("failed to pause video wallpaper via GNOME extension")?;
        Ok(())
    }

    fn resume_video(&self) -> Result<()> {
        let _: bool = self
            .proxy()?
            .call("ResumeVideo", &())
            .context("failed to resume video wallpaper via GNOME extension")?;
        Ok(())
    }

    fn proxy(&self) -> Result<Proxy<'_>> {
        Proxy::new(&self.connection, self.bus_name.as_str(), DBUS_PATH, DBUS_INTERFACE)
            .context("failed to bind GNOME extension D-Bus proxy")
    }
}
