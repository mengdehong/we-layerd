use std::{
    env, fmt,
    path::PathBuf,
    process::{Child, Command},
};

use iced::{
    alignment::{Horizontal, Vertical},
    widget::{
        button, checkbox, column, container, image, pick_list, row, scrollable, stack, svg, text,
        text_input,
    },
    window, Background, Border, Color, ContentFit, Element, Fill, Size, Subscription, Task, Theme,
};
use we_core::{
    config::{build_config, save_config, LaunchSettings},
    steam::{self, WallpaperEngineInstallState},
    wallpaper::{self, WallpaperEntry, WallpaperType},
};

fn main() -> iced::Result {
    iced::application("we-gui", update, view).subscription(subscription).run_with(App::init)
}

struct App {
    entries: Vec<WallpaperEntry>,
    selected_id: Option<String>,
    config_path: PathBuf,
    runtime_child: Option<Child>,
    selected_type: Option<WallpaperType>,
    selected_video_file: Option<PathBuf>,
    viewport_width: f32,
    layerd_available: bool,
    launch_settings: LaunchSettings,
    ui_settings: UiSettings,
    show_settings: bool,
    supported_resolutions: Vec<ResolutionOption>,
    install_notice: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    AutoScan,
    Scanned(Result<Vec<WallpaperEntry>, String>),
    SelectWallpaper(usize),
    PlayPressed,
    StopPressed,
    SettingsPressed,
    PickWallpaperExe,
    PickWorkshopPath,
    WallpaperExePicked(Option<PathBuf>),
    WorkshopPathPicked(Option<PathBuf>),
    FpsLimitChanged(String),
    ShowFpsToggled(bool),
    ResolutionSelected(ResolutionOption),
    WindowResized(Size),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolutionOption {
    width: u32,
    height: u32,
}

impl fmt::Display for ResolutionOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} x {}", self.width, self.height)
    }
}

#[derive(Debug, Clone)]
struct UiSettings {
    wallpaper_exe: String,
    workshop_path: String,
    fps_limit: String,
    show_fps: bool,
    selected_resolution: Option<ResolutionOption>,
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::AutoScan => Task::perform(
            scan_wallpapers_from(app.ui_settings.workshop_path.clone()),
            Message::Scanned,
        ),
        Message::Scanned(result) => match result {
            Ok(entries) => {
                app.entries = entries;
                Task::none()
            }
            Err(_err) => Task::none(),
        },
        Message::SelectWallpaper(index) => {
            let Some(entry) = app.entries.get(index).cloned() else {
                return Task::none();
            };

            app.selected_id = Some(entry.id.clone());
            app.selected_type = Some(entry.ty);
            app.selected_video_file = entry.source_file.clone();
            let cfg = build_config(
                &app.launch_settings,
                entry.ty,
                &entry.project_json,
                entry.source_file.as_deref(),
            );
            let _ = save_config(&app.config_path, &cfg);
            Task::none()
        }
        Message::PlayPressed => {
            if let Some(child) = app.runtime_child.as_mut() {
                if let Ok(Some(_)) = child.try_wait() {
                    app.runtime_child = None;
                }
            }

            if app.runtime_child.is_some() {
                return Task::none();
            }

            if !app.layerd_available {
                return Task::none();
            }

            let spawn =
                Command::new("we-layerd").arg("run").arg("--config").arg(&app.config_path).spawn();

            match spawn {
                Ok(child) => {
                    app.runtime_child = Some(child);
                }
                Err(_err) => {}
            }
            Task::none()
        }
        Message::StopPressed => {
            if let Some(mut child) = app.runtime_child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
            Task::none()
        }
        Message::SettingsPressed => {
            app.show_settings = !app.show_settings;
            Task::none()
        }
        Message::PickWallpaperExe => Task::perform(
            async { rfd::FileDialog::new().set_title("Select wallpaper64.exe").pick_file() },
            Message::WallpaperExePicked,
        ),
        Message::PickWorkshopPath => Task::perform(
            async {
                rfd::FileDialog::new().set_title("Select workshop 431960 folder").pick_folder()
            },
            Message::WorkshopPathPicked,
        ),
        Message::WallpaperExePicked(path) => {
            if let Some(path) = path {
                app.ui_settings.wallpaper_exe = path.display().to_string();
                sync_launch_settings(app);
            }
            Task::none()
        }
        Message::WorkshopPathPicked(path) => {
            if let Some(path) = path {
                app.ui_settings.workshop_path = path.display().to_string();
                return Task::perform(
                    scan_wallpapers_from(app.ui_settings.workshop_path.clone()),
                    Message::Scanned,
                );
            }
            Task::none()
        }
        Message::FpsLimitChanged(value) => {
            app.ui_settings.fps_limit = value;
            sync_launch_settings(app);
            Task::none()
        }
        Message::ShowFpsToggled(value) => {
            app.ui_settings.show_fps = value;
            sync_launch_settings(app);
            Task::none()
        }
        Message::ResolutionSelected(value) => {
            app.ui_settings.selected_resolution = Some(value);
            sync_launch_settings(app);
            Task::none()
        }
        Message::WindowResized(size) => {
            app.viewport_width = size.width;
            Task::none()
        }
    }
}

