# Advanced

## IPC And Single-Instance

- On Linux, control IPC uses an abstract Unix socket name (`we-layerd.control.<uid>`).
- File-socket fallback is kept for compatibility.
- Daemon startup acquires an instance lock; launching a second instance under the same user returns an `already running` error.

## Runtime Control

Control a running daemon:
```bash
we-layerd ctl stop
we-layerd ctl pause
we-layerd ctl resume
we-layerd ctl reload
we-layerd ctl status
we-layerd ctl hide-window
we-layerd ctl show-window
```

Other commands:
```bash
we-layerd doctor
we-layerd print-config --config ~/.config/we-layerd/config.toml
```

## cgroup

- Linux cgroup v2 is optional and only needed when the cgroup feature is enabled in config.
- Use `detect` to let `we-layerd` choose a runtime strategy.
- Use `limit_wine` when you want explicit limits applied to the Wine process tree.

See [CONFIGURATION.md](./CONFIGURATION.md) for the config block.
