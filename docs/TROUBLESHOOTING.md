# Troubleshooting

- `WAYLAND_DISPLAY` missing: you are not in a Wayland session shell.
- `DISPLAY` missing: XWayland/X11 bridge is not visible to the process.
- `Invalid MIT-MAGIC-COOKIE-1 key`: X11 authentication failed. Start `we-layerd` from the same graphical login session, or set `XAUTHORITY` to the matching X11 cookie file before launch.
- `Wallpaper Engine is not installed. Please install it, or choose paths in Settings.`: Steam common path does not contain `wallpaper_engine`. Install Wallpaper Engine first, or set paths manually in Settings.
- `Wallpaper Engine first-run setup is pending. Launch it once in Steam to run installer.exe.`: `installer.exe` exists but `wallpaper64.exe` is missing. Run Wallpaper Engine once in Steam to complete first-run setup.
- Cannot find window: relax `capture.wm_class_contains` / `capture.title_contains`, or pin `capture.net_wm_pid`.
- niri shows half-screen / odd scaling at startup: add a niri `window-rule` with `match app-id="WE-DEBUG-WINDOW"` and `open-maximized-to-edges true`.
- Capture errors: ensure XComposite is available and the window still exists.
- No layer surface: compositor may not expose `zwlr_layer_shell_v1`.
- GNOME bridge unavailable: confirm the bundled `we-layerd@aromatic` extension is installed, enabled, and exporting `io.github.weLayerd.Gnome` on the session bus.
- Wine path error: verify `wine.wallpaper_exe` points to an existing `.exe`.
- `ctl` cannot connect: check if daemon is running and user/session match (`WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`).
- cgroup metrics empty / limits not applied: verify cgroup v2 and user delegation permissions; see `ctl status` `status.cgroup.last_error`.
