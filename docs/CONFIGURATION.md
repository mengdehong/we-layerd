# Configuration

Start from:
```bash
cp config.example.toml ~/.config/we-layerd/config.toml
```

Minimal required fields:
- `wine.wallpaper_exe` (usually `~/.local/share/Steam/steamapps/common/wallpaper_engine/wallpaper64.exe`).
- `wine.args` for scene/web mode launch (`openWallpaper` args).
- `runtime` block (`wine_layerd` or `video_native`).
- `runtime.video_file` when `runtime.mode = "video_native"`.
- Optionally tune `capture` match rules.

Optional cgroup block:
```toml
[cgroup]
enabled = false
mode = "detect"      # detect | limit_wine
memory_max = "max"   # optional, e.g. "2147483648"
cpu_max = "max 100000" # optional, e.g. "50000 100000"
```

Hard-isolate the debug window:
```toml
[isolation]
mode = "gamescope_headless" # none | gamescope_headless
command = "gamescope"
width = 2560
height = 1600
startup_timeout_secs = 10
```
- `none`: Wine still creates a host XWayland/Niri window, and `hide_debug_window` moves or hides it.
- `gamescope_headless`: starts a headless gamescope instance, runs Wine against gamescope's internal XWayland display, and captures XComposite from that internal `DISPLAY`. Niri never sees the Wallpaper Engine window, so switching workspaces or opening overview cannot reveal it.
- `width`/`height` are optional. When omitted, we-layerd uses `-width` / `-height` from `wine.args`, then falls back to 1920x1080.

Debug window visibility:
```toml
[general]
hide_debug_window = true
hidden_workspace_name = "top"
```
`hide_debug_window` defaults to `true`. `hidden_workspace_name` controls the hide target:
- Hyprland: special workspace name (`special:<name>`).
- sway: uses scratchpad behavior.
- niri: target workspace spec; use `top` to move to the top/first workspace.
For niri, hide flow is `move-window-to-workspace` first, then `move-window-to-floating`.
When `isolation.mode = "gamescope_headless"` is enabled, host WM hiding is skipped because the real Wine window is no longer in the host window tree.

niri startup sizing:
- Wallpaper Engine debug window may open as half-screen by default under niri tiling.
- Do not resize this window after launch via IPC actions; it can lead to black output.
- Define a niri `window-rule` that matches `WE-DEBUG-WINDOW` at open time.
- In this project, use `match app-id="WE-DEBUG-WINDOW"` directly.

Backend selection:
```toml
[general]
backend = "auto" # auto | layer_shell | gnome_shell
```
- `auto`: uses `gnome_shell` when `XDG_CURRENT_DESKTOP` indicates GNOME, otherwise `layer_shell` (for example KDE Plasma).
- `layer_shell`: always use the native Wayland background renderer.
- `gnome_shell`: require the GNOME Shell extension D-Bus bridge.

GNOME extension:
```toml
[gnome]
extension_dbus_name = "io.github.weLayerd.Gnome"
```
Install the extension directory [contrib/gnome-shell-extension/we-layerd@aromatic](../contrib/gnome-shell-extension/we-layerd@aromatic)
into `~/.local/share/gnome-shell/extensions/`, then enable it in GNOME Extensions before launching `we-layerd`.
For `runtime.mode = "video_native"` on GNOME, the extension also launches a bundled `gjs` + Gtk 4 renderer process and clones it into the desktop background.

Example niri config:
```kdl
window-rule {
    match app-id="WE-DEBUG-WINDOW"
    open-floating false
    open-maximized-to-edges true
    open-focused false
}
```
Check with:
```bash
niri msg -j windows
```

Wine/Proton launch behavior:
```toml
[wine]
command = "wine"
command_mode = "exe_with_args" # exe_with_args | command_only
```
- `exe_with_args`: runs `command wallpaper_exe ...args` (Wine mode).
- `command_only`: runs `command ...args` (Proton mode via `proton run ...`).
