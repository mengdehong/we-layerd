use std::{
    io,
    os::unix::process::CommandExt,
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use tracing::{info, warn};

use crate::config::WineConfig;

#[derive(Clone)]
pub struct WineProcessHandle {
    child: Arc<Mutex<Option<Child>>>,
    pgid: Arc<Mutex<Option<i32>>>,
    stop: Arc<AtomicBool>,
}

impl WineProcessHandle {
    pub fn spawn(config: &WineConfig) -> Result<Self> {
        let child = spawn_child(config)?;
        let pgid = child.id() as i32;
        info!(pid = child.id(), pgid, "spawned wine wallpaper process");
        Ok(Self {
            child: Arc::new(Mutex::new(Some(child))),
            pgid: Arc::new(Mutex::new(Some(pgid))),
            stop: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn install_ctrlc_handler(&self) -> Result<()> {
        let handle = self.clone();
        ctrlc::set_handler(move || {
            warn!("received Ctrl+C, terminating wine process");
            if let Err(err) = handle.terminate() {
                warn!(error = %err, "failed to terminate wine process on Ctrl+C");
            }
            std::process::exit(130);
        })
        .context("failed to register Ctrl+C handler")
    }

    pub fn install_exit_monitor(&self, config: WineConfig, restart_on_exit: bool) {
        let child = self.child.clone();
        let pgid = self.pgid.clone();
        let stop = self.stop.clone();
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let mut guard = match child.lock() {
                    Ok(guard) => guard,
                    Err(_) => {
                        warn!("wine child process lock poisoned");
                        break;
                    }
                };

                if let Some(proc) = guard.as_mut() {
                    match proc.try_wait() {
                        Ok(Some(status)) => {
                            warn!(?status, "wine process exited");
                            *guard = None;
                            if let Ok(mut pgid_guard) = pgid.lock() {
                                *pgid_guard = None;
                            }

                            if stop.load(Ordering::Relaxed) {
                                drop(guard);
                                break;
                            }

                            let should_restart = restart_on_exit && !status.success();
                            if should_restart {
                                match spawn_child(&config) {
                                    Ok(new_child) => {
                                        let new_pgid = new_child.id() as i32;
                                        info!(
                                            pid = new_child.id(),
                                            pgid = new_pgid,
                                            "restarted wine process"
                                        );
                                        if let Ok(mut pgid_guard) = pgid.lock() {
                                            *pgid_guard = Some(new_pgid);
                                        }
                                        *guard = Some(new_child);
                                    }
                                    Err(err) => {
                                        warn!(error = %err, "failed to restart wine process");
                                    }
                                }
                            } else if status.success() {
                                info!("wine process exited successfully; skip auto-restart");
                            }
                        }
                        Ok(None) => {}
                        Err(err) => {
                            warn!(error = %err, "failed to poll wine process status");
                        }
                    }
                }

                drop(guard);
                std::thread::sleep(Duration::from_secs(2));
            }
        });
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.lock().ok().and_then(|guard| guard.as_ref().map(std::process::Child::id))
    }

    pub fn terminate(&self) -> Result<()> {
        self.stop.store(true, Ordering::Relaxed);

        let pgid = {
            let guard =
                self.pgid.lock().map_err(|_| anyhow!("wine process group lock poisoned"))?;
            *guard
        };

        if let Some(pgid) = pgid {
            terminate_process_group(pgid)?;
        }

        let mut guard =
            self.child.lock().map_err(|_| anyhow!("wine child process lock poisoned"))?;

        if let Some(child) = guard.as_mut() {
            let _ = child.wait();
            info!("wine process terminated");
        }

        *guard = None;
        if let Ok(mut pgid_guard) = self.pgid.lock() {
            *pgid_guard = None;
        }
        Ok(())
    }
}

impl Drop for WineProcessHandle {
    fn drop(&mut self) {
        if Arc::strong_count(&self.child) > 1 {
            return;
        }

        if let Err(err) = self.terminate() {
            warn!(error = %err, "failed to cleanup wine process on drop");
        }
    }
}

fn spawn_child(config: &WineConfig) -> Result<Child> {
    if config.wallpaper_exe.is_empty() {
        return Err(anyhow!(
            "wine.wallpaper_exe is empty; set the Wallpaper Engine executable path"
        ));
    }

    let exe_path = Path::new(&config.wallpaper_exe);
    if !exe_path.exists() {
        return Err(anyhow!("Wallpaper executable does not exist: {}", exe_path.display()));
    }

    let mut cmd = Command::new(&config.command);
    cmd.arg(&config.wallpaper_exe)
        .args(&config.args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    unsafe {
        cmd.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }

    cmd.spawn().with_context(|| {
        format!("failed to spawn wine command '{}' for {}", config.command, config.wallpaper_exe)
    })
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
    Err(err).with_context(|| format!("failed to send signal {} to process group {}", signal, pgid))
}
