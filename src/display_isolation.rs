use std::{
    env,
    ffi::OsString,
    fs, io,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context, Result};
use tracing::{info, warn};

use crate::config::{Config, IsolationConfig, IsolationMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DisplaySize {
    width: u32,
    height: u32,
}

const DEFAULT_WIDTH: u32 = 1920;
const DEFAULT_HEIGHT: u32 = 1080;

fn resolve_display_size(config: &IsolationConfig, wine_args: &[String]) -> DisplaySize {
    DisplaySize {
        width: config.width.or_else(|| numeric_arg(wine_args, "-width")).unwrap_or(DEFAULT_WIDTH),
        height: config
            .height
            .or_else(|| numeric_arg(wine_args, "-height"))
            .unwrap_or(DEFAULT_HEIGHT),
    }
}

struct DisplayEnvGuard {
    previous: Option<OsString>,
}

impl DisplayEnvGuard {
    fn set(display: &str) -> Self {
        let previous = env::var_os("DISPLAY");
        env::set_var("DISPLAY", display);
        Self { previous }
    }
}

impl Drop for DisplayEnvGuard {
    fn drop(&mut self) {
        match self.previous.as_ref() {
            Some(value) => env::set_var("DISPLAY", value),
            None => env::remove_var("DISPLAY"),
        }
    }
}

pub struct DisplayIsolation {
    child: Option<Child>,
    _env_guard: DisplayEnvGuard,
    pgid: i32,
    display_file: PathBuf,
}

pub fn start_for_config(config: &mut Config) -> Result<Option<DisplayIsolation>> {
    if config.isolation.mode == IsolationMode::None {
        return Ok(None);
    }

    let isolated = start_gamescope_headless(&config.isolation, &config.wine.args)?;
    config.wine.env.insert("DISPLAY".to_string(), isolated.display.clone());
    let env_guard = DisplayEnvGuard::set(&isolated.display);

    Ok(Some(DisplayIsolation {
        child: Some(isolated.child),
        _env_guard: env_guard,
        pgid: isolated.pgid,
        display_file: isolated.display_file,
    }))
}

struct StartedDisplay {
    display: String,
    child: Child,
    pgid: i32,
    display_file: PathBuf,
}

fn start_gamescope_headless(
    config: &IsolationConfig,
    wine_args: &[String],
) -> Result<StartedDisplay> {
    let size = resolve_display_size(config, wine_args);
    let display_file = next_display_file()?;
    let display_file_str = display_file.to_string_lossy().to_string();
    let args = gamescope_headless_args(&size, &display_file_str);

    let _ = fs::remove_file(&display_file);
    let mut cmd = Command::new(&config.command);
    cmd.args(&args).stdout(Stdio::inherit()).stderr(Stdio::inherit());
    unsafe {
        cmd.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn isolated display '{}'", config.command))?;
    let pgid = child.id() as i32;
    let timeout = Duration::from_secs(config.startup_timeout_secs.max(1));
    let x_display_value = match wait_for_display(&mut child, &display_file, timeout) {
        Ok(display) => display,
        Err(err) => {
            let _ = terminate_process_group(pgid);
            let _ = child.wait();
            let _ = fs::remove_file(&display_file);
            return Err(err);
        }
    };
    info!(
        pid = child.id(),
        pgid,
        x_display = %x_display_value,
        width = size.width,
        height = size.height,
        "started gamescope headless isolated display"
    );

    Ok(StartedDisplay { display: x_display_value, child, pgid, display_file })
}

fn gamescope_headless_args(size: &DisplaySize, display_file: &str) -> Vec<String> {
    vec![
        "--backend".to_string(),
        "headless".to_string(),
        "-W".to_string(),
        size.width.to_string(),
        "-H".to_string(),
        size.height.to_string(),
        "-w".to_string(),
        size.width.to_string(),
        "-h".to_string(),
        size.height.to_string(),
        "--xwayland-count".to_string(),
        "1".to_string(),
        "--".to_string(),
        "/bin/sh".to_string(),
        "-c".to_string(),
        "printf '%s\n' \"$DISPLAY\" > \"$1\"; exec sleep infinity".to_string(),
        "we-layerd-gamescope-display".to_string(),
        display_file.to_string(),
    ]
}

fn next_display_file() -> Result<PathBuf> {
    let runtime_dir = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("XDG_RUNTIME_DIR is not set for display isolation"))?;
    let dir = runtime_dir.join("we-layerd");
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create runtime directory {}", dir.display()))?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before UNIX_EPOCH")?
        .as_nanos();
    Ok(dir.join(format!("gamescope-display-{}-{nanos}.txt", std::process::id())))
}

fn wait_for_display(child: &mut Child, display_file: &Path, timeout: Duration) -> Result<String> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Some(status) = child.try_wait().context("failed to poll isolated display process")? {
            return Err(anyhow!("isolated display exited before publishing DISPLAY: {status:?}"));
        }

        if let Ok(raw) = fs::read_to_string(display_file) {
            let display = raw.trim().to_string();
            if !display.is_empty() {
                return Ok(display);
            }
        }

        std::thread::sleep(Duration::from_millis(50));
    }

    Err(anyhow!("timed out waiting for isolated display DISPLAY file: {}", display_file.display()))
}

impl Drop for DisplayIsolation {
    fn drop(&mut self) {
        if let Err(err) = terminate_process_group(self.pgid) {
            warn!(error = %err, pgid = self.pgid, "failed to terminate isolated display process group");
        }
        if let Some(child) = self.child.as_mut() {
            let _ = child.wait();
        }
        let _ = fs::remove_file(&self.display_file);
    }
}

fn terminate_process_group(pgid: i32) -> Result<()> {
    if !signal_process_group(-pgid, libc::SIGTERM)? {
        return Ok(());
    }

    for _ in 0..20 {
        if !signal_process_group(-pgid, 0)? {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    signal_process_group(-pgid, libc::SIGKILL)?;
    Ok(())
}

fn signal_process_group(pgid: i32, signal: i32) -> Result<bool> {
    let rc = unsafe { libc::kill(pgid, signal) };
    if rc == 0 {
        return Ok(true);
    }
    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        return Ok(false);
    }
    Err(err).with_context(|| format!("failed to send signal {signal} to process group {pgid}"))
}

fn numeric_arg(args: &[String], flag: &str) -> Option<u32> {
    let index = args.iter().position(|arg| arg == flag)?;
    args.get(index + 1)?.parse::<u32>().ok().filter(|value| *value > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{IsolationConfig, IsolationMode};

    #[test]
    fn display_size_prefers_config_and_falls_back_to_wine_args() {
        let cfg = IsolationConfig {
            mode: IsolationMode::GamescopeHeadless,
            width: Some(1280),
            height: Some(720),
            ..IsolationConfig::default()
        };
        let args = vec![
            "-width".to_string(),
            "2560".to_string(),
            "-height".to_string(),
            "1600".to_string(),
        ];

        let size = resolve_display_size(&cfg, &args);

        assert_eq!(size.width, 1280);
        assert_eq!(size.height, 720);

        let cfg = IsolationConfig {
            mode: IsolationMode::GamescopeHeadless,
            ..IsolationConfig::default()
        };
        let args = vec![
            "-height".to_string(),
            "1600".to_string(),
            "-width".to_string(),
            "2560".to_string(),
        ];
        let size = resolve_display_size(&cfg, &args);

        assert_eq!(size.width, 2560);
        assert_eq!(size.height, 1600);
    }
}
