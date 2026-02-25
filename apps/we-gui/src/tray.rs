use std::sync::mpsc::{self, Receiver};

use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

#[derive(Debug, Clone, Copy)]
pub enum TrayAction {
    PlaySwitch,
    Stop,
    Pause,
    Resume,
    Quit,
}

pub struct TrayController {
    _tray: TrayIcon,
    rx: Receiver<TrayAction>,
}

impl TrayController {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let menu = Menu::new();
        let play = MenuItem::new("Play / Switch", true, None);
        let stop = MenuItem::new("Stop", true, None);
        let pause = MenuItem::new("Pause", true, None);
        let resume = MenuItem::new("Resume", true, None);
        let quit = MenuItem::new("Quit", true, None);

        menu.append(&play)?;
        menu.append(&stop)?;
        menu.append(&pause)?;
        menu.append(&resume)?;
        menu.append(&quit)?;

        let icon = simple_icon()?;
        let tray = TrayIconBuilder::new()
            .with_tooltip("we-gui")
            .with_menu(Box::new(menu))
            .with_icon(icon)
            .build()?;

        let (tx, rx) = mpsc::channel::<TrayAction>();
        let menu_rx = MenuEvent::receiver();
        std::thread::spawn({
            let play_id = play.id().0.clone();
            let stop_id = stop.id().0.clone();
            let pause_id = pause.id().0.clone();
            let resume_id = resume.id().0.clone();
            let quit_id = quit.id().0.clone();
            move || loop {
                let Ok(event) = menu_rx.recv() else {
                    break;
                };
                let id = event.id.0;
                let action = if id == play_id {
                    Some(TrayAction::PlaySwitch)
                } else if id == stop_id {
                    Some(TrayAction::Stop)
                } else if id == pause_id {
                    Some(TrayAction::Pause)
                } else if id == resume_id {
                    Some(TrayAction::Resume)
                } else if id == quit_id {
                    Some(TrayAction::Quit)
                } else {
                    None
                };

                if let Some(action) = action {
                    let _ = tx.send(action);
                }
            }
        });

        Ok(Self { _tray: tray, rx })
    }

    pub fn poll_action(&mut self) -> Option<TrayAction> {
        self.rx.try_recv().ok()
    }
}

fn simple_icon() -> Result<Icon, Box<dyn std::error::Error + Send + Sync>> {
    let width = 16;
    let height = 16;
    let mut rgba = vec![0u8; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) * 4;
            let edge = x == 0 || y == 0 || x == width - 1 || y == height - 1;
            if edge {
                rgba[i] = 30;
                rgba[i + 1] = 140;
                rgba[i + 2] = 240;
                rgba[i + 3] = 255;
            } else {
                rgba[i] = 10;
                rgba[i + 1] = 45;
                rgba[i + 2] = 70;
                rgba[i + 3] = 220;
            }
        }
    }
    Ok(Icon::from_rgba(rgba, width as u32, height as u32)?)
}
