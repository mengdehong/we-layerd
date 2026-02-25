# Architecture

## Workspace layout

- `we-layerd` (root crate): Linux runtime daemon (Wayland + X11 + Wine).
  - Single-instance process lock (`flock`).
  - IPC control server (`ctl stop/pause/resume/reload/status`).
  - Optional cgroup runtime monitor/limiter (`detect` / `limit_wine`).
- `crates/we-core`: Cross-platform shared library.
  - Steam / workshop path discovery.
  - Wallpaper metadata parsing (`project.json` type/title).
  - Preview image detection.
  - Shared config model used by GUI to generate daemon config.
- `apps/we-gui`: Iced GUI app.
  - Scan workshop wallpapers using `we-core`.
  - Present wallpaper list and metadata.
  - Generate `~/.config/we-layerd/config.toml`.
  - Tray lifecycle + runtime controls (play/switch, stop, pause, resume, exit).
  - Settings panel (resolution/fps/path/cgroup settings + runtime status polling).

## Notes

- `we-core` is intended to compile on macOS for GUI-first development.
- Keep runtime Linux-specific integrations in `we-layerd` crate.
