# 布局与样式

**目录：** [概述](#概述) · [快速入门](#快速入门) · [常见模式](#常见模式) · [样式方法](#样式方法) · [h_flex / v_flex](#h_flex--v_flex-辅助函数) · [Tailwind 简写](#tailwind-样式简写) · [Overflow 与滚动](#overflow-与滚动) · [绝对定位](#绝对定位) · [层叠顺序](#层叠顺序) · [主题集成](#主题集成) · [条件样式](#条件样式) · [文本样式](#文本样式)

## 概述

rgpui 提供类似 CSS 的样式系统，具有 Rust 类型安全。

**核心概念：**

- Flexbox 布局系统
- Styled trait 用于链式样式
- 尺寸单位：`px()`、`rems()`、`relative()`
- 颜色、边框、阴影

## 快速入门

### 基础样式

```rust
use rgpui::*;

div()
    .w(px(200.))
    .h(px(100.))
    .bg(rgb(0x2196F3))
    .text_color(rgb(0xFFFFFF))
    .rounded(px(8.))
    .p(px(16.))
    .child("带样式的コンテンツ")
```

### Flexbox 布局

```rust
div()
    .flex()
    .flex_row()  // 或 flex_col() 用于列
    .gap(px(8.))
    .items_center()
    .justify_between()
    .children([
        div().child("项目 1"),
        div().child("项目 2"),
        div().child("项目 3"),
    ])
```

### 尺寸单位

```rust
div()
    .w(px(200.))           // 像素
    .h(rems(10.))          // 相对于字体大小
    .w(relative(0.5))      // 父级的 50%
    .min_w(px(100.))
    .max_w(px(400.))
```

## 常见模式

### 居中内容

```rust
div()
    .flex()
    .items_center()
    .justify_center()
    .size_full()
    .child("居中")
```

### 卡片布局

```rust
div()
    .w(px(300.))
    .bg(cx.theme().surface)
    .rounded(px(8.))
    .shadow_md()
    .p(px(16.))
    .gap(px(12.))
    .flex()
    .flex_col()
    .child(heading())
    .child(content())
```

### 响应式间距

```rust
div()
    .p(px(16.))           // 所有方向内边距
    .px(px(20.))          // 水平内边距
    .py(px(12.))          // 垂直内边距
    .pt(px(8.))           // 顶部内边距
    .gap(px(8.))          // 子元素间距
```

## 样式方法

### 尺寸

```rust
.w(px(200.))              // 宽度
.h(px(100.))              // 高度
.size(px(200.))           // 宽度和高度
.min_w(px(100.))          // 最小宽度
.max_w(px(400.))          // 最大宽度
```

### 颜色

```rust
.bg(rgb(0x2196F3))        // 背景
.text_color(rgb(0xFFFFFF)) // 文本颜色
.border_color(rgb(0x000000)) // 边框颜色
```

### 边框

```rust
.border(px(1.))           // 边框宽度
.rounded(px(8.))          // 圆角
.rounded_t(px(8.))        // 顶部圆角
.border_color(rgb(0x000000))
```

### 间距

```rust
.p(px(16.))               // 内边距
.m(px(8.))                // 外边距
.gap(px(8.))              // flex 子元素间距
```

### Flexbox

```rust
.flex()                   // 启用 flexbox
.flex_row()               // 行方向
.flex_col()               // 列方向
.items_center()           // 项目居中对齐
.justify_between()        // 项目之间分配空间
.flex_grow_1()              // 填充剩余空间
```

## h_flex / v_flex 辅助函数

rgpui-component 提供简写辅助函数（从 `rgpui_component` 导入）：

```rust
use rgpui_component::{h_flex, v_flex};

// h_flex() = div().flex().flex_row().items_center()
h_flex()
    .gap_2()
    .child(icon)
    .child(label)

// v_flex() = div().flex().flex_col()
v_flex()
    .gap_4()
    .p_4()
    .child(input1)
    .child(input2)
    .child(submit_btn)
```

这些是 rgpui-component 中的标准布局原语 — 优先使用它们而不是原始的 `div().flex()`。

## Tailwind 风格简写

rgpui 提供 Tailwind 风格的间距/尺寸简写：

```rust
// 间距（0=0, 1=4px, 2=8px, 3=12px, 4=16px, ...）
.p_2()    // padding: 8px
.px_4()   // padding x: 16px
.py_3()   // padding y: 12px
.m_2()    // margin: 8px
.gap_3()  // gap: 12px

// 尺寸
.size_full()   // width: 100%, height: 100%
.size_4()      // width: 16px, height: 16px
.w_full()      // width: 100%
.h_full()      // height: 100%
.flex_1()      // flex: 1 1 0（填充剩余空间）
.flex_shrink_0() // 防止收缩
```

## Overflow 与滚动

```rust
div()
    .overflow_hidden()          // 裁剪内容
    .overflow_x_hidden()        // 裁剪水平
    .overflow_y_scrollbar()     // y 轴显示滚动条
    .overflow_scroll()          // 两个轴都滚动
```

## 绝对定位

```rust
div()
    .relative()                 // position: relative（容器）
    .child(
        div()
            .absolute()         // position: absolute
            .top_0()
            .right_0()
            .child("徽章")
    )

// inset 辅助函数
div().absolute().inset_0()      // top/right/bottom/left: 0（填充父级）
div().absolute().top(px(8.)).left(px(8.))
```

## 层叠顺序

```rust
div()
    .relative()
    .child(content)
    .child(
        div()
            .absolute()
            .top_0()
            .right_0()
            .child("徽章")
    ) // 后面的子元素通常绘制在前面的兄弟元素之上
```

rgpui 的通用 `Styled` API **不**提供 `z_index(...)` 方法。

对于普通元素，层叠通常由以下方式控制：

- 父/子组合
- 绝对定位
- 兄弟元素的渲染顺序（后面的兄弟绘制在前面的之上）

如果在此仓库中看到 `z_index(...)` 方法，确保它属于你使用的特定组件。例如，停靠磁贴系统中的 `TileItem::z_index(...)` 是自定义组件 API，而不是通用 rgpui `Div` 样式方法。

## 主题集成

```rust
div()
    .bg(cx.theme().surface)
    .text_color(cx.theme().foreground)
    .border_color(cx.theme().border)
    .when(is_hovered, |el| {
        el.bg(cx.theme().hover)
    })
```

## 条件样式

```rust
use rgpui::prelude::FluentBuilder as _;

div()
    .when(is_active, |el| el.bg(cx.theme().primary))
    .when(!is_active, |el| el.opacity(0.5))
    .when_some(optional_color.as_ref(), |el, color| el.bg(*color))
```

## 文本样式

```rust
div()
    .text_sm()          // font-size: 小
    .text_base()        // font-size: 基础
    .text_lg()          // font-size: 大
    .font_bold()        // font-weight: 粗体
    .line_height_snug() // 更紧凑的行高
    .truncate()         // overflow: ellipsis, 单行
    .whitespace_nowrap()
```
