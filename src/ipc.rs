use std::{
    fs,
    io::{self, Read, Write},
    net::Shutdown,
    os::fd::AsRawFd,
    os::unix::net::{SocketAddr, UnixListener, UnixStream},
    path::Path,
    path::PathBuf,
    sync::mpsc::Sender,
    thread,
};

#[cfg(target_os = "linux")]
use std::os::linux::net::SocketAddrExt;

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

#[derive(Debug, Clone)]
enum ControlRequest {
    Command(ControlCommand),
    ShowConfig,
}

impl ControlRequest {
    fn parse(raw: &str) -> Option<Self> {
        let normalized = raw.trim().to_ascii_lowercase();
        if normalized == "show-config" {
            return Some(Self::ShowConfig);
        }
        ControlCommand::parse(&normalized).map(Self::Command)
    }
}

pub struct ControlServer {
    socket_path: Option<PathBuf>,
    _instance_lock: fs::File,
}

impl ControlServer {
    pub fn start(tx: Sender<ControlCommand>, running_config_toml: String) -> Result<Self> {
        let instance_lock = acquire_instance_lock()?;
        let endpoint = default_endpoint()?;
        let listener = bind_listener(&endpoint)?;
        let socket_path = endpoint.socket_path();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else {
                    continue;
                };
                let mut buf = String::new();
                if stream.read_to_string(&mut buf).is_err() {
                    continue;
                }
                let Some(request) = ControlRequest::parse(&buf) else {
                    let _ = stream.write_all(b"ERR unknown command\n");
                    continue;
                };
                match request {
                    ControlRequest::ShowConfig => {
                        let _ = stream.write_all(running_config_toml.as_bytes());
                    }
                    ControlRequest::Command(cmd) => {
                        if tx.send(cmd).is_ok() {
                            let _ = stream.write_all(b"OK\n");
                        } else {
                            let _ = stream.write_all(b"ERR daemon not running\n");
                        }
                    }
                }
            }
        });

        Ok(Self {
            socket_path,
            _instance_lock: instance_lock,
        })
    }
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        if let Some(socket_path) = &self.socket_path {
            let _ = fs::remove_file(socket_path);
        }
    }
}

pub fn send_command(command: ControlCommand) -> Result<()> {
    let response = send_request(command.as_str())?;
    if response.trim_start().starts_with("ERR") {
        return Err(anyhow!(response.trim().to_string()));
    }
    Ok(())
}

pub fn request_running_config() -> Result<String> {
    let response = send_request("show-config")?;
    if response.trim_start().starts_with("ERR") {
        return Err(anyhow!(response.trim().to_string()));
    }
    Ok(response)
}

fn send_request(request: &str) -> Result<String> {
    let mut last_error: Option<anyhow::Error> = None;
    for endpoint in control_endpoints() {
        match connect_stream(&endpoint) {
            Ok(mut stream) => {
                stream
                    .write_all(request.as_bytes())
                    .with_context(|| format!("failed to send IPC request '{request}'"))?;
                let _ = stream.shutdown(Shutdown::Write);
                let mut response = String::new();
                stream.read_to_string(&mut response).context("failed to read IPC response")?;
                return Ok(response);
            }
            Err(err) => {
                last_error = Some(err);
            }
        }
    }

    Err(anyhow!("failed to reach we-layerd control endpoint (daemon may not be running)"))
        .context(last_error.unwrap_or_else(|| anyhow!("no endpoint available")))
}

#[derive(Debug, Clone)]
enum Endpoint {
    Path(PathBuf),
    #[cfg(target_os = "linux")]
    Abstract(Vec<u8>),
}

impl Endpoint {
    fn socket_path(&self) -> Option<PathBuf> {
        match self {
            Self::Path(path) => Some(path.clone()),
            #[cfg(target_os = "linux")]
            Self::Abstract(_) => None,
        }
    }
}

fn default_endpoint() -> Result<Endpoint> {
    #[cfg(target_os = "linux")]
    {
        return Ok(Endpoint::Abstract(abstract_socket_name()));
    }

    #[cfg(not(target_os = "linux"))]
    {
        Ok(Endpoint::Path(default_socket_path()?))
    }
}

fn control_endpoints() -> Vec<Endpoint> {
    #[cfg(target_os = "linux")]
    {
        vec![
            Endpoint::Abstract(abstract_socket_name()),
            Endpoint::Path(default_socket_path().unwrap_or_else(|_| PathBuf::from("/tmp/we-layerd-control.sock"))),
        ]
    }

    #[cfg(not(target_os = "linux"))]
    {
        vec![Endpoint::Path(
            default_socket_path().unwrap_or_else(|_| PathBuf::from("/tmp/we-layerd-control.sock")),
        )]
    }
}

fn bind_listener(endpoint: &Endpoint) -> Result<UnixListener> {
    match endpoint {
        Endpoint::Path(socket_path) => bind_file_listener(socket_path),
        #[cfg(target_os = "linux")]
        Endpoint::Abstract(name) => {
            let addr =
                SocketAddr::from_abstract_name(name).context("failed to build abstract IPC socket")?;
            UnixListener::bind_addr(&addr).context("failed to bind abstract IPC socket for we-layerd")
        }
    }
}

fn bind_file_listener(socket_path: &Path) -> Result<UnixListener> {
    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    if socket_path.exists() {
        if UnixStream::connect(socket_path).is_ok() {
            return Err(anyhow!("we-layerd is already running"));
        }
        let _ = fs::remove_file(socket_path);
    }

    UnixListener::bind(socket_path)
        .with_context(|| format!("failed to bind IPC socket {}", socket_path.display()))
}

fn connect_stream(endpoint: &Endpoint) -> Result<UnixStream> {
    match endpoint {
        Endpoint::Path(path) => UnixStream::connect(path)
            .with_context(|| format!("failed to connect IPC socket {}", path.display())),
        #[cfg(target_os = "linux")]
        Endpoint::Abstract(name) => {
            let addr =
                SocketAddr::from_abstract_name(name).context("failed to build abstract IPC socket")?;
            UnixStream::connect_addr(&addr).context("failed to connect abstract IPC socket")
        }
    }
}

fn default_socket_path() -> Result<PathBuf> {
    Ok(ipc_runtime_dir()?.join("control.sock"))
}

fn instance_lock_path() -> Result<PathBuf> {
    Ok(ipc_runtime_dir()?.join("instance.lock"))
}

fn ipc_runtime_dir() -> Result<PathBuf> {
    #[cfg(target_os = "linux")]
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        return Ok(PathBuf::from(runtime_dir).join("we-layerd"));
    }

    let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home).join(".config/we-layerd"))
}

fn acquire_instance_lock() -> Result<fs::File> {
    let lock_path = instance_lock_path()?;
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open lock file {}", lock_path.display()))?;

    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EWOULDBLOCK) {
            return Err(anyhow!("we-layerd is already running"));
        }
        return Err(err).with_context(|| format!("failed to lock {}", lock_path.display()));
    }

    Ok(file)
}

#[cfg(target_os = "linux")]
fn abstract_socket_name() -> Vec<u8> {
    let uid = unsafe { libc::geteuid() };
    format!("we-layerd.control.{uid}").into_bytes()
}
