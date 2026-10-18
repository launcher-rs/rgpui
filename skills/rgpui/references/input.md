# 输入（Input）

**目录：** [概述](#概述) · [快速入门](#快速入门) · [InputState](#inputstate) · [Input 组件](#input-组件) · [TextArea](#textarea) · [NumberInput](#numberinput) · [密码与掩码](#密码与掩码) · [InputEvent](#inputevent) · [装饰与其它](#装饰与其它) · [最佳实践](#最佳实践)

## 概述

输入子系统位于 `crates/rgpui/src/input_ui/`，`pub mod input_ui;`（`rgpui.rs:217`）公开。
**状态与组件分离**：`InputState`（实体，持有文本/光标/选区）驱动
`Input` / `TextArea` / `NumberInput`（渲染元素）。经 `pub use input_ui::*`（`rgpui.rs:529`）
在 crate 根导出，如 `rgpui::Input`、`rgpui::InputState`。**不在 prelude**。

初始化：`rgpui::input_ui::init(cx)`（`input_ui/mod.rs:76`），注册全局按键绑定
（`Input` 上下文，见 `input/state.rs:162` 起）。

> **纠偏：** **不存在** `MaskedInput` / `PasswordInput` / `MaskedInputState` /
> `NumberInputState`。密码用 `masked(true)` + `Input::mask_toggle()`；数字输入复用
> `InputState` + `NumberInput`。`InputEvent` **无** `PressEscape`（Escape 是 action，
> `state.rs:117`；方法 `escape()` `:1782`）。

## 快速入门

```rust
use rgpui::{Input, InputState, InputEvent};
use rgpui::prelude::*;

// 1) 在 View::new 创建实体
let state = cx.new(|cx| InputState::new(window, cx).placeholder("请输入名字"));

// 2) 监听事件
cx.subscribe(&state, |this, _, ev: &InputEvent, cx| {
    if let InputEvent::Change = ev { cx.notify(); }
}).detach();

// 3) 渲染
Input::new(&state)
```

示例：`cargo run -p showcase --bin input`（`examples/showcase/src/bin/input.rs`）。

## InputState

`InputState`（`input/input/state.rs:332`，`impl EventEmitter<InputEvent>` `:475`）。
`new(window, cx)`（`:481`，在 `cx.new(|cx| InputState::new(window, cx))` 内调用）。
`impl Focusable`（`:3145`）、`impl Render`（`:3151`）。

**链式配置（构建期）：**

| 类别 | 方法 | 位置 |
|------|------|------|
| 文本 | `default_value(impl Into<SharedString>)` | `:1241` |
| 文本 | `placeholder(impl Into<SharedString>)` | `:617` |
| 布局 | `multi_line(bool)` | `:605` |
| 布局 | `rows(usize)` | `:677` |
| 布局 | `auto_grow(min_rows, max_rows)` | `:611` |
| 布局 | `soft_wrap(bool)` / `wrapping_indent(WrappingIndent)` | `:1012` / `:1025` |
| 显示 | `line_number(bool)` / `folding(bool)` | `:655` / `:625` |
| 显示 | `show_whitespaces(bool)` | `:1019` |
| 显示 | `read_only(bool)` | `:728` |
| 编辑 | `masked(bool)` | `:980` |
| 编辑 | `clean_on_escape()` | `:998` |
| 编辑 | `submit_on_enter(bool)` | `:1006` |
| 编辑 | `auto_close_pairs(bool)` / `bracket_match(bool)` | `:758` / `:772` |
| 编辑 | `bracket_rainbow(bool)` / `current_line_highlight(bool)` | `:791` / `:810` |
| 编辑 | `line_comment_prefix(impl Into<SharedString>)` | `:707` |
| 校验 | `pattern(Regex)` | `:1125` |
| 校验 | `validate(impl Fn(&str, &mut Context<Self>) -> bool + 'static)` | `:1142` |
| 数字 | `step(impl Into<NumberStep>)` / `step_by(..)` / `min(f64)` / `max(f64)` | `:1157`/`:1172`/`:1187`/`:1199` |
| 掩码 | `mask_pattern(impl Into<MaskPattern>)` | `:2673` |
| 滚动 | `scroll_beyond_last_line(Option<usize>)` / `cursor_surrounding_lines(Option<usize>)` | `:1078` / `:1103` |

**运行时方法（需 `&mut Window`/`Context`）：**

`set_value(value, window, cx)`（`:860`）、`insert(text,cx)`（`:904`）、
`replace(range, text, cx)`（`:923`）、`replace_all`（`:887`）、`set_placeholder`（`:693`）、
`set_masked`（`:990`）、`set_read_only`（`:734`）、`set_line_number`（`:664`）、
`set_folding`（`:636`）、`set_auto_close_pairs`（`:765`）、`set_bracket_match_enabled`（`:779`）、
`set_current_line_highlight`（`:817`）、`set_pattern`（`:1134`）、`set_min`/`set_max`（`:1220`/`:1226`）、
`set_mask_pattern`（`:2685`）、`focus(window,cx)`（`:1294`）、`refresh(cx)`（`:1302`）、
`unselect(window,cx)`（`:2413`）、`set_selected_range(range,cx)`（`:2280`）。

**查询：** `value()`（`:1251`）、`selected_value()`（`:1256`）、`unmask_value()`（`:1261`）、
`text() -> &Rope`（`:1266`）、`cursor()`（`:2237`）、`cursor_position()`（`:1271`）、
`selected_range()`（`:2272`）、`is_read_only()`（`:740`）、`is_focused(window)`（`:2501`）、
`visible_row_range()`（`:2242`）、`scroll_offset()`（`:2247`）、`line_height()`（`:2264`）、
`range_to_bounds(&Range<usize>)`（`:2729`）。

**编辑器相关透传：** `reveal_offset(offset,cx)`（`:2047`）、`reveal_range(range,cx)`（`:2054`）、
`document_symbols()`（`:2063`）、`goto_symbol(...)`（`:2072`）、`set_highlighter(...)`（`:2087`）。

## Input 组件

`Input`（`input/input/widget.rs:33`），`impl Sizable`/`Selectable`，`impl RenderOnce`。

| 方法 | 位置 | 说明 |
|------|------|------|
| `new(&Entity<InputState>)` | `:77` | |
| `aria_label(impl Into<SharedString>)` | `:121` | |
| `prefix(impl IntoElement)` / `suffix(...)` | `:127` / `:133` | 前后缀 |
| `h_full()` / `h(impl Into<DefiniteLength>)` | `:139` / `:145` | 高度 |
| `appearance(bool)` | `:151` | 背景外观 |
| `bordered(bool)` / `focus_bordered(bool)` | `:157` / `:163` | 边框 |
| `cleanable(bool)` | `:169` | 清除按钮 |
| `mask_toggle()` | `:175` | 密码显隐切换 |
| `content_type(InputContentType)` | `:185` | 输入类型提示 |
| `role(Role)` / `disabled(bool)` / `read_only(bool)` | `:193`/`:199`/`:208` | |
| `tab_index(isize)` | `:214` | |
| `show_context_menu(bool)` | `:223` | 右键菜单 |
| `context_menu_extra(builder)` / `context_menu_override(builder)` | `:232` / `:251` | 扩展菜单 |

## TextArea

`TextArea`（`input/input/textarea.rs:14`）：`new(&Entity<InputState>)`（`:24`）、
`rows(usize)`（`:35`）、`read_only(bool)`（`:41`），`impl Disableable`。
配合 `InputState::multi_line(true)` 使用。

## NumberInput

`NumberInput`（`input/input/number_input.rs:27`）：`new(&Entity<InputState>)`（`:40`）、
`placeholder`（`:54`）、`prefix`/`suffix`（`:60`/`:66`）、`appearance`（`:72`），
`impl Disableable`/`Sizable`/`Focusable`。
事件 `NumberInputEvent`（`impl EventEmitter for InputState` `:251`）。
`NumberStep`（`:156`）：`ByValue(...)` / `Percent(f64)`，`From<f64>`；`by_value(...)`（`:164`）。
`InputState` 数字方法见上表。

## 密码与掩码

- **密码**：`InputState::new(...).masked(true)` + 渲染 `Input::new(&state).mask_toggle()`。
- **掩码格式**：`MaskPattern::new("(###) ###-####")`（`mask_pattern.rs:119`）、
  `MaskPattern::number(sep)`（`:138`）；`InputState::mask_pattern(pattern)`。
  方法 `is_valid`（`:168`）、`is_valid_at`（`:242`）、`mask(&str)`（`:278`）、
  `unmask(&str)`（`:369`）、`mask()` 值用 `InputState::unmask_value()` 获取。

## InputEvent

`InputEvent`（`input/input/state.rs:142`）：

```rust
pub enum InputEvent {
    Change,                                        // 文本变化
    PressEnter { secondary: bool, shift: bool },   // 回车
    Focus,
    Blur,
}
```

> Escape 是 action（`state.rs:117` `Escape`，绑定见 init），非事件。

## 装饰与其它

- `TextDecoration` / `TextDecorationCollection`（`input_ui/decorations.rs`，
  经 `input_ui/mod.rs:41` 导出）。
- `InputContentType`（`:69`）、`TabSize`（`:52`，`input_ui/indent.rs`）。
- `AsyncFileLoader` / `FileLoadResult` / `LargeFileConfig` / `LoadProgress`（`:38`）。
- `InputContextMenuBuilder`（`:39`）。
- 根导出：`pub use input_ui::*`（`rgpui.rs:529`），故 `rgpui::Input` 等可用。

## 最佳实践

- **状态用实体**：`cx.new(|cx| InputState::new(window, cx)...)`，组件只持有 `&Entity`。
- **监听 `InputEvent::Change`** 后 `cx.notify()` 重绘。
- **多行用 `TextArea` + `multi_line(true)`**；单行数字用 `NumberInput` + `step/min/max`。
- **密码不要找 `PasswordInput`**：`masked(true)` + `Input::mask_toggle()` 即可。
- **校验用 `validate(...)`**，返回值驱动内部校验态。
