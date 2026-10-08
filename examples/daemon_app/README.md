# daemon_app（守护进程 + 覆盖层）

演示无主窗口常驻应用：单实例进程（`SingleInstance`，重复启动时激活已有实例）、
常置顶透明覆盖层窗口（`WindowKind::Overlay`）、系统托盘控制、系统电源事件与
电源抑制。

托盘里的三项电源菜单演示阻止器的生命周期：按下「阻止系统休眠 / 阻止息屏」向平台
申请句柄并持有，「取消电源阻止」把句柄丢掉 —— **持有即生效、`Drop` 即恢复**系统的
省电策略，Linux 上可用 `systemd-inhibit --list` 对照。`on_system_power_event` 则在
即将睡眠与已唤醒两个时机各打印一行。

## 运行

```text
cargo run -p daemon_app
```

排障时带上日志，可看到抑制被拒（例如 polkit 未授权）的具体原因：

```text
RUST_LOG=warn cargo run -p daemon_app
```
