# we-layerd

`we-layerd` is a Rust daemon that hosts Wallpaper Engine rendering inside a Wayland layer-shell background.

## Goals
- Launch Wallpaper Engine with Wine.
- Discover the X11/XWayland render window.
- Capture frames using XComposite.
- Present frames on Wayland background layer surfaces.

## Status
Bootstrap project. Core features are implemented incrementally in small commits.

## Build
```bash
cargo build
```