fn view(app: &App) -> Element<'_, Message> {
    let grid = build_wallpaper_grid(&app.entries, app.selected_id.as_ref(), app.viewport_width);

    let content = container(scrollable(grid).width(Fill).height(Fill)).width(Fill).height(Fill);

    let floating = container(
        column![
            button(
                svg(svg::Handle::from_memory(include_bytes!("../assets/icons/stop.svg")))
                    .width(24)
                    .height(24),
            )
            .width(52)
            .height(52)
            .style(secondary_fab_style)
            .on_press(Message::StopPressed),
            button(
                svg(svg::Handle::from_memory(include_bytes!("../assets/icons/settings.svg")))
                    .width(24)
                    .height(24),
            )
            .width(52)
            .height(52)
            .style(secondary_fab_style)
            .on_press(Message::SettingsPressed),
            button(
                svg(svg::Handle::from_memory(include_bytes!("../assets/icons/play_arrow.svg")))
                    .width(28)
                    .height(28),
            )
            .width(60)
            .height(60)
            .style(primary_fab_style)
            .on_press(Message::PlayPressed),
        ]
        .spacing(12),
    )
    .width(Fill)
    .height(Fill)
    .align_x(Horizontal::Right)
    .align_y(Vertical::Bottom)
    .padding(20);

    let mut notice_lines: Vec<String> = Vec::new();
    if !app.layerd_available {
        notice_lines.push("we-layerd not found in PATH".to_string());
    }
    if let Some(msg) = &app.install_notice {
        notice_lines.push(msg.clone());
    }

    let runtime_warning: Option<Element<'_, Message>> = if notice_lines.is_empty() {
        None
    } else {
        let mut warning_col = column!().spacing(6);
        for line in notice_lines {
            warning_col =
                warning_col.push(text(line).size(28).color(Color::from_rgb8(150, 205, 255)));
        }
        let warning = container(warning_col)
            .width(Fill)
            .height(Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Top)
            .padding(24);
        Some(warning.into())
    };

    let settings_overlay: Option<Element<'_, Message>> =
        if app.show_settings { Some(build_settings_overlay(app)) } else { None };

    match (runtime_warning, settings_overlay) {
        (Some(w), Some(s)) => stack![content, w, s, floating].into(),
        (Some(w), None) => stack![content, w, floating].into(),
        (None, Some(s)) => stack![content, s, floating].into(),
        (None, None) => stack![content, floating].into(),
    }
}

async fn scan_wallpapers_from(workshop_path: String) -> Result<Vec<WallpaperEntry>, String> {
    let workshop_root = if workshop_path.trim().is_empty() {
        steam::discover_workshop_wallpaper_root()
            .ok_or_else(|| "cannot find Steam workshop path for app 431960".to_string())?
    } else {
        PathBuf::from(workshop_path)
    };
    wallpaper::scan_workshop_wallpapers(&workshop_root).map_err(|e| e.to_string())
}

fn wallpaper_type_name(ty: WallpaperType) -> &'static str {
    match ty {
        WallpaperType::Video => "video",
        WallpaperType::Scene => "scene",
        WallpaperType::Web => "web",
        WallpaperType::Unknown => "unknown",
    }
}

fn subscription(_app: &App) -> Subscription<Message> {
    window::resize_events().map(|(_id, size)| Message::WindowResized(size))
}

