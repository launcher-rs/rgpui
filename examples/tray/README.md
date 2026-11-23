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
| `tray`（默认） | 完整托盘：自定义图标、tooltip、自绘标题栏、窗口内「隐藏到托盘 / 激活窗口 / 退出」按钮、关闭按钮隐藏到托盘；托盘菜单「显示窗口」恢复 | `cargo run -p tray` |
| `tray_simple` | 最小托盘：无窗口纯托盘 + 简单菜单（参考 adabraka-gpui 的 tray_test） | `cargo run -p tray --bin tray_simple` |
| `tray_menu_action` | 旧 `set_tray` API：菜单项自带 `Action`，由 `on_app_menu_action` 收到（Linux/Windows 均按此派发） | `cargo run -p tray --bin tray_menu_action` |

## 验证托盘菜单与恢复

运行默认 binary 后：

1. 点窗口内「隐藏到托盘」（或标题栏关闭按钮），窗口从任务栏 / 切换器移除但进程仍在
2. 左键或右键点托盘图标 → 应弹出菜单（GNOME/Ubuntu 主机经 dbusmenu 弹菜单）
3. 点菜单「显示窗口」→ 窗口重新映射并恢复

> 托盘菜单依赖主机在注册后拉取到非空菜单。框架在向 watcher 注册前留出初始设置
> 窗口，避免主机过早拉到空菜单后缓存、不再重拉（GNOME ubuntu-appindicators 对空
> 菜单不会再次拉取）。Wayland 的 xdg-activation 令牌可能被合成器拒绝（协议层面
> 限制），此时可再点一次托盘图标。
