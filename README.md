# we-layerd

`we-layerd` is a Rust daemon for running Wallpaper Engine on Linux compositors.

Chinese documentation: [docs/README.zh-CN.md](./docs/README.zh-CN.md)

## Features
- Wine mode: launch `wallpaper64.exe`, capture XWayland/X11 output, render to Wayland layer-shell.
- Native video mode: FFmpeg + `wgpu` pipeline.
- GUI companion (`we-gui`) with tray controls.
- Runtime control commands: `stop`, `pause`, `resume`, `reload`, `status`, `hide-window`, `show-window`.
- Single-instance daemon lock per user.
- Optional cgroup monitor/limit support.

## Dependencies
- Rust toolchain (`cargo`, `rustc`) for building.
- `pkg-config` (`pkgconf`) for native library detection during build.
- Wayland compositor with `zwlr_layer_shell_v1` (target: niri; should also work on Hyprland/sway).
- XWayland/X11 for Wine render window capture.
- X11 Composite extension.
- Vulkan/GL stack usable by `wgpu`.
- FFmpeg libraries and headers (`libavformat`, `libavcodec`, `libavutil`, `libswscale`).
- Wine + Wallpaper Engine executable (`wallpaper64.exe` or `wallpaper32.exe`).
- Linux cgroup v2 (optional, only when cgroup feature is enabled in config).

Example packages (Arch Linux):
```bash
sudo pacman -S --needed rustup pkgconf ffmpeg libx11 libxcomposite libxfixes libxdamage libxrender vulkan-icd-loader wine wlr-randr
```

## Build
Build release binaries:
```bash
cargo build --release -p we-layerd -p we-gui
```

Install binaries to a directory in `PATH` (example: `~/.local/bin`):
```bash
install -Dm755 target/release/we-layerd ~/.local/bin/we-layerd
install -Dm755 target/release/we-gui ~/.local/bin/we-gui
```

## Config
Start from:
```bash
cp config.example.toml ~/.config/we-layerd/config.toml
```

Minimal required fields:
- `wine.wallpaper_exe` (usually `~/.local/share/Steam/steamapps/common/wallpaper_engine/wallpaper64.exe`).
- `wine.args` for scene/web mode launch (`openWallpaper` args).
- `runtime` block (`wine_layerd` or `video_native`).
- Optionally tune `capture` match rules.

Optional cgroup block:
```toml
[cgroup]
enabled = false
mode = "detect"      # detect | limit_wine
memory_max = "max"   # optional, e.g. "2147483648"
cpu_max = "max 100000" # optional, e.g. "50000 100000"
```

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

## Usage
After `we-layerd`/`we-gui` are in `PATH`:

Start GUI:
```bash
we-gui
```

Start daemon directly:
```bash
we-layerd run --config ~/.config/we-layerd/config.toml
```

Control a running daemon:
```bash
we-layerd ctl stop
we-layerd ctl pause
we-layerd ctl resume
we-layerd ctl reload
we-layerd ctl status
we-layerd ctl hide-window
we-layerd ctl show-window
```

Other commands:
```bash
we-layerd doctor
we-layerd print-config --config ~/.config/we-layerd/config.toml
```

## IPC and single-instance
- On Linux, control IPC uses an abstract Unix socket name (`we-layerd.control.<uid>`).
- File-socket fallback is kept for compatibility.
- Daemon startup acquires an instance lock; launching a second instance under the same user returns an `already running` error.

## Troubleshooting
- `WAYLAND_DISPLAY` missing: you are not in a Wayland session shell.
- `DISPLAY` missing: XWayland/X11 bridge is not visible to the process.
- `Wallpaper Engine is not installed. Please install it, or choose paths in Settings.`: Steam common path does not contain `wallpaper_engine`. Install Wallpaper Engine first, or set paths manually in Settings.
- `Wallpaper Engine first-run setup is pending. Launch it once in Steam to run installer.exe.`: `installer.exe` exists but `wallpaper64.exe` is missing. Run Wallpaper Engine once in Steam to complete first-run setup.
- Cannot find window: relax `capture.wm_class_contains` / `capture.title_contains`, or pin `capture.net_wm_pid`.
- Capture errors: ensure XComposite is available and the window still exists.
- No layer surface: compositor may not expose `zwlr_layer_shell_v1`.
- Wine path error: verify `wine.wallpaper_exe` points to an existing `.exe`.
- `ctl` cannot connect: check if daemon is running and user/session match (`WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`).
- cgroup metrics empty / limits not applied: verify cgroup v2 and user delegation permissions; see `ctl status` `status.cgroup.last_error`.