impl App {
    fn init() -> (Self, Task<Message>) {
        let config_path =
            steam::default_config_path().unwrap_or_else(|| PathBuf::from("config.toml"));
        let mut launch_settings = LaunchSettings::default();
        let install_state = steam::detect_wallpaper_engine_install_state();
        let install_notice = match &install_state {
            WallpaperEngineInstallState::NotInstalled => Some(
                "Wallpaper Engine is not installed. Please install it, or choose paths in Settings."
                    .to_string(),
            ),
            WallpaperEngineInstallState::FirstRunRequired { .. } => Some(
                "Wallpaper Engine first-run setup is pending. Launch it once in Steam to run installer.exe."
                    .to_string(),
            ),
            WallpaperEngineInstallState::Installed { exe_path, .. } => {
                launch_settings.wallpaper_exe = exe_path.display().to_string();
                None
            }
        };
        let workshop_path = steam::discover_workshop_wallpaper_root()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let supported_resolutions = detect_supported_resolutions();
        let selected_resolution = pick_initial_resolution(
            &supported_resolutions,
            launch_settings.width,
            launch_settings.height,
        );
        let ui_settings = UiSettings {
            wallpaper_exe: launch_settings.wallpaper_exe.clone(),
            workshop_path,
            fps_limit: launch_settings.fps_limit.to_string(),
            show_fps: launch_settings.show_fps,
            selected_resolution,
        };
        (
            Self {
                entries: Vec::new(),
                selected_id: None,
                config_path,
                runtime_child: None,
                selected_type: None,
                selected_video_file: None,
                viewport_width: 1280.0,
                layerd_available: command_exists_in_path("we-layerd"),
                launch_settings,
                ui_settings,
                show_settings: false,
                supported_resolutions,
                install_notice,
            },
            Task::done(Message::AutoScan),
        )
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if let Some(mut child) = self.runtime_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn build_wallpaper_grid<'a>(
    entries: &'a [WallpaperEntry],
    selected_id: Option<&String>,
    width: f32,
) -> Element<'a, Message> {
    let spacing = 12.0;
    let card_width = 360.0;
    let cols = ((width - spacing) / (card_width + spacing)).floor().max(1.0) as usize;

    let mut root = column!().spacing(spacing as u16).padding(spacing as u16);

    for (row_index, chunk) in entries.chunks(cols).enumerate() {
        let mut r = row!().spacing(spacing as u16);
        for (inner, entry) in chunk.iter().enumerate() {
            let index = row_index * cols + inner;
            let is_selected = selected_id.map(|id| id == &entry.id).unwrap_or(false);
            r = r.push(make_wallpaper_card(entry, index, card_width, is_selected));
        }
        root = root.push(r);
    }

    root.into()
}

fn make_wallpaper_card<'a>(
    entry: &'a WallpaperEntry,
    index: usize,
    card_width: f32,
    is_selected: bool,
) -> Element<'a, Message> {
    let card_height = (card_width * 9.0 / 16.0).round();

    let media: Element<'a, Message> = if let Some(path) = &entry.preview {
        image(image::Handle::from_path(path))
            .width(card_width)
            .height(card_height)
            .content_fit(ContentFit::Cover)
            .into()
    } else {
        container(text(""))
            .width(card_width)
            .height(card_height)
            .style(|_theme: &Theme| container::Style {
                background: Some(Background::Color(Color::from_rgb8(18, 18, 18))),
                ..Default::default()
            })
            .into()
    };

    let chip = container(text(wallpaper_type_name(entry.ty)).size(12)).padding([3, 8]).style(
        |_theme: &Theme| container::Style {
            text_color: Some(Color::WHITE),
            background: Some(Background::Color(Color { r: 0.0, g: 0.0, b: 0.0, a: 0.45 })),
            border: Border { radius: 10.0.into(), ..Default::default() },
            ..Default::default()
        },
    );

    let chip_overlay = container(chip)
        .width(Fill)
        .height(Fill)
        .align_x(Horizontal::Right)
        .align_y(Vertical::Bottom)
        .padding(8);

    let composed = stack![media, chip_overlay];

    let border_color = if is_selected {
        Color::from_rgb8(255, 255, 255)
    } else {
        Color { r: 1.0, g: 1.0, b: 1.0, a: 0.1 }
    };

    let frame =
        container(composed).width(card_width).height(card_height).style(move |_theme: &Theme| {
            container::Style {
                border: Border {
                    radius: 14.0.into(),
                    width: if is_selected { 2.0 } else { 1.0 },
                    color: border_color,
                },
                ..Default::default()
            }
        });

    button(frame).on_press(Message::SelectWallpaper(index)).style(image_card_button_style).into()
}

