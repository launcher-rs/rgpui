# window_showcase（窗口管理展示）

窗口相关示例合集。`window` / `custom_title_bar` / `overlay_probe` 保持独立 binary，
其余合并进默认 binary，用 `-- <demo>` 选择。

| binary / demo | 说明 | 运行 |
|---------------|------|------|
| `window` | 窗口类型（Normal/Popup/Floating/Dialog）、自定义标题栏、prompt | `cargo run -p window_showcase --bin window` |
| `custom_title_bar` | 自定义标题栏完整用法（`TitleBar::window_options` + 可交互标题栏 + 计数器 + 颜色块） | `cargo run -p window_showcase --bin custom_title_bar` |
| `window_movable` | `is_movable` 可移动开关 × 原生/自定义标题栏 4 种组合 | `cargo run -p window_showcase -- window_movable` |
| `window_positioning` | 窗口定位与多窗口布局 | `cargo run -p window_showcase -- window_positioning` |
| `window_shadow` | 窗口阴影效果 | `cargo run -p window_showcase -- window_shadow` |
| `transparent` | 透明背景窗口（`WindowBackgroundAppearance::Transparent`）；部分 X11/软件渲染环境下透明窗口可能整层不可见 | `cargo run -p window_showcase -- transparent` |
| `opacity` | 窗口整体不透明度 | `cargo run -p window_showcase -- opacity` |
| `shadow` | 阴影样式大全 | `cargo run -p window_showcase -- shadow` |

不带参数默认运行 `window_movable`。
