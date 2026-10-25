# 任务窗格（Task Pane）

**目录：** [概述](#概述) · [类型与常量](#类型与常量) · [TaskPaneState](#taskpanestate) · [显隐与自动隐藏](#显隐与自动隐藏) · [页签组](#页签组) · [浮动与停靠](#浮动与停靠) · [交互与键盘](#交互与键盘) · [几何函数](#几何函数) · [多窗格堆叠](#多窗格堆叠) · [示例与陷阱](#示例与陷阱)

## 概述

`crates/rgpui/src/task_pane.rs`，`pub mod task_pane;`（`rgpui.rs:307`）——**核心，无 feature 门控**，
不在 prelude、根也不 glob 重导出，须显式 `use rgpui::task_pane::{...}`。
状态由 `TaskPaneState` 持有（停靠侧 / 宽度 / 可见性 / 浮动矩形 / 拖拽会话），渲染由
`TaskPane` 元素承担，正文经 `body` 回调现渲染。
**宿主必须 `cx.observe(&pane, |_, _, cx| cx.notify())`**（或订阅 `TaskPaneEvent`）才会实时重绘。
浮动态是 `absolute` 定位，因此**父容器必须是定位祖先**（`relative()`）。
浮动缩放复用 `crate::mdi` 的 `ResizeEdge`/`resize_bounds`/`resize_edge_at`/`resize_edge_rects`
（`task_pane.rs:41`），与 `viewport` feature 的 `object_toolkit` 无关。

## 类型与常量

`TaskPaneSide`（`:73`）：`Left`（贴父容器左缘，内缘在右）/`Right`；方法 `grip_edge() -> ResizeEdge`
（`:82`，左停靠 → `ResizeEdge::Right`，右停靠 → `Left`）。
`TaskPaneHit`（`:92`）：`None`/`TitleBar`/`Tab(usize)`/`Close`/`Grip`/`Resize(ResizeEdge)`/`Body`。
`TaskPaneEvent`（`:111`）：`VisibilityChanged(bool)`/`SideChanged(TaskPaneSide)`/
`WidthChanged(Pixels)`/`FloatingChanged(bool)`/`Collapsed(bool)`/`TabSelected(usize)`。

常量（`:51`-`:69`）：`TITLE_HEIGHT`、`GRIP_WIDTH: f32`、`DEFAULT_WIDTH`、`MIN_WIDTH`、`MAX_WIDTH`、
`CLOSE_BUTTON_WIDTH`、`MIN_FLOATING_SIZE`、`FLOATING_MARGIN`、`RAIL_WIDTH`（自动隐藏收起后的
**竖排轨道宽度**，即 `effective_width()` 收起时的返回值）、`TITLE_PADDING`。

## TaskPaneState

`TaskPaneState::new(cx)`（`:268`）默认**右侧停靠、可见、宽 `DEFAULT_WIDTH`、closable**；
链式配置：`with_side`（`:300`）、`with_width`（`:306`，夹到 min..=max）、`with_width_range`（`:312`）、
`with_visible`（`:320`）、`with_closable`（`:326`）、`with_tabs<T: Into<SharedString>>(impl IntoIterator<Item = T>)`（`:332`）。
读取：`side()`/`width()`/`width_range() -> (Pixels, Pixels)`/`is_visible()`/`is_floating()`/
`is_closable()`/`tabs() -> &[SharedString]`/`active_tab()`/`active_tab_title() -> Option<&SharedString>`/
`floating_bounds()`/`rect()`/`effective_width()`。`impl Focusable`（`:822`）。

## 显隐与自动隐藏

`show`/`hide`/`toggle`（`:394`-`:404`）都转发到 `set_visible(visible, cx) -> bool`（`:409`，
emit `VisibilityChanged`；隐藏时清拖拽会话）。
自动隐藏：`set_auto_hide(bool, cx)`（`:431`，关闭时立即展开；开启时若鼠标不在窗格上则收起）、
`is_auto_hide()`（`:426`）、`set_collapsed`/`toggle_collapsed`（`:447`/`:461`，自动隐藏关掉也能手动收）、
`is_collapsed()`（`:442`）、`set_hovered(bool, cx)`（`:466`，由元素 `on_hover` 自动调用，
`:1129`）、`effective_width()`（`:479`，收起时返回 `RAIL_WIDTH`）。
收起为轨道时整条按 `TaskPaneHit::TitleBar` 处理（`:664`），不提供按钮 / 缩放；
浮动态不参与自动隐藏，`float` 会清 `collapsed`（`:601`）。

## 页签组

`with_tabs`/`set_tabs(Vec<SharedString>, cx)`（`:510`，非空时标题栏渲染为页签组，正文由应用
按激活页签自行切换）。`active_tab()`/`active_tab_title()`/`set_active_tab(index, cx)`（`:521`，
越界或已是当前页签返回 `false`，成功 emit `TabSelected`）/`cycle_tab(delta: isize, cx)`（`:532`，
按 `rem_euclid` 循环）。`tab_rects()` 方法（`:542`）走局部矩形；
`tab_at(pointer)`（`:547`）收的是**窗口坐标**，关闭按钮区优先返回 `None`。

## 浮动与停靠

`float(bounds, cx)`（`:586`，`bounds` 为**父容器坐标**，尺寸下限 `MIN_FLOATING_SIZE`；
隐藏的窗格会被顺带显示）、`dock(cx)`（`:608`，清拖拽并重新套用自动隐藏）、
`toggle_float(parent, cx)`（`:630`，浮动位取 `floating_default_bounds(parent, size(self.width, DEFAULT_WIDTH))`
—— 注意**高度用 `DEFAULT_WIDTH`**）。
`set_rect(rect, cx)`（`:640`）与 `set_parent_bounds(bounds, cx)`（`:291`，窗口坐标）：
前者由元素 `on_prepaint` 自动回写（`:1099`），**后者不会自动设置**——换边规则依赖它，
宿主要在 render 里自己喂父容器矩形（示例用 `window.viewport_size()`）。
换边规则（`:742` 实证）：停靠态拖内缘把手时，指针越过 `parent_bounds` 的水平中线即
`set_side` 到另一侧，并重置拖拽基线避免宽度跳变。

## 交互与键盘

`TaskPane::new(&Entity<TaskPaneState>)`（`:846`）、`title(impl Into<SharedString>)`（`:855`）、
`body(F)`（`:861`，`F: Fn(&mut Window, &mut App) -> AnyElement + 'static`）。
元素内部自动把 `on_mouse_down`/`on_mouse_move`/`on_key_down`/`on_hover` 转发到状态，宿主不用手接。
可直接调用的接口（`pointer` 均为**窗口坐标**）：`hit_test(pointer) -> TaskPaneHit`（`:654`）、
`on_pointer_down(pointer)`（`:690`，**无返回值、无 cx**）、`on_pointer_move(pointer, cx)`（`:717`）、
`on_pointer_up(pointer, cx)`（`:787`，按下与松开都命中关闭按钮才 `hide`）。
键盘（`:798` 实证）：`Esc` 取消当前拖拽（仅在有会话时 `stop_propagation`）、
`Ctrl+Tab` / `Ctrl+Shift+Tab` 循环切页签。停靠态点标题栏**不产生拖拽**（`:699`，只有浮动态可拖）。

## 几何函数

`clamp_width(width, min, max)`（`:127`，非有限值回落 `min`）、
`resize_width(start, edge, delta_x, min, max)`（`:137`，拖左缘取负号）、
`grip_rect(rect, side)`（`:153`，局部坐标内缘把手）、`close_button_rect(rect)`（`:163`，标题栏右上角）、
`tab_rects(rect, count)`（`:174`，自由函数：标题栏内等宽，右侧预留 `CLOSE_BUTTON_WIDTH`）、
`floating_default_bounds(parent, size)`（`:192`，父容器右下角内侧，留白 `FLOATING_MARGIN`）。

## 多窗格堆叠

同侧多窗格垂直分配高度：`TaskPaneStackState`（`:1197`）+ `TaskPaneStack`（`:1352`）。
状态：`new(_cx)`（`:1214`）、`with_weights(impl IntoIterator<Item = f32>)`（`:1226`）、
`weights() -> &[f32]`、`weight(index) -> f32`（`:1237`，缺失回退 `1.0`）、
`set_weights(...)`（`:1246`）、`set_weight(index, weight, cx)`（`:1252`，越界补 `1.0`）、
`rect()`、`slots()`、`usable_height()`（`:1276`，扣掉分隔条后的可用高度）、`dragging() -> Option<usize>`，
拖拽三段 `begin_drag(index, pointer_y, cx)`（`:1287`，index 是分隔条**上方**子元素下标）/
`update_drag(pointer_y, cx)`（`:1295`，始终按「起点权重 + 总位移」重算，不逐帧累积）/
`end_drag(cx)`（`:1312`）。
元素：`TaskPaneStack::new(&Entity<TaskPaneStackState>)`（`:1361`）、`child(impl IntoElement)`（`:1369`）、
`children`（`:1375`）；子元素通常是 `TaskPane`，**不可见的窗格不要加入**（会占位）。
几何：`normalize_stack_weight(weight)`（`:1147`，非有限/非正回退 `1.0`，过小夹到 `MIN_STACK_WEIGHT`）、
`resize_stack_weights(&mut weights, index, delta, usable) -> bool`（`:1160`，只改相邻两格、
保持两者之和不变）。常量：`STACK_SEPARATOR_HEIGHT`（`:1140`）、`MIN_STACK_WEIGHT: f32`（`:1143`）。

```rust
use rgpui::task_pane::{TaskPane, TaskPaneSide, TaskPaneState};

let pane = cx.new(|cx| {
    TaskPaneState::new(cx)
        .with_side(TaskPaneSide::Right)
        .with_tabs(["属性", "大纲"])
});
cx.observe(&pane, |_this, _pane, cx| cx.notify()).detach();
// render 里：父容器 .relative().flex().flex_row()
// let parent = Bounds::new(point(px(0.0), px(0.0)), window.viewport_size());
// pane.update(cx, |p, cx| p.set_parent_bounds(parent, cx));
// div().child(TaskPane::new(&pane).title("属性").body(|_w, _cx| ...))
```

## 示例与陷阱

- 示例：`cargo run -p showcase --bin task_pane`（`examples/showcase/src/bin/task_pane.rs`，无 feature 门控）。
- `set_parent_bounds` **没有元素侧的自动喂值**（`set_rect` 有）：不调它，`parent.size.width <= px(0.0)`
  会让 `cross_side_at` 恒为 `false`，「拖过中线换边」静默失效。
- 坐标口径混用：`hit_test`/`on_pointer_*`/`tab_at`/`set_parent_bounds` 收窗口坐标，
  `grip_rect`/`close_button_rect`/自由函数 `tab_rects` 算的是局部坐标。
- 事件方法命名不对称：`TaskPaneState` 上没有 `set_floating`，浮动只能走 `float`/`dock`/`toggle_float`。
