# we-layerd

`we-layerd` is a Rust daemon for running Wallpaper Engine on Linux compositors.

Chinese documentation: [docs/README.zh-CN.md](./docs/README.zh-CN.md)

## Features
- Wine mode: launch `wallpaper64.exe`, capture XWayland/X11 output, render to Wayland layer-shell.
- GNOME mode: register the Wallpaper Engine XWayland window with a GNOME Shell extension, or render native video wallpaper through the bundled GNOME bridge.
- Desktop environment support: KDE Plasma and GNOME Shell.
- Native video mode: FFmpeg + `wgpu` pipeline.
- Windows launcher mode: Wine / Proton (Proton auto-discovery from Steam paths).
- GUI companion (`we-gui`) with tray controls.
- Runtime control commands: `stop`, `pause`, `resume`, `reload`, `status`, `hide-window`, `show-window`.
- Single-instance daemon lock per user.
- Optional cgroup monitor/limit support.

## Dependencies
- Rust toolchain (`cargo`, `rustc`) for building.
- `pkg-config` (`pkgconf`) for native library detection during build.
- Supported desktop environments:
  - KDE Plasma (via layer-shell mode on Wayland).
  - GNOME Shell 45+ (via the bundled GNOME Shell extension bridge for Wine scenes/web and native video).
- Wayland compositor with `zwlr_layer_shell_v1` for layer-shell mode (target: niri; should also work on Hyprland/sway).
- GNOME Shell 45+ for GNOME window-bridge mode, plus the bundled extension from [contrib/gnome-shell-extension](./contrib/gnome-shell-extension).
- `gjs` with Gtk 4 support for GNOME native video mode.
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
See [docs/CONFIGURATION.md](./docs/CONFIGURATION.md) for:
- config file setup
- backend selection
- GNOME extension setup
- niri sizing rules
- Wine / Proton launch modes
- optional cgroup config

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

More docs:
- [docs/CONFIGURATION.md](./docs/CONFIGURATION.md)
- [docs/ADVANCED.md](./docs/ADVANCED.md)
- [docs/TROUBLESHOOTING.md](./docs/TROUBLESHOOTING.md)
- [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md)
