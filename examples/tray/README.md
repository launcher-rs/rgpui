# tray（系统托盘）

系统托盘示例，原为 2 个独立 crate（`tray`、`tray_simple`），现合并为一个
crate 下的多个 binary。

| binary | 说明 | 运行 |
|--------|------|------|
| `tray`（默认） | 完整托盘：自定义图标、 tooltip、图标点击事件、关闭按钮最小化到托盘、菜单恢复窗口 | `cargo run -p tray` |
| `tray_simple` | 最小托盘：无窗口纯托盘 + 简单菜单（参考 adabraka-gpui 的 tray_test） | `cargo run -p tray --bin tray_simple` |
