# Ink 笔迹（ink）

**目录：** [概述](#概述) · [笔画模型](#笔画模型) · [绘制生命周期](#绘制生命周期) · [擦除与选择](#擦除与选择) · [多图层文档](#多图层文档) · [几何工具](#几何工具) · [绘制元素 InkLayerElement](#绘制元素-inklayerelement) · [示例与最佳实践](#示例与最佳实践)

## 概述

`crates/rgpui/src/ink.rs`（`pub mod ink;`，`rgpui.rs:295`），核心子系统、无 feature
门控。**crate 根与 prelude 均不重导出**，导入走 `rgpui::ink::{...}`。
模型是「采样点序列 + 画笔样式」的笔画 `Stroke`，按工具编辑整个 `InkLayer`；
多图层用 `InkDocument` 组织（编辑落在激活层）；渲染由 `InkLayerElement` 承担
（逐笔画 `window.paint_path`），指针事件由应用从平台层接入后驱动。

## 笔画模型

- `InkPoint`（`:109`）：`position: Point<Pixels>` + `pressure: f32`（0..=1，鼠标恒
  `1.0`）；`new(position)`（`:118`，压感 1.0）、`with_pressure(position, pressure)`（`:126`，自动夹取）。
- `Stroke`（`:136`）：公开字段 `points: Vec<InkPoint>`、`style: InkStyle`；
  `new(position, style)`（`:145`，单点起笔）、`len()` / `is_empty()`（`:153` / `:158`）、
  `bounds()`（`:163`）、`length()`（`:168`，折线总长）、`nearest_distance(position)`（`:176`）、
  `intersects_circle(center, radius)`（`:189`）、`is_inside_polygon(polygon)`（`:194`，
  **任一采样点**在多边形内即命中）、`simplify(tolerance)`（`:201`）。
- `InkStyle`（`:64`，Copy）：`tool: InkTool`、`color: Hsla`、`width: Pixels`；
  构造 `pen(color, width)`（`:75`）/ `highlighter(color, width)`（`:84`）、
  `is_highlighter()`（`:93`）、`effective_color()`（`:98`，荧光笔叠 `HIGHLIGHTER_ALPHA = 0.35`）。
- `InkTool`（`:51`）：`Pen` / `Highlighter` / `Eraser` / `Lasso`；笔画只用前两者，
  `Eraser`/`Lasso` 是交互工具不产生笔画。
- 公开常量：`MIN_SAMPLE_DISTANCE`（`px(0.75)`，采样最小间距）、`HIGHLIGHTER_ALPHA`、
  `LASSO_STROKE_WIDTH`、`MIN_PRESSURE_SCALE`（`:41`–`:47`）。

## 绘制生命周期

`InkLayer`（`:208`）：公开字段只有 `strokes: Vec<Stroke>`，进行中的笔画存私有字段
`pending`，用 `pending()`（`:222`）/ `is_drawing()`（`:227`）/ `is_empty()`（`:232`）查看。

```rust
layer.begin_stroke(position, InkStyle::pen(color, px(3.0)));   // 落笔（会丢弃旧 pending）
layer.extend_stroke(position);                                  // 移动（距离 < MIN_SAMPLE_DISTANCE 被丢弃）
if !layer.end_stroke() { /* 无笔画提交 */ }                     // 抬笔，返回是否提交
layer.cancel_stroke();                                          // 放弃当前笔画
```

带压感的等价入口：`begin_stroke_with_pressure(position, pressure, style)`（`:242`，
注意参数顺序是**位置、压感、样式**）、`extend_stroke_with_pressure(position, pressure)`（`:261`）。
`undo()`（`:292`）只弹出最近一条**已提交**笔画；`clear()`（`:297`）清空全部含 pending。

## 擦除与选择

- `erase_at(center, radius) -> usize`（`:331`）：整笔删除——笔画与圆相交即**全条**移除，
  返回删除笔画数。
- `erase_local(center, radius) -> usize`（`:342`）：局部切割——只剪掉圆内的**采样点**，
  笔画切成前后两段保留，残段不足 2 点视为擦除；返回剪掉的点数（不是笔画数）。
- `hit_test(position, tolerance) -> Option<usize>`（`:377`）：容差内最近的笔画索引。
- `lasso(polygon) -> Vec<usize>`（`:388`）：多边形（≥3 点）命中的笔画索引；`remove(indices)`（`:401`）按索引删除。
- `simplify(tolerance)`（`:417`）：全部笔画（含 pending）Douglas–Peucker 抽稀。
- `bounds()`（`:303`）：所有笔画包围盒并集。
- `semantics() -> CanvasSemantics`（`:320`）：每笔一个 `GraphicsObject` 语义节点
  （点数/工具/宽进播报文本），交给 `InkLayerElement::a11y` 桥接辅助技术。

## 多图层文档

`InkDocument`（`:470`，数组自底向上叠放，`layers`/`active` 为私有字段）：
`new()`（`:485`，默认含一层「图层 1」）、`layers()`（`:493`）、`layer_count()`（`:498`）、
`active_index()` / `active()`（`:503` / `:508`）、`layer(index)`（`:513`）、
`visible_layers()`（`:518`，返回 `impl Iterator<Item = (usize, &InkDocumentLayer)>`，
自底向上、只含可见层）；结构操作 `set_active(index)`（`:526`）、
`add_layer(name) -> usize`（`:535`，最上方新增并激活）、`remove_layer(index)`（`:542`，
至少保留一层）、`move_layer(from, to)`（`:556`，`to` 为移除后下标、自动夹取）、
`set_visible` / `set_locked` / `set_opacity`（`:578` / `:589` / `:600`，均返回是否实际变更）。

`InkDocumentLayer`（`:429`）公开字段 `name: SharedString`、`layer: InkLayer`、
`visible`、`locked`、`opacity: f32`；`is_editable()`（`:455`，可见且未锁定）、
`fade(color)`（`:460`，把层透明度乘进颜色）。

文档级编辑方法把操作路由到**激活层**，隐藏/锁定层一律返回 `false`：
`begin_stroke` / `begin_stroke_with_pressure` / `extend_stroke` /
`extend_stroke_with_pressure` / `end_stroke` / `cancel_stroke` / `undo` / `clear`（`:616`–`:687`）。
例外：`erase_at` / `erase_local`（`:690` / `:699`）作用于**所有可编辑层**并累加计数；
`hit_test` / `lasso`（`:708` / `:723`）只查可见层，返回 `(图层下标, 笔画下标)` 二元组；
`remove(indices: &[(usize, usize)])`（`:739`）跳过隐藏/锁定层；`bounds()`（`:754`）
取可见层并集；`simplify`（`:771`）只作用于激活层。

## 几何工具

均为模块级公开函数（`:932` 起）：

| 函数 | 说明 |
|------|------|
| `stroke_path(points, width) -> Option<Path>` | 中点二次贝塞尔平滑，圆头圆角 |
| `catmull_rom_path(points, width) -> Option<Path>` | Catmull-Rom 三次贝塞尔，**穿过全部采样点** |
| `stroke_paths(points, width) -> Vec<Path>` | 逐段按两端压感均值变宽；压感全 1.0 或 ≤2 点时退化为单条 `stroke_path` |
| `pressure_width(base, pressure) -> Pixels` | 压感 0..=1 线性映射到基础的 `MIN_PRESSURE_SCALE`..=1 倍 |
| `smooth_pressure(pressure, gamma) -> f32` | 幂曲线整形压感（gamma<1 轻压更饱满） |
| `point_segment_distance(p, a, b) -> f32` | 点到线段最短距离 |
| `point_in_polygon(p, polygon) -> bool` | 射线法内嵌判定（<3 点恒 false） |
| `polygon_bounds(polygon) -> Option<Bounds>` | 多边形包围盒 |
| `simplify_points(points, tolerance) -> Vec<InkPoint>` | Douglas–Peucker 抽稀（保留首尾与拐点） |

## 绘制元素 InkLayerElement

`InkLayerElement`（`:778`，`impl RenderOnce` `:834`）：`new(layer: InkLayer)`（`:793`，
**按值收取**，多图层绘制时传 `layer.layer.clone()`）、`.opacity(f32)`（`:812`，整层
透明度，夹取 0..=1）、`.highlight(Vec<usize>)`（`:822`，高亮选中笔画）、
`.lasso(Vec<Point<Pixels>>)`（`:828`，画进行中套索，虚线描边）、
`.a11y(id, label)`（`:806`，开启后每笔发布语义节点；同一层跨帧用同一 id）。
绘制时不透明笔画走 `stroke_paths`（压感逐段），半透明（荧光笔/层透明度）走等宽
单路径 `stroke_path` 避免圆头接缝「串珠」加深。

## 示例与最佳实践

示例：`cargo run -p showcase --bin ink`（`examples/showcase/src/bin/ink.rs`，
钢笔/荧光笔落笔含压感、`erase_local` 橡皮、套索选中 + 撤销/清空 + 多图层）。

```rust
use rgpui::ink::{InkDocument, InkLayerElement, InkStyle};
use rgpui::{hsla, px};

let mut doc = InkDocument::new();
doc.add_layer("批注");
doc.begin_stroke(start_pos, InkStyle::pen(hsla(0.6, 0.8, 0.4, 1.0), px(3.0)));
doc.extend_stroke(next_pos);
doc.end_stroke();
// 逐层绘制：隐藏层跳过，层透明度传给元素
let layer_elements = doc.visible_layers()
    .map(|(_, l)| InkLayerElement::new(l.layer.clone()).opacity(l.opacity));
```

- **橡皮二选一**：删整条用 `erase_at`，剪局部用 `erase_local`（返回的是点数不是笔画数）。
- **压感入口成对**：鼠标走 `begin_stroke`/`extend_stroke`，数位板走 `*_with_pressure`。
- **编辑路由看激活层**，但擦除看**所有可编辑层**、命中/套索看**所有可见层**——语义不同。
- **无障碍**要配 `.a11y()` + `layer.semantics()`，笔画才进屏幕阅读器。
