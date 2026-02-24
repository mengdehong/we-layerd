use std::{
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Context, Result};
use tracing::{info, warn};

use crate::config::WineConfig;

#[derive(Clone)]
pub struct WineProcessHandle {
    child: Arc<Mutex<Option<Child>>>,
}

impl WineProcessHandle {
    pub fn spawn(config: &WineConfig) -> Result<Self> {
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

        let child = cmd.spawn().with_context(|| {
            format!(
                "failed to spawn wine command '{}' for {}",
                config.command,
                config.wallpaper_exe
            )
        })?;

        info!(pid = child.id(), "spawned wine wallpaper process");
        Ok(Self {
            child: Arc::new(Mutex::new(Some(child))),
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

    pub fn pid(&self) -> Option<u32> {
        self.child
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(std::process::Child::id))
    }

    pub fn terminate(&self) -> Result<()> {
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
