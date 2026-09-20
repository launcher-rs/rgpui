# v1_3_showcase

1.3 新 API 集中演示（二进制）：

- 静态 `Tabs`（值驱动 + `on_change(SharedString)` 按值 + `listener_value`）
- `DialogId`（开两框，按标识关下层，上层不受影响）
- 树 `TreeEvent::{Selected, Confirmed}`（点击选中 / 回车确认）
- `i18n`（全局管理器 + `I18nText::translate_global` + 语言切换）
- C1 新签名（`Checkbox(bool)` / `Select(usize, SharedString)` 按值）

运行：

```text
cargo run -p v1_3_showcase
```
