use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{steam::WALLPAPER_ENGINE_APP_ID, wallpaper::WallpaperType};

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
    pub disable_debug_window_input: bool,
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
    #[serde(default)]
    pub command_mode: WineCommandMode,
    pub wallpaper_exe: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WineCommandMode {
    ExeWithArgs,
    CommandOnly,
}

impl Default for WineCommandMode {
    fn default() -> Self {
        Self::ExeWithArgs
    }
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
    pub launcher: WindowsLauncher,
    pub wine_command: String,
    pub proton_path: Option<String>,
    pub fps_limit: u32,
    pub show_fps: bool,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub play_in_window_title: String,
    pub borderless: bool,
    pub wm_class_contains: String,
    pub cgroup_enabled: bool,
    pub cgroup_mode: CgroupMode,
    pub cgroup_memory_max: Option<String>,
    pub cgroup_cpu_max: Option<String>,
    pub hide_debug_window: bool,
    pub hidden_workspace_name: String,
    pub disable_debug_window_input: bool,
}

impl Default for LaunchSettings {
    fn default() -> Self {
        Self {
            wallpaper_exe: String::new(),
            launcher: WindowsLauncher::Wine,
            wine_command: "wine".to_string(),
            proton_path: None,
            fps_limit: 30,
            show_fps: false,
            width: 2560,
            height: 1600,
            x: 0,
            y: 0,
            play_in_window_title: "WE-DEBUG-WINDOW".to_string(),
            borderless: true,
            wm_class_contains: "wallpaper64".to_string(),
            cgroup_enabled: false,
            cgroup_mode: CgroupMode::Detect,
            cgroup_memory_max: None,
            cgroup_cpu_max: None,
            hide_debug_window: true,
            hidden_workspace_name: "top".to_string(),
            disable_debug_window_input: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowsLauncher {
    Wine,
    Proton,
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
            hidden_workspace_name: "top".to_string(),
            disable_debug_window_input: false,
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
                hidden_workspace_name: "top".to_string(),
                disable_debug_window_input: false,
            },
            wine: WineConfig {
                command: "wine".to_string(),
                command_mode: WineCommandMode::ExeWithArgs,
                wallpaper_exe: String::new(),
                args: Vec::new(),
                env: BTreeMap::new(),
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
    cfg.general.disable_debug_window_input = settings.disable_debug_window_input;
    cfg.wine.command = settings.wine_command.clone();
    cfg.wine.command_mode = WineCommandMode::ExeWithArgs;
    cfg.wine.wallpaper_exe = settings.wallpaper_exe.clone();
    cfg.wine.env.clear();
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
            if settings.borderless {
                cfg.wine.args.push("-borderless".to_string());
            }
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
            if settings.borderless {
                cfg.wine.args.push("-borderless".to_string());
            }
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
            if settings.borderless {
                cfg.wine.args.push("-borderless".to_string());
            }
        }
    }

    if settings.launcher == WindowsLauncher::Proton {
        if let Some(proton) = settings.proton_path.as_ref().filter(|s| !s.trim().is_empty()) {
            let exe_path = Path::new(&settings.wallpaper_exe);
            let launcher_exe = exe_path
                .parent()
                .map(|p| p.join("launcher.exe"))
                .unwrap_or_else(|| Path::new("launcher.exe").to_path_buf());
            let wallpaper_name = exe_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("wallpaper64.exe")
                .to_string();
            cfg.wine.command = proton.clone();
            cfg.wine.command_mode = WineCommandMode::CommandOnly;
            let mut proton_args = vec![
                "run".to_string(),
                launcher_exe.display().to_string(),
                "-run".to_string(),
                wallpaper_name,
                "-nobrowse".to_string(),
            ];
            proton_args.extend(cfg.wine.args.clone());
            cfg.wine.args = proton_args;
            if let Some(steam_root) = derive_steam_root_from_proton_path(proton) {
                cfg.wine.env.insert(
                    "STEAM_COMPAT_CLIENT_INSTALL_PATH".to_string(),
                    steam_root.display().to_string(),
                );
                cfg.wine.env.insert(
                    "STEAM_COMPAT_DATA_PATH".to_string(),
                    steam_root
                        .join("steamapps")
                        .join("compatdata")
                        .join(WALLPAPER_ENGINE_APP_ID.to_string())
                        .display()
                        .to_string(),
                );
            }
        }
    }

    cfg
}

fn derive_steam_root_from_proton_path(proton_path: &str) -> Option<std::path::PathBuf> {
    let p = Path::new(proton_path);
    let parent = p.parent()?;
    let parent_name = parent.file_name()?.to_str()?;
    if parent_name.is_empty() {
        return None;
    }

    if parent.parent()?.file_name()?.to_str()? == "common"
        && parent.parent()?.parent()?.file_name()?.to_str()? == "steamapps"
    {
        return parent.parent()?.parent()?.parent().map(|v| v.to_path_buf());
    }

    if parent.parent()?.file_name()?.to_str()? == "compatibilitytools.d" {
        return parent.parent()?.parent().map(|v| v.to_path_buf());
    }

    None
}

pub fn save_config(path: &Path, config: &AppConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let toml = toml::to_string_pretty(config).context("failed to serialize config")?;
    fs::write(path, toml).with_context(|| format!("failed to write {}", path.display()))
}

pub fn load_launch_settings(path: &Path) -> Result<LaunchSettings> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let cfg: AppConfig =
        toml::from_str(&raw).with_context(|| format!("invalid TOML in {}", path.display()))?;
    let mut settings = LaunchSettings::default();

    settings.fps_limit = cfg.general.fps_limit;
    settings.show_fps = cfg.general.show_fps;
    settings.hide_debug_window = cfg.general.hide_debug_window;
    settings.hidden_workspace_name = cfg.general.hidden_workspace_name;
    settings.disable_debug_window_input = cfg.general.disable_debug_window_input;

    settings.wm_class_contains = cfg.capture.wm_class_contains;
    settings.play_in_window_title = cfg.capture.title_contains;

    settings.cgroup_enabled = cfg.cgroup.enabled;
    settings.cgroup_mode = cfg.cgroup.mode;
    settings.cgroup_memory_max = cfg.cgroup.memory_max;
    settings.cgroup_cpu_max = cfg.cgroup.cpu_max;

    settings.wallpaper_exe = cfg.wine.wallpaper_exe;
    match cfg.wine.command_mode {
        WineCommandMode::ExeWithArgs => {
            settings.launcher = WindowsLauncher::Wine;
            settings.wine_command = cfg.wine.command;
            settings.proton_path = None;
        }
        WineCommandMode::CommandOnly => {
            settings.launcher = WindowsLauncher::Proton;
            settings.proton_path = Some(cfg.wine.command);
        }
    }

    if let Some(width) = arg_value(&cfg.wine.args, "-width").and_then(|v| v.parse::<u32>().ok()) {
        settings.width = width.max(1);
    }
    if let Some(height) = arg_value(&cfg.wine.args, "-height").and_then(|v| v.parse::<u32>().ok()) {
        settings.height = height.max(1);
    }
    if let Some(x) = arg_value(&cfg.wine.args, "-x").and_then(|v| v.parse::<i32>().ok()) {
        settings.x = x;
    }
    if let Some(y) = arg_value(&cfg.wine.args, "-y").and_then(|v| v.parse::<i32>().ok()) {
        settings.y = y;
    }
    settings.borderless = cfg.wine.args.iter().any(|v| v == "-borderless");

    Ok(settings)
}

fn arg_value<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    let idx = args.iter().position(|arg| arg == key)?;
    args.get(idx + 1).map(String::as_str)
}
