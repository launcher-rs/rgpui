# 滚动（Scroll）

**目录：** [概述](#概述) · [快速入门](#快速入门) · [两种滚动入口](#两种滚动入口) · [ScrollableElement](#scrollableelement) · [Scrollbar](#scrollbar) · [ScrollHandle](#scrollhandle) · [虚拟列表](#虚拟列表) · [越界回弹](#越界回弹) · [最佳实践](#最佳实践)

## 概述

滚动子系统提供：滚动容器（`Scrollable` / `Scrollbar`）、可编程滚动句柄
（`ScrollHandle` / `UniformListScrollHandle` / `VirtualListScrollHandle`）、
虚拟列表（`UniformList` / `VirtualList`）与越界回弹（`Overscroll`）。

**导出：** `elements/scroll/mod.rs` 经 `pub use elements::*`（`rgpui.rs:391`）
在 crate 根导出（`rgpui.rs`）。**滚动类型都不在 prelude**，需显式
`use rgpui::ScrollableElement;` 等（`prelude.rs` 不 glob elements）。

**关键陷阱：** `overflow_scroll` / `track_scroll` 属于 `StatefulInteractiveElement`，
**必须先 `.id()`**；而 `overflow_y_scrollbar` 属于 `ScrollableElement`，
`div()` 直接可用。

## 快速入门

```rust
use rgpui::prelude::*;                    // InteractiveElement / StatefulInteractiveElement
use rgpui::{ScrollableElement, Scrollbar, ScrollHandle, div, px, v_flex};

// 变体 A：自动创建滚动句柄 + 滚动条（Div 直接可用，无需 .id()）
fn auto() -> impl IntoElement {
    div()
        .w(px(300.)).h(px(200.))
        .overflow_y_scrollbar()            // ScrollableElement
        .overscroll(true)                  // 可选：越界回弹
        .child(v_flex().children((0..50).map(|i| {
            div().h(px(20.)).flex_shrink_0().child(format!("第 {i} 行"))
        })))
}

// 变体 B：手动 ScrollHandle（可编程滚动）+ 自置滚动条
fn manual(handle: &ScrollHandle) -> impl IntoElement {
    div()
        .id("my-scroll-area")              // 必须先 .id() → Stateful<Div>
        .relative()
        .w(px(300.)).h(px(200.))
        .overflow_y_scroll()               // StatefulInteractiveElement
        .track_scroll(handle)
        .child(v_flex().children(/* ... */))
        .child(Scrollbar::vertical(handle))
}
```

示例：运行 `cargo run -p showcase --bin scrollable`（`examples/showcase/src/bin/scrollable.rs`）。

## 两种滚动入口

| 方法 | 所属 trait | 是否需要 `.id()` | 说明 |
|------|-----------|-----------------|------|
| `overflow_scroll` | `StatefulInteractiveElement`（`div.rs:1389`） | 是 | x、y 双向滚动 |
| `overflow_x_scroll` | `StatefulInteractiveElement`（`:1396`） | 是 | 水平 |
| `overflow_y_scroll` | `StatefulInteractiveElement`（`:1402`） | 是 | 垂直 |
| `track_scroll(&ScrollHandle)` | `StatefulInteractiveElement`（`:1416`） | 是 | 绑定句柄 |
| `restrict_scroll_to_axis` | `StatefulInteractiveElement`（`:1410`） | 是 | 轴限制 |
| `anchor_scroll` | `StatefulInteractiveElement`（`:1422`） | 是 | 滚动锚点 |
| `overflow_scrollbar` | `ScrollableElement`（`scrollable.rs:47`） | **否** | 双向滚动 + 自动滚动条 |
| `overflow_x_scrollbar` | `ScrollableElement`（`:54`） | **否** | 水平 + 滚动条 |
| `overflow_y_scrollbar` | `ScrollableElement`（`:61`） | **否** | 垂直 + 滚动条 |
| `scrollbar(&handle, axis)` | `ScrollableElement`（`:20`） | **否** | 叠加指定滚动条 |
| `on_scroll_wheel(..)` | `InteractiveElement`（`div.rs:956`） | 否 | 滚轮事件（冒泡） |

## ScrollableElement

`pub trait ScrollableElement: InteractiveElement + Styled + ParentElement + Element`
（`scroll/scrollable.rs:17`）。impl：`Div`（`:187`）与 `Stateful<E>`（`:188`）。

| 方法 | 签名 | 位置 |
|------|------|------|
| `scrollbar` | `#[track_caller] fn scrollbar<H: ScrollbarHandle + Clone>(self, &H, axis: impl Into<ScrollbarAxis>) -> Self` | `:20` |
| `vertical_scrollbar` | `#[track_caller] fn vertical_scrollbar<H>(self, &H) -> Self` | `:34` |
| `horizontal_scrollbar` | `#[track_caller] fn horizontal_scrollbar<H>(self, &H) -> Self` | `:40` |
| `overflow_scrollbar` | `#[track_caller] fn overflow_scrollbar(self) -> Scrollable<Self>` | `:47` |
| `overflow_x_scrollbar` | `#[track_caller] fn overflow_x_scrollbar(self) -> Scrollable<Self>` | `:54` |
| `overflow_y_scrollbar` | `#[track_caller] fn overflow_y_scrollbar(self) -> Scrollable<Self>` | `:61` |

`Scrollable<E>`（`:68`）由上述方法返回，其唯一公开构建器是
`overscroll(bool)`（`:91`）。

> 全部方法标 `#[track_caller]`，默认 id 取调用点；同一调用点创建多个滚动区会撞 id。

## Scrollbar

`ScrollbarAxis`（`scroll/scrollbar.rs:221`）：`Vertical` / `Horizontal` / `Both`，
`impl From<Axis>`。

`Scrollbar`（`:282`）：

| 方法 | 签名 | 位置 |
|------|------|------|
| `new` | `#[track_caller] fn new<H: ScrollbarHandle + Clone>(&H) -> Self` | `:297` |
| `horizontal` / `vertical` | `#[track_caller] fn horizontal/vertical<H>(&H) -> Self` | `:311` / `:317` |
| `id` | `fn id(self, impl Into<ElementId>) -> Self` | `:324` |
| `scrollbar_show` | `fn scrollbar_show(self, theme::ScrollbarShow) -> Self` | `:330` |
| `scroll_size` | `fn scroll_size(self, Size<Pixels>) -> Self` | `:338` |
| `axis` | `fn axis(self, impl Into<ScrollbarAxis>) -> Self` | `:344` |
| `max_fps` | `fn max_fps(self, usize) -> Self`（clamp 30..120） | `:354` |
| `width` | `const fn width() -> Pixels` | `:360` |

显示模式 `ScrollbarShow`（`theme/settings.rs:11`）：`Scrolling`(默认) / `Hover` / `Always`。

## ScrollbarHandle

`pub trait ScrollbarHandle: 'static`（`scroll/scrollbar.rs:31`）：
`offset() -> Point<Pixels>`、`set_offset(Point<Pixels>)`、
`content_size() -> Size<Pixels>`、`start_drag()`、`end_drag()`。

impl：`ScrollHandle`（`:44`）、`UniformListScrollHandle`（`:58`）、
`ListState`（`:73`，旧式列表）、`VirtualListScrollHandle`（`elements/virtual_list.rs:58`）。

## ScrollHandle

`crates/rgpui/src/elements/div.rs:4072`，`Clone`。

| 方法 | 位置 | 说明 |
|------|------|------|
| `new()` | `:4082` | |
| `offset()` / `max_offset()` | `:4087` / `:4097` | 像素偏移 |
| `top_item()` / `bottom_item()` | `:4102` / `:4121` | 当前可见项 |
| `bounds()` / `bounds_for_item(ix)` | `:4140` / `:4145` | |
| `scroll_to_item(ix)` / `scroll_to_top_of_item(ix)` | `:4150` / `:4161` | |
| `scroll_to_bottom()` | `:4220` | |
| `set_offset(Point<Pixels>)` | `:4228` | |
| `logical_scroll_top()` / `logical_scroll_bottom()` | `:4236` / `:4251` | `(usize, Pixels)` |
| `children_count()` | `:4266` | |

## 虚拟列表

三个互补方案：

| 类型 | 构造 | 适用 |
|------|------|------|
| `UniformList` | `uniform_list(id, item_count, f)`（`uniform_list.rs:21`） | 等高行；内部已设 y 滚动 |
| `VirtualList` | `v_virtual_list(view, id, item_sizes: Rc<Vec<Size<Pixels>>>, f)` / `h_virtual_list(...)`（`virtual_list.rs:132`/`:152`） | 高度不一、需按需测量 |
| `List` / `ListState`（旧式） | `list(ListState, render_item)`（`elements/list.rs:24`） | 自动测量、跟随尾部 |

`UniformList` 构建器（`uniform_list.rs`）：`with_width_from_item`（`:626`）、
`with_sizing_behavior`（`:632`）、`with_horizontal_sizing_behavior`（`:640`）、
`with_decoration`（`:657`）、`track_scroll(&UniformListScrollHandle)`（`:687`）、
`y_flipped`（`:694`）。

`UniformListScrollHandle`（`:79`）：`new`、`scroll_to_item(ix, ScrollStrategy)`、
`scroll_to_item_strict`、`scroll_to_bottom`、`is_scrollable`、`is_scrolled_to_end`。
`ScrollStrategy`（`:83`）：`Top` / `Center` / `Bottom` / `Nearest`。

`VirtualList` 构建器（`virtual_list.rs`）：`track_scroll`（`:228`）、
`with_sizing_behavior`（`:235`）、`with_item_to_measure_index`（`:241`）、
`with_scroll_handle`（`:249`）。句柄 `VirtualListScrollHandle`（`:39`）：
`new`、`base_handle`、`scroll_to_item`、`scroll_to_bottom`。

### 长列表：虚拟化是默认选择

> 实测（2026-10-30，Windows，release + LTO，1000 行 × 约 80 字符，900×600 窗口）：
> 逐行 `div` 每帧全量刷新 **46.3 ms/帧（22.3 FPS）**；`uniform_list` 同负载
> **2.8 ms/帧**，内存低约 13 MB。行数越多差距越大，超过 100 行不要逐行拼 `div`。

```rust
// 反例：1000 个 div 全部参与布局与绘制，每帧刷新时整表重排
div().flex().flex_col().children((0..1000).map(row))

// 正例：等高行交给 uniform_list，只渲染可见区间
uniform_list(
    "entries",
    1000,
    cx.processor(|_this, range, _window, _cx| range.map(row).collect::<Vec<_>>()),
)
.h_full() // 必须：撑满有确定高度的父容器
```

**`.h_full()` 坑**：`uniform_list` 依赖父容器给出确定高度，列表自身必须
`.h_full()` 或 `flex_grow()`。若父容器是 `div().size_full()` 而列表没有高度约束，
整块列表不渲染（本次实测踩到，无报错）。参考实现：
`examples/list_showcase/src/bin/uniform_list.rs:36`。

不等高行用 `v_virtual_list`，它需要先提供每项尺寸（`Rc<Vec<Size<Pixels>>>`）；
**该路径没有与本条同口径的实测数据**，不要假设与 `uniform_list` 一样快。

## 越界回弹

`Overscroll<E>`（`scroll/overscroll.rs:176`）：`new(child, &ScrollHandle, axis)`
（`:186`，`#[track_caller]`）。通常不必直接用 —— `Scrollable::overscroll(true)`
（`scrollable.rs:91`）即自动包裹；它遵循 `cx.reduce_motion()`。

```rust
div().overflow_y_scrollbar().overscroll(true)   // 拖到边界有回弹
div().flex().overflow_scrollbar().overscroll(true)  // Both 轴两向都回弹
```

- **`Both` 轴可用**（1.6.2 修复）：`Scrollable` 的内容层此前被行向 flex 容器的
  默认 `align-items: stretch` 压成视口高度，`max_offset.y = 0` → 纵向既不滚动也不
  回弹；现在是 `self_start()` + `min_size_full()`，交叉轴尺寸由内容驱动、不满一屏
  仍撑满。自搭 `Both` 滚动区若把内容写成固定 `h_full()`，同样会丢掉纵向溢出。
- **到边后首个滚轮不被吞**（1.6.2 修复）：`last_offset` 随帧同步（不只滚轮路径），
  滚条拖动 / `set_offset` / `scroll_to_bottom` / 内容变矮后的 prepaint 钳制都先对齐
  再判定，紧接的滚轮会正常拉伸。

## 最佳实践

- **想要滚动条就用 `overflow_*_scrollbar`**：自动句柄 + 滚动条，无需 `.id()`。
- **需要编程滚动才手动 `.id() + track_scroll`**，并自行叠 `Scrollbar`（需 `.relative()`）。
- **长列表（>100 行）虚拟化是默认选择**：等高用 `uniform_list`，不等高用
  `v_virtual_list`；逐行 `div` 在 1000 行实测 46.3 ms/帧，虚拟化后 2.8 ms/帧。
  正反例与 `.h_full()` 坑见上文「长列表：虚拟化是默认选择」。
- **`#[track_caller]` 撞 id**：循环内创建多个滚动/滚动条时显式 `.id(...)`。
- **记得 `use rgpui::ScrollableElement;`**：滚动类型不在 prelude。
