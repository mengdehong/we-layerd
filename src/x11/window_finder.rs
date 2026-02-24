use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tracing::{info, warn};
use x11rb::{
    connection::Connection,
    protocol::xproto::{
        Atom, AtomEnum, ConnectionExt as _, GetPropertyReply, MapState, Window,
    },
    rust_connection::RustConnection,
};

use crate::config::CaptureConfig;

const POLL_INTERVAL: Duration = Duration::from_millis(200);
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct WindowFinderResult {
    pub window: Window,
    pub scanned_windows: usize,
}

#[derive(Debug, Clone)]
struct Atoms {
    wm_class: Atom,
    net_wm_pid: Atom,
    net_wm_name: Atom,
    utf8_string: Atom,
}

#[derive(Default, Debug)]
struct WindowMetadata {
    wm_class: String,
    title: String,
    pid: Option<u32>,
    is_viewable: bool,
    width: u16,
    height: u16,
}

pub fn find_window_for_process(
    config: &CaptureConfig,
    fallback_pid: Option<u32>,
) -> Result<Option<WindowFinderResult>> {
    let (conn, screen_num) = RustConnection::connect(None).context("failed to connect to X11 display")?;
    let root = conn.setup().roots[screen_num].root;
    let atoms = intern_atoms(&conn)?;

    let target_pid = config.net_wm_pid.or(fallback_pid);
    let start = Instant::now();
    let mut last_scan_count = 0;

    while start.elapsed() < DEFAULT_TIMEOUT {
        let mut windows = Vec::new();
        collect_windows(&conn, root, &mut windows)?;
        last_scan_count = windows.len();

        let mut best: Option<(i64, Window, WindowMetadata)> = None;

        for window in windows {
            let meta = window_metadata(&conn, &atoms, window)?;
            if !is_match(config, target_pid, &meta) {
                continue;
            }
            let score = score_window(target_pid, &meta);
            if score < 0 {
                continue;
            }

            let is_better = best.as_ref().map(|(s, _, _)| score > *s).unwrap_or(true);
            if is_better {
                best = Some((score, window, meta));
            }
        }

        if let Some((score, window, meta)) = best {
            info!(
                window,
                score,
                ?target_pid,
                wm_class = %meta.wm_class,
                title = %meta.title,
                width = meta.width,
                height = meta.height,
                "matched X11 window"
            );
            return Ok(Some(WindowFinderResult {
                window,
                scanned_windows: last_scan_count,
            }));
        }

        std::thread::sleep(POLL_INTERVAL);
    }

    warn!(
        ?target_pid,
        wm_class_contains = ?config.wm_class_contains,
        title_contains = ?config.title_contains,
        scanned_windows = last_scan_count,
        "failed to find matching X11 window within timeout"
    );
    Ok(None)
}

fn score_window(target_pid: Option<u32>, meta: &WindowMetadata) -> i64 {
    if !meta.is_viewable {
        return -1;
    }

    if meta.width < 64 || meta.height < 64 {
        return -1;
    }

    let mut score: i64 = 0;
    if target_pid.is_some() && meta.pid == target_pid {
        score += 200;
    }

    if !meta.title.is_empty() {
        score += 80;
    }

    if contains_case_insensitive(&meta.wm_class, "wallpaper") {
        score += 40;
    }

    score += (meta.width as i64 * meta.height as i64 / 10000).min(120);
    score
}

fn intern_atoms(conn: &RustConnection) -> Result<Atoms> {
    Ok(Atoms {
        wm_class: intern_atom(conn, b"WM_CLASS")?,
        net_wm_pid: intern_atom(conn, b"_NET_WM_PID")?,
        net_wm_name: intern_atom(conn, b"_NET_WM_NAME")?,
        utf8_string: intern_atom(conn, b"UTF8_STRING")?,
    })
}

fn intern_atom(conn: &RustConnection, name: &[u8]) -> Result<Atom> {
    conn.intern_atom(false, name)
        .with_context(|| format!("failed to intern atom {}", String::from_utf8_lossy(name)))?
        .reply()
        .context("failed to receive atom reply")
        .map(|r| r.atom)
}

fn collect_windows(conn: &RustConnection, root: Window, out: &mut Vec<Window>) -> Result<()> {
    let mut stack = vec![root];
    while let Some(window) = stack.pop() {
        out.push(window);
        let tree = conn
            .query_tree(window)
            .with_context(|| format!("query_tree failed for window {}", window))?
            .reply()
            .context("query_tree reply failed")?;
        stack.extend(tree.children);
    }
    Ok(())
}

fn window_metadata(conn: &RustConnection, atoms: &Atoms, window: Window) -> Result<WindowMetadata> {
    let attrs = conn
        .get_window_attributes(window)
        .with_context(|| format!("get_window_attributes failed for window {}", window))?
        .reply()
        .context("get_window_attributes reply failed")?;

    let geom = conn
        .get_geometry(window)
        .with_context(|| format!("get_geometry failed for window {}", window))?
        .reply()
        .context("get_geometry reply failed")?;

    Ok(WindowMetadata {
        wm_class: read_text_property(conn, window, atoms.wm_class, AtomEnum::STRING.into())?
            .unwrap_or_default(),
        title: read_text_property(conn, window, atoms.net_wm_name, atoms.utf8_string)?
            .or_else(|| {
                read_text_property(conn, window, AtomEnum::WM_NAME.into(), AtomEnum::STRING.into())
                    .ok()
                    .flatten()
            })
            .unwrap_or_default(),
        pid: read_pid_property(conn, window, atoms.net_wm_pid).ok().flatten(),
        is_viewable: attrs.map_state == MapState::VIEWABLE,
        width: geom.width,
        height: geom.height,
    })
}

fn is_match(config: &CaptureConfig, target_pid: Option<u32>, meta: &WindowMetadata) -> bool {
    let pid_ok = target_pid.map_or(true, |pid| meta.pid == Some(pid));
    let class_ok = config
        .wm_class_contains
        .as_ref()
        .map_or(true, |needle| contains_case_insensitive(&meta.wm_class, needle));
    let title_ok = config
        .title_contains
        .as_ref()
        .map_or(true, |needle| contains_case_insensitive(&meta.title, needle));

    pid_ok && class_ok && title_ok
}

fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

fn read_text_property(
    conn: &RustConnection,
    window: Window,
    property: Atom,
    property_type: Atom,
) -> Result<Option<String>> {
    let reply = get_property(conn, window, property, property_type)?;
    if reply.value.is_empty() {
        return Ok(None);
    }

    let text = reply
        .value
        .split(|byte| *byte == 0)
        .filter(|slice| !slice.is_empty())
        .map(|slice| String::from_utf8_lossy(slice).to_string())
        .collect::<Vec<_>>()
        .join(" ");
    Ok(Some(text))
}

fn read_pid_property(conn: &RustConnection, window: Window, property: Atom) -> Result<Option<u32>> {
    let reply = get_property(conn, window, property, AtomEnum::CARDINAL.into())?;
    Ok(reply.value32().and_then(|mut vals| vals.next()))
}

fn get_property(
    conn: &RustConnection,
    window: Window,
    property: Atom,
    property_type: Atom,
) -> Result<GetPropertyReply> {
    conn.get_property(false, window, property, property_type, 0, 1024)
        .with_context(|| format!("get_property failed for window {}", window))?
        .reply()
        .context("get_property reply failed")
}
