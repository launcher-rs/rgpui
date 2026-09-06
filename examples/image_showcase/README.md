# image_showcase（图片与 SVG 展示）

图片、GIF、SVG 相关示例合集，原为 5 个独立 crate（`image`、`image_loading`、
`image_gallery`、`gif_viewer`、`svg`），现合并为一个 crate 下的多个 binary。

| binary | 说明 | 运行 |
|--------|------|------|
| `image` | 本地文件 / 远程 URL / 注册资产三种图片来源，自适应宽高 | `cargo run -p image_showcase --bin image` |
| `image_loading` | 图片加载态（骨架动画）与失败态（占位符），点击重试 | `cargo run -p image_showcase --bin image_loading` |
| `image_gallery` | 图片画廊：手动图片缓存 vs 自动 LRU 图片缓存，内存占用对比 | `cargo run -p image_showcase --bin image_gallery` |
| `gif_viewer` | GIF 动图播放（`object_fit: Contain`） | `cargo run -p image_showcase --bin gif_viewer` |
| `svg` | SVG 矢量图加载与 `text_color` 着色 | `cargo run -p image_showcase --bin svg` |

不带 `--bin` 直接 `cargo run -p image_showcase` 会打印本列表。
