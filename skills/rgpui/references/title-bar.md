# 标题栏（TitleBar）

**目录：** [概述](#概述) · [快速入门](#快速入门) · [TitleBar](#titlebar) · [常量](#常量) · [WindowBorder](#windowborder) · [最佳实践](#最佳实践)

## 概述

标题栏子系统位于 `crates/rgpui/src/title_bar/`，`pub mod title_bar;`（`rgpui.rs:240`）。
文件：`mod.rs`、`title_bar.rs`、`window_border.rs`。
导出（`title_bar/mod.rs:6-7`）：`title_bar::*`、`WindowBorder`/`window_border`/`window_paddings`。

> **重要：`TitleBar` 不在 crate 根、也不在 prelude** —— 必须
> `use rgpui::title_bar::TitleBar;`（`rgpui::TitleBar` 不存在）。

## 快速入门

```rust
use rgpui::title_bar::{TitleBar, TITLE_BAR_HEIGHT};
use rgpui::prelude::*;

// 窗口选项（无系统装饰、自绘标题栏）
cx.open_window(
    rgpui::WindowOptions {
        titlebar: Some(TitleBar::title_bar_options()),
        ..TitleBar::window_options()
    },
    |window, cx| {
        cx.new(|cx| {
            v_flex()
                .size_full()
                .child(TitleBar::new().child("我的应用"))
                .child(/* 内容 */)
        })
    },
)
```

示例：`cargo run -p showcase --bin title_bar`（若存在）/ `hello_world` 等基础示例。

## TitleBar

`TitleBar`（`title_bar/title_bar.rs:28`），`impl IntoElement`（`RenderOnce` `:303`），
`impl Styled`（`:286`）/`ParentElement`（`:292`）。

| 方法 | 签名 | 位置 |
|------|------|------|
| `new` | `fn new() -> Self` | `:36` |
| `title_bar_options` | `fn title_bar_options() -> TitlebarOptions` | `:45` |
| `window_options` | `fn window_options() -> WindowOptions` | `:66` |
| `on_close_window` | `fn on_close_window(handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self` | `:88` |

`TitleBar::new()` 支持 `ParentElement::child`（`:292`）挂子元素（标题、按钮等）。
`ControlIcon`（`:113`）与 `WindowControls`（`:257`）为内部窗口控制按钮。

## 常量

| 常量 | 值 | 位置 |
|------|----|------|
| `TITLE_BAR_HEIGHT` | `px(34.)` | `title_bar/title_bar.rs:16`（`theme/settings.rs:6` 也有一份，经根导出） |
| `TITLE_BAR_LEFT_PADDING` | macOS `px(80.)` / 其它 `px(12.)` | `title_bar/title_bar.rs:19` / `:22` |

## WindowBorder

`WindowBorder`（`title_bar/window_border.rs:34`，自由函数 `window_border()` `:28`）：
`new()`（`:52`）、`shadow_size(Pixels)`（`:59`）、`resize_hit_size(Pixels)`（`:67`）。
`window_paddings(window) -> Edges<Pixels>`（`:92`）——自绘窗口内容内边距。

```rust
use rgpui::title_bar::{window_border, window_paddings};
div().size_full().child(window_border()).child(/* ... */)
```

## 最佳实践

- **必须 `use rgpui::title_bar::TitleBar;`**（不在根、不在 prelude）。
- **自绘标题栏**：`WindowOptions { titlebar: Some(TitleBar::title_bar_options()), ..TitleBar::window_options() }`。
- **`TITLE_BAR_HEIGHT` 两处定义**（title_bar 与 theme/settings），按用途选择；根导出的是
  `theme/settings.rs` 那份。
- **窗口阴影/缩放热区**用 `window_border()` + `window_paddings(window)` 包裹内容。
