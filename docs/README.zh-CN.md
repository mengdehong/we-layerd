# we-layerd（中文文档）

`we-layerd` 是一个在 Linux 合成器上运行 Wallpaper Engine 的 Rust 守护进程。

## 功能
- Wine 模式：启动 `wallpaper64.exe`，捕获 XWayland/X11 画面，渲染到 Wayland layer-shell。
- GNOME 模式：通过 GNOME Shell 扩展注册 Wallpaper Engine 的 XWayland 窗口，并将真实窗口固定在桌面底层。
- 桌面环境支持：KDE Plasma 与 GNOME Shell。
- 原生视频模式：使用 FFmpeg + `wgpu` 播放视频壁纸。
- Windows 启动器模式：支持 Wine / Proton（自动扫描 Steam 下 Proton 版本）。
- GUI 程序 `we-gui`：壁纸浏览、配置编辑、托盘控制、运行状态查看。
- 运行时控制命令：`stop`、`pause`、`resume`、`reload`、`status`、`hide-window`、`show-window`。
- 单实例守护进程锁（同一用户不可重复启动）。
- 可选 cgroup 监控/限制。

## 依赖
- 构建依赖：Rust 工具链（`cargo`、`rustc`）、`pkg-config`（`pkgconf`）。
- 运行依赖：
  - 已支持的桌面环境：
    - KDE Plasma（Wayland 下通过 layer-shell 模式）。
    - GNOME Shell 45+（通过仓库内置的扩展桥接）。
  - 支持 `zwlr_layer_shell_v1` 的 Wayland 合成器（niri / Hyprland / sway 等）。
  - XWayland/X11 与 XComposite 扩展（Wine 窗口捕获需要）。
  - Vulkan/GL 运行环境（`wgpu`）。
  - FFmpeg 库与头文件（`libavformat`、`libavcodec`、`libavutil`、`libswscale`）。
  - Wine 与 Wallpaper Engine 可执行文件。
  - cgroup v2（仅在启用 cgroup 功能时需要）。

Arch Linux 依赖示例：
```bash
sudo pacman -S --needed rustup pkgconf ffmpeg libx11 libxcomposite libxfixes libxdamage libxrender vulkan-icd-loader wine wlr-randr xdotool
```
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
配置细节请见 [CONFIGURATION.zh-CN.md](./CONFIGURATION.zh-CN.md)，包含：
- 配置文件初始化
- 后端选择
- GNOME 扩展安装
- niri 窗口规则
- Wine / Proton 启动模式
- 可选 cgroup 配置

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

更多文档：
- [CONFIGURATION.zh-CN.md](./CONFIGURATION.zh-CN.md)
- [ADVANCED.zh-CN.md](./ADVANCED.zh-CN.md)
- [TROUBLESHOOTING.zh-CN.md](./TROUBLESHOOTING.zh-CN.md)
- [ARCHITECTURE.md](./ARCHITECTURE.md)
