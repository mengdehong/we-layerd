use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    process::Command,
};

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use tracing::warn;

use crate::config::CaptureConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesktopWm {
    Hyprland,
    Sway,
    Niri,
    Kde,
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
            DesktopWm::Kde => self.hide_kde(),
            DesktopWm::Unknown => Err(anyhow!("no supported WM detected for hide-window action")),
        }
    }

    pub fn show(&self) -> Result<()> {
        match detect_wm() {
            DesktopWm::Hyprland => self.show_hyprland(),
            DesktopWm::Sway => self.show_sway(),
            DesktopWm::Niri => self.show_niri(),
            DesktopWm::Kde => self.show_kde(),
            DesktopWm::Unknown => Err(anyhow!("no supported WM detected for show-window action")),
        }
    }

    fn selector_class(&self) -> String {
        self.class_hint
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "wallpaper64".to_string())
    }

    fn selector_title(&self) -> Option<String> {
        self.title_hint.clone().filter(|s| !s.trim().is_empty())
    }

    fn hide_hyprland(&self) -> Result<()> {
        let target = format!("special:{}", self.hidden_workspace_name);
        let addresses = self.matching_hypr_addresses()?;
        move_hypr_windows(&addresses, &target).context("hyprland hide failed")
    }

    fn show_hyprland(&self) -> Result<()> {
        let addresses = self.matching_hypr_addresses()?;
        move_hypr_windows(&addresses, "+0").context("hyprland show failed")
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

    fn hide_kde(&self) -> Result<()> {
        let ids = self.matching_kde_window_ids()?;
        let mut changed = false;
        for id in ids {
            let _ = set_kde_window_state(id.as_str(), true, "below");
            let _ = set_kde_window_state(id.as_str(), true, "skip_taskbar");
            let _ = set_kde_window_state(id.as_str(), true, "skip_pager");
            let _ = set_kde_window_state(id.as_str(), true, "no_border");
            changed = true;
        }
        if changed {
            Ok(())
        } else {
            Err(anyhow!("kde hide did not adjust any matching window"))
        }
    }

    fn show_kde(&self) -> Result<()> {
        let ids = self.matching_kde_window_ids()?;
        let mut changed = false;
        for id in ids {
            let _ = set_kde_window_state(id.as_str(), false, "below");
            let _ = set_kde_window_state(id.as_str(), false, "skip_taskbar");
            let _ = set_kde_window_state(id.as_str(), false, "skip_pager");
            if run_kdotool(&["windowactivate", id.as_str()]).is_ok() {
                changed = true;
            }
        }
        if changed {
            Ok(())
        } else {
            Err(anyhow!("kde show did not activate any matching window"))
        }
    }

    fn hide_niri(&self) -> Result<()> {
        let target_ws_id = target_niri_workspace_id(&self.hidden_workspace_name)?;
        let ids = self.matching_niri_window_ids()?;
        let mut moved = false;
        for id in ids {
            if move_niri_window_to_workspace_id(id, target_ws_id, false).is_ok() {
                moved = true;
                let _ = run_cmd(
                    "niri",
                    &["msg", "action", "move-window-to-floating", "--id", &id.to_string()],
                );
                let _ = move_niri_window_to_corner(id);
            }
        }
        if moved {
            Ok(())
        } else {
            Err(anyhow!("niri hide did not move any matching window"))
        }
    }

    fn show_niri(&self) -> Result<()> {
        let current_ws_id = current_niri_workspace_id()?;
        let ids = self.matching_niri_window_ids()?;
        let mut moved = false;
        for id in ids {
            if move_niri_window_to_workspace_id(id, current_ws_id, true).is_ok() {
                moved = true;
                let _ =
                    run_cmd("niri", &["msg", "action", "focus-window", "--id", &id.to_string()]);
            }
        }
        if moved {
            Ok(())
        } else {
            Err(anyhow!("niri show did not move any matching window"))
        }
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

    fn matching_hypr_addresses(&self) -> Result<Vec<String>> {
        let output = Command::new("hyprctl")
            .args(["clients", "-j"])
            .output()
            .context("failed to query hypr clients")?;
        if !output.status.success() {
            return Err(anyhow!("hyprctl clients -j failed"));
        }
        let value: Value =
            serde_json::from_slice(&output.stdout).context("invalid hypr clients json")?;
        let Some(arr) = value.as_array() else {
            return Err(anyhow!("unexpected hypr clients json"));
        };

        let class_hint = self.selector_class().to_ascii_lowercase();
        let title_hint = self.selector_title().map(|s| s.to_ascii_lowercase());
        let mut addresses = Vec::new();
        for client in arr {
            let class = client
                .get("class")
                .and_then(|v| v.as_str())
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();
            let initial_class = client
                .get("initialClass")
                .and_then(|v| v.as_str())
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();
            let title = client
                .get("title")
                .and_then(|v| v.as_str())
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();
            let class_match = !class_hint.is_empty()
                && (class.contains(&class_hint) || initial_class.contains(&class_hint));
            let title_match = title_hint.as_ref().is_some_and(|t| title.contains(t));
            if !(class_match || title_match) {
                continue;
            }
            if let Some(addr) = client.get("address").and_then(|v| v.as_str()) {
                addresses.push(addr.to_string());
            }
        }
        Ok(addresses)
    }

    fn matching_kde_window_ids(&self) -> Result<Vec<String>> {
        ensure_kdotool_available()?;
        let title = self.selector_title();
        let class = self.selector_class();

        let mut ids = Vec::new();
        if let Some(t) = title.as_deref() {
            ids.extend(kdotool_search_ids("--name", t)?);
        }
        if ids.is_empty() && !class.trim().is_empty() {
            ids.extend(kdotool_search_ids("--class", &class)?);
        }
        ids.sort();
        ids.dedup();

        if ids.is_empty() {
            return Err(anyhow!("no matching KDE window found via kdotool"));
        }
        Ok(ids)
    }
}

fn run_cmd(bin: &str, args: &[&str]) -> Result<()> {
    let output =
        Command::new(bin).args(args).output().with_context(|| format!("failed to run {bin}"))?;
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
    if std::env::var_os("KDE_FULL_SESSION").is_some()
        || std::env::var_os("KDE_SESSION_VERSION").is_some()
    {
        return DesktopWm::Kde;
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
    if desktop.contains("kde") || desktop.contains("plasma") {
        return DesktopWm::Kde;
    }
    warn!("cannot detect supported WM for debug-window visibility");
    DesktopWm::Unknown
}

fn ensure_kdotool_available() -> Result<()> {
    let output = Command::new("kdotool")
        .arg("--version")
        .output()
        .context("failed to run kdotool --version")?;
    if output.status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "kdotool is required for KDE window hide/show logic; install it first"
        ))
    }
}