fn build_settings_overlay(app: &App) -> Element<'_, Message> {
    let wallpaper_path_display = format_path_for_display(&app.ui_settings.wallpaper_exe, 56);
    let workshop_path_display = format_path_for_display(&app.ui_settings.workshop_path, 56);

    let card = container(
        column![
            text("Settings").size(26).color(Color::from_rgb8(150, 205, 255)),
            setting_path_row(
                "Wallpaper Engine Path",
                wallpaper_path_display,
                Message::PickWallpaperExe
            ),
            setting_path_row("Workshop Path", workshop_path_display, Message::PickWorkshopPath),
            text_input("FPS Limit", &app.ui_settings.fps_limit)
                .on_input(Message::FpsLimitChanged)
                .padding(10),
            checkbox("Show realtime FPS", app.ui_settings.show_fps)
                .on_toggle(Message::ShowFpsToggled),
            pick_list(
                app.supported_resolutions.clone(),
                app.ui_settings.selected_resolution.clone(),
                Message::ResolutionSelected,
            )
            .placeholder("Resolution")
            .padding(10),
        ]
        .spacing(12),
    )
    .width(640)
    .padding(16)
    .style(|_theme: &Theme| container::Style {
        text_color: Some(Color::WHITE),
        background: Some(Background::Color(Color::from_rgba(0.08, 0.08, 0.10, 0.95))),
        border: Border {
            radius: 16.0.into(),
            width: 1.0,
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.12),
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.40),
            blur_radius: 20.0,
            offset: iced::Vector::new(0.0, 6.0),
        },
    });

    container(card)
        .width(Fill)
        .height(Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .padding(20)
        .into()
}

fn sync_launch_settings(app: &mut App) {
    app.launch_settings.wallpaper_exe = app.ui_settings.wallpaper_exe.clone();
    app.launch_settings.show_fps = app.ui_settings.show_fps;
    app.launch_settings.play_in_window_title = "WE-DEBUG-WINDOW".to_string();
    app.launch_settings.wm_class_contains = "wallpaper64".to_string();
    app.launch_settings.x = 100;
    app.launch_settings.y = 100;

    if let Ok(v) = app.ui_settings.fps_limit.parse::<u32>() {
        app.launch_settings.fps_limit = v.clamp(1, 360);
    }
    if let Some(res) = &app.ui_settings.selected_resolution {
        app.launch_settings.width = res.width;
        app.launch_settings.height = res.height;
    }
}

fn setting_path_row<'a>(label: &'a str, value: String, on_press: Message) -> Element<'a, Message> {
    row![
        container(text(label).size(14)).width(170),
        container(text(value).size(14).color(Color::from_rgb8(220, 228, 236)))
            .width(Fill)
            .padding([10, 12])
            .style(|_theme: &Theme| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.12, 0.12, 0.15, 0.9))),
                border: Border {
                    radius: 10.0.into(),
                    width: 1.0,
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.12),
                },
                ..Default::default()
            }),
        button(text("Browse").size(13)).padding([10, 14]).on_press(on_press),
    ]
    .align_y(Vertical::Center)
    .spacing(10)
    .into()
}

fn detect_supported_resolutions() -> Vec<ResolutionOption> {
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

fn pick_initial_resolution(
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

fn image_card_button_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: None,
        text_color: Color::WHITE,
        border: Border::default(),
        shadow: iced::Shadow::default(),
    }
}

fn primary_fab_style(_theme: &Theme, status: button::Status) -> button::Style {
    let (r, g, b) = match status {
        button::Status::Hovered => (0.13, 0.56, 0.96),
        button::Status::Pressed => (0.09, 0.48, 0.88),
        _ => (0.11, 0.53, 0.93),
    };

    button::Style {
        background: Some(Background::Color(Color::from_rgb(r, g, b))),
        text_color: Color::WHITE,
        border: Border { radius: 30.0.into(), ..Default::default() },
        shadow: iced::Shadow {
            color: Color { a: 0.35, ..Color::BLACK },
            blur_radius: 12.0,
            offset: iced::Vector::new(0.0, 4.0),
        },
    }
}

fn secondary_fab_style(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Color::from_rgba(0.14, 0.14, 0.14, 0.82),
        button::Status::Pressed => Color::from_rgba(0.10, 0.10, 0.10, 0.88),
        _ => Color::from_rgba(0.12, 0.12, 0.12, 0.78),
    };

    button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::WHITE,
        border: Border {
            radius: 26.0.into(),
            width: 1.0,
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.14),
        },
        shadow: iced::Shadow {
            color: Color { a: 0.28, ..Color::BLACK },
            blur_radius: 10.0,
            offset: iced::Vector::new(0.0, 3.0),
        },
    }
}

fn command_exists_in_path(name: &str) -> bool {
    let Some(path_os) = env::var_os("PATH") else {
        return false;
    };

    for dir in env::split_paths(&path_os) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return true;
        }
    }

    false
}
