# 多文档界面（MDI）

**目录：** [概述](#概述) · [类型](#类型) · [MdiState 构造与配置](#mdistate-构造与配置) · [生命周期与状态](#生命周期与状态) · [页签式排布](#页签式排布) · [交互入口](#交互入口) · [几何函数](#几何函数) · [示例与陷阱](#示例与陷阱)

## 概述

`crates/rgpui/src/mdi.rs`，`pub mod mdi;`（`rgpui.rs:304`）——**核心，无 feature 门控**，
不在 prelude、根也不 glob 重导出，须显式 `use rgpui::mdi::{...}`。
状态由 `MdiState` 实体持有（窗口集合按 z 序，**末尾在最前**），渲染由 `MdiArea`
元素承担：正文经 `body` 回调按窗口 id 现渲染，状态不持有视图。
`MdiArea` 只是薄壳，宿主视图必须 `cx.observe(&mdi, |_, _, cx| cx.notify())`
（或订阅 `MdiEvent`）才能在拖拽 / 缩放 / 排布时实时重绘。

**`ResizeEdge` / `resize_bounds` / `resize_edge_at` / `resize_edge_rects` 就定义在 `mdi.rs` 里**
（同 crate 的 `task_pane.rs:41` 反过来 `use crate::mdi::{...}` 复用它们），
与 `viewport` feature 门控的 `object_toolkit`（其同名 `resize_bounds` 签名不同，
按 `ResizeHandle` 而非 `ResizeEdge`）**互不相干**，不要混引。

## 类型

`MdiWindow`（`:235`）公开字段：`id: u64`、`title: SharedString`、`bounds`、
`state: WindowState`、`restore_bounds`、`closable`/`minimizable`/`maximizable: bool`。
`MdiFrame`（`:256`，渲染快照）：`id`/`title`/`rect`/`minimized`/`maximized`/`active`/
`closable`/`minimizable`/`maximizable`。
`MdiTab`（`:424`）：`id`/`title`/`rect`/`active`/`closable`。
`MdiWindowFlags`（`:214`）：pub 字段 `closable`/`minimizable`/`maximizable`，
`Default` 三者全 `true`；**只作 `open_with` 的入参**，`MdiWindow` 上是三个散装 bool 字段。

`WindowState`（`:68`）：`Normal`/`Minimized`/`Maximized`。
`MdiLayout`（`:79`）：`Cascade`/`TileHorizontal`/`TileVertical`/`Tabbed`。
`WindowButton`（`:161`）：`Minimize`/`Maximize`/`Close`。
`ResizeEdge`（`:92`）：`Left`/`Right`/`Top`/`Bottom`/`TopLeft`/`TopRight`/`BottomLeft`/`BottomRight`；
方法 `touches_left`/`touches_right`/`touches_top`/`touches_bottom`（`:113`-`:137`）、`cursor()`（`:145`）。
`MdiHit`（`:172`）：`None`/`TitleBar(u64)`/`Body(u64)`/`Icon(u64)`/
`Button { id, button }`/`Resize { id, edge }`/`Tab(u64)`。
`MdiEvent`（`:201`）：`Opened(u64)`/`Activated(u64)`/`Closed(u64)`/`StateChanged(u64)`。

## MdiState 构造与配置

`MdiState::new(cx)`（`:604`）+ 链式配置：`with_min_size(Size<Pixels>)`（`:622`）、
`with_cascade_offset(Pixels)`（`:628`，下限 1px）、`with_tile_gap(Pixels)`（`:634`，
默认 `px(6.0)`）、`with_snap(Pixels)`（`:640`，开启松手吸附；默认 `None` 不吸附）。
读取：`windows()`（`:646`）、`len()`/`is_empty()`、`active() -> Option<u64>`、
`get(id)`、`area()`、`rect_of(id) -> Option<Bounds<Pixels>>`（`:724`，局部坐标）、
`layout() -> Vec<MdiFrame>`（`:731`）。`impl Focusable`（`:1376`）。

## 生命周期与状态

- `open(title, cx) -> u64`（`:754`）＝ `open_with(title, MdiWindowFlags::default(), cx)`（`:759`）。
- `close(id, cx) -> bool`（`:785`）/ `close_active(cx)`（`:808`）/ `close_all(cx)`（`:816`，逐扇 emit `Closed`）。
- `activate(id, cx) -> bool`（`:824`）：提到 z 序最前，返回**是否发生变化**（已在最前且已激活返回 `false`）。
- `cycle(forward, cx) -> Option<u64>`（`:845`）：`forward` 真为下一个（z 序下标 +1）。
- `set_bounds(id, bounds, cx) -> bool`（`:864`，内部 `clamp_rect` 夹取并把状态改回 `Normal`）、
  `move_by(id, delta, cx)`（`:880`）、`snap_active(threshold, cx)`（`:892`，候选线 = 工作区 + 其余 `Normal` 窗口）。
- `minimize`/`maximize`/`restore`（`:917`-`:927`）、`toggle_maximize`（`:932`）。
- `arrange(layout, cx) -> bool`（`:941`）：排布所有非最小化窗口并把最大化的还原；
  区域尚未取得时请求挂起到 `pending_layout`，`set_area` 首次拿到区域时自动补做。
- `set_area(area, cx)`（`:950`）由 `MdiArea` 在 `on_prepaint` 自动调用（`:1613`），一般无需手动。

## 页签式排布

`arrange(MdiLayout::Tabbed, cx)` 后置 `tabbed` 标志：`is_tabbed()`（`:676`）、
`tab_bar_rect()`（`:681`，贴区域顶部、高 `TAB_BAR_HEIGHT`）、`tabbed_content_rect()`（`:687`，
标签条以下铺满）、`tabs() -> Vec<MdiTab>`（`:699`，**非页签式返回空**；按 id 升序、
跳过最小化窗口）。页签式下所有窗口被铺到正文区但 `layout()` 只下发激活窗口 + 最小化图标；
此模式下**禁止拖动标题栏**（`:1070`），`set_state` 也拒绝 `Maximized`（`:1285`）。

## 交互入口

`MdiArea::new(&Entity<MdiState>)`（`:1399`，正文默认空白）+ `body(F)`（`:1407`，
`F: Fn(u64, &mut Window, &mut App) -> AnyElement + 'static`）。指针 / 键盘事件由元素内部
自动转发到 `MdiState`，宿主不用手接：
`on_pointer_down(local, click_count, cx)`（`:1051`，标题栏双击 = `toggle_maximize`）、
`on_pointer_move(local, cx)`（`:1121`）、`on_pointer_up(local, cx)`（`:1151`，按钮按「按下与松开
命中同一按钮」才执行；`Move` 松手时按 `with_snap` 吸附一次）、`on_key_down(event, cx)`（`:1176`）。
`hit_test(pointer) -> MdiHit`（`:987`，局部坐标）。
键盘（`:1176` 实证）：`Ctrl+Tab`/`Ctrl+F6` 循环（带 `Shift` 反向）、`Ctrl+F4` 关闭激活、
`Ctrl+M` 最小化、`Esc` 清拖拽会话。

## 几何函数

| 函数 | 签名要点 | 行为 |
|------|----------|------|
| `cascade_rects`（`:280`） | `(area, count, offset)` | 从左上按 `offset` 错开，尺寸取区域 60%×70%，越界回绕成第二圈 |
| `tile_rects`（`:311`） | `(area, count, horizontal, gap)` | 均分；区域过小**不做最小尺寸兜底** |
| `clamp_rect`（`:349`） | `(rect, area, min)` | 不小于 `min`，水平至少留 `MIN_VISIBLE` 宽、标题栏不掉出上下边 |
| `snap_rect`（`:365`） | `(rect, targets, threshold)` | 边/中线吸到候选矩形边/中线，位移**严格小于** `threshold` 才生效，尺寸不变 |
| `icon_rect`（`:415`） | `(area, index)` | 底部图标条从左到右，宽 `ICON_WIDTH` |
| `tab_rects`（`:438`）/`tab_at`（`:454`） | `(bar, count[, pointer])` | 标签条内等分 / 命中序号 |
| `title_button_rects`（`:460`） | `(rect) -> [(WindowButton, Bounds); 3]` | 自右向左：Close、Maximize、Minimize |
| `resize_edge_at`（`:484`）/`resize_edge_rects`（`:1648`） | `(rect, pointer)` / `(rect)` | 角优先于边；后者给 8 块命中矩形 |
| `resize_bounds`（`:509`） | `(start, edge, delta, min)` | 拖左/上以右/下为锚，够不到 `min` 整体回退 |

常量（`:44`-`:64`）：`TITLE_BAR_HEIGHT`、`MIN_WINDOW_SIZE`、`CASCADE_OFFSET`、`MARGIN`、
`RESIZE_HANDLE: f32`、`ICON_BAR_HEIGHT`、`ICON_WIDTH`、`TITLE_BUTTON_WIDTH`、
`TAB_BAR_HEIGHT`、`TAB_CLOSE_WIDTH`、`MIN_VISIBLE`。

## 示例与陷阱

```rust
use rgpui::mdi::{MdiArea, MdiLayout, MdiState};

let mdi = cx.new(|cx| MdiState::new(cx).with_snap(px(6.0)));
cx.observe(&mdi, |_this, _mdi, cx| cx.notify()).detach();
let id = mdi.update(cx, |s, cx| s.open("季度报告", cx));
// 渲染：div().size_full().child(MdiArea::new(&mdi).body(move |id, _w, _cx| ...))
// 排布：mdi.update(cx, |s, cx| s.arrange(MdiLayout::Tabbed, cx));
```

- 示例：`cargo run -p showcase --bin mdi`（`examples/showcase/src/bin/mdi.rs`，无 feature 门控）。
- **`with_snap` 只在松手那一刻吸附**：`on_pointer_move` 内部不调 `snap_rect`，
  拖动过程没有吸附预览（要预览得自己在 move 里调 `snap_rect`）。
- 几何一律用**局部坐标**（相对 `MdiArea` 原点）：`hit_test`/`on_pointer_*` 收的都是 local。
- `close` 后激活窗口可能换人（页签式下跳过最小化窗口），别缓存 `active()` 结果。
