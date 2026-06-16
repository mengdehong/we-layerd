# 配置

复制示例配置：
```bash
cp config.example.toml ~/.config/we-layerd/config.toml
```

关键字段：
- `wine.wallpaper_exe`：Wallpaper Engine 可执行文件路径。
- `wine.args`：scene/web 模式启动参数（`openWallpaper`）。
- `runtime`：`wine_layerd` 或 `video_native`。
- `runtime.video_file`：当 `runtime.mode = "video_native"` 时必填。
- 可按需调整 `capture` 匹配规则。

可选 cgroup 配置：
```toml
[cgroup]
enabled = false
mode = "detect"      # detect | limit_wine
memory_max = "max"   # 可选，如 "2147483648"
cpu_max = "max 100000" # 可选，如 "50000 100000"
```

调试窗口隐藏配置：
```toml
[general]
hide_debug_window = true
hidden_workspace_name = "top"
```
`hidden_workspace_name` 的行为：
- Hyprland：作为 special workspace 名称（`special:<name>`）。
- sway：使用 scratchpad 机制。
- niri：作为目标工作区标识；`top` 表示最上/第一个工作区。
在 niri 下隐藏顺序是先 `move-window-to-workspace`，再 `move-window-to-floating`。

niri 启动尺寸：
- niri 的默认平铺行为可能让 Wallpaper Engine 调试窗口初始为半屏。
- 不要在窗口打开后通过 IPC 动作二次改尺寸，这可能导致黑屏。
- 请在 niri 配置中使用 `window-rule`，并且只用 `match app-id="WE-DEBUG-WINDOW"`。

后端选择：
```toml
[general]
backend = "auto" # auto | layer_shell | gnome_shell
```
- `auto`：当 `XDG_CURRENT_DESKTOP` 指示为 GNOME 时使用 `gnome_shell`，否则使用 `layer_shell`（例如 KDE Plasma）。
- `layer_shell`：始终使用原生 Wayland 背景渲染。
- `gnome_shell`：要求 GNOME Shell 扩展提供 D-Bus 桥接。

GNOME 扩展：
```toml
[gnome]
extension_dbus_name = "io.github.weLayerd.Gnome"
```
请将 [contrib/gnome-shell-extension/we-layerd@aromatic](../contrib/gnome-shell-extension/we-layerd@aromatic)
安装到 `~/.local/share/gnome-shell/extensions/`，并在启动 `we-layerd` 前于 GNOME Extensions 中启用。
当 `runtime.mode = "video_native"` 且后端为 GNOME 时，扩展还会启动仓库内置的 `gjs` + Gtk 4 视频渲染器，并将其 clone 到桌面背景。

niri 配置示例：
```kdl
window-rule {
    match app-id="WE-DEBUG-WINDOW"
    open-floating false
    open-maximized-to-edges true
    open-focused false
}
```
可通过以下命令确认匹配字段：
```bash
niri msg -j windows
```

Wine / Proton 启动配置：
```toml
[wine]
command = "wine"
command_mode = "exe_with_args" # exe_with_args | command_only
```
- `exe_with_args`：执行 `command wallpaper_exe ...args`（Wine 模式）。
- `command_only`：执行 `command ...args`（Proton 模式，使用 `proton run ...`）。
