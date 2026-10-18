# 表单（Form）

**目录：** [概述](#概述) · [快速入门](#快速入门) · [Form](#form) · [Field](#field) · [FieldBuilder](#fieldbuilder) · [网格布局](#网格布局) · [最佳实践](#最佳实践)

## 概述

表单子系统提供 `Form`（字段容器 / 网格）与 `Field`（单个字段：标签 + 描述 + 控件）。
定义在 `crates/rgpui/src/form/`，`mod form;` 私有（`rgpui.rs:214`）但经
`pub use form::*`（`rgpui.rs:526`）在 crate 根导出：`rgpui::{Form, Field, FieldBuilder}`。
**不在 prelude**，需显式 import。

> **纠偏：** 文档旧称 label 默认宽 100px，实际为 `Some(px(140.))`
> （`form/field.rs:28`）。`Field` **没有 error API**。

## 快速入门

```rust
use rgpui::{Form, Field, Input, Switch};
use rgpui::prelude::*;

v_form()
    .child(Field::new().label("用户名").child(
        Input::new(&name_state).placeholder("请输入"),
    ))
    .child(Field::new().label("启用").child(
        Switch::new("enabled").checked(true),
    ))
```

示例：`cargo run -p showcase --bin form`（`examples/showcase/src/bin/form.rs`）。

## Form

`Form`（`form/form.rs:13`），`impl IntoElement`。

| 方法 | 签名 | 位置 |
|------|------|------|
| `horizontal` | `fn horizontal() -> Self` | `:29` |
| `vertical` | `fn vertical() -> Self` | `:34` |
| `layout` | `fn layout(mut self, Axis) -> Self` | `:39` |
| `label_width` | `fn label_width(mut self, Pixels) -> Self` | `:45` |
| `label_text_size` | `fn label_text_size(mut self, Rems) -> Self` | `:51` |
| `child` | `fn child(mut self, impl Into<Field>) -> Self` | `:57` |
| `children` | `fn children(mut self, impl IntoIterator<Item = Field>) -> Self` | `:63` |
| `columns` | `fn columns(mut self, usize) -> Self` | `:71` |

自由函数（`form/mod.rs`）：`v_form() -> Form`（`:8`，= `Form::vertical()`）、
`h_form() -> Form`（`:13`，= `Form::horizontal()`）、`field() -> Field`（`:18`，= `Field::new()`）。

`impl Sizable`（`form.rs:83`）设置整体尺寸；`impl Styled`。`columns(>1)` 触发网格布局。

## Field

`Field`（`form/field.rs:86`），`impl IntoElement` + `ParentElement` + `Styled`。

| 方法 | 签名 | 位置 |
|------|------|------|
| `new` | `fn new() -> Self` | `:106` |
| `label` | `fn label(impl Into<FieldBuilder>)` | `:125` |
| `label_indent` | `fn label_indent(bool)`（水平布局缩进，默认 `true`） | `:135` |
| `label_fn` | `fn label_fn<F, E>(F) where F: Fn(&mut Window, &mut App) -> E` | `:141` |
| `description` | `fn description(impl Into<FieldBuilder>)` | `:153` |
| `description_fn` | `fn description_fn<F, E>(F)` | `:159` |
| `visible` | `fn visible(bool)` | `:171` |
| `required` | `fn required(bool)`（显示必填标记） | `:177` |
| `items_start`/`items_end`/`items_center` | 对齐方式 | `:192`/`:198`/`:204` |
| `col_span` | `fn col_span(u16)` | `:212` |
| `col_start`/`col_end` | `fn col_start(i16)` / `fn col_end(i16)` | `:218`/`:224` |

字段控件用 `ParentElement::child`（`field.rs:230`）挂入，如
`Field::new().label("昵称").child(Input::new(&state))`。

`impl Into<Field>` 对 `AnyElement` / `&'static str` / `String` 已实现（见 field.rs 相关 impl），
便于直接把简单元素当字段使用。

## FieldBuilder

`FieldBuilder`（`form/field.rs:35`）用于 label / description：

```rust
pub enum FieldBuilder {
    String(SharedString),                                // 纯文本
    Element(Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>),  // 渲染函数
    View(AnyView),                                       // 已构建视图
}
```

`From`：`&'static str`（`:66`）、`String`（`:72`）、`SharedString`（`:78`）、`AnyView`（`:50`）。

## 网格布局

`Form::columns(n)`（`form.rs:71`）开启网格；每个 `Field` 通过 `col_span` 跨列、
`col_start`/`col_end` 定位。`horizontal()` 布局下 label 占 `label_width`
（默认 140px），字段在右；`vertical()` 则 label 在上。

```rust
Form::horizontal()
    .columns(2)
    .label_width(rgpui::px(90.))
    .child(Field::new().label("姓").child(Input::new(&first)))
    .child(Field::new().label("名").child(Input::new(&last)))
    .child(Field::new().label("地址").col_span(2).child(Input::new(&addr)))
```

## 最佳实践

- **用 `v_form()` / `h_form()` 简写**，等价 `Form::vertical()` / `Form::horizontal()`。
- **label 默认宽 140px**（非 100px），用 `label_width(px(..))` 覆盖。
- **没有内建校验/错误显示**：校验逻辑自行实现，错误用 `Alert` 或自定义元素展示。
- **`Field` 显式 `.child(控件)`**，可挂 `Input`/`Switch`/`RadioGroup` 等任意元素。
- **需显式 import `Form`/`Field`**（不在 prelude）。
