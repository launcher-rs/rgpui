# text_showcase（文本展示）

文本相关示例合集，合并为一个 binary，用 `-- <demo>` 选择具体演示。

| demo | 说明 | 运行 |
|------|------|------|
| `text` | 字体排印上下文（`TextContext` 全局字号/行高/缩放）与文本样式 | `cargo run -p text_showcase -- text` |
| `text_layout` | 文本对齐（左/中/右）、下划线、删除线、高亮 | `cargo run -p text_showcase -- text_layout` |
| `text_wrapper` | 换行、省略号（单行/多行截断）、`line_clamp`、中日英混排 | `cargo run -p text_showcase -- text_wrapper` |

不带参数默认运行 `text`。
