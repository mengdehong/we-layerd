use std::{env, fmt, process::Command};

use iced::{
    widget::{button, checkbox, column, container, pick_list, row, scrollable, text, text_input},
    Background, Border, Color, Element, Fill, Theme,
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
    pub executable_variant: ExecutableVariantOption,
    pub workshop_path: String,
    pub launcher_mode: LauncherModeOption,
    pub wine_command: String,
    pub proton_path: String,
    pub fps_limit: String,
    pub show_fps: bool,
    pub borderless: bool,
    pub hide_debug_window: bool,
    pub hidden_workspace_name: String,
    pub selected_resolution: Option<ResolutionOption>,
    pub cgroup_enabled: bool,
    pub cgroup_mode: CgroupModeOption,
    pub cgroup_memory_max: String,
    pub cgroup_cpu_max: String,
    pub status_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutableVariantOption {
    Wallpaper64,
    Wallpaper32,
}

impl ExecutableVariantOption {
    pub const fn filename(self) -> &'static str {
        match self {
            Self::Wallpaper64 => "wallpaper64.exe",
            Self::Wallpaper32 => "wallpaper32.exe",
        }
    }
}

impl fmt::Display for ExecutableVariantOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.filename())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LauncherModeOption {
    Wine,
    Proton,
}

impl fmt::Display for LauncherModeOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wine => write!(f, "Wine"),
            Self::Proton => write!(f, "Proton"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LauncherChoice {
    pub label: String,
    pub value: String,
}

impl fmt::Display for LauncherChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CgroupModeOption {
    Detect,
    LimitWine,
}

impl fmt::Display for CgroupModeOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Detect => write!(f, "Detect (observe self + wine)"),
            Self::LimitWine => write!(f, "Limit Wine only"),
        }
    }
}

pub fn build_settings_overlay<'a>(
    ui_settings: &'a UiSettings,
    supported_resolutions: &'a [ResolutionOption],
    wine_commands: &'a [LauncherChoice],
    proton_versions: &'a [LauncherChoice],
) -> Element<'a, Message> {
    let wallpaper_path_display = format_path_for_display(&ui_settings.wallpaper_exe, 64);
    let workshop_path_display = format_path_for_display(&ui_settings.workshop_path, 64);

    let mut content = column![
        text("Settings").size(26),
        text("Windows Launcher").size(14),
        pick_list(
            vec![LauncherModeOption::Wine, LauncherModeOption::Proton],
            Some(ui_settings.launcher_mode),
            Message::LauncherModeSelected,
        )
        .padding(10),
        text("Wine Command").size(14),
        pick_list(
            wine_commands.to_vec(),
            pick_selected_choice(wine_commands, &ui_settings.wine_command),
            Message::WineCommandSelected,
        )
        .placeholder("Select Wine command")
        .padding(10),
        text("Proton Version").size(14),
        pick_list(
            proton_versions.to_vec(),
            pick_selected_choice(proton_versions, &ui_settings.proton_path),
            Message::ProtonVersionSelected,
        )
        .placeholder("Select Proton version")
        .padding(10),
        text_input("/path/to/proton", &ui_settings.proton_path)
            .on_input(Message::ProtonPathChanged)
            .padding(10),
        text("Wallpaper Engine Path").size(14),
        text("Executable Variant").size(14),
        pick_list(
            vec![ExecutableVariantOption::Wallpaper64, ExecutableVariantOption::Wallpaper32],
            Some(ui_settings.executable_variant),
            Message::ExecutableVariantSelected,
        )
        .padding(10),
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
        checkbox(ui_settings.borderless)
            .label("Open scene/web wallpaper window as borderless")
            .on_toggle(Message::BorderlessToggled),
        checkbox(ui_settings.hide_debug_window)
            .label("Hide WE debug window automatically")
            .on_toggle(Message::HideDebugWindowToggled),
        text("Hidden Workspace").size(14),
        text_input("top", &ui_settings.hidden_workspace_name)
            .on_input(Message::HiddenWorkspaceNameChanged)
            .padding(10),
        text("Use `top` for niri first workspace; on Hyprland this is the special workspace name.")
            .size(12),
        text("Resolution").size(14),
        pick_list(
            supported_resolutions.to_vec(),
            ui_settings.selected_resolution.clone(),
            Message::ResolutionSelected,
        )
        .placeholder("Choose a resolution")
        .padding(10),
        text("Cgroup").size(18),
        checkbox(ui_settings.cgroup_enabled)
            .label("Enable cgroup metrics / limits")
            .on_toggle(Message::CgroupEnabledToggled),
    ]
    .spacing(10);

    if ui_settings.cgroup_enabled {
        content = content
            .push(text("Cgroup Mode").size(14))
            .push(
                pick_list(
                    vec![CgroupModeOption::Detect, CgroupModeOption::LimitWine],
                    Some(ui_settings.cgroup_mode),
                    Message::CgroupModeSelected,
                )
                .padding(10),
            )
            .push(text("Memory Limit (memory.max)").size(14))
            .push(
                text_input("e.g. 2147483648 or max", &ui_settings.cgroup_memory_max)
                    .on_input(Message::CgroupMemoryMaxChanged)
                    .padding(10),
            )
            .push(text("CPU Limit (cpu.max)").size(14))
            .push(
                text_input("e.g. 50000 100000 or max 100000", &ui_settings.cgroup_cpu_max)
                    .on_input(Message::CgroupCpuMaxChanged)
                    .padding(10),
            )
            .push(
                row![
                    text("Runtime Status").size(14),
                    button(text("Refresh")).on_press(Message::RefreshStatus),
                ]
                .spacing(10),
            )
            .push(
                container(text(&ui_settings.status_text).size(12))
                    .width(Fill)
                    .padding(10)
                    .style(status_panel_style),
            );
    }

    let card_content = scrollable(content);

    let card =
        container(card_content).width(760).height(620).padding(16).style(settings_card_style);

    container(card)
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .center_y(Fill)
        .padding(20)
        .style(settings_overlay_bg_style)
        .into()
}

