# image_showcase（图片与 SVG 展示）

图片、GIF、SVG 相关示例合集。`image` 保持独立 binary，其余合并进默认 binary，
用 `-- <demo>` 选择。

| binary / demo | 说明 | 运行 |
|---------------|------|------|
| `image` | 本地文件 / 远程 URL / 注册资产三种图片来源，自适应宽高 | `cargo run -p image_showcase --bin image` |
| `image_loading` | 图片加载态（骨架动画）与失败态（占位符），点击重试 | `cargo run -p image_showcase -- image_loading` |
| `image_gallery` | 图片画廊：手动图片缓存 vs 自动 LRU 图片缓存，内存占用对比 | `cargo run -p image_showcase -- image_gallery` |
| `gif_viewer` | GIF 动图播放（`object_fit: Contain`） | `cargo run -p image_showcase -- gif_viewer` |
| `svg` | SVG 矢量图加载与 `text_color` 着色 | `cargo run -p image_showcase -- svg` |

不带参数默认运行 `image_loading`。
