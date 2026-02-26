use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Context, Result};

use crate::config::{CgroupConfig, CgroupMode};

#[derive(Clone)]
pub struct RuntimeCgroup {
    cfg: CgroupConfig,
    wine_pid: Arc<Mutex<Option<u32>>>,
    last_error: Arc<Mutex<Option<String>>>,
}

impl RuntimeCgroup {
    pub fn new(cfg: CgroupConfig) -> Self {
        Self { cfg, wine_pid: Arc::new(Mutex::new(None)), last_error: Arc::new(Mutex::new(None)) }
    }

    pub fn on_wine_spawn(&self, pid: u32) {
        if let Ok(mut lock) = self.wine_pid.lock() {
            *lock = Some(pid);
        }

        #[cfg(target_os = "linux")]
        if self.cfg.enabled && self.cfg.mode == CgroupMode::LimitWine {
            if let Err(err) = attach_wine_to_limited_group(pid, &self.cfg) {
                self.set_error(format!("cgroup limit attach failed: {err}"));
            }
        }
    }

    pub fn render_status_toml(&self) -> String {
        let mut out = String::new();
        out.push_str("[status.cgroup]\n");
        out.push_str(&format!("enabled = {}\n", self.cfg.enabled));
        out.push_str(&format!("mode = \"{}\"\n", cgroup_mode_name(self.cfg.mode)));

        if let Some(memory_max) = &self.cfg.memory_max {
            out.push_str(&format!("memory_max = \"{}\"\n", escape_toml(memory_max)));
        }
        if let Some(cpu_max) = &self.cfg.cpu_max {
            out.push_str(&format!("cpu_max = \"{}\"\n", escape_toml(cpu_max)));
        }

        #[cfg(not(target_os = "linux"))]
        {
            out.push_str("supported = false\n");
            return out;
        }

        #[cfg(target_os = "linux")]
        {
            out.push_str("supported = true\n");
            out.push_str(&format!("self_pid = {}\n", std::process::id()));

            if let Ok(lock) = self.wine_pid.lock() {
                if let Some(pid) = *lock {
                    out.push_str(&format!("wine_pid = {}\n", pid));
                }
            }

            if self.cfg.enabled {
                if self.cfg.mode == CgroupMode::Detect {
                    append_proc_cgroup_stats(&mut out, "self", std::process::id());
                    if let Ok(lock) = self.wine_pid.lock() {
                        if let Some(pid) = *lock {
                            append_proc_cgroup_stats(&mut out, "wine", pid);
                        }
                    }
                } else {
                    append_limited_wine_group_stats(&mut out);
                }
            }

            if let Ok(lock) = self.last_error.lock() {
                if let Some(err) = lock.as_ref() {
                    out.push_str(&format!("last_error = \"{}\"\n", escape_toml(err)));
                }
            }
        }

        out
    }

    fn set_error(&self, message: String) {
        if let Ok(mut lock) = self.last_error.lock() {
            *lock = Some(message);
        }
    }
}

fn cgroup_mode_name(mode: CgroupMode) -> &'static str {
    match mode {
        CgroupMode::Detect => "detect",
        CgroupMode::LimitWine => "limit_wine",
    }
}

fn escape_toml(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "linux")]
fn append_proc_cgroup_stats(out: &mut String, prefix: &str, pid: u32) {
    if let Ok(path) = read_cgroup_path_for_pid(pid) {
        out.push_str(&format!("{prefix}_cgroup = \"{}\"\n", escape_toml(&path)));
        if let Ok(Some(usage)) = read_cpu_usage_usec(&path) {
            out.push_str(&format!("{prefix}_cpu_usage_usec = {}\n", usage));
        }
        if let Ok(Some(mem)) = read_memory_current(&path) {
            out.push_str(&format!("{prefix}_memory_current = {}\n", mem));
        }
    }
}

