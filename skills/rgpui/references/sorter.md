# 缩略图排序器（Sorter）

**目录：** [概述](#概述) · [几何自由函数](#几何自由函数) · [SorterDelegate](#sorterdelegate) · [SorterState：选择](#sorterstate选择) · [SorterState：拖拽](#sorterstate拖拽) · [SorterState：缩放与滚动](#sorterstate缩放与滚动) · [Sorter 元素](#sorter-元素) · [最佳实践](#最佳实践)

## 概述

`crates/rgpui/src/sorter.rs`，`pub mod sorter;`（`rgpui.rs:298`）—— **核心，无 feature 门控，
不在 prelude**，路径 `rgpui::sorter::*`。面向「一堆等尺寸缩略图 + 拖拽重排」的二维网格：
自动换行、单选/范围/多选、缩放、键盘导航、拖拽重排。框架只管控件与交互，内容与顺序由
应用经 [`SorterDelegate`] 提供；重排只抛事件（`Reorder`/`ReorderMany`），真正移动数据由应用完成。

类型：`Sorter<D>`（`:882`）、`SorterState<D>`（`:248`）、`SorterDelegate`（`:224`）、
`SorterEvent`（`:198`）。常量：`DEFAULT_CELL_WIDTH`(`:66`)/`DEFAULT_GAP`(`:68`)/
`MIN_ZOOM`(`:70`)/`MAX_ZOOM`(`:72`)/`DEFAULT_ASPECT_RATIO`(`:74`=16:9)/
`AUTOSCROLL_EDGE`(`:76`)/`AUTOSCROLL_MAX_SPEED`(`:78`)。

示例：`cargo run -p showcase --bin sorter`（`examples/showcase/src/bin/sorter.rs`）。

## 几何自由函数

以下都是模块级 `pub fn`（非方法），便于单测与复用：

| 函数 | 位置 | 说明 |
|------|------|------|
| `clamp_zoom(zoom) -> f32` | `:81` | 夹取到 `MIN_ZOOM..=MAX_ZOOM`；非有限值回落 `1.0` |
| `normalize_aspect_ratio(f) -> f32` | `:90` | 非有限/非正回落 `DEFAULT_ASPECT_RATIO` |
| `cell_size(base_width, zoom, aspect_ratio) -> Size<Pixels>` | `:99` | 宽=基准宽×缩放，高=宽÷宽高比 |
| `grid_columns(available_width, cell_width, gap) -> usize` | `:107` | 一行可放列数（至少 1，与 flex-wrap 一致） |
| `drop_slot(cells: &[Bounds], pointer) -> usize` | `:124` | 指针 → 插入槽 `0..=count`（命中格按水平中线分左右；空白→末尾） |
| `move_dest(from, slot) -> Option<usize>` | `:141` | 槽=源或紧随源→`None`（无位移），否则目标索引 |
| `move_group_dest(selected: &[usize], slot) -> Option<usize>` | `:184` | 多选整组落位起始索引（`selected` 升序非空；拖回自身区间→`None`） |
| `autoscroll_velocity(pointer_y, viewport, edge, max_speed) -> Pixels` | `:156` | 边缘自动滚动速度（正下负上，中部 0） |

## SorterDelegate

`trait SorterDelegate: Sized + 'static`（`:224`）：

- **必填** `count(&self, cx) -> usize`（`:226`）
- **可选** `aspect_ratio(&self, cx) -> f32`（`:229`，默认 `DEFAULT_ASPECT_RATIO`）
- **可选** `render_item(&mut self, index, selected, window, cx) -> impl IntoElement`
  （`:236`，默认渲染序号；`cx: &mut Context<SorterState<Self>>`）

```rust
use rgpui::sorter::{Sorter, SorterDelegate, SorterEvent, SorterState};

struct Slides { items: Vec<String> }
impl SorterDelegate for Slides {
    fn count(&self, _: &App) -> usize { self.items.len() }
    fn render_item(&mut self, index: usize, selected: bool, _: &mut Window,
                   _: &mut Context<SorterState<Self>>) -> impl IntoElement {
        div().size_full().child(self.items[index].clone())
    }
}
let state = cx.new(|cx| SorterState::new(Slides { items }, cx));
// 订阅 SorterEvent::Reorder { from, to } 后由应用 remove+insert 数据
```

## SorterState：选择

`SorterState::new(delegate, cx)`（`:283`）。只读：`delegate`/`delegate_mut`（`:304`/`:309`）、
`count(cx)`（`:314`）、`selected() -> &BTreeSet<usize>`（`:319`）、`primary()`（`:324`，最小索引）、
`is_selected(index)`（`:329`）、`selection_len()`（`:334`）。

选择方法（均带 `cx: &mut Context<Self>`）：

| 方法 | 位置 | 行为 |
|------|------|------|
| `select(index, cx)` | `:412` | 单选（越界忽略） |
| `toggle(index, cx)` | `:426` | 切换多选 |
| `select_range(anchor, cursor, cx)` | `:438` | 范围选择（顺序无关，夹到末项） |
| `select_all(cx)` | `:454` | 全选 |
| `clear_selection(cx)` | `:464` | 清空 |

每次变更抛 `SorterEvent::SelectionChanged(Vec<usize>)`（升序）。

## SorterState：拖拽

拖拽期间渲染两层视觉：**跟随指针的半透明预览副本**（`opacity(0.85)`，`:789`-`:813`）+
**插入指示线**（按 `hover_slot` 落在对应格左缘，`:816`）；被拖的原格保留 `opacity(0.4)`
占位（`:781`）。

| 方法 | 位置 | 说明 |
|------|------|------|
| `begin_drag(index, cx)` | `:473` | 进入拖拽，初始槽=自身 |
| `set_drop_slot(slot, cx)` | `:480` | 直接设插入槽（未拖拽则忽略） |
| `update_drag(position, cx)` | `:491` | 记录指针位置并按 `drop_slot` 推算槽 |
| `drag_autoscroll(pointer, cx) -> Pixels` | `:504` | 边缘滚动视口，返回本帧增量（0 可停定时器）；**不持定时器，应用每帧驱动** |
| `finish_drag(cx)` | `:529` | 位移有效抛 `Reorder{from,to}`；拖多选之一抛 `ReorderMany{from:Vec,to}` |
| `cancel_drag(cx)` | `:548` | 取消，不抛事件 |
| `is_dragging() -> bool` | `:402` | 是否拖拽中 |

## SorterState：缩放与滚动

| 方法 | 位置 | 说明 |
|------|------|------|
| `zoom() / set_zoom(f, cx) / zoom_by(delta, cx)` | `:339`/`:344`/`:353` | `set_zoom` 自动 `clamp_zoom` |
| `show_index() / set_show_index(bool, cx)` | `:358`/`:363` | 序号角标开关 |
| `base_width() / set_base_width(px, cx)` | `:371`/`:376` | 单元格基准宽 |
| `gap() / set_gap(px, cx)` | `:384`/`:389` | 间距（兼作网格内边距） |
| `columns() -> usize` | `:397` | 每行列数（最近一次渲染缓存） |
| `scroll_to_item(index, cx)` | `:556` | 滚动使该项可见 |
| `focus(window, cx)` | `:407` | 聚焦（键盘导航：←→↑↓/Home/End/Ctrl+A/Enter/Space/Delete） |

## Sorter 元素

`Sorter<D>`（`:882`，`#[derive(IntoElement)]`）：`new(&Entity<SorterState<D>>)`（`:888`）。
薄包装：`key_context("Sorter")` + `track_focus` + `on_key_down` + `capture_any_mouse_up`，
内部渲染 `SorterState`。视图树里 `Sorter::new(&state)` 即可。

## 最佳实践

- **几何是自由函数**：`cell_size`/`grid_columns`/`drop_slot`/`move_dest`/`move_group_dest`/
  `autoscroll_velocity`/`clamp_zoom`/`normalize_aspect_ratio` 直接从 `rgpui::sorter::` 导入。
- **重排由应用落地**：收到 `Reorder{from,to}` 先 `remove(from)` 再 `insert(to,…)`；
  `ReorderMany` 按「升序抽出、整体插入到 `to`」。
- **边缘自动滚动要自己按帧驱动**：拖拽期间用前台定时器循环调 `drag_autoscroll`，返回 0 就停。
- **`SorterState` 需 `cx.subscribe`** 收 `SorterEvent`，选择/重排/激活/删除都走事件。
