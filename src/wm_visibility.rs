use std::process::Command;

use anyhow::{anyhow, Context, Result};
use tracing::warn;

use crate::config::CaptureConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesktopWm {
    Hyprland,
    Sway,
    Niri,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct DebugWindowVisibility {
    pub auto_hide: bool,
    pub hidden_workspace_name: String,
    title_hint: Option<String>,
    class_hint: Option<String>,
}

impl DebugWindowVisibility {
    pub fn new(auto_hide: bool, hidden_workspace_name: String, capture: &CaptureConfig) -> Self {
        Self {
            auto_hide,
            hidden_workspace_name,
            title_hint: capture.title_contains.clone(),
            class_hint: capture.wm_class_contains.clone(),
        }
    }

    pub fn hide(&self) -> Result<()> {
        match detect_wm() {
            DesktopWm::Hyprland => self.hide_hyprland(),
            DesktopWm::Sway => self.hide_sway(),
            DesktopWm::Niri => self.hide_niri(),
            DesktopWm::Unknown => Err(anyhow!("no supported WM detected for hide-window action")),
        }
    }

    pub fn show(&self) -> Result<()> {
        match detect_wm() {
            DesktopWm::Hyprland => self.show_hyprland(),
            DesktopWm::Sway => self.show_sway(),
            DesktopWm::Niri => self.show_niri(),
            DesktopWm::Unknown => Err(anyhow!("no supported WM detected for show-window action")),
        }
    }

    fn selector_class(&self) -> String {
        self.class_hint.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "wallpaper64".to_string())
    }

    fn selector_title(&self) -> Option<String> {
        self.title_hint.clone().filter(|s| !s.trim().is_empty())
    }

    fn hide_hyprland(&self) -> Result<()> {
        let class = self.selector_class();
        let target = format!("special:{}", self.hidden_workspace_name);
        run_cmd(
            "hyprctl",
            &[
                "dispatch",
                "movetoworkspacesilent",
                &format!("{target},class:^(?:{class})$"),
            ],
        )
        .context("hyprland hide failed")
    }

    fn show_hyprland(&self) -> Result<()> {
        let class = self.selector_class();
        run_cmd(
            "hyprctl",
            &[
                "dispatch",
                "movetoworkspacesilent",
                &format!("+0,class:^(?:{class})$"),
            ],
        )
        .context("hyprland show failed")
    }

    fn hide_sway(&self) -> Result<()> {
        let class = self.selector_class();
        run_cmd("swaymsg", &[&format!(r#"[class="{class}"]"#), "move", "scratchpad"])
            .context("sway hide failed")
    }

    fn show_sway(&self) -> Result<()> {
        let class = self.selector_class();
        run_cmd("swaymsg", &[&format!(r#"[class="{class}"]"#), "scratchpad", "show"])
            .context("sway show failed")
    }

    fn hide_niri(&self) -> Result<()> {
        for id in self.matching_niri_window_ids()? {
            let _ = run_cmd(
                "niri",
                &[
                    "msg",
                    "action",
                    "move-window-to-workspace",
                    "--window-id",
                    &id.to_string(),
                    "--focus=false",
                    &self.hidden_workspace_name,
                ],
            );
        }
        Ok(())
    }

    fn show_niri(&self) -> Result<()> {
        let current_ws = current_niri_workspace_spec()?;
        for id in self.matching_niri_window_ids()? {
            let _ = run_cmd(
                "niri",
                &[
                    "msg",
                    "action",
                    "move-window-to-workspace",
                    "--window-id",
                    &id.to_string(),
                    &current_ws,
                ],
            );
            let _ = run_cmd("niri", &["msg", "action", "focus-window", "--id", &id.to_string()]);
        }
        Ok(())
    }

    fn matching_niri_window_ids(&self) -> Result<Vec<u64>> {
        let output = Command::new("niri")
            .args(["msg", "-j", "windows"])
            .output()
            .context("failed to query niri windows")?;
        if !output.status.success() {
            return Err(anyhow!("niri msg -j windows failed"));
        }

        let class = self.selector_class().to_ascii_lowercase();
        let title_hint = self.selector_title().map(|s| s.to_ascii_lowercase());
        let value: serde_json::Value =
            serde_json::from_slice(&output.stdout).context("invalid niri windows json")?;
        let mut ids = Vec::new();
        if let Some(arr) = value.as_array() {
            for item in arr {
                let id = item.get("id").and_then(|v| v.as_u64());
                let app_id = item
                    .get("app_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_ascii_lowercase())
                    .unwrap_or_default();
                let title = item
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_ascii_lowercase())
                    .unwrap_or_default();
                let class_match = !class.is_empty() && app_id.contains(&class);
                let title_match = title_hint.as_ref().is_some_and(|t| title.contains(t));
                if (class_match || title_match) && id.is_some() {
                    ids.push(id.unwrap_or_default());
                }
            }
        }
        Ok(ids)
    }
}

fn run_cmd(bin: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(bin)
        .args(args)
        .output()
        .with_context(|| format!("failed to run {bin}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() { stderr } else { stdout };
    Err(anyhow!("{} failed: {}", bin, detail))
}

fn detect_wm() -> DesktopWm {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        return DesktopWm::Hyprland;
    }
    if std::env::var_os("SWAYSOCK").is_some() {
        return DesktopWm::Sway;
    }
    if std::env::var_os("NIRI_SOCKET").is_some() {
        return DesktopWm::Niri;
    }
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_ascii_lowercase();
    if desktop.contains("hyprland") {
        return DesktopWm::Hyprland;
    }
    if desktop.contains("sway") {
        return DesktopWm::Sway;
    }
    if desktop.contains("niri") {
        return DesktopWm::Niri;
    }
    warn!("cannot detect supported WM for debug-window visibility");
    DesktopWm::Unknown
}

fn current_niri_workspace_spec() -> Result<String> {
    let output = Command::new("niri")
        .args(["msg", "-j", "workspaces"])
        .output()
        .context("failed to query niri workspaces")?;
    if !output.status.success() {
        return Err(anyhow!("niri msg -j workspaces failed"));
    }
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("invalid niri workspaces json")?;
    let Some(arr) = value.as_array() else {
        return Err(anyhow!("unexpected niri workspace json"));
    };
    for ws in arr {
        let focused = ws.get("is_focused").and_then(|v| v.as_bool()).unwrap_or(false)
            || ws.get("is_active").and_then(|v| v.as_bool()).unwrap_or(false);
        if !focused {
            continue;
        }
        if let Some(name) = ws.get("name").and_then(|v| v.as_str()) {
            if !name.is_empty() {
                return Ok(name.to_string());
            }
        }
        if let Some(idx) = ws.get("idx").and_then(|v| v.as_i64()) {
            return Ok(idx.to_string());
        }
        if let Some(id) = ws.get("id").and_then(|v| v.as_i64()) {
            return Ok(id.to_string());
        }
    }
    Err(anyhow!("cannot determine focused niri workspace"))
}
