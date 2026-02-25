use std::{env, fmt, process::Command};

use iced::{
    widget::{button, checkbox, column, container, pick_list, row, text, text_input},
    Element, Fill,
};

use crate::Message;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionOption {
    pub width: u32,
    pub height: u32,
}

impl fmt::Display for ResolutionOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} x {}", self.width, self.height)
    }
}

#[derive(Debug, Clone)]
pub struct UiSettings {
    pub wallpaper_exe: String,
    pub workshop_path: String,
    pub fps_limit: String,
    pub show_fps: bool,
    pub selected_resolution: Option<ResolutionOption>,
}

pub fn build_settings_overlay<'a>(
    ui_settings: &'a UiSettings,
    supported_resolutions: &'a [ResolutionOption],
) -> Element<'a, Message> {
    let wallpaper_path_display = format_path_for_display(&ui_settings.wallpaper_exe, 64);
    let workshop_path_display = format_path_for_display(&ui_settings.workshop_path, 64);

    let card = container(
        column![
            text("Settings").size(26),
            text("Wallpaper Engine Path").size(14),
            row![
                text_input("/path/to/wallpaper64.exe", &ui_settings.wallpaper_exe)
                    .on_input(Message::WallpaperExeChanged)
                    .padding(10)
                    .on_submit(Message::AutoScan)
                    .width(Fill),
                button(text("Browse")).on_press(Message::PickWallpaperExe),
            ]
            .spacing(10),
            text(wallpaper_path_display).size(12),
            text("Workshop Path").size(14),
            row![
                text_input("/path/to/workshop/content/431960", &ui_settings.workshop_path)
                    .on_input(Message::WorkshopPathChanged)
                    .padding(10)
                    .on_submit(Message::AutoScan)
                    .width(Fill),
                button(text("Browse")).on_press(Message::PickWorkshopPath),
            ]
            .spacing(10),
            text(workshop_path_display).size(12),
            text("Frame Rate Limit (FPS)").size(14),
            text_input("30", &ui_settings.fps_limit).on_input(Message::FpsLimitChanged).padding(10),
            checkbox(ui_settings.show_fps)
                .label("Show realtime FPS")
                .on_toggle(Message::ShowFpsToggled),
            text("Resolution").size(14),
            pick_list(
                supported_resolutions.to_vec(),
                ui_settings.selected_resolution.clone(),
                Message::ResolutionSelected,
            )
            .placeholder("Choose a resolution")
            .padding(10),
        ]
        .spacing(10),
    )
    .width(680)
    .padding(16);

    container(card).width(Fill).height(Fill).center_x(Fill).center_y(Fill).padding(20).into()
}

pub fn detect_supported_resolutions() -> Vec<ResolutionOption> {
    let mut values = parse_xrandr_resolutions();
    if values.is_empty() {
        values = parse_wlrandr_resolutions();
    }
    if values.is_empty() {
        values.push(ResolutionOption { width: 1920, height: 1080 });
    }
    values.sort_by_key(|v| (v.width, v.height));
    values.dedup();
    values
}

pub fn pick_initial_resolution(
    supported: &[ResolutionOption],
    width: u32,
    height: u32,
) -> Option<ResolutionOption> {
    if let Some(found) = supported.iter().find(|r| r.width == width && r.height == height).cloned()
    {
        return Some(found);
    }
    supported.last().cloned()
}

fn parse_xrandr_resolutions() -> Vec<ResolutionOption> {
    let Ok(output) = Command::new("xrandr").arg("--query").output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    parse_resolutions_from_text(String::from_utf8_lossy(&output.stdout).as_ref())
}

fn parse_wlrandr_resolutions() -> Vec<ResolutionOption> {
    let Ok(output) = Command::new("wlr-randr").output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    parse_resolutions_from_text(String::from_utf8_lossy(&output.stdout).as_ref())
}

fn parse_resolutions_from_text(raw: &str) -> Vec<ResolutionOption> {
    let mut result = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        for token in line.split_whitespace() {
            let Some((w, h)) = token.split_once('x') else {
                continue;
            };
            if let (Ok(width), Ok(height)) = (w.parse::<u32>(), h.parse::<u32>()) {
                if width >= 640 && height >= 360 {
                    result.push(ResolutionOption { width, height });
                }
            }
        }
    }
    result
}

fn format_path_for_display(path: &str, max_chars: usize) -> String {
    let mut rendered = path.to_string();
    if let Ok(home) = env::var("HOME") {
        if rendered.starts_with(&home) {
            rendered = rendered.replacen(&home, "~", 1);
        }
    }
    if rendered.chars().count() <= max_chars {
        return rendered;
    }

    let keep_head = max_chars.saturating_sub(3) / 2;
    let keep_tail = max_chars.saturating_sub(3) - keep_head;
    let head: String = rendered.chars().take(keep_head).collect();
    let tail: String =
        rendered.chars().rev().take(keep_tail).collect::<String>().chars().rev().collect();
    format!("{head}...{tail}")
}
