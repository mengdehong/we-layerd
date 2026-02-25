use std::{
    env,
    path::{Path, PathBuf},
    process::{Child, Command},
    time::Duration,
};

use iced::{
    alignment::{Horizontal, Vertical},
    widget::{button, column, container, image, row, scrollable, stack, svg, text},
    window, Background, Border, Color, ContentFit, Element, Fill, Size, Subscription, Task, Theme,
};
use settings_panel::{
    build_settings_overlay, detect_supported_resolutions, pick_initial_resolution,
    ResolutionOption, UiSettings,
};
use we_core::{
    config::{build_config, save_config, LaunchSettings},
    steam::{self, WallpaperEngineInstallState},
    wallpaper::{self, WallpaperEntry, WallpaperType},
};

mod settings_panel;
mod tray;

fn main() -> iced::Result {
    iced::application("we-gui", update, view)
        .theme(|app: &App| app.theme.clone())
        .subscription(subscription)
        .exit_on_close_request(false)
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
    launch_settings: LaunchSettings,
    ui_settings: UiSettings,
    show_settings: bool,
    supported_resolutions: Vec<ResolutionOption>,
    install_notice: Option<String>,
    tray: Option<tray::TrayController>,
    main_window_id: Option<window::Id>,
    theme: Theme,
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
    PickWallpaperExe,
    PickWorkshopPath,
    WallpaperExePicked(Option<PathBuf>),
    WorkshopPathPicked(Option<PathBuf>),
    FpsLimitChanged(String),
    ShowFpsToggled(bool),
    ResolutionSelected(settings_panel::ResolutionOption),
    WindowResized(Size),
    WindowCloseRequested(window::Id),
    WindowOpened(window::Id),
    TrayTick,
    ThemeTick,
    TrayAction(tray::TrayAction),
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
            stop_runtime(app);

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
            if !stop_runtime(app) {
                let _ = send_layerd_ctl("stop");
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
            app.ui_settings.workshop_path = value.clone();
            if Path::new(&value).is_dir() {
                return Task::perform(
                    scan_wallpapers_from(app.ui_settings.workshop_path.clone()),
                    Message::Scanned,
                );
            }
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
                sync_launch_settings(app);
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
        Message::WindowCloseRequested(id) => window::change_mode(id, window::Mode::Hidden),
        Message::WindowOpened(id) => {
            app.main_window_id = Some(id);
            Task::none()
        }
        Message::TrayTick => {
            if let Some(tray) = app.tray.as_mut() {
                if let Some(action) = tray.poll_action() {
                    return Task::done(Message::TrayAction(action));
                }
            }
            Task::none()
        }
        Message::ThemeTick => {
            app.theme = detect_system_theme();
            Task::none()
        }
        Message::TrayAction(action) => match action {
            tray::TrayAction::ShowWindow => {
                if let Some(id) = app.main_window_id {
                    return Task::batch(vec![
                        window::change_mode(id, window::Mode::Windowed),
                        window::minimize(id, false),
                        window::gain_focus(id),
                    ]);
                }
                let (_id, task) = window::open(window::Settings::default());
                task.map(Message::WindowOpened)
            }
            tray::TrayAction::PlaySwitch => Task::done(Message::PlayPressed),
            tray::TrayAction::Stop => Task::done(Message::StopPressed),
            tray::TrayAction::Pause => {
                let _ = send_layerd_ctl("pause");
                Task::none()
            }
            tray::TrayAction::Resume => {
                let _ = send_layerd_ctl("resume");
                Task::none()
            }
            tray::TrayAction::Quit => {
                let _ = stop_runtime(app);
                std::process::exit(0);
            }
        },
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

    let settings_overlay: Option<Element<'_, Message>> = if app.show_settings {
        Some(build_settings_overlay(&app.ui_settings, &app.supported_resolutions))
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
    Subscription::batch(vec![
        window::resize_events().map(|(_id, size)| Message::WindowResized(size)),
        window::open_events().map(Message::WindowOpened),
        window::close_requests().map(Message::WindowCloseRequested),
        iced::time::every(std::time::Duration::from_millis(250)).map(|_| Message::TrayTick),
        iced::time::every(std::time::Duration::from_secs(2)).map(|_| Message::ThemeTick),
    ])
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
                tray: tray::TrayController::new().ok(),
                main_window_id: None,
                theme: detect_system_theme(),
            },
            Task::done(Message::AutoScan),
        )
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = stop_runtime(self);
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
        Color::from_rgb8(45, 175, 255)
    } else {
        Color { r: 1.0, g: 1.0, b: 1.0, a: 0.1 }
    };

    let frame =
        container(composed).width(card_width).height(card_height).style(move |_theme: &Theme| {
            container::Style {
                border: Border {
                    radius: 14.0.into(),
                    width: if is_selected { 6.0 } else { 1.0 },
                    color: border_color,
                },
                shadow: if is_selected {
                    iced::Shadow {
                        color: Color::from_rgba8(45, 175, 255, 0.85),
                        blur_radius: 24.0,
                        offset: iced::Vector::new(0.0, 0.0),
                    }
                } else {
                    iced::Shadow::default()
                },
                ..Default::default()
            }
        });

    button(frame).on_press(Message::SelectWallpaper(index)).style(image_card_button_style).into()
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

fn image_card_button_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: None,
        text_color: Color::WHITE,
        border: Border::default(),
        shadow: iced::Shadow::default(),
    }
}

fn primary_fab_style(_theme: &Theme, status: button::Status) -> button::Style {
    let is_light = matches!(_theme, Theme::Light);
    let (r, g, b) = match (is_light, status) {
        (true, button::Status::Hovered) => (0.08, 0.47, 0.86),
        (true, button::Status::Pressed) => (0.06, 0.40, 0.78),
        (true, _) => (0.07, 0.44, 0.82),
        (false, button::Status::Hovered) => (0.13, 0.56, 0.96),
        (false, button::Status::Pressed) => (0.09, 0.48, 0.88),
        (false, _) => (0.11, 0.53, 0.93),
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
    let is_light = matches!(_theme, Theme::Light);
    let bg = match (is_light, status) {
        (true, button::Status::Hovered) => Color::from_rgba(0.95, 0.95, 0.95, 0.95),
        (true, button::Status::Pressed) => Color::from_rgba(0.90, 0.90, 0.90, 0.98),
        (true, _) => Color::from_rgba(0.93, 0.93, 0.93, 0.92),
        (false, button::Status::Hovered) => Color::from_rgba(0.14, 0.14, 0.14, 0.82),
        (false, button::Status::Pressed) => Color::from_rgba(0.10, 0.10, 0.10, 0.88),
        (false, _) => Color::from_rgba(0.12, 0.12, 0.12, 0.78),
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

fn detect_system_theme() -> Theme {
    match dark_light::detect() {
        dark_light::Mode::Light => Theme::Light,
        dark_light::Mode::Dark => Theme::Dark,
        dark_light::Mode::Default => Theme::Dark,
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

fn stop_runtime(app: &mut App) -> bool {
    if let Some(mut child) = app.runtime_child.take() {
        let _ = send_layerd_ctl("stop");
        for _ in 0..30 {
            match child.try_wait() {
                Ok(Some(_)) => return true,
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(_) => break,
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        return true;
    }
    let _ = send_layerd_ctl("stop");
    false
}

fn send_layerd_ctl(action: &str) -> bool {
    Command::new("we-layerd").arg("ctl").arg(action).status().map(|s| s.success()).unwrap_or(false)
}
