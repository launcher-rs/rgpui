# 对话框（Dialog）

**目录：** [概述](#概述) · [Dialog](#dialog) · [AlertDialog](#alertdialog) · [结构组件](#结构组件) · [DialogButtonProps](#dialogbuttonprops) · [焦点陷阱](#焦点陷阱) · [最佳实践](#最佳实践)

## 概述

对话框子系统位于 `crates/rgpui/src/dialog/`，`mod dialog;` 私有（`rgpui.rs:208`）
但 `pub use dialog::*`（`rgpui.rs:524`）→ 在 crate 根导出（**不在 prelude**）。

文件：`dialog.rs`、`alert_dialog.rs`、`header.rs`、`title.rs`、`description.rs`、
`content.rs`、`footer.rs`、`mod.rs`。
导出（`dialog/mod.rs:11-17`）：`alert_dialog::*`、`DialogContent`、`DialogDescription`、
`dialog::*`、`footer::*`、`DialogHeader`、`DialogTitle`。
故根可得：`Dialog`、`AlertDialog`、`DialogHeader`、`DialogTitle`、`DialogDescription`、
`DialogContent`、`DialogFooter`、`DialogClose`、`DialogAction`、`DialogButtonProps`、
`DialogFooterButton`、`DialogProps`。

## Dialog

`Dialog`（`dialog/dialog.rs:186`），`impl IntoElement`（`RenderOnce` `:423`）。

| 方法 | 位置 | 说明 |
|------|------|------|
| `new(cx: &mut App)` | `:214` | |
| `trigger(impl IntoElement)` | `:234` | 触发元素 |
| `title(impl IntoElement)` | `:249` | |
| `content(builder: impl Fn(&mut Window, &mut App) -> AnyElement)` | `:240` | |
| `footer(impl IntoElement)` | `:263` | |
| `button_props(DialogButtonProps)` | `:269` | 底部按钮 |
| `on_ok(f)` / `on_cancel(f)` / `on_close(f)` | `:284`/`:293`/`:275` | |
| `close_button(bool)` | `:302` | 右上关闭按钮 |
| `margin_top(Pixels)` | `:308` | |
| `w(Pixels)` / `width(Pixels)` | `:314`/`:320` | |
| `max_w(Pixels)` | `:326` | |
| `overlay(bool)` / `overlay_visible(bool)` / `overlay_closable(bool)` | `:332`/`:348`/`:338` | |
| `keyboard(bool)` | `:354` | Esc/Enter 支持 |

```rust
Dialog::new(cx)
    .title("确认删除")
    .content(|_, _| div().child("此操作不可撤销。").into_any_element())
    .button_props(DialogButtonProps::default().ok_text("删除").ok_variant(ButtonVariant::Danger))
    .on_ok(|_, window, cx| { /* ... */ window.remove_window(); })
    .trigger(Button::new("del").danger().label("删除"))
```

## AlertDialog

`AlertDialog`（`dialog/alert_dialog.rs:61`）：`new(cx)`（`:77`）、`confirm()`（`:92`，
预设确认样式）、`trigger`（`:105`）、`content`（`:128`）、`footer`（`:142`）、
`icon`（`:157`）、`title`（`:165`）、`description`（`:173`）、`button_props`（`:195`）、
`width`（`:202`）、`show_cancel(bool)`（`:208`）、`overlay_closable`（`:216`）、
`close_button`（`:222`）、`keyboard`（`:228`）、`on_close`（`:236`）、`on_ok`（`:247`）、
`on_cancel`（`:258`），`impl Styled`/`ParentElement`。

## 结构组件

用于自定义对话框内部布局（均 `new()` + `ParentElement` + `Styled`）：

| 组件 | 位置 | 说明 |
|------|------|------|
| `DialogHeader` | `header.rs:11` | 头部容器 |
| `DialogTitle` | `title.rs:12` | 标题 |
| `DialogDescription` | `description.rs:12` | 描述 |
| `DialogContent` | `content.rs:11` | 内容区 |
| `DialogFooter` | `footer.rs:24` | 底部按钮行 |
| `DialogClose` | `footer.rs:78` | 关闭按钮 |
| `DialogAction` | `footer.rs:109` | 主操作按钮 |
| `DialogFooterButton` | `footer.rs:64` | trait（按钮样式钩子） |

## DialogButtonProps

`DialogButtonProps`（`dialog/dialog.rs:28`），`impl Default`（`:41`）：
`ok_text(SharedString)`（`:58`）、`ok_variant(ButtonVariant)`（`:64`）、
`cancel_text`（`:70`）、`cancel_variant`（`:76`）、`show_cancel(bool)`（`:82`）、
`on_ok`（`:88`）、`on_cancel`（`:97`）。

## 焦点陷阱

`FocusTrapElement`（`system/focus_trap.rs:13`，根导出 `rgpui.rs:406`）为
`InteractiveElement` 提供焦点陷阱：`FocusTrapContainer<E>`（`:112`）。
对话框内部用其把焦点限制在弹层内。

## 最佳实践

- **`Dialog` 用 `new(cx)` + `trigger(..)`**，内容用 `content(builder)`（返回 `AnyElement`）。
- **要确认/取消语义用 `AlertDialog::new(cx).confirm()`** 或设 `DialogButtonProps`。
- **`on_ok`/`on_cancel` 里的关闭**通常调 `window.remove_window()`。
- **自定义复杂布局**用 `DialogHeader`/`DialogTitle`/`DialogContent`/`DialogFooter` 拼装。
- **`keyboard(false)`** 可关闭 Esc 关闭；`overlay_closable(false)` 阻止点遮罩关闭。
