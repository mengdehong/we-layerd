use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command},
};

use iced::{
    alignment::{Horizontal, Vertical},
    widget::{button, column, container, image, row, scrollable, stack, text},
    window, Background, Border, Color, Element, Fill, Size, Subscription, Task, Theme,
};
use we_core::{
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
    layerd_child: Option<Child>,
    viewport_width: f32,
}

#[derive(Debug, Clone)]
enum Message {
    AutoScan,
    Scanned(Result<Vec<WallpaperEntry>, String>),
    SelectWallpaper(usize),
    PlayPressed,
    SettingsPressed,
    WindowResized(Size),
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
            let _ = write_config_for_wallpaper(&app.config_path, &entry.project_json);
            Task::none()
        }
        Message::PlayPressed => {
            if let Some(child) = app.layerd_child.as_mut() {
                if let Ok(Some(_)) = child.try_wait() {
                    app.layerd_child = None;
                }
            }

            if app.layerd_child.is_some() {
                return Task::none();
            }

            let spawn = Command::new("we-layerd")
                .arg("run")
                .arg("--config")
                .arg(&app.config_path)
                .spawn();

            match spawn {
                Ok(child) => {
                    app.layerd_child = Some(child);
                }
                Err(_err) => {}
            }
            Task::none()
        }
        Message::SettingsPressed => Task::none(),
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
            button(text("⚙").size(24))
                .style(material_icon_button_style)
                .on_press(Message::SettingsPressed),
            button(text("▶").size(26))
                .style(material_icon_button_style)
                .on_press(Message::PlayPressed),
        ]
        .spacing(12),
    )
    .width(Fill)
    .height(Fill)
    .align_x(Horizontal::Right)
    .align_y(Vertical::Bottom)
    .padding(20);

    stack![content, floating].into()
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
        (
            Self {
                entries: Vec::new(),
                selected_id: None,
                config_path,
                layerd_child: None,
                viewport_width: 1280.0,
            },
            Task::done(Message::AutoScan),
        )
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if let Some(mut child) = self.layerd_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn write_config_for_wallpaper(config_path: &Path, project_json: &Path) -> Result<(), String> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let wallpaper_exe = steam::discover_wallpaper_engine_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let project = project_json.display().to_string();

    let content = format!(
        r#"[general]
fps_limit = 30
restart_wine_on_exit = true
refind_window_on_capture_error = true
show_fps = false
fps_report_interval_secs = 1

[wine]
command = "wine"
wallpaper_exe = "{wallpaper_exe}"
args = [
  "-control", "openWallpaper",
  "-file", "{project}",
  "-playInWindow", "WE-DEBUG-WINDOW",
  "-width", "2560",
  "-height", "1600",
  "-x", "100",
  "-y", "100",
]

[capture]
wm_class_contains = "wallpaper64"
title_contains = "WE-DEBUG-WINDOW"

[capture.output_window_map]
"#
    );

    fs::write(config_path, content).map_err(|e| e.to_string())
}

fn build_wallpaper_grid<'a>(
    entries: &'a [WallpaperEntry],
    selected_id: Option<&String>,
    width: f32,
) -> Element<'a, Message> {
    let spacing = 12.0;
    let card_width = 260.0;
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

fn image_card_button_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: None,
        text_color: Color::WHITE,
        border: Border::default(),
        shadow: iced::Shadow::default(),
    }
}

fn material_icon_button_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: None,
        text_color: Color::WHITE,
        border: Border::default(),
        shadow: iced::Shadow::default(),
    }
}
