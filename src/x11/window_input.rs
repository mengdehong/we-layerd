use anyhow::{Context, Result};
use x11rb::{
    connection::Connection,
    protocol::{
        shape,
        xfixes::ConnectionExt as _,
        xproto::{
            Atom, ClientMessageData, ClientMessageEvent, ConnectionExt as _, EventMask, MapState,
            Window,
        },
    },
    rust_connection::RustConnection,
};

pub fn set_mouse_passthrough(window: u32) -> Result<()> {
    let (conn, _) =
        RustConnection::connect(None).context("failed to connect to X11 display for input setup")?;

    let _ = conn
        .xfixes_query_version(5, 0)
        .context("failed to query XFixes extension")?
        .reply()
        .context("failed to receive XFixes version reply")?;

    conn.xfixes_set_window_shape_region(window, shape::SK::INPUT, 0, 0, 0u32)
        .context("failed to set empty X11 input shape for window")?;
    conn.flush().context("failed to flush X11 input-shape requests")?;
    Ok(())
}

pub fn apply_wallpaper_window_hints(window: u32) -> Result<()> {
    let (conn, screen_num) =
        RustConnection::connect(None).context("failed to connect to X11 display for EWMH hints")?;
    let root = conn.setup().roots[screen_num].root;

    let net_wm_state = intern_atom(&conn, b"_NET_WM_STATE")?;
    let skip_taskbar = intern_atom(&conn, b"_NET_WM_STATE_SKIP_TASKBAR")?;
    let skip_pager = intern_atom(&conn, b"_NET_WM_STATE_SKIP_PAGER")?;
    let below = intern_atom(&conn, b"_NET_WM_STATE_BELOW")?;

    send_state_change(&conn, root, window, net_wm_state, 1, skip_taskbar, skip_pager)?;
    send_state_change(&conn, root, window, net_wm_state, 1, below, 0)?;
    conn.flush().context("failed to flush X11 EWMH hint requests")?;
    Ok(())
}

pub fn restore_if_minimized(window: u32) -> Result<bool> {
    let (conn, screen_num) = RustConnection::connect(None)
        .context("failed to connect to X11 display for restore-minimized flow")?;
    let root = conn.setup().roots[screen_num].root;

    let attrs = conn
        .get_window_attributes(window)
        .context("failed to query window attributes for restore-minimized flow")?
        .reply()
        .context("failed to read window attributes for restore-minimized flow")?;
    if attrs.map_state == MapState::VIEWABLE {
        return Ok(false);
    }

    let net_wm_state = intern_atom(&conn, b"_NET_WM_STATE")?;
    let hidden = intern_atom(&conn, b"_NET_WM_STATE_HIDDEN")?;
    send_state_change(&conn, root, window, net_wm_state, 0, hidden, 0)?;
    conn.map_window(window).context("failed to map minimized wallpaper window")?;
    conn.flush().context("failed to flush restore-minimized requests")?;
    Ok(true)
}

fn intern_atom(conn: &RustConnection, name: &[u8]) -> Result<Atom> {
    conn.intern_atom(false, name)
        .with_context(|| format!("failed to intern atom {}", String::from_utf8_lossy(name)))?
        .reply()
        .context("failed to receive atom reply")
        .map(|r| r.atom)
}

fn send_state_change(
    conn: &RustConnection,
    root: Window,
    window: Window,
    net_wm_state: Atom,
    action: u32,
    first: Atom,
    second: Atom,
) -> Result<()> {
    let event = ClientMessageEvent::new(
        32,
        window,
        net_wm_state,
        ClientMessageData::from([action, first, second, 1, 0]),
    );
    conn.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        event,
    )
    .context("failed to send _NET_WM_STATE client message")?;
    Ok(())
}
