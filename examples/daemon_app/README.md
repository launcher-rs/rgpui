# daemon_app（守护进程 + 覆盖层）

演示无主窗口常驻应用：单实例进程（`SingleInstance`，重复启动时激活已有实例）、
常置顶透明覆盖层窗口（`WindowKind::Overlay`）、系统托盘控制。

## 运行

```text
cargo run -p daemon_app
```
