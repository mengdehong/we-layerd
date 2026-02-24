use std::{
    path::Path,
    process::{Child, Command, Stdio},
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use tracing::{info, warn};

use crate::config::WineConfig;

#[derive(Clone)]
pub struct WineProcessHandle {
    child: Arc<Mutex<Option<Child>>>,
    stop: Arc<AtomicBool>,
}

impl WineProcessHandle {
    pub fn spawn(config: &WineConfig) -> Result<Self> {
        let child = spawn_child(config)?;
        info!(pid = child.id(), "spawned wine wallpaper process");
        Ok(Self {
            child: Arc::new(Mutex::new(Some(child))),
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
                            if restart_on_exit {
                                match spawn_child(&config) {
                                    Ok(new_child) => {
                                        info!(pid = new_child.id(), "restarted wine process");
                                        *guard = Some(new_child);
                                    }
                                    Err(err) => {
                                        warn!(error = %err, "failed to restart wine process");
                                    }
                                }
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
        self.child
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(std::process::Child::id))
    }

    pub fn terminate(&self) -> Result<()> {
        self.stop.store(true, Ordering::Relaxed);

        let mut guard = self
            .child
            .lock()
            .map_err(|_| anyhow!("wine child process lock poisoned"))?;

        if let Some(child) = guard.as_mut() {
            child.kill().context("failed to send kill to wine process")?;
            let _ = child.wait();
            info!("wine process terminated");
        }

        *guard = None;
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
        return Err(anyhow!(
            "Wallpaper executable does not exist: {}",
            exe_path.display()
        ));
    }

    let mut cmd = Command::new(&config.command);
    cmd.args(&config.args)
        .arg(&config.wallpaper_exe)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    cmd.spawn().with_context(|| {
        format!(
            "failed to spawn wine command '{}' for {}",
            config.command,
            config.wallpaper_exe
        )
    })
}
