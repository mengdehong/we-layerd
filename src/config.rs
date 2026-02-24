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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_fps")]
    pub fps_limit: u32,
    #[serde(default = "default_restart_wine")]
    pub restart_wine_on_exit: bool,
    #[serde(default = "default_refind_window")]
    pub refind_window_on_capture_error: bool,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            wine: WineConfig::default(),
            capture: CaptureConfig::default(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            fps_limit: default_fps(),
            restart_wine_on_exit: default_restart_wine(),
            refind_window_on_capture_error: default_refind_window(),
        }
    }
}

impl Default for WineConfig {
    fn default() -> Self {
        Self {
            command: default_wine_cmd(),
            args: Vec::new(),
            wallpaper_exe: String::new(),
        }
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
