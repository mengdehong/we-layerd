# we-layerd（中文文档）

`we-layerd` 是一个在 Linux 合成器上运行 Wallpaper Engine 的 Rust 守护进程。

## 功能
- Wine 模式：启动 `wallpaper64.exe`，捕获 XWayland/X11 画面，渲染到 Wayland layer-shell。
- 原生视频模式：使用 FFmpeg + `wgpu` 播放视频壁纸。
- Windows 启动器模式：支持 Wine / Proton（自动扫描 Steam 下 Proton 版本）。
- GUI 程序 `we-gui`：壁纸浏览、配置编辑、托盘控制、运行状态查看。
- 运行时控制命令：`stop`、`pause`、`resume`、`reload`、`status`、`hide-window`、`show-window`。
- 单实例守护进程锁（同一用户不可重复启动）。
- 可选 cgroup 监控/限制。

## 依赖
- 构建依赖：Rust 工具链（`cargo`、`rustc`）、`pkg-config`（`pkgconf`）。
- 运行依赖：
  - 支持 `zwlr_layer_shell_v1` 的 Wayland 合成器（niri / Hyprland / sway 等）。
  - XWayland/X11 与 XComposite 扩展（Wine 窗口捕获需要）。
  - Vulkan/GL 运行环境（`wgpu`）。
  - FFmpeg 库与头文件（`libavformat`、`libavcodec`、`libavutil`、`libswscale`）。
  - Wine 与 Wallpaper Engine 可执行文件。
  - cgroup v2（仅在启用 cgroup 功能时需要）。

Arch Linux 依赖示例：
```bash
sudo pacman -S --needed rustup pkgconf ffmpeg libx11 libxcomposite libxfixes libxdamage libxrender vulkan-icd-loader wine wlr-randr xdotool
## 构建
构建发布版二进制：
```bash
cargo build --release -p we-layerd -p we-gui
```

安装到 `PATH`（示例：`~/.local/bin`）：
```bash
install -Dm755 target/release/we-layerd ~/.local/bin/we-layerd
install -Dm755 target/release/we-gui ~/.local/bin/we-gui
```

## 配置
复制示例配置：
```bash
cp config.example.toml ~/.config/we-layerd/config.toml
```

关键字段：
- `wine.wallpaper_exe`：Wallpaper Engine 可执行文件路径。
- `wine.args`：scene/web 模式启动参数（`openWallpaper`）。
- `runtime`：`wine_layerd` 或 `video_native`。

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

niri 启动尺寸（重要）：
- niri 的默认平铺行为可能让 Wallpaper Engine 调试窗口初始为半屏。
- 不要在窗口打开后通过 IPC 动作二次改尺寸，这可能导致黑屏。
- 请在 niri 配置中使用 `window-rule`，并且只用 `match app-id="WE-DEBUG-WINDOW"`。

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

## 使用
确保 `we-layerd` / `we-gui` 在 `PATH` 后：

启动 GUI：
```bash
we-gui
```

直接启动守护进程：
```bash
we-layerd run --config ~/.config/we-layerd/config.toml
```

控制运行中的守护进程：
```bash
we-layerd ctl stop
we-layerd ctl pause
we-layerd ctl resume
we-layerd ctl reload
we-layerd ctl status
we-layerd ctl hide-window
we-layerd ctl show-window
```

其他命令：
```bash
we-layerd doctor
we-layerd print-config --config ~/.config/we-layerd/config.toml
```

## 故障排查
- `WAYLAND_DISPLAY` 缺失：当前 shell 不在 Wayland 会话环境内。
- `DISPLAY` 缺失：进程看不到 XWayland/X11 桥接。
- `Invalid MIT-MAGIC-COOKIE-1 key`：X11 鉴权失败。请从当前图形登录会话里直接启动 `we-layerd`，或在启动前通过 `XAUTHORITY` 指向当前会话使用的 X11 cookie 文件。
- `Wallpaper Engine is not installed. Please install it, or choose paths in Settings.`：
  未发现 `wallpaper_engine` 目录。请先安装，或在设置中手动指定路径。
- `Wallpaper Engine first-run setup is pending. Launch it once in Steam to run installer.exe.`：
  存在 `installer.exe` 但缺少 `wallpaper64.exe`。请先在 Steam 里运行一次 Wallpaper Engine 完成首次安装。
- 找不到窗口：调整 `capture.wm_class_contains` / `capture.title_contains`，或指定 `capture.net_wm_pid`。
- niri 启动半屏/缩放异常：添加 `window-rule`，使用 `match app-id="WE-DEBUG-WINDOW"` 并设置 `open-maximized-to-edges true`。
- `ctl` 无法连接：确认 daemon 正在运行，且用户/会话环境匹配（`WAYLAND_DISPLAY`、`XDG_RUNTIME_DIR`）。
- cgroup 无数据或限制无效：确认系统为 cgroup v2 并且当前用户有 delegation 权限，可在 `ctl status` 的 `status.cgroup.last_error` 查看原因。