fn kdotool_search_ids(flag: &str, pattern: &str) -> Result<Vec<String>> {
    let output = Command::new("kdotool")
        .args(["search", flag, pattern])
        .output()
        .with_context(|| format!("failed to run kdotool search {flag} {pattern}"))?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let ids = stdout
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    Ok(ids)
}

fn run_kdotool(args: &[&str]) -> Result<()> {
    let output = Command::new("kdotool")
        .args(args)
        .output()
        .with_context(|| format!("failed to run kdotool {}", args.join(" ")))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        Err(anyhow!("kdotool failed: {}", detail))
    }
}

fn set_kde_window_state(window_id: &str, add: bool, state: &str) -> Result<()> {
    let mode = if add { "--add" } else { "--remove" };
    run_kdotool(&["windowstate", mode, state, window_id])
}

fn current_niri_workspace_id() -> Result<u64> {
    let arr = query_niri_workspaces()?;
    for ws in arr {
        let focused = ws.get("is_focused").and_then(|v| v.as_bool()).unwrap_or(false)
            || ws.get("is_active").and_then(|v| v.as_bool()).unwrap_or(false);
        if !focused {
            continue;
        }
        if let Some(id) = ws.get("id").and_then(|v| v.as_u64()) {
            return Ok(id);
        }
    }
    Err(anyhow!("cannot determine focused niri workspace"))
}

fn target_niri_workspace_id(configured: &str) -> Result<u64> {
    let trimmed = configured.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("top") {
        return top_niri_workspace_id();
    }
    if let Ok(id) = trimmed.parse::<u64>() {
        return Ok(id);
    }
    workspace_id_by_name(trimmed)
}

