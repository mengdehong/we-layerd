# 故障排查

- `WAYLAND_DISPLAY` 缺失：当前 shell 不在 Wayland 会话环境内。
- `DISPLAY` 缺失：进程看不到 XWayland/X11 桥接。
- `Invalid MIT-MAGIC-COOKIE-1 key`：X11 鉴权失败。请从当前图形登录会话里直接启动 `we-layerd`，或在启动前通过 `XAUTHORITY` 指向当前会话使用的 X11 cookie 文件。
- `Wallpaper Engine is not installed. Please install it, or choose paths in Settings.`：未发现 `wallpaper_engine` 目录。请先安装，或在设置中手动指定路径。
- `Wallpaper Engine first-run setup is pending. Launch it once in Steam to run installer.exe.`：存在 `installer.exe` 但缺少 `wallpaper64.exe`。请先在 Steam 里运行一次 Wallpaper Engine 完成首次安装。
- 找不到窗口：调整 `capture.wm_class_contains` / `capture.title_contains`，或指定 `capture.net_wm_pid`。
- niri 启动半屏/缩放异常：添加 `window-rule`，使用 `match app-id="WE-DEBUG-WINDOW"` 并设置 `open-maximized-to-edges true`。
- Capture errors：确认 XComposite 可用，并且目标窗口仍然存在。
- No layer surface：当前合成器可能没有暴露 `zwlr_layer_shell_v1`。
- GNOME bridge unavailable：确认仓库内置的 `we-layerd@aromatic` 扩展已安装、启用，并在 session bus 上导出了 `io.github.weLayerd.Gnome`。
- Wine path error：确认 `wine.wallpaper_exe` 指向有效的 `.exe` 文件。
- `ctl` 无法连接：确认 daemon 正在运行，且用户/会话环境匹配（`WAYLAND_DISPLAY`、`XDG_RUNTIME_DIR`）。
- cgroup 无数据或限制无效：确认系统为 cgroup v2 且当前用户有 delegation 权限，可在 `ctl status` 的 `status.cgroup.last_error` 查看原因。
