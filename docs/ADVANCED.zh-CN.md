# 高级主题

## IPC 与单实例

- Linux 下控制 IPC 默认使用抽象 Unix socket 名称（`we-layerd.control.<uid>`）。
- 同时保留文件 socket 回退逻辑以兼容更多环境。
- 守护进程启动时会获取单实例锁；同一用户重复启动会返回 `already running`。

## 运行时控制

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

## cgroup

- Linux cgroup v2 是可选能力，只有在配置里启用 cgroup 功能时才需要。
- `detect` 会让 `we-layerd` 自动选择运行时策略。
- `limit_wine` 用于对 Wine 进程树施加显式限制。

具体配置块见 [CONFIGURATION.zh-CN.md](./CONFIGURATION.zh-CN.md)。
