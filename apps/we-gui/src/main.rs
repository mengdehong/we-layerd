use iced::{
    widget::{button, column, container, row, scrollable, text},
    Element, Fill, Task,
};
use we_core::{
    steam,
    wallpaper::{self, WallpaperEntry, WallpaperType},
};

fn main() -> iced::Result {
    iced::application("we-gui", update, view).run_with(|| (App::default(), Task::none()))
}

#[derive(Default)]
struct App {
    status: String,
    entries: Vec<WallpaperEntry>,
}

#[derive(Debug, Clone)]
enum Message {
    Scan,
    Scanned(Result<Vec<WallpaperEntry>, String>),
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Scan => {
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
        },
    }
}

fn view(app: &App) -> Element<'_, Message> {
    let head = row![
        button("Scan").on_press(Message::Scan),
        text(&app.status),
    ]
    .spacing(12);

    let mut list = column!().spacing(8);
    for entry in &app.entries {
        let line = format!(
            "[{}] {} ({})",
            entry.id,
            entry.title,
            wallpaper_type_name(entry.ty)
        );
        let detail = format!(
            "project: {} | preview: {}",
            entry.project_json.display(),
            entry
                .preview
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<none>".to_string())
        );
        list = list.push(column![text(line), text(detail).size(14)].spacing(2));
    }

    container(column![head, scrollable(list)].spacing(12))
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
