# we-layerd

`we-layerd` is a Rust daemon that launches Wallpaper Engine through Wine, captures frames from the XWayland/X11 render window, and draws them as Wayland `wlr-layer-shell-unstable-v1` background surfaces.

## Features (current)
- `run`, `doctor`, `print-config` CLI commands.
- Configurable Wine launch command and Wallpaper Engine executable path.
- X11 window discovery via `_NET_WM_PID`, `WM_CLASS`, title keywords.
- XComposite single-frame and streaming capture (`XGetImage` path).
- `wgpu` rendering pipeline that uploads captured RGBA frames to textures.
- Layer-shell background surfaces with click-through input regions.
- Basic multi-output scaffold (`output-<global-id>` mapping).
- Resilience basics: readable errors, capture failure refind, optional Wine auto-restart.

## Build
```bash
cargo build
```

## Runtime dependencies
- Wayland compositor with `zwlr_layer_shell_v1` (target: niri; should also work on Hyprland/sway).
- XWayland/X11 for Wine render window capture.
- X11 Composite extension.
- Vulkan/GL stack usable by `wgpu`.
- Wine + Wallpaper Engine executable (`wallpaper64.exe` or `wallpaper32.exe`).

## Config
Start from:
```bash
cp config.example.toml config.toml
```

Minimal required fields:
- `wine`
- `steam` and `wallpaper64.exe` usually in `~/.local/share/Steam/steamapps/common/wallpaper_engine/wallpaper64.exe`.
- wallpapaers, usually in `~/.local/share/Steam/steamapps/workshop/content/`, please verify your path.
- Optionally tune `capture` match rules.

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

## Troubleshooting
- `WAYLAND_DISPLAY` missing: you are not in a Wayland session shell.
- `DISPLAY` missing: XWayland/X11 bridge is not visible to the process.
- Cannot find window: relax `capture.wm_class_contains` / `capture.title_contains`, or pin `capture.net_wm_pid`.
- Capture errors: ensure XComposite is available and the window still exists.
- No layer surface: compositor may not expose `zwlr_layer_shell_v1`.
- Wine path error: verify `wine.wallpaper_exe` points to an existing `.exe`.