#[cfg(target_os = "linux")]
fn append_limited_wine_group_stats(out: &mut String) {
    if let Ok(path) = wine_limit_group_path_abs() {
        if path.exists() {
            let display =
                path.strip_prefix("/sys/fs/cgroup").unwrap_or(&path).display().to_string();
            out.push_str(&format!("wine_limit_cgroup = \"{}\"\n", escape_toml(&display)));
            if let Ok(Some(usage)) = read_cpu_usage_usec_from_abs(&path) {
                out.push_str(&format!("wine_limit_cpu_usage_usec = {}\n", usage));
            }
            if let Ok(Some(mem)) = read_memory_current_from_abs(&path) {
                out.push_str(&format!("wine_limit_memory_current = {}\n", mem));
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn attach_wine_to_limited_group(pid: u32, cfg: &CgroupConfig) -> Result<()> {
    let group_path = wine_limit_group_path_abs()?;
    fs::create_dir_all(&group_path)
        .with_context(|| format!("failed to create cgroup {}", group_path.display()))?;

    if let Some(memory_max) = &cfg.memory_max {
        fs::write(group_path.join("memory.max"), memory_max.as_bytes())
            .with_context(|| format!("failed to write memory.max in {}", group_path.display()))?;
    }
    if let Some(cpu_max) = &cfg.cpu_max {
        fs::write(group_path.join("cpu.max"), cpu_max.as_bytes())
            .with_context(|| format!("failed to write cpu.max in {}", group_path.display()))?;
    }

    fs::write(group_path.join("cgroup.procs"), pid.to_string().as_bytes())
        .with_context(|| format!("failed to move pid {} to {}", pid, group_path.display()))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn read_cgroup_path_for_pid(pid: u32) -> Result<String> {
    let content = fs::read_to_string(format!("/proc/{pid}/cgroup"))
        .with_context(|| format!("failed to read /proc/{pid}/cgroup"))?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("0::") {
            return Ok(rest.to_string());
        }
    }
    Err(anyhow!("cgroup v2 path not found for pid {}", pid))
}

#[cfg(target_os = "linux")]
fn cgroup_abs_path(relative: &str) -> PathBuf {
    PathBuf::from("/sys/fs/cgroup").join(relative.trim_start_matches('/'))
}

#[cfg(target_os = "linux")]
fn wine_limit_group_path_abs() -> Result<PathBuf> {
    let current = read_cgroup_path_for_pid(std::process::id())?;
    Ok(cgroup_abs_path(&format!("{}/we-layerd/wine", current.trim_end_matches('/'))))
}

#[cfg(target_os = "linux")]
fn read_cpu_usage_usec(relative: &str) -> Result<Option<u64>> {
    read_cpu_usage_usec_from_abs(&cgroup_abs_path(relative))
}

#[cfg(target_os = "linux")]
fn read_memory_current(relative: &str) -> Result<Option<u64>> {
    read_memory_current_from_abs(&cgroup_abs_path(relative))
}

#[cfg(target_os = "linux")]
fn read_cpu_usage_usec_from_abs(path: &Path) -> Result<Option<u64>> {
    let content = fs::read_to_string(path.join("cpu.stat"))
        .with_context(|| format!("failed to read {}/cpu.stat", path.display()))?;
    for line in content.lines() {
        let mut parts = line.split_whitespace();
        if parts.next() == Some("usage_usec") {
            if let Some(value) = parts.next() {
                if let Ok(parsed) = value.parse::<u64>() {
                    return Ok(Some(parsed));
                }
            }
        }
    }
    Ok(None)
}

#[cfg(target_os = "linux")]
fn read_memory_current_from_abs(path: &Path) -> Result<Option<u64>> {
    let content = fs::read_to_string(path.join("memory.current"))
        .with_context(|| format!("failed to read {}/memory.current", path.display()))?;
    let parsed = content.trim().parse::<u64>().ok();
    Ok(parsed)
}
