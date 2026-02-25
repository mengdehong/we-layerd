use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command},
};

use iced::{
    widget::{button, column, container, image, row, scrollable, text},
    Element, Fill, Task,
};
use we_core::{
    steam,
    wallpaper::{self, WallpaperEntry, WallpaperType},
};

fn main() -> iced::Result {
    iced::application("we-gui", update, view).run_with(App::init)
}

struct App {
    status: String,
    entries: Vec<WallpaperEntry>,
    selected_id: Option<String>,
    config_path: PathBuf,
    layerd_child: Option<Child>,
}

#[derive(Debug, Clone)]
enum Message {
    AutoScan,
    Scanned(Result<Vec<WallpaperEntry>, String>),
    SelectWallpaper(usize),
    PlayPressed,
    SettingsPressed,
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::AutoScan => {
            app.status = "Scanning workshop wallpapers...".to_string();
            Task::perform(scan_wallpapers(), Message::Scanned)
        }
        Message::Scanned(result) => match result {
            Ok(entries) => {
                app.status = format!("Found {} wallpapers", entries.len());
                app.entries = entries;
                Task::none()
            }
            Err(err) => {
                app.status = format!("Scan failed: {err}");
                Task::none()
            }
        }
        Message::SelectWallpaper(index) => {
            let Some(entry) = app.entries.get(index).cloned() else {
                return Task::none();
            };

            app.selected_id = Some(entry.id.clone());
            match write_config_for_wallpaper(&app.config_path, &entry.project_json) {
                Ok(()) => {
                    app.status = format!(
                        "Selected [{}] {}. Config updated: {}",
                        entry.id,
                        entry.title,
                        app.config_path.display()
                    );
                }
                Err(err) => {
                    app.status = format!("Failed to update config: {err}");
                }
            }
            Task::none()
        }
        Message::PlayPressed => {
            if let Some(child) = app.layerd_child.as_mut() {
                if let Ok(Some(_)) = child.try_wait() {
                    app.layerd_child = None;
                }
            }

            if app.layerd_child.is_some() {
                app.status = "we-layerd is already running".to_string();
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
                    app.status = format!("Started we-layerd with {}", app.config_path.display());
                }
                Err(err) => {
                    app.status = format!("Failed to start we-layerd: {err}");
                }
            }
            Task::none()
        }
        Message::SettingsPressed => {
            app.status = "Settings panel is not implemented yet".to_string();
            Task::none()
        }
    }
}

fn view(app: &App) -> Element<'_, Message> {
    let head = row![text("we-gui").size(28), text(&app.status)].spacing(12);

    let mut list = column!().spacing(8);
    for (index, entry) in app.entries.iter().enumerate() {
        let selected = app
            .selected_id
            .as_ref()
            .map(|id| id == &entry.id)
            .unwrap_or(false);

        let preview_box: Element<'_, Message> = if let Some(path) = &entry.preview {
            image(image::Handle::from_path(path))
                .width(220)
                .height(124)
                .into()
        } else {
            container(text("No Preview"))
                .width(220)
                .height(124)
                .center_x(Fill)
                .center_y(Fill)
                .into()
        };

        let card = column![
            preview_box,
            text(format!("{} [{}]", entry.title, wallpaper_type_name(entry.ty))).size(16),
            text(entry.id.as_str()).size(13)
        ]
        .spacing(6);

        let label = if selected {
            format!("Selected • {}", entry.id)
        } else {
            format!("Use • {}", entry.id)
        };

        list = list.push(
            container(
                row![
                    button(card).on_press(Message::SelectWallpaper(index)),
                    button(text(label)).on_press(Message::SelectWallpaper(index))
                ]
                .spacing(10)
            )
            .padding(8),
        );
    }

    let content = column![head, scrollable(list).height(Fill)].spacing(12);

    let floating = container(
        row![
            button(text("▶")).on_press(Message::PlayPressed),
            button(text("⚙")).on_press(Message::SettingsPressed),
        ]
        .spacing(10),
    )
    .align_x(iced::alignment::Horizontal::Right)
    .align_y(iced::alignment::Vertical::Bottom);

    container(column![content, floating].spacing(8))
        .padding(16)
        .center_x(Fill)
        .center_y(Fill)
        .into()
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

impl App {
    fn init() -> (Self, Task<Message>) {
        let config_path = steam::default_config_path().unwrap_or_else(|| PathBuf::from("config.toml"));
        (
            Self {
                status: "Initializing...".to_string(),
                entries: Vec::new(),
                selected_id: None,
                config_path,
                layerd_child: None,
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
