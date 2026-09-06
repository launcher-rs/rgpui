# text_showcase（文本展示）

文本相关示例合集，原为 3 个独立 crate（`text`、`text_layout`、`text_wrapper`），
现合并为一个 crate 下的多个 binary。

| binary | 说明 | 运行 |
|--------|------|------|
| `text` | 字体排印上下文（`TextContext` 全局字号/行高/缩放）与文本样式 | `cargo run -p text_showcase --bin text` |
| `text_layout` | 文本对齐（左/中/右）、下划线、删除线、高亮 | `cargo run -p text_showcase --bin text_layout` |
| `text_wrapper` | 换行、省略号（单行/多行截断）、`line_clamp`、中日英混排 | `cargo run -p text_showcase --bin text_wrapper` |

不带 `--bin` 直接 `cargo run -p text_showcase` 会打印本列表。