pub fn detect_supported_resolutions() -> Vec<ResolutionOption> {
    let mut values = parse_wlrandr_resolutions();
    if values.is_empty() {
        values = parse_xrandr_resolutions();
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
    supported.iter().find(|r| r.width == width && r.height == height).cloned()
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

fn pick_selected_choice(choices: &[LauncherChoice], value: &str) -> Option<LauncherChoice> {
    choices.iter().find(|c| c.value == value).cloned()
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

fn settings_overlay_bg_style(theme: &Theme) -> container::Style {
    let is_light = matches!(theme, Theme::Light);
    container::Style {
        background: Some(Background::Color(if is_light {
            Color::from_rgba(1.0, 1.0, 1.0, 0.45)
        } else {
            Color::from_rgba(0.0, 0.0, 0.0, 0.45)
        })),
        ..Default::default()
    }
}

fn settings_card_style(theme: &Theme) -> container::Style {
    let is_light = matches!(theme, Theme::Light);
    container::Style {
        background: Some(Background::Color(if is_light {
            Color::from_rgb8(247, 250, 255)
        } else {
            Color::from_rgb8(26, 30, 34)
        })),
        text_color: Some(if is_light { Color::from_rgb8(20, 27, 34) } else { Color::WHITE }),
        border: Border {
            radius: 16.0.into(),
            width: 1.0,
            color: if is_light {
                Color::from_rgba8(58, 94, 132, 0.25)
            } else {
                Color::from_rgba8(180, 210, 255, 0.18)
            },
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
            blur_radius: 18.0,
            offset: iced::Vector::new(0.0, 6.0),
        },
        ..Default::default()
    }
}

fn status_panel_style(theme: &Theme) -> container::Style {
    let is_light = matches!(theme, Theme::Light);
    container::Style {
        background: Some(Background::Color(if is_light {
            Color::from_rgb8(236, 242, 249)
        } else {
            Color::from_rgb8(20, 24, 28)
        })),
        border: Border {
            radius: 12.0.into(),
            width: 1.0,
            color: if is_light {
                Color::from_rgba8(70, 110, 155, 0.28)
            } else {
                Color::from_rgba8(160, 198, 241, 0.2)
            },
        },
        ..Default::default()
    }
}
