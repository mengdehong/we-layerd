use std::{
    env,
    path::PathBuf,
    process::{Child, Command},
};

use iced::{
    alignment::{Horizontal, Vertical},
    widget::{button, checkbox, column, container, image, row, scrollable, slider, stack, svg, text, text_input},
    window, Background, Border, Color, ContentFit, Element, Fill, Size, Subscription, Task, Theme,
};
use we_core::{
    config::{build_config, save_config, LaunchSettings},
    steam,
    wallpaper::{self, WallpaperEntry, WallpaperType},
};

fn main() -> iced::Result {
    iced::application("we-gui", update, view)
        .subscription(subscription)
        .run_with(App::init)
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
    mpv_available: bool,
    launch_settings: LaunchSettings,
    ui_settings: UiSettings,
    show_settings: bool,
}

#[derive(Debug, Clone)]
enum Message {
    AutoScan,
    Scanned(Result<Vec<WallpaperEntry>, String>),
    SelectWallpaper(usize),
    PlayPressed,
    StopPressed,
    SettingsPressed,
    WallpaperExeChanged(String),
    WorkshopPathChanged(String),
    WineCommandChanged(String),
    FpsLimitChanged(u32),
    ShowFpsToggled(bool),
    WidthChanged(String),
    HeightChanged(String),
    XChanged(String),
    YChanged(String),
    WindowTitleChanged(String),
    WmClassChanged(String),
    WindowResized(Size),
}

#[derive(Debug, Clone)]
struct UiSettings {
    wallpaper_exe: String,
    workshop_path: String,
    wine_command: String,
    fps_limit: u32,
    show_fps: bool,
    width: String,
    height: String,
    x: String,
    y: String,
    window_title: String,
    wm_class: String,
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::AutoScan => {
            Task::perform(scan_wallpapers(), Message::Scanned)
        }
        Message::Scanned(result) => match result {
            Ok(entries) => {
                app.entries = entries;
                Task::none()
            }
            Err(_err) => Task::none(),
        }
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

