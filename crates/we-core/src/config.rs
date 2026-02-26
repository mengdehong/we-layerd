use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::wallpaper::WallpaperType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub wine: WineConfig,
    #[serde(default)]
    pub capture: CaptureConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RuntimeConfig>,
    #[serde(default)]
    pub cgroup: CgroupConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub fps_limit: u32,
    pub restart_wine_on_exit: bool,
    pub refind_window_on_capture_error: bool,
    pub show_fps: bool,
    pub fps_report_interval_secs: u64,
    pub scale_mode: ScaleMode,
    pub hide_debug_window: bool,
    pub hidden_workspace_name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WineConfig {
    pub command: String,
    pub wallpaper_exe: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CaptureConfig {
    pub wm_class_contains: String,
    pub title_contains: String,
    #[serde(default)]
    pub output_window_map: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub mode: RuntimeMode,
    pub wallpaper_type: RuntimeWallpaperType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_file: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    VideoNative,
    WineLayerd,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeWallpaperType {
    Video,
    Scene,
    Web,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CgroupConfig {
    pub enabled: bool,
    pub mode: CgroupMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_max: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_max: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CgroupMode {
    Detect,
    LimitWine,
}

#[derive(Debug, Clone)]
pub struct LaunchSettings {
    pub wallpaper_exe: String,
    pub fps_limit: u32,
    pub show_fps: bool,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub play_in_window_title: String,
    pub wm_class_contains: String,
    pub cgroup_enabled: bool,
    pub cgroup_mode: CgroupMode,
    pub cgroup_memory_max: Option<String>,
    pub cgroup_cpu_max: Option<String>,
    pub hide_debug_window: bool,
    pub hidden_workspace_name: String,
}

impl Default for LaunchSettings {
    fn default() -> Self {
        Self {
            wallpaper_exe: String::new(),
            fps_limit: 30,
            show_fps: false,
            width: 2560,
            height: 1600,
            x: 100,
            y: 100,
            play_in_window_title: "WE-DEBUG-WINDOW".to_string(),
            wm_class_contains: "wallpaper64".to_string(),
            cgroup_enabled: false,
            cgroup_mode: CgroupMode::Detect,
            cgroup_memory_max: None,
            cgroup_cpu_max: None,
            hide_debug_window: true,
            hidden_workspace_name: "we-hidden".to_string(),
        }
    }
}

impl Default for CgroupConfig {
    fn default() -> Self {
        Self { enabled: false, mode: CgroupMode::Detect, memory_max: None, cpu_max: None }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            fps_limit: 30,
            restart_wine_on_exit: true,
            refind_window_on_capture_error: true,
            show_fps: false,
            fps_report_interval_secs: 1,
            scale_mode: ScaleMode::Cover,
            hide_debug_window: true,
            hidden_workspace_name: "we-hidden".to_string(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                fps_limit: 30,
                restart_wine_on_exit: true,
                refind_window_on_capture_error: true,
                show_fps: false,
                fps_report_interval_secs: 1,
                scale_mode: ScaleMode::Cover,
                hide_debug_window: true,
                hidden_workspace_name: "we-hidden".to_string(),
            },
            wine: WineConfig {
                command: "wine".to_string(),
                wallpaper_exe: String::new(),
                args: Vec::new(),
            },
            capture: CaptureConfig {
                wm_class_contains: "wallpaper64".to_string(),
                title_contains: "WE-DEBUG-WINDOW".to_string(),
                output_window_map: BTreeMap::new(),
            },
            runtime: None,
            cgroup: CgroupConfig::default(),
        }
    }
}

pub fn build_config(
    settings: &LaunchSettings,
    wallpaper_type: WallpaperType,
    project_json: &Path,
    video_file: Option<&Path>,
) -> AppConfig {
    let mut cfg = AppConfig::default();

    cfg.general.fps_limit = settings.fps_limit;
    cfg.general.show_fps = settings.show_fps;
    cfg.general.hide_debug_window = settings.hide_debug_window;
    cfg.general.hidden_workspace_name = settings.hidden_workspace_name.clone();
    cfg.wine.command = "wine".to_string();
    cfg.wine.wallpaper_exe = settings.wallpaper_exe.clone();
    cfg.capture.wm_class_contains = settings.wm_class_contains.clone();
    cfg.capture.title_contains = settings.play_in_window_title.clone();
    cfg.cgroup.enabled = settings.cgroup_enabled;
    cfg.cgroup.mode = settings.cgroup_mode;
    cfg.cgroup.memory_max = settings.cgroup_memory_max.clone();
    cfg.cgroup.cpu_max = settings.cgroup_cpu_max.clone();

    match wallpaper_type {
        WallpaperType::Video => {
            cfg.general.restart_wine_on_exit = false;
            cfg.general.refind_window_on_capture_error = false;
            cfg.runtime = Some(RuntimeConfig {
                mode: RuntimeMode::VideoNative,
                wallpaper_type: RuntimeWallpaperType::Video,
                video_file: video_file.map(|p| p.display().to_string()),
            });
            cfg.wine.args.clear();
        }
        WallpaperType::Scene => {
            cfg.runtime = Some(RuntimeConfig {
                mode: RuntimeMode::WineLayerd,
                wallpaper_type: RuntimeWallpaperType::Scene,
                video_file: None,
            });
            cfg.wine.args = vec![
                "-control".to_string(),
                "openWallpaper".to_string(),
                "-file".to_string(),
                project_json.display().to_string(),
                "-playInWindow".to_string(),
                settings.play_in_window_title.clone(),
                "-width".to_string(),
                settings.width.to_string(),
                "-height".to_string(),
                settings.height.to_string(),
                "-x".to_string(),
                settings.x.to_string(),
                "-y".to_string(),
                settings.y.to_string(),
            ];
        }
        WallpaperType::Web => {
            cfg.runtime = Some(RuntimeConfig {
                mode: RuntimeMode::WineLayerd,
                wallpaper_type: RuntimeWallpaperType::Web,
                video_file: None,
            });
            cfg.wine.args = vec![
                "-control".to_string(),
                "openWallpaper".to_string(),
                "-file".to_string(),
                project_json.display().to_string(),
                "-playInWindow".to_string(),
                settings.play_in_window_title.clone(),
                "-width".to_string(),
                settings.width.to_string(),
                "-height".to_string(),
                settings.height.to_string(),
                "-x".to_string(),
                settings.x.to_string(),
                "-y".to_string(),
                settings.y.to_string(),
            ];
        }
        WallpaperType::Unknown => {
            cfg.runtime = Some(RuntimeConfig {
                mode: RuntimeMode::WineLayerd,
                wallpaper_type: RuntimeWallpaperType::Unknown,
                video_file: None,
            });
            cfg.wine.args = vec![
                "-control".to_string(),
                "openWallpaper".to_string(),
                "-file".to_string(),
                project_json.display().to_string(),
                "-playInWindow".to_string(),
                settings.play_in_window_title.clone(),
                "-width".to_string(),
                settings.width.to_string(),
                "-height".to_string(),
                settings.height.to_string(),
                "-x".to_string(),
                settings.x.to_string(),
                "-y".to_string(),
                settings.y.to_string(),
            ];
        }
    }

    cfg
}

pub fn save_config(path: &Path, config: &AppConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let toml = toml::to_string_pretty(config).context("failed to serialize config")?;
    fs::write(path, toml).with_context(|| format!("failed to write {}", path.display()))
}
