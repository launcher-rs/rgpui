# overlay_showcase（浮层定位展示）

浮层相关示例合集，原为 2 个独立 crate（`popover`、`anchor`），现合并为一个
crate 下的多个 binary。两者都演示 `anchored` + `deferred` 浮层机制。

| binary | 说明 | 运行 |
|--------|------|------|
| `popover` | `deferred` 创建浮动层：主/次两级 popover 开关 | `cargo run -p overlay_showcase --bin popover` |
| `anchor` | 9 种锚点（`Anchor` 上/中/下 × 左/中/右）定位演示 | `cargo run -p overlay_showcase --bin anchor` |

不带 `--bin` 直接 `cargo run -p overlay_showcase` 会打印本列表。
