# 画布语义（Canvas Accessibility）

**目录：** [为什么需要](#为什么需要) · [CanvasNode](#canvasnode) · [CanvasSemantics](#canvassemantics) · [几何自由函数](#几何自由函数) · [AccessKit 桥](#accesskit-桥) · [Canvas 元素接线](#canvas-元素接线) · [已接入的组件](#已接入的组件) · [表格/网格交叉引用](#表格网格交叉引用) · [最佳实践](#最佳实践)

## 为什么需要

自绘组件（网格、页视图、笔迹、图表、时间线、MDI 子窗口…）用 `Window::paint_*` 直接画像素，
元素树里只有**一个**叶子节点，屏幕阅读器看不到「一格单元格」「一笔笔迹」。
`crates/rgpui/src/canvas_a11y.rs`（`pub mod canvas_a11y;`，`rgpui.rs:310`，核心无 feature）把
画布上每个自绘对象描述成 [`CanvasNode`]，经 `Element::a11y_synthetic_children` 发布为**合成子节点**，
并把辅助技术派发到这些节点的动作路由回应用。类型 `Role`/`Action`/`ActionData` 来自 `accesskit`
（root 另有别名 `rgpui::AccessibleAction` = `accesskit::Action`、`rgpui::Role`、`rgpui::accesskit`）。

示例：`cargo run -p showcase --bin canvas_a11y`（`examples/showcase/src/bin/canvas_a11y.rs`）。

## CanvasNode

`CanvasNodeId = u64`（`:54`）—— 应用自定义的**稳定标识**：同一对象跨帧用同一 id，辅助技术才认得是
「同一个」（否则被当成删一增一而反复播报）。

`CanvasNode`（`:63`，字段私有）构造/构建器（`bounds` 为**画布局部坐标**）：
`new(id, role, label, bounds)`（`:89`）、`with_value`（`:110`）、`with_description`（`:116`）、
`with_bounds`（`:123`）、`disabled`（`:129`）、`selected`（`:135`）、`focused`（`:141`，发布时登记为父节点
active descendant）、`with_action`（`:147`）/`with_actions`（`:155`）。
读取器：`id`/`role`/`label`/`bounds`/`value`/`description`/`is_disabled`/`is_selected`/`is_focused`/`actions`。
就地改：`set_selected`/`set_focused`/`set_disabled`/`set_bounds`（`:213`-`:230`）。

## CanvasSemantics

`CanvasSemantics`（`:237`）= 本帧语义节点集合，**按绘制顺序存放（先画的在前，后画者在叠放上层）**；
应用每帧重建即可。

| 方法 | 位置 | 说明 |
|------|------|------|
| `new()` / `clear()` | `:244`/`:260` | 空集合 / 清空（保留容差设置） |
| `push(node)` / `extend(nodes)` | `:265`/`:270` | 追加 |
| `len()` / `is_empty()` / `nodes()` / `iter()` | `:275`-`:306` | 数量 / 绘制序列表 |
| `get(id)` / `get_mut(id)` / `index_of(id)` | `:311`/`:316`/`:321` | 按标识查询 / 下标 |
| `hit_test(point) -> Option<CanvasNodeId>` | `:326` | 该点**最上层**（最后绘制）对象 |
| `reading_order() -> Vec<CanvasNodeId>` | `:331` | 先按行（y 容差内）、行内按 x |
| `first_in_reading_order()` | `:339` | 阅读顺序首个 |
| `next_in_reading_order(current, forward)` | `:346` | 前后移动（两端环绕；`current=None` 从首/末起步） |
| `window_bounds(canvas_origin, id)` | `:357` | 局部矩形 → 窗口矩形 |
| `with_row_tolerance(px)` / `row_tolerance()` | `:249`/`:255` | 分行 y 容差（默认 `READING_ROW_TOLERANCE`=4px，`:57`） |
| `fill_empty_bounds(bounds) -> usize` | `:294` | 把**零尺寸**节点展开成给定矩形（整体摘要节点用），返回填充数 |

## 几何自由函数

模块级 `pub fn`：`window_bounds(canvas_origin, node) -> Bounds`（`:367`，局部→窗口）、
`node_at_point(nodes, point) -> Option<usize>`（`:378`，**逆绘制序**命中下标）、
`reading_order_indices(nodes, row_tolerance) -> Vec<usize>`（`:386`，两趟排序避免非全序 panic）、
`next_in_order(order, current, forward) -> Option<usize>`（`:423`，下标版环绕）。

## AccessKit 桥

- `accesskit_node(node, window_bounds, scale_factor) -> accesskit::Node`（`:450`）：把语义节点写成
  AccessKit 节点（名称/值/说明/禁用/选中/动作，矩形按 `scale_factor` 转物理像素）。
- `push_canvas_a11y(builder: &mut A11ySubtreeBuilder, semantics, canvas_origin) -> Vec<CanvasNodeId>`
  （`:489`）：在 `a11y_synthetic_children` 里逐个发布合成子节点，返回**成功推入**的 id（重复 id 被框架丢弃）。
  内部用 `builder.scale_factor()`、`builder.synthetic_node_id(node.id())`、
  `builder.push_child_with_bounds(id, node, bounds)`；`focused` 节点会 `builder.parent_node().set_active_descendant(id)`。
- `CanvasActionHandler`（`:514`）= `Rc<RefCell<dyn FnMut(CanvasNodeId, Action, Option<&ActionData>, &mut Window, &mut App)>>`。
- `register_canvas_a11y_actions(window, canvas_id: &GlobalElementId, semantics, handler) -> usize`
  （`:524`）：在 `paint` 里为节点声明的每个动作登记监听器，返回登记数量。合成子节点 id 由画布自身
  `canvas_id.accesskit_node_id()` 派生 —— 用 crate 级 `synthetic_child_node_id(parent, id)`
  （`window/a11y.rs:319`）复算，与 `builder.synthetic_node_id`（同基于父 id + key 哈希，`:354`）两侧一致。
  仅在 `window.is_a11y_active()` 时调用。

## Canvas 元素接线

`crates/rgpui/src/elements/canvas.rs`（`canvas(prepaint, paint)` 构造 `Canvas<T>`，`:39`）提供四个 builder：

| 方法 | 位置 | 说明 |
|------|------|------|
| `.id(impl Into<ElementId>)` | `:109` | **前提**：只有带 id 才分配 `GlobalElementId`、才进无障碍树挂合成子节点 |
| `.a11y(role: Role, semantics: CanvasSemantics)` | `:120` | 声明画布角色 + 本帧语义节点（需先 `.id(..)`） |
| `.a11y_label(impl Into<SharedString>)` | `:134` | 画布自身容器名；未 `.a11y` 过则以 `Role::Canvas` 建配置 |
| `.on_canvas_a11y_action(handler)` | `:146` | 处理语义节点动作（只有节点自己 `with_action` 声明过的才派发） |

> 注意：`Canvas::a11y(role, semantics)` 与下列组件级 `.a11y(id, label)` **签名不同**，别混用。

零尺寸规则：`a11y_synthetic_children`（`:260`）先 clone 语义、`fill_empty_bounds(Bounds::new(Point::default(),
self.bounds.size))` 把宽高都为 0 的节点展开成整块画布（构建语义时不知道布局大小的整体摘要节点用），
再 `push_canvas_a11y`；已有尺寸的节点（单元格、笔画）原样保留。`paint`（`:221`）在 `is_a11y_active()` 且
设了 handler 时调 `register_canvas_a11y_actions`。

## 已接入的组件

这些组件的 `.a11y(id, label)` 是便捷封装（内部自建画布 + 语义）：

- 核心（无 feature）：`InkLayerElement`（`ink.rs:806`）、`Sparkline`（`components/sparkline.rs:334`）、
  `Waveform`（`components/waveform.rs:94`）。
- 随 `charts` feature（`components/charts/*`）：`PieChart`（`:216`）、`Gauge`（`:138`）、
  `RadarChart`（`:218`）、`TreeMap`（`:385`）、`LineChart`（`:300`）、`AreaChart`（`:340`）、
  `Chart`（`:800`）—— 后七个多为「整体摘要」型（用零尺寸节点 + 画布大小填充）。

## 表格/网格交叉引用

表格/网格类**不走**画布语义（它们是真实元素树），而是给 `Stateful<Div>` 挂 `StatefulInteractiveElement`
的 `aria_row_index`/`aria_column_index`（`elements/div.rs:1239`/`:1372`/`:1378`）：
`DataTable` 根 `Role::Table`、行 `Role::Row`、单元格 `Role::Cell` + 行列索引；
`SheetGrid`（`spreadsheet` feature）根 `Role::Grid`、单元格 `Role::Cell`、列头 `Role::ColumnHeader`、
行头 `Role::RowHeader`。详见 `list-table-tabs.md`。

## 最佳实践

- **先 `.id(..)` 再 `.a11y(..)`**：没有元素 id，语义节点根本进不了无障碍树。
- **id 跨帧稳定**：同一自绘对象每帧复用同一 `CanvasNodeId`，否则焦点/播报会漂。
- **整体摘要节点给零尺寸矩形**：让画布元素按自身 bounds 填充（`fill_empty_bounds`），不必预先知道大小。
- **动作要在节点上声明**：只有 `with_action`/`with_actions` 登记过的 `Action` 才会被 `on_canvas_a11y_action` 派发。