fn top_niri_workspace_id() -> Result<u64> {
    let arr = query_niri_workspaces()?;
    let mut best_idx: Option<i64> = None;
    let mut best_id: Option<u64> = None;

    for ws in arr {
        let Some(idx) = ws.get("idx").and_then(|v| v.as_i64()) else {
            continue;
        };
        let Some(id) = ws.get("id").and_then(|v| v.as_u64()) else {
            continue;
        };
        let should_replace = match best_idx {
            Some(best) => idx < best,
            None => true,
        };
        if should_replace {
            best_idx = Some(idx);
            best_id = Some(id);
        }
    }

    if let Some(id) = best_id {
        return Ok(id);
    }
    Err(anyhow!("cannot determine top niri workspace id"))
}

fn workspace_id_by_name(name: &str) -> Result<u64> {
    let arr = query_niri_workspaces()?;
    for ws in arr {
        let ws_name = ws.get("name").and_then(|v| v.as_str()).unwrap_or_default();
        if ws_name == name {
            if let Some(id) = ws.get("id").and_then(|v| v.as_u64()) {
                return Ok(id);
            }
        }
    }
    Err(anyhow!("cannot resolve niri workspace id by name: {name}"))
}

fn query_niri_workspaces() -> Result<Vec<Value>> {
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
    Ok(arr.clone())
}

fn move_niri_window_to_workspace_id(window_id: u64, workspace_id: u64, focus: bool) -> Result<()> {
    let req = json!({
        "Action": {
            "MoveWindowToWorkspace": {
                "window_id": window_id,
                "reference": { "Id": workspace_id },
                "focus": focus
            }
        }
    });
    niri_send_request(req).map(|_| ())
}

fn niri_send_request(req: Value) -> Result<Value> {
    let socket_path =
        std::env::var("NIRI_SOCKET").context("NIRI_SOCKET is not set for niri IPC access")?;
    let mut stream = UnixStream::connect(&socket_path)
        .with_context(|| format!("cannot connect {socket_path}"))?;
    let line = format!("{}\n", req);
    stream.write_all(line.as_bytes()).context("failed to write niri IPC request")?;
    stream.flush().context("failed to flush niri IPC request")?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).context("failed to read niri IPC reply")?;
    let line = line.trim();
    if line.is_empty() {
        return Err(anyhow!("empty niri IPC reply"));
    }
    let reply: Value = serde_json::from_str(line).context("invalid niri IPC reply JSON")?;
    if let Some(err) = reply.get("Err") {
        return Err(anyhow!("niri IPC action error: {err}"));
    }
    if reply.get("Ok").is_some() {
        return Ok(reply);
    }
    Err(anyhow!("unexpected niri IPC reply: {reply}"))
}

fn move_niri_window_to_corner(window_id: u64) -> Result<()> {
    let (x, y) = niri_hidden_corner_position().unwrap_or((0, 0));
    run_cmd(
        "niri",
        &[
            "msg",
            "action",
            "move-floating-window",
            "--id",
            &window_id.to_string(),
            "-x",
            &x.to_string(),
            "-y",
            &y.to_string(),
        ],
    )
}

fn niri_hidden_corner_position() -> Result<(i32, i32)> {
    let output = Command::new("niri")
        .args(["msg", "-j", "focused-output"])
        .output()
        .context("failed to query niri focused-output")?;
    if !output.status.success() {
        return Err(anyhow!("niri msg -j focused-output failed"));
    }
    let value: Value =
        serde_json::from_slice(&output.stdout).context("invalid focused-output json")?;
    let logical = value.get("logical").ok_or_else(|| anyhow!("focused-output lacks logical"))?;
    let width =
        logical.get("width").and_then(|v| v.as_f64()).map(|v| v.round() as i32).unwrap_or(1920);
    let x = (width - 8).max(0);
    Ok((x, 0))
}

fn move_hypr_windows(addresses: &[String], workspace: &str) -> Result<()> {
    if addresses.is_empty() {
        return Err(anyhow!("no matching hyprland windows found"));
    }
    let mut moved = false;
    for address in addresses {
        let selector = format!("{workspace},address:{address}");
        if run_cmd("hyprctl", &["dispatch", "movetoworkspacesilent", &selector]).is_ok() {
            moved = true;
        }
    }
    if moved {
        Ok(())
    } else {
        Err(anyhow!("failed to move matching hyprland windows"))
    }
}
