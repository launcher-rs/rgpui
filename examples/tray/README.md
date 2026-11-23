# tray（系统托盘）

系统托盘示例，原为 2 个独立 crate（`tray`、`tray_simple`），现合并为一个
crate 下的多个 binary。

Linux Wayland 的普通窗口支持真正隐藏到托盘：`hide_window()` 取消窗口映射，
从任务栏移除但保留应用与会话；`activate_window()` 重新映射并在首帧后激活。
`minimize_window()` 仍然只最小化。KDE 提供的托盘激活令牌会保留到恢复首帧后使用。
Layer shell 和 popup 不属于此普通窗口隐藏/恢复流程。

手工回归时检查：关闭后任务栏入口消失、托盘恢复可交互、后台更新不会重新显示窗口、
快速重复隐藏/显示、最大化/全屏恢复，以及隐藏期间切换显示器缩放。

| binary | 说明 | 运行 |
|--------|------|------|
| `tray`（默认） | 完整托盘：自定义图标、 tooltip、图标点击事件、关闭按钮最小化到托盘、菜单恢复窗口 | `cargo run -p tray` |
| `tray_simple` | 最小托盘：无窗口纯托盘 + 简单菜单（参考 adabraka-gpui 的 tray_test） | `cargo run -p tray --bin tray_simple` |
| `tray_menu_action` | 旧 `set_tray` API：菜单项自带 `Action`，由 `on_app_menu_action` 收到（Linux/Windows 均按此派发） | `cargo run -p tray --bin tray_menu_action` |
