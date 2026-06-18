use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{steam::WALLPAPER_ENGINE_APP_ID, wallpaper::WallpaperType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub isolation: IsolationConfig,
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
    #[serde(default)]
    pub workshop_path: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsolationConfig {
    #[serde(default)]
    pub mode: IsolationMode,
    #[serde(default = "default_isolation_command")]
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default = "default_isolation_startup_timeout_secs")]
    pub startup_timeout_secs: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IsolationMode {
    #[default]
    None,
    GamescopeHeadless,
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
    pub workshop_path: String,
    pub launcher: WindowsLauncher,
    pub wine_command: String,
    pub proton_path: Option<String>,
    pub fps_limit: u32,
    pub show_fps: bool,
    pub isolation_mode: IsolationMode,
    pub isolation_command: String,
    pub isolation_width: Option<u32>,
    pub isolation_height: Option<u32>,
    pub isolation_startup_timeout_secs: u64,
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
            workshop_path: String::new(),
            launcher: WindowsLauncher::Wine,
            wine_command: "wine".to_string(),
            proton_path: None,
            fps_limit: 30,
            show_fps: false,
            isolation_mode: IsolationMode::None,
            isolation_command: default_isolation_command(),
            isolation_width: None,
            isolation_height: None,
            isolation_startup_timeout_secs: default_isolation_startup_timeout_secs(),
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
            isolation: IsolationConfig::default(),
            wine: WineConfig {
                command: "wine".to_string(),
                command_mode: WineCommandMode::ExeWithArgs,
                wallpaper_exe: String::new(),
                workshop_path: String::new(),
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

impl Default for IsolationConfig {
    fn default() -> Self {
        Self {
            mode: IsolationMode::None,
            command: default_isolation_command(),
            width: None,
            height: None,
            startup_timeout_secs: default_isolation_startup_timeout_secs(),
        }
    }
}

fn default_isolation_command() -> String {
    "gamescope".to_string()
}

fn default_isolation_startup_timeout_secs() -> u64 {
    10
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
    cfg.isolation.mode = settings.isolation_mode;
    cfg.isolation.command = settings.isolation_command.clone();
    cfg.isolation.width = settings.isolation_width;
    cfg.isolation.height = settings.isolation_height;
    cfg.isolation.startup_timeout_secs = settings.isolation_startup_timeout_secs.max(1);
    cfg.wine.command = settings.wine_command.clone();
    cfg.wine.command_mode = WineCommandMode::ExeWithArgs;
    cfg.wine.wallpaper_exe = settings.wallpaper_exe.clone();
    cfg.wine.workshop_path = settings.workshop_path.clone();
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
            apply_window_wallpaper_config(
                &mut cfg,
                settings,
                project_json,
                RuntimeWallpaperType::Scene,
            );
        }
        WallpaperType::Web => {
            apply_window_wallpaper_config(
                &mut cfg,
                settings,
                project_json,
                RuntimeWallpaperType::Web,
            );
        }
        WallpaperType::Unknown => {
            apply_window_wallpaper_config(
                &mut cfg,
                settings,
                project_json,
                RuntimeWallpaperType::Unknown,
            );
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
            let proton_steam_root = derive_steam_root_from_path(Path::new(proton));
            if let Some(steam_root) = proton_steam_root.as_ref() {
                cfg.wine.env.insert(
                    "STEAM_COMPAT_CLIENT_INSTALL_PATH".to_string(),
                    steam_root.display().to_string(),
                );
            }
            if let Some(wallpaper_root) =
                derive_steam_root_from_path(exe_path).or(proton_steam_root)
            {
                cfg.wine.env.insert(
                    "STEAM_COMPAT_DATA_PATH".to_string(),
                    wallpaper_root
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

fn apply_window_wallpaper_config(
    cfg: &mut AppConfig,
    settings: &LaunchSettings,
    project_json: &Path,
    wallpaper_type: RuntimeWallpaperType,
) {
    cfg.runtime =
        Some(RuntimeConfig { mode: RuntimeMode::WineLayerd, wallpaper_type, video_file: None });
    cfg.wine.args = build_window_wallpaper_args(settings, project_json);
}

fn build_window_wallpaper_args(settings: &LaunchSettings, project_json: &Path) -> Vec<String> {
    let mut args = vec![
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
        args.push("-borderless".to_string());
    }
    args
}

fn derive_steam_root_from_path(p: &Path) -> Option<std::path::PathBuf> {
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
    settings.isolation_mode = cfg.isolation.mode;
    settings.isolation_command = cfg.isolation.command;
    settings.isolation_width = cfg.isolation.width;
    settings.isolation_height = cfg.isolation.height;
    settings.isolation_startup_timeout_secs = cfg.isolation.startup_timeout_secs.max(1);

    settings.wm_class_contains = cfg.capture.wm_class_contains;
    settings.play_in_window_title = cfg.capture.title_contains;

    settings.cgroup_enabled = cfg.cgroup.enabled;
    settings.cgroup_mode = cfg.cgroup.mode;
    settings.cgroup_memory_max = cfg.cgroup.memory_max;
    settings.cgroup_cpu_max = cfg.cgroup.cpu_max;

    settings.wallpaper_exe = cfg.wine.wallpaper_exe;
    settings.workshop_path = cfg.wine.workshop_path;

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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        build_config, load_launch_settings, IsolationMode, LaunchSettings, WindowsLauncher,
    };
    use crate::wallpaper::WallpaperType;

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("we-layerd-{name}-{}-{nanos}", std::process::id()))
    }

    #[test]
    fn load_launch_settings_accepts_legacy_config_without_workshop_path() {
        let path = unique_temp_path("legacy-config.toml");
        let toml = r#"
[general]
fps_limit = 30
restart_wine_on_exit = true
refind_window_on_capture_error = true
show_fps = false
fps_report_interval_secs = 1
scale_mode = "cover"
hide_debug_window = true
hidden_workspace_name = "top"
disable_debug_window_input = false

[wine]
command = "wine"
command_mode = "exe_with_args"
wallpaper_exe = "/tmp/wallpaper64.exe"
args = ["-playInWindow", "WE-DEBUG-WINDOW"]

[capture]
wm_class_contains = "wallpaper64"
title_contains = "WE-DEBUG-WINDOW"

[cgroup]
enabled = false
mode = "detect"
"#;

        fs::write(&path, toml).expect("failed to write temp config");

        let settings = load_launch_settings(&path).expect("legacy config should load");
        assert_eq!(settings.wallpaper_exe, "/tmp/wallpaper64.exe");
        assert_eq!(settings.workshop_path, "");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn proton_mode_falls_back_to_proton_root_for_compat_data_path() {
        let mut settings = LaunchSettings::default();
        settings.launcher = WindowsLauncher::Proton;
        settings.proton_path = Some("/steam-root/steamapps/common/Proton 9/proton".to_string());
        settings.wallpaper_exe = "/opt/custom/wallpaper32.exe".to_string();
        settings.workshop_path = "/tmp/workshop".to_string();

        let cfg =
            build_config(&settings, WallpaperType::Scene, Path::new("/tmp/project.json"), None);

        assert_eq!(
            cfg.wine.env.get("STEAM_COMPAT_CLIENT_INSTALL_PATH").map(String::as_str),
            Some("/steam-root")
        );
        assert_eq!(
            cfg.wine.env.get("STEAM_COMPAT_DATA_PATH").map(String::as_str),
            Some("/steam-root/steamapps/compatdata/431960")
        );
    }

    #[test]
    fn proton_mode_prefers_wallpaper_exe_root_for_compat_data_path() {
        let mut settings = LaunchSettings::default();
        settings.launcher = WindowsLauncher::Proton;
        settings.proton_path = Some("/steam-root/steamapps/common/Proton 9/proton".to_string());
        settings.wallpaper_exe =
            "/other-steam-root/steamapps/common/wallpaper_engine/wallpaper64.exe".to_string();

        let cfg =
            build_config(&settings, WallpaperType::Scene, Path::new("/tmp/project.json"), None);

        assert_eq!(
            cfg.wine.env.get("STEAM_COMPAT_DATA_PATH").map(String::as_str),
            Some("/other-steam-root/steamapps/compatdata/431960")
        );
    }

    #[test]
    fn load_launch_settings_reads_isolation_config() {
        let path = unique_temp_path("isolation-config.toml");
        let toml = r#"
[general]
fps_limit = 30
restart_wine_on_exit = true
refind_window_on_capture_error = true
show_fps = false
fps_report_interval_secs = 1
scale_mode = "cover"
hide_debug_window = true
hidden_workspace_name = "top"
disable_debug_window_input = false

[isolation]
mode = "gamescope_headless"
command = "gamescope-custom"
width = 2560
height = 1600
startup_timeout_secs = 12

[wine]
command = "wine"
command_mode = "exe_with_args"
wallpaper_exe = "/tmp/wallpaper64.exe"
args = ["-playInWindow", "WE-DEBUG-WINDOW"]

[capture]
wm_class_contains = "wallpaper64"
title_contains = "WE-DEBUG-WINDOW"

[cgroup]
enabled = false
mode = "detect"
"#;

        fs::write(&path, toml).expect("failed to write temp config");

        let settings = load_launch_settings(&path).expect("isolation config should load");
        assert_eq!(settings.isolation_mode, IsolationMode::GamescopeHeadless);
        assert_eq!(settings.isolation_command, "gamescope-custom");
        assert_eq!(settings.isolation_width, Some(2560));
        assert_eq!(settings.isolation_height, Some(1600));
        assert_eq!(settings.isolation_startup_timeout_secs, 12);

        let _ = fs::remove_file(path);
    }
}