            let spawn = match app.selected_type {
                Some(WallpaperType::Video) => {
                    if !app.mpv_available {
                        return Task::none();
                    }
                    let Some(video_file) = app.selected_video_file.as_ref() else {
                        return Task::none();
                    };
                    Command::new("mpv")
                        .arg("--loop=inf")
                        .arg("--no-terminal")
                        .arg("--no-input-default-bindings")
                        .arg(video_file)
                        .spawn()
                }
                _ => {
                    if !app.layerd_available {
                        return Task::none();
                    }
                    Command::new("we-layerd")
                        .arg("run")
                        .arg("--config")
                        .arg(&app.config_path)
                        .spawn()
                }
            };

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
        Message::WallpaperExeChanged(value) => {
            app.ui_settings.wallpaper_exe = value;
            sync_launch_settings(app);
            Task::none()
        }
        Message::WorkshopPathChanged(value) => {
            app.ui_settings.workshop_path = value;
            Task::none()
        }
        Message::WineCommandChanged(value) => {
            app.ui_settings.wine_command = value;
            sync_launch_settings(app);
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
        Message::WidthChanged(value) => {
            app.ui_settings.width = value;
            sync_launch_settings(app);
            Task::none()
        }
        Message::HeightChanged(value) => {
            app.ui_settings.height = value;
            sync_launch_settings(app);
            Task::none()
        }
        Message::XChanged(value) => {
            app.ui_settings.x = value;
            sync_launch_settings(app);
            Task::none()
        }
        Message::YChanged(value) => {
            app.ui_settings.y = value;
            sync_launch_settings(app);
            Task::none()
        }
        Message::WindowTitleChanged(value) => {
            app.ui_settings.window_title = value;
            sync_launch_settings(app);
            Task::none()
        }
        Message::WmClassChanged(value) => {
            app.ui_settings.wm_class = value;
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

    let content = container(scrollable(grid).width(Fill).height(Fill))
        .width(Fill)
        .height(Fill);

    let floating = container(
        column![
            button(
                svg(svg::Handle::from_memory(include_bytes!(
                    "../assets/icons/stop.svg"
                )))
                .width(24)
                .height(24),
            )
                .width(52)
                .height(52)
                .style(secondary_fab_style)
                .on_press(Message::StopPressed),
            button(
                svg(svg::Handle::from_memory(include_bytes!(
                    "../assets/icons/settings.svg"
                )))
                .width(24)
                .height(24),
            )
                .width(52)
                .height(52)
                .style(secondary_fab_style)
                .on_press(Message::SettingsPressed),
            button(
                svg(svg::Handle::from_memory(include_bytes!(
                    "../assets/icons/play_arrow.svg"
                )))
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

    let runtime_warning: Option<Element<'_, Message>> = if app.layerd_available || app.mpv_available {
        None
    } else {
        let warning = container(
            text("we-layerd / mpv not found in PATH")
                .size(30)
                .color(Color::from_rgb8(150, 205, 255)),
        )
        .width(Fill)
        .height(Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Top)
        .padding(24);
        Some(warning.into())
    };

    let settings_overlay: Option<Element<'_, Message>> = if app.show_settings {
        Some(build_settings_overlay(app))
    } else {
        None
    };

    match (runtime_warning, settings_overlay) {
        (Some(w), Some(s)) => stack![content, w, s, floating].into(),
        (Some(w), None) => stack![content, w, floating].into(),
        (None, Some(s)) => stack![content, s, floating].into(),
        (None, None) => stack![content, floating].into(),
    }
}

async fn scan_wallpapers() -> Result<Vec<WallpaperEntry>, String> {
    let workshop_root = steam::discover_workshop_wallpaper_root()
        .ok_or_else(|| "cannot find Steam workshop path for app 431960".to_string())?;
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
        let config_path = steam::default_config_path().unwrap_or_else(|| PathBuf::from("config.toml"));
        let mut launch_settings = LaunchSettings::default();
        if let Some(exe) = steam::discover_wallpaper_engine_exe() {
            launch_settings.wallpaper_exe = exe.display().to_string();
        }
        let workshop_path = steam::discover_workshop_wallpaper_root()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let ui_settings = UiSettings {
            wallpaper_exe: launch_settings.wallpaper_exe.clone(),
            workshop_path,
            wine_command: launch_settings.wine_command.clone(),
            fps_limit: launch_settings.fps_limit,
            show_fps: launch_settings.show_fps,
            width: launch_settings.width.to_string(),
            height: launch_settings.height.to_string(),
            x: launch_settings.x.to_string(),
            y: launch_settings.y.to_string(),
            window_title: launch_settings.play_in_window_title.clone(),
            wm_class: launch_settings.wm_class_contains.clone(),
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
                mpv_available: command_exists_in_path("mpv"),
                launch_settings,
                ui_settings,
                show_settings: false,
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

    let chip = container(text(wallpaper_type_name(entry.ty)).size(12))
        .padding([3, 8])
        .style(|_theme: &Theme| container::Style {
            text_color: Some(Color::WHITE),
            background: Some(Background::Color(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.45,
            })),
            border: Border {
                radius: 10.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

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
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 0.1,
        }
    };

    let frame = container(composed)
        .width(card_width)
        .height(card_height)
        .style(move |_theme: &Theme| container::Style {
            border: Border {
                radius: 14.0.into(),
                width: if is_selected { 2.0 } else { 1.0 },
                color: border_color,
            },
            ..Default::default()
        });

    button(frame)
        .on_press(Message::SelectWallpaper(index))
        .style(image_card_button_style)
        .into()
}

fn build_settings_overlay(app: &App) -> Element<'_, Message> {
    let card = container(
        column![
            text("Settings").size(26).color(Color::from_rgb8(150, 205, 255)),
            text_input("Wallpaper Engine exe", &app.ui_settings.wallpaper_exe)
                .on_input(Message::WallpaperExeChanged)
                .padding(10),
            text_input("Workshop wallpapers path", &app.ui_settings.workshop_path)
                .on_input(Message::WorkshopPathChanged)
                .padding(10),
            text_input("Wine command", &app.ui_settings.wine_command)
                .on_input(Message::WineCommandChanged)
                .padding(10),
            row![
                text("FPS Limit"),
                slider(15..=144, app.ui_settings.fps_limit, Message::FpsLimitChanged),
                text(app.ui_settings.fps_limit.to_string())
            ]
            .spacing(10)
            .align_y(Vertical::Center),
            checkbox("Show realtime FPS", app.ui_settings.show_fps).on_toggle(Message::ShowFpsToggled),
            row![
                text_input("Width", &app.ui_settings.width).on_input(Message::WidthChanged).padding(8),
                text_input("Height", &app.ui_settings.height).on_input(Message::HeightChanged).padding(8),
                text_input("X", &app.ui_settings.x).on_input(Message::XChanged).padding(8),
                text_input("Y", &app.ui_settings.y).on_input(Message::YChanged).padding(8),
            ]
            .spacing(8),
            text_input("Window title", &app.ui_settings.window_title)
                .on_input(Message::WindowTitleChanged)
                .padding(10),
            text_input("WM class filter", &app.ui_settings.wm_class)
                .on_input(Message::WmClassChanged)
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
    app.launch_settings.wine_command = app.ui_settings.wine_command.clone();
    app.launch_settings.fps_limit = app.ui_settings.fps_limit;
    app.launch_settings.show_fps = app.ui_settings.show_fps;
    app.launch_settings.play_in_window_title = app.ui_settings.window_title.clone();
    app.launch_settings.wm_class_contains = app.ui_settings.wm_class.clone();

    if let Ok(v) = app.ui_settings.width.parse::<u32>() {
        app.launch_settings.width = v;
    }
    if let Ok(v) = app.ui_settings.height.parse::<u32>() {
        app.launch_settings.height = v;
    }
    if let Ok(v) = app.ui_settings.x.parse::<i32>() {
        app.launch_settings.x = v;
    }
    if let Ok(v) = app.ui_settings.y.parse::<i32>() {
        app.launch_settings.y = v;
    }
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
        border: Border {
            radius: 30.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color {
                a: 0.35,
                ..Color::BLACK
            },
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
            color: Color {
                a: 0.28,
                ..Color::BLACK
            },
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
