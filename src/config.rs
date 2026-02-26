use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub wine: WineConfig,
    #[serde(default)]
    pub capture: CaptureConfig,
    #[serde(default)]
    pub runtime: Option<RuntimeConfig>,
    #[serde(default)]
    pub cgroup: CgroupConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_fps")]
    pub fps_limit: u32,
    #[serde(default = "default_restart_wine")]
    pub restart_wine_on_exit: bool,
    #[serde(default = "default_refind_window")]
    pub refind_window_on_capture_error: bool,
    #[serde(default)]
    pub show_fps: bool,
    #[serde(default = "default_fps_report_interval_secs")]
    pub fps_report_interval_secs: u64,
    #[serde(default)]
    pub scale_mode: ScaleMode,
    #[serde(default = "default_hide_debug_window")]
    pub hide_debug_window: bool,
    #[serde(default = "default_hidden_workspace_name")]
    pub hidden_workspace_name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScaleMode {
    Fit,
    Cover,
    Stretch,
}

impl Default for ScaleMode {
    fn default() -> Self {
        Self::Cover
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WineConfig {
    #[serde(default = "default_wine_cmd")]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub wallpaper_exe: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    #[serde(default)]
    pub wm_class_contains: Option<String>,
    #[serde(default)]
    pub title_contains: Option<String>,
    #[serde(default)]
    pub net_wm_pid: Option<u32>,
    #[serde(default)]
    pub debug_save_frame_png: Option<String>,
    #[serde(default)]
    pub output_window_map: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default)]
    pub mode: RuntimeMode,
    #[serde(default)]
    pub wallpaper_type: RuntimeWallpaperType,
    #[serde(default)]
    pub video_file: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    VideoNative,
    WineLayerd,
}

impl Default for RuntimeMode {
    fn default() -> Self {
        Self::WineLayerd
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeWallpaperType {
    Video,
    Scene,
    Web,
    Unknown,
}

impl Default for RuntimeWallpaperType {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CgroupConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub mode: CgroupMode,
    #[serde(default)]
    pub memory_max: Option<String>,
    #[serde(default)]
    pub cpu_max: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CgroupMode {
    Detect,
    LimitWine,
}

impl Default for CgroupMode {
    fn default() -> Self {
        Self::Detect
    }
}

fn default_fps() -> u32 {
    30
}

fn default_wine_cmd() -> String {
    "wine".to_string()
}

fn default_restart_wine() -> bool {
    true
}

fn default_refind_window() -> bool {
    true
}

fn default_fps_report_interval_secs() -> u64 {
    1
}

fn default_hide_debug_window() -> bool {
    true
}

fn default_hidden_workspace_name() -> String {
    "top".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            wine: WineConfig::default(),
            capture: CaptureConfig::default(),
            runtime: None,
            cgroup: CgroupConfig::default(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            fps_limit: default_fps(),
            restart_wine_on_exit: default_restart_wine(),
            refind_window_on_capture_error: default_refind_window(),
            show_fps: false,
            fps_report_interval_secs: default_fps_report_interval_secs(),
            scale_mode: ScaleMode::default(),
            hide_debug_window: default_hide_debug_window(),
            hidden_workspace_name: default_hidden_workspace_name(),
        }
    }
}

impl Default for WineConfig {
    fn default() -> Self {
        Self { command: default_wine_cmd(), args: Vec::new(), wallpaper_exe: String::new() }
    }
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            wm_class_contains: Some("wallpaper".to_string()),
            title_contains: None,
            net_wm_pid: None,
            debug_save_frame_png: None,
            output_window_map: BTreeMap::new(),
        }
    }
}

impl Default for CgroupConfig {
    fn default() -> Self {
        Self { enabled: false, mode: CgroupMode::Detect, memory_max: None, cpu_max: None }
    }
}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        match path {
            Some(path) => {
                let raw = fs::read_to_string(path)
                    .with_context(|| format!("failed to read config file: {}", path.display()))?;
                toml::from_str(&raw)
                    .with_context(|| format!("invalid TOML in config file: {}", path.display()))
            }
            None => Ok(Self::default()),
        }
    }

    pub fn to_toml_pretty(&self) -> Result<String> {
        toml::to_string_pretty(self).context("failed to serialize config")
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn default_config_has_expected_fps() {
        let cfg = Config::default();
        assert_eq!(cfg.general.fps_limit, 30);
    }
}
