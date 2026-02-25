use std::{
    fs,
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    sync::mpsc::Sender,
    thread,
};

use anyhow::{anyhow, Context, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCommand {
    Stop,
    Pause,
    Resume,
    Reload,
}

impl ControlCommand {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stop => "stop",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Reload => "reload",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "stop" => Some(Self::Stop),
            "pause" => Some(Self::Pause),
            "resume" => Some(Self::Resume),
            "reload" => Some(Self::Reload),
            _ => None,
        }
    }
}

pub struct ControlServer {
    socket_path: PathBuf,
}

impl ControlServer {
    pub fn start(tx: Sender<ControlCommand>) -> Result<Self> {
        let socket_path = default_socket_path()?;
        if socket_path.exists() {
            let _ = fs::remove_file(&socket_path);
        }
        if let Some(parent) = socket_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let listener = UnixListener::bind(&socket_path)
            .with_context(|| format!("failed to bind IPC socket {}", socket_path.display()))?;
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else {
                    continue;
                };
                let mut buf = String::new();
                if stream.read_to_string(&mut buf).is_err() {
                    continue;
                }
                let Some(cmd) = ControlCommand::parse(&buf) else {
                    let _ = stream.write_all(b"ERR unknown command\n");
                    continue;
                };
                if tx.send(cmd).is_ok() {
                    let _ = stream.write_all(b"OK\n");
                } else {
                    let _ = stream.write_all(b"ERR daemon not running\n");
                }
            }
        });

        Ok(Self { socket_path })
    }
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.socket_path);
    }
}

pub fn send_command(command: ControlCommand) -> Result<()> {
    let socket_path = default_socket_path()?;
    let mut stream = UnixStream::connect(&socket_path)
        .with_context(|| format!("failed to connect IPC socket {}", socket_path.display()))?;
    stream
        .write_all(command.as_str().as_bytes())
        .with_context(|| format!("failed to send IPC command '{}'", command.as_str()))?;
    Ok(())
}

fn default_socket_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home).join(".config/we-layerd/control.sock"))
}
