# overlay_showcase（浮层定位展示）

浮层相关示例合集，合并为一个 binary，用 `-- <demo>` 选择具体演示。两者都演示
`anchored` + `deferred` 浮层机制。

| demo | 说明 | 运行 |
|------|------|------|
| `popover` | `deferred` 创建浮动层：主/次两级 popover 开关 | `cargo run -p overlay_showcase -- popover` |
| `anchor` | 9 种锚点（`Anchor` 上/中/下 × 左/中/右）定位演示 | `cargo run -p overlay_showcase -- anchor` |

不带参数默认运行 `popover`。
