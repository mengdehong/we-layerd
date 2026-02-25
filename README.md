# we-layerd

`we-layerd` is a Rust daemon for running Wallpaper Engine on Linux compositors.

It supports:
- Wine mode: launch `wallpaper64.exe`, capture XWayland/X11 output, and render to Wayland `wlr-layer-shell`.
- Native video mode: decode video wallpapers in-process (FFmpeg path) and render with `wgpu`.
- GUI companion (`we-gui`): wallpaper browser, config editor, tray control, and runtime status view.

## Features (current)
- `run`, `doctor`, `print-config`, `ctl` CLI commands.
- Configurable Wine launch command and Wallpaper Engine executable path.
- X11 window discovery via `_NET_WM_PID`, `WM_CLASS`, title keywords.
- XComposite single-frame and streaming capture (`XGetImage` path).
- `wgpu` rendering pipeline that uploads captured RGBA frames to textures.
- Layer-shell background surfaces with click-through input regions.
- Basic multi-output scaffold (`output-<global-id>` mapping).
- Resilience basics: readable errors, capture failure refind, optional Wine auto-restart.
- Single-instance daemon lock (same user cannot run two daemon instances).
- IPC control channel for runtime commands (`stop`, `pause`, `resume`, `reload`, `status`).
- Optional cgroup integration:
  - `detect`: monitor daemon + wine usage.
  - `limit_wine`: apply limits to Wine process group only.
- `ctl status` returns effective runtime config plus runtime/cgroup status block.

## Build
```bash
cargo build
```

Workspace targets:
```bash
# core library
cargo build -p we-core

# GUI app (cross-platform dev target)
cargo run -p we-gui

# daemon
cargo run -p we-layerd -- run --config ./config.toml
```

## Runtime dependencies
- Wayland compositor with `zwlr_layer_shell_v1` (target: niri; should also work on Hyprland/sway).
- XWayland/X11 for Wine render window capture.
- X11 Composite extension.
- Vulkan/GL stack usable by `wgpu`.
- Wine + Wallpaper Engine executable (`wallpaper64.exe` or `wallpaper32.exe`).
- Linux cgroup v2 (optional, only when cgroup feature is enabled in config).

## Config
Start from:
```bash
cp config.example.toml config.toml
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

Print effective config defaults:
```bash
cargo run -- print-config
```

## Commands
Run daemon:
```bash
cargo run -- run --config ./config.toml
```

Diagnostics:
```bash
cargo run -- doctor
```

Control a running daemon:
```bash
# stop / pause / resume / reload
cargo run -- ctl stop
cargo run -- ctl pause
cargo run -- ctl resume
cargo run -- ctl reload

# query runtime status (effective config + runtime status block)
cargo run -- ctl status
```

## IPC and single-instance behavior
- On Linux, control IPC uses an abstract Unix socket name (`we-layerd.control.<uid>`).
- File-socket fallback is kept for compatibility.
- Daemon startup acquires an instance lock; launching a second instance under the same user returns an `already running` error.

## Troubleshooting
- `WAYLAND_DISPLAY` missing: you are not in a Wayland session shell.
- `DISPLAY` missing: XWayland/X11 bridge is not visible to the process.
- Cannot find window: relax `capture.wm_class_contains` / `capture.title_contains`, or pin `capture.net_wm_pid`.
- Capture errors: ensure XComposite is available and the window still exists.
- No layer surface: compositor may not expose `zwlr_layer_shell_v1`.
- Wine path error: verify `wine.wallpaper_exe` points to an existing `.exe`.
- `ctl` cannot connect: check if daemon is running and user/session match (`WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`).
- cgroup metrics empty / limits not applied: verify cgroup v2 and user delegation permissions; see `ctl status` `status.cgroup.last_error`.
