# Architecture

## Workspace layout

- `we-layerd` (root crate): Linux runtime daemon (Wayland + X11 + Wine).
- `crates/we-core`: Cross-platform shared library.
  - Steam / workshop path discovery.
  - Wallpaper metadata parsing (`project.json` type/title).
  - Preview image detection.
- `apps/we-gui`: Iced GUI app.
  - Scan workshop wallpapers using `we-core`.
  - Present wallpaper list and metadata.
  - Next step: generate `~/.config/we-layerd/config.toml`, start/stop/reload `we-layerd`.

## Notes

- `we-core` is intended to compile on macOS for GUI-first development.
- Keep runtime Linux-specific integrations in `we-layerd` crate.
