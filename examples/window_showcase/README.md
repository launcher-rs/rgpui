# window_showcase（窗口管理展示）

窗口相关示例合集，原为 7 个独立 crate（`window`、`window_movable`、
`window_positioning`、`window_shadow`、`transparent`、`opacity`、`shadow`），
现合并为一个 crate 下的多个 binary。

| binary | 说明 | 运行 |
|--------|------|------|
| `window` | 窗口类型（Normal/Popup/Floating/Dialog）、自定义标题栏、prompt | `cargo run -p window_showcase --bin window` |
| `window_movable` | `is_movable` 可移动开关 × 原生/自定义标题栏 4 种组合 | `cargo run -p window_showcase --bin window_movable` |
| `window_positioning` | 窗口定位与多窗口布局 | `cargo run -p window_showcase --bin window_positioning` |
| `window_shadow` | 窗口阴影效果 | `cargo run -p window_showcase --bin window_shadow` |
| `transparent` | 透明背景窗口（`WindowBackgroundAppearance::Transparent`） | `cargo run -p window_showcase --bin transparent` |
| `opacity` | 窗口整体不透明度 | `cargo run -p window_showcase --bin opacity` |
| `shadow` | 阴影样式大全 | `cargo run -p window_showcase --bin shadow` |

不带 `--bin` 直接 `cargo run -p window_showcase` 会打印本列表。
