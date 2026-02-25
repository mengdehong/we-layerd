use std::{env, path::PathBuf};

const STEAM_COMMON_PATHS: &[&str] = &[
    ".local/share/Steam/steamapps/common",
    ".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/common",
    "snap/steam/common/.local/share/Steam/steamapps/common",
];

const STEAM_WORKSHOP_PATHS: &[&str] = &[
    ".local/share/Steam/steamapps/workshop/content",
    ".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/workshop/content",
    "snap/steam/common/.local/share/Steam/steamapps/workshop/content",
];

pub const WALLPAPER_ENGINE_APP_ID: u32 = 431960;

pub fn discover_wallpaper_engine_exe() -> Option<PathBuf> {
    let names = ["wallpaper64.exe", "wallpaper32.exe"];
    for root in steam_common_roots() {
        let app_dir = root.join("wallpaper_engine");
        for name in names {
            let candidate = app_dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn discover_workshop_wallpaper_root() -> Option<PathBuf> {
    for root in steam_workshop_roots() {
        let candidate = root.join(WALLPAPER_ENGINE_APP_ID.to_string());
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

pub fn default_config_path() -> Option<PathBuf> {
    let home = env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/we-layerd/config.toml"))
}

fn steam_common_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = env::var_os("HOME") {
        let base = PathBuf::from(home);
        for rel in STEAM_COMMON_PATHS {
            roots.push(base.join(rel));
        }
    }
    roots
}

fn steam_workshop_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = env::var_os("HOME") {
        let base = PathBuf::from(home);
        for rel in STEAM_WORKSHOP_PATHS {
            roots.push(base.join(rel));
        }
    }
    roots
}
