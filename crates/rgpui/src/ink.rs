//! Ink 笔迹层 —— 墨迹笔画模型、编辑操作与绘制元素。
//!
//! 面向白板 / 手写批注 / 手绘应用的**墨迹层**：以「采样点序列 + 画笔样式」为笔画
//! （[`Stroke`]），按工具（钢笔 / 荧光笔 / 橡皮 / 套索）编辑一整个 [`InkLayer`]：
//! 落笔-移动-抬笔生成笔画、按半径擦除、套索选择、撤销等；多图层文档用 [`InkDocument`]
//! 组织（可见 / 锁定 / 不透明度 / 叠放顺序，编辑落在激活图层）；并提供**纯几何**工具函数
//! （点到线段距离、点在多边形内、Douglas–Peucker 抽稀）与把笔画平滑成 [`crate::Path`]
//! 的构建器。渲染由 [`InkLayerElement`] 承担（逐笔画 `window.paint_path`）。
//!
//! ## 用法
//!
//! ```ignore
//! use rgpui::ink::{InkDocument, InkLayer, InkLayerElement, InkStyle};
//! use rgpui::{hsla, px};
//!
//! let mut doc = InkDocument::new();
//! doc.add_layer("批注");
//! doc.begin_stroke(point, InkStyle::pen(hsla(0.6, 0.8, 0.4, 1.0), px(3.0)));
//! doc.extend_stroke(point2);
//! doc.end_stroke();
//! // 逐层绘制：隐藏层跳过，图层透明度传给元素。
//! let elements = doc
//!     .visible_layers()
//!     .map(|(_, layer)| InkLayerElement::new(layer.layer.clone()).opacity(layer.opacity));
//! ```
//!
//! **本版暂缓**（后续增补）：橡皮的局部切割、触摸 / 数位板事件接入
//! （现由 [`crate::Platform`] 的指针事件驱动）。

use lyon::path::{LineCap, LineJoin};

use crate::canvas_a11y::{CanvasNode, CanvasSemantics};
use crate::{
    ActiveTheme as _, App, Bounds, ElementId, Hsla, IntoElement, Path, PathBuilder, PathStyle,
    Pixels, Point, RenderOnce, SharedString, StrokeOptions, Styled as _, Window, canvas, point, px,
    size,
};
use accesskit::Role;

/// 采样最小间距：鼠标移动事件比采样更密，移动不足该距离的点被丢弃。
pub const MIN_SAMPLE_DISTANCE: Pixels = px(0.75);
/// 荧光笔的不透明度（重叠处自然加深）。
pub const HIGHLIGHTER_ALPHA: f32 = 0.35;
/// 套索选区描边的宽度。
pub const LASSO_STROKE_WIDTH: Pixels = px(1.5);
/// 压感 0 时的笔画宽度下限（相对基础宽度的比例）。
pub const MIN_PRESSURE_SCALE: f32 = 0.35;

/// 墨迹工具。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InkTool {
    /// 钢笔：不透明细描边。
    Pen,
    /// 荧光笔：半透明粗描边。
    Highlighter,
    /// 橡皮：按半径擦除整条笔画。
    Eraser,
    /// 套索：框选笔画。
    Lasso,
}

/// 画笔样式（决定一条笔画如何绘制）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InkStyle {
    /// 工具（笔画仅使用 `Pen`/`Highlighter`；`Eraser`/`Lasso` 不产生笔画）。
    pub tool: InkTool,
    /// 颜色。
    pub color: Hsla,
    /// 描边宽度。
    pub width: Pixels,
}

impl InkStyle {
    /// 钢笔样式。
    pub fn pen(color: Hsla, width: Pixels) -> Self {
        Self {
            tool: InkTool::Pen,
            color,
            width,
        }
    }

    /// 荧光笔样式（渲染时叠加固定不透明度）。
    pub fn highlighter(color: Hsla, width: Pixels) -> Self {
        Self {
            tool: InkTool::Highlighter,
            color,
            width,
        }
    }

    /// 是否为荧光笔。
    pub fn is_highlighter(&self) -> bool {
        self.tool == InkTool::Highlighter
    }

    /// 实际绘制颜色（荧光笔降低不透明度）。
    pub fn effective_color(&self) -> Hsla {
        if self.is_highlighter() {
            self.color.opacity(HIGHLIGHTER_ALPHA)
        } else {
            self.color
        }
    }
}

/// 一个采样点。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InkPoint {
    /// 位置（画布 / 窗口坐标）。
    pub position: Point<Pixels>,
    /// 压感（0..=1）；鼠标输入恒为 `1.0`，数位板可带真实压力。
    pub pressure: f32,
}

impl InkPoint {
    /// 以压感 `1.0` 创建采样点。
    pub fn new(position: Point<Pixels>) -> Self {
        Self {
            position,
            pressure: 1.0,
        }
    }

    /// 以指定压感创建采样点。
    pub fn with_pressure(position: Point<Pixels>, pressure: f32) -> Self {
        Self {
            position,
            pressure: pressure.clamp(0.0, 1.0),
        }
    }
}

/// 一条墨迹笔画。
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    /// 采样点序列。
    pub points: Vec<InkPoint>,
    /// 画笔样式。
    pub style: InkStyle,
}

impl Stroke {
    /// 以单个采样点起笔。
    pub fn new(position: Point<Pixels>, style: InkStyle) -> Self {
        Self {
            points: vec![InkPoint::new(position)],
            style,
        }
    }

    /// 采样点数。
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// 是否无采样点。
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// 笔画的包围盒。
    pub fn bounds(&self) -> Option<Bounds<Pixels>> {
        bounds_of(self.points.iter().map(|p| p.position))
    }

    /// 折线总长度（像素）。
    pub fn length(&self) -> f32 {
        self.points
            .windows(2)
            .map(|w| distance(w[0].position, w[1].position))
            .sum()
    }

    /// 到给定点的最近距离（考虑每条线段）。
    pub fn nearest_distance(&self, position: Point<Pixels>) -> f32 {
        match self.points.len() {
            0 => f32::INFINITY,
            1 => distance(self.points[0].position, position),
            _ => self
                .points
                .windows(2)
                .map(|w| point_segment_distance(position, w[0].position, w[1].position))
                .fold(f32::INFINITY, f32::min),
        }
    }

    /// 笔画是否与给定圆相交（橡皮命中判定）。
    pub fn intersects_circle(&self, center: Point<Pixels>, radius: Pixels) -> bool {
        self.nearest_distance(center) <= radius.0
    }

    /// 是否有任一采样点落在多边形内（套索选择判定）。
    pub fn is_inside_polygon(&self, polygon: &[Point<Pixels>]) -> bool {
        self.points
            .iter()
            .any(|p| point_in_polygon(p.position, polygon))
    }

    /// Douglas–Peucker 抽稀（保留拐点，去掉近似共线的中间点）。
    pub fn simplify(&mut self, tolerance: Pixels) {
        self.points = simplify_points(&self.points, tolerance);
    }
}

/// 墨迹层：若干已提交笔画 + 一条进行中的笔画（抬笔前）。
#[derive(Clone, Debug, Default)]
pub struct InkLayer {
    /// 已提交的笔画。
    pub strokes: Vec<Stroke>,
    /// 进行中的笔画（落笔到抬笔之间）。
    pending: Option<Stroke>,
}

impl InkLayer {
    /// 空墨迹层。
    pub fn new() -> Self {
        Self::default()
    }

    /// 进行中的笔画（若有）。
    pub fn pending(&self) -> Option<&Stroke> {
        self.pending.as_ref()
    }

    /// 是否正在落笔（存在进行中的笔画）。
    pub fn is_drawing(&self) -> bool {
        self.pending.is_some()
    }

    /// 是否既无已提交笔画也无进行中笔画。
    pub fn is_empty(&self) -> bool {
        self.strokes.is_empty() && self.pending.is_none()
    }

    /// 落笔：开始一条新笔画（会丢弃已有的进行中笔画）。
    pub fn begin_stroke(&mut self, position: Point<Pixels>, style: InkStyle) {
        self.pending = Some(Stroke::new(position, style));
    }

    /// 带压感落笔。
    pub fn begin_stroke_with_pressure(
        &mut self,
        position: Point<Pixels>,
        pressure: f32,
        style: InkStyle,
    ) {
        let mut stroke = Stroke::new(position, style);
        if let Some(first) = stroke.points.first_mut() {
            first.pressure = pressure.clamp(0.0, 1.0);
        }
        self.pending = Some(stroke);
    }

    /// 继续移动：追加采样点（移动不足 [`MIN_SAMPLE_DISTANCE`] 时忽略）。
    pub fn extend_stroke(&mut self, position: Point<Pixels>) {
        self.extend_stroke_with_pressure(position, 1.0);
    }

    /// 带压感继续移动：追加采样点（移动不足 [`MIN_SAMPLE_DISTANCE`] 时忽略）。
    pub fn extend_stroke_with_pressure(&mut self, position: Point<Pixels>, pressure: f32) {
        if let Some(pending) = &mut self.pending {
            let far_enough = pending
                .points
                .last()
                .is_none_or(|last| distance(last.position, position) >= MIN_SAMPLE_DISTANCE.0);
            if far_enough {
                pending
                    .points
                    .push(InkPoint::with_pressure(position, pressure));
            }
        }
    }

    /// 抬笔：把进行中的笔画并入已提交集合；返回是否真的提交了非空笔画。
    pub fn end_stroke(&mut self) -> bool {
        match self.pending.take() {
            Some(stroke) if !stroke.is_empty() => {
                self.strokes.push(stroke);
                true
            }
            _ => false,
        }
    }

    /// 取消进行中的笔画（不提交）。
    pub fn cancel_stroke(&mut self) {
        self.pending = None;
    }

    /// 撤销最近一条已提交笔画；返回是否有可撤销的笔画。
    pub fn undo(&mut self) -> bool {
        self.strokes.pop().is_some()
    }

    /// 清空全部笔画。
    pub fn clear(&mut self) {
        self.strokes.clear();
        self.pending = None;
    }

    /// 全部笔画的包围盒。
    pub fn bounds(&self) -> Option<Bounds<Pixels>> {
        let mut result: Option<Bounds<Pixels>> = None;
        for stroke in &self.strokes {
            if let Some(stroke_bounds) = stroke.bounds() {
                result = Some(match result {
                    Some(current) => current.union(&stroke_bounds),
                    None => stroke_bounds,
                });
            }
        }
        result
    }

    /// 本层笔画的画布语义：每笔一个 `GraphicsObject` 节点（节点 id = 笔画下标 + 1）。
    ///
    /// 坐标是画布局部坐标；交给 [`InkLayerElement::a11y`] 后由画布桥接为合成子节点，
    /// 屏幕阅读器按阅读顺序播报「笔画 N、点数、工具、选中态」。
    pub fn semantics(&self) -> CanvasSemantics {
        let mut semantics = CanvasSemantics::new();
        for (index, stroke) in self.strokes.iter().enumerate() {
            if let Some(node) = stroke_semantics(index, stroke) {
                semantics.push(node);
            }
        }
        semantics
    }

    /// 橡皮：删除与给定圆相交的笔画，返回删除数量。
    pub fn erase_at(&mut self, center: Point<Pixels>, radius: Pixels) -> usize {
        let before = self.strokes.len();
        self.strokes
            .retain(|stroke| !stroke.intersects_circle(center, radius));
        before - self.strokes.len()
    }

    /// 局部擦除：剪掉落在圆内的**采样点**，笔画被切成前后两段。
    ///
    /// 与 [`Self::erase_at`]（整笔命中即删除）不同，本方法只剪掉圆内的点，笔画
    /// 的其余部分保留；返回被剪掉的点数。
    pub fn erase_local(&mut self, center: Point<Pixels>, radius: Pixels) -> usize {
        let mut removed_points = 0usize;
        let mut kept: Vec<Stroke> = Vec::with_capacity(self.strokes.len());
        for stroke in self.strokes.drain(..) {
            let mut part: Vec<InkPoint> = Vec::new();
            for sample in stroke.points {
                let dx = sample.position.x.0 - center.x.0;
                let dy = sample.position.y.0 - center.y.0;
                if dx * dx + dy * dy <= radius.0 * radius.0 {
                    removed_points += 1;
                    // 命中即断开当前段（残留不足两点视为整段被擦除）。
                    if part.len() >= 2 {
                        kept.push(Stroke {
                            points: std::mem::take(&mut part),
                            style: stroke.style,
                        });
                    } else {
                        part.clear();
                    }
                    continue;
                }
                part.push(sample);
            }
            if part.len() >= 2 {
                kept.push(Stroke {
                    points: part,
                    style: stroke.style,
                });
            }
        }
        self.strokes = kept;
        removed_points
    }

    /// 命中最近的笔画（距离在 `tolerance` 内）；返回笔画索引。
    pub fn hit_test(&self, position: Point<Pixels>, tolerance: Pixels) -> Option<usize> {
        self.strokes
            .iter()
            .enumerate()
            .map(|(index, stroke)| (index, stroke.nearest_distance(position)))
            .filter(|(_, dist)| *dist <= tolerance.0)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(core::cmp::Ordering::Equal))
            .map(|(index, _)| index)
    }

    /// 套索：返回被多边形命中的笔画索引。
    pub fn lasso(&self, polygon: &[Point<Pixels>]) -> Vec<usize> {
        if polygon.len() < 3 {
            return Vec::new();
        }
        self.strokes
            .iter()
            .enumerate()
            .filter(|(_, stroke)| stroke.is_inside_polygon(polygon))
            .map(|(index, _)| index)
            .collect()
    }

    /// 删除给定索引的笔画。
    pub fn remove(&mut self, indices: &[usize]) {
        let mut removed = vec![false; self.strokes.len()];
        for &index in indices {
            if let Some(flag) = removed.get_mut(index) {
                *flag = true;
            }
        }
        let mut cursor = 0usize;
        self.strokes.retain(|_| {
            let keep = !removed.get(cursor).copied().unwrap_or(false);
            cursor += 1;
            keep
        });
    }

    /// 对全部笔画（含进行中）抽稀。
    pub fn simplify(&mut self, tolerance: Pixels) {
        for stroke in &mut self.strokes {
            stroke.simplify(tolerance);
        }
        if let Some(pending) = &mut self.pending {
            pending.simplify(tolerance);
        }
    }
}

/// 墨迹文档中的一层：墨迹 + 显示 / 编辑属性。
#[derive(Clone, Debug)]
pub struct InkDocumentLayer {
    /// 图层名。
    pub name: SharedString,
    /// 该层的墨迹。
    pub layer: InkLayer,
    /// 是否可见（隐藏层不参与绘制、命中与擦除）。
    pub visible: bool,
    /// 是否锁定（锁定层不可编辑，但仍可选择 / 命中）。
    pub locked: bool,
    /// 不透明度（`0..=1`，渲染时乘到笔画颜色上）。
    pub opacity: f32,
}

impl InkDocumentLayer {
    /// 新建可见、未锁定、完全不透明的图层。
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            layer: InkLayer::new(),
            visible: true,
            locked: false,
            opacity: 1.0,
        }
    }

    /// 是否可编辑（可见且未锁定）。
    pub fn is_editable(&self) -> bool {
        self.visible && !self.locked
    }

    /// 把本层的不透明度乘到颜色上（供渲染使用）。
    pub fn fade(&self, color: Hsla) -> Hsla {
        color.opacity(self.opacity)
    }
}

/// 多图层墨迹文档：数组顺序自底向上叠放，编辑落在激活图层。
///
/// 只负责图层结构与编辑路由（可见 / 锁定 / 不透明度 / 叠放顺序）；绘制由应用逐层创建
/// [`InkLayerElement`]（`.opacity(layer.opacity)`）叠加，隐藏层跳过即可。
#[derive(Clone, Debug)]
pub struct InkDocument {
    /// 图层（自底向上）。
    layers: Vec<InkDocumentLayer>,
    /// 激活图层下标。
    active: usize,
}

impl Default for InkDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl InkDocument {
    /// 新建只含一层「图层 1」的文档。
    pub fn new() -> Self {
        Self {
            layers: vec![InkDocumentLayer::new("图层 1")],
            active: 0,
        }
    }

    /// 全部图层（自底向上）。
    pub fn layers(&self) -> &[InkDocumentLayer] {
        &self.layers
    }

    /// 图层数量。
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// 激活图层下标。
    pub fn active_index(&self) -> usize {
        self.active
    }

    /// 激活图层。
    pub fn active(&self) -> &InkDocumentLayer {
        &self.layers[self.active]
    }

    /// 按索引取图层。
    pub fn layer(&self, index: usize) -> Option<&InkDocumentLayer> {
        self.layers.get(index)
    }

    /// 可见图层（自底向上，带下标），供应用逐层绘制。
    pub fn visible_layers(&self) -> impl Iterator<Item = (usize, &InkDocumentLayer)> {
        self.layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| layer.visible)
    }

    /// 切换激活图层。
    pub fn set_active(&mut self, index: usize) -> bool {
        if index >= self.layers.len() || index == self.active {
            return false;
        }
        self.active = index;
        true
    }

    /// 在最上方新增一层并激活，返回其下标。
    pub fn add_layer(&mut self, name: impl Into<SharedString>) -> usize {
        self.layers.push(InkDocumentLayer::new(name));
        self.active = self.layers.len() - 1;
        self.active
    }

    /// 删除图层（至少保留一层），并修正激活下标。
    pub fn remove_layer(&mut self, index: usize) -> bool {
        if index >= self.layers.len() || self.layers.len() <= 1 {
            return false;
        }
        self.layers.remove(index);
        if index < self.active {
            self.active -= 1;
        } else if index == self.active {
            self.active = self.active.min(self.layers.len() - 1);
        }
        true
    }

    /// 调整叠放顺序（`to` 为移除后的目标下标，越界自动夹取）。
    pub fn move_layer(&mut self, from: usize, to: usize) -> bool {
        if from >= self.layers.len() {
            return false;
        }
        let to = to.min(self.layers.len() - 1);
        if from == to {
            return false;
        }
        let layer = self.layers.remove(from);
        self.layers.insert(to, layer);
        // 激活下标跟随被移动的图层。
        if self.active == from {
            self.active = to;
        } else if from < self.active && to >= self.active {
            self.active -= 1;
        } else if from > self.active && to <= self.active {
            self.active += 1;
        }
        true
    }

    /// 设置图层可见性。
    pub fn set_visible(&mut self, index: usize, visible: bool) -> bool {
        match self.layers.get_mut(index) {
            Some(layer) if layer.visible != visible => {
                layer.visible = visible;
                true
            }
            _ => false,
        }
    }

    /// 设置图层锁定。
    pub fn set_locked(&mut self, index: usize, locked: bool) -> bool {
        match self.layers.get_mut(index) {
            Some(layer) if layer.locked != locked => {
                layer.locked = locked;
                true
            }
            _ => false,
        }
    }

    /// 设置图层不透明度（夹取到 `0..=1`，非有限值按 0 处理）。
    pub fn set_opacity(&mut self, index: usize, opacity: f32) -> bool {
        let opacity = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            0.0
        };
        match self.layers.get_mut(index) {
            Some(layer) if layer.opacity != opacity => {
                layer.opacity = opacity;
                true
            }
            _ => false,
        }
    }

    /// 在激活图层落笔（隐藏 / 锁定时返回 `false`）。
    pub fn begin_stroke(&mut self, position: Point<Pixels>, style: InkStyle) -> bool {
        let layer = &mut self.layers[self.active];
        if !layer.is_editable() {
            return false;
        }
        layer.layer.begin_stroke(position, style);
        true
    }

    /// 在激活图层落笔并记录压感。
    pub fn begin_stroke_with_pressure(
        &mut self,
        position: Point<Pixels>,
        pressure: f32,
        style: InkStyle,
    ) -> bool {
        let layer = &mut self.layers[self.active];
        if !layer.is_editable() {
            return false;
        }
        layer
            .layer
            .begin_stroke_with_pressure(position, pressure, style);
        true
    }

    /// 延长激活图层的进行中笔画。
    pub fn extend_stroke(&mut self, position: Point<Pixels>) -> bool {
        self.extend_stroke_with_pressure(position, 1.0)
    }

    /// 带压感延长激活图层的进行中笔画。
    pub fn extend_stroke_with_pressure(&mut self, position: Point<Pixels>, pressure: f32) -> bool {
        let layer = &mut self.layers[self.active];
        if !layer.is_editable() || !layer.layer.is_drawing() {
            return false;
        }
        layer.layer.extend_stroke_with_pressure(position, pressure);
        true
    }

    /// 结束激活图层的进行中笔画（是否有笔画被提交）。
    pub fn end_stroke(&mut self) -> bool {
        let layer = &mut self.layers[self.active];
        layer.is_editable() && layer.layer.end_stroke()
    }

    /// 丢弃激活图层的进行中笔画。
    pub fn cancel_stroke(&mut self) -> bool {
        let layer = &mut self.layers[self.active];
        if !layer.is_editable() || !layer.layer.is_drawing() {
            return false;
        }
        layer.layer.cancel_stroke();
        true
    }

    /// 撤销激活图层最后一笔（锁定 / 隐藏时无效）。
    pub fn undo(&mut self) -> bool {
        let layer = &mut self.layers[self.active];
        layer.is_editable() && layer.layer.undo()
    }

    /// 清空激活图层（锁定 / 隐藏时无效）。
    pub fn clear(&mut self) -> bool {
        let layer = &mut self.layers[self.active];
        if !layer.is_editable() || layer.layer.is_empty() {
            return false;
        }
        layer.layer.clear();
        true
    }

    /// 对所有可编辑图层按半径擦除，返回删除的笔画总数。
    pub fn erase_at(&mut self, center: Point<Pixels>, radius: Pixels) -> usize {
        self.layers
            .iter_mut()
            .filter(|layer| layer.is_editable())
            .map(|layer| layer.layer.erase_at(center, radius))
            .sum()
    }

    /// 局部擦除（多层文档）：逐个可编辑图层剪掉圆内的采样点，返回剪掉的点数。
    pub fn erase_local(&mut self, center: Point<Pixels>, radius: Pixels) -> usize {
        self.layers
            .iter_mut()
            .filter(|layer| layer.is_editable())
            .map(|layer| layer.layer.erase_local(center, radius))
            .sum()
    }

    /// 命中最近的笔画（自顶向下取第一个可见层），返回 `(图层下标, 笔画下标)`。
    pub fn hit_test(&self, position: Point<Pixels>, tolerance: Pixels) -> Option<(usize, usize)> {
        self.layers
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, layer)| layer.visible)
            .find_map(|(index, layer)| {
                layer
                    .layer
                    .hit_test(position, tolerance)
                    .map(|stroke| (index, stroke))
            })
    }

    /// 套索选择：返回所有可见层中被框住的 `(图层下标, 笔画下标)`。
    pub fn lasso(&self, polygon: &[Point<Pixels>]) -> Vec<(usize, usize)> {
        self.layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| layer.visible)
            .flat_map(|(index, layer)| {
                layer
                    .layer
                    .lasso(polygon)
                    .into_iter()
                    .map(move |stroke| (index, stroke))
            })
            .collect()
    }

    /// 删除给定 `(图层下标, 笔画下标)` 的笔画（隐藏 / 锁定层跳过）。
    pub fn remove(&mut self, indices: &[(usize, usize)]) {
        let mut per_layer: Vec<Vec<usize>> = vec![Vec::new(); self.layers.len()];
        for &(layer, stroke) in indices {
            if let Some(slot) = per_layer.get_mut(layer) {
                slot.push(stroke);
            }
        }
        for (layer, strokes) in self.layers.iter_mut().zip(per_layer) {
            if layer.is_editable() && !strokes.is_empty() {
                layer.layer.remove(&strokes);
            }
        }
    }

    /// 所有可见图层的包围盒并集。
    pub fn bounds(&self) -> Option<Bounds<Pixels>> {
        self.layers
            .iter()
            .filter(|layer| layer.visible)
            .filter_map(|layer| layer.layer.bounds())
            .reduce(|acc, bounds| {
                let left = acc.origin.x.min(bounds.origin.x);
                let top = acc.origin.y.min(bounds.origin.y);
                let right =
                    (acc.origin.x + acc.size.width).max(bounds.origin.x + bounds.size.width);
                let bottom =
                    (acc.origin.y + acc.size.height).max(bounds.origin.y + bounds.size.height);
                Bounds::new(point(left, top), size(right - left, bottom - top))
            })
    }

    /// 对激活图层抽稀。
    pub fn simplify(&mut self, tolerance: Pixels) {
        self.layers[self.active].layer.simplify(tolerance);
    }
}

/// 墨迹层绘制元素：逐笔画描边，可高亮选中笔画并绘制套索选区。
#[derive(IntoElement)]
pub struct InkLayerElement {
    /// 待绘制的墨迹层。
    layer: InkLayer,
    /// 高亮的笔画索引（套索选中）。
    highlight: Vec<usize>,
    /// 进行中的套索多边形。
    lasso: Vec<Point<Pixels>>,
    /// 整层不透明度（`0..=1`）。
    opacity: f32,
    /// 无障碍配置：元素 id + 画布名称（开启后每笔发布一个语义节点）。
    a11y: Option<(ElementId, SharedString)>,
}

impl InkLayerElement {
    /// 创建绘制元素。
    pub fn new(layer: InkLayer) -> Self {
        Self {
            layer,
            highlight: Vec::new(),
            lasso: Vec::new(),
            opacity: 1.0,
            a11y: None,
        }
    }

    /// 把笔画暴露给辅助技术（每笔一个 `GraphicsObject` 节点，播报点数 / 工具 / 选中态）。
    ///
    /// `id` 是画布元素自身的标识（合成子节点的容器）；同一图层跨帧应使用同一 id。
    pub fn a11y(mut self, id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        self.a11y = Some((id.into(), label.into()));
        self
    }

    /// 设置整层不透明度（夹取到 `0..=1`，用于多图层文档的图层透明度）。
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self
    }

    /// 设置要高亮的笔画索引。
    pub fn highlight(mut self, indices: Vec<usize>) -> Self {
        self.highlight = indices;
        self
    }

    /// 设置进行中的套索多边形。
    pub fn lasso(mut self, polygon: Vec<Point<Pixels>>) -> Self {
        self.lasso = polygon;
        self
    }
}

impl RenderOnce for InkLayerElement {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let accent = cx.theme().primary;
        let layer = self.layer;
        let highlight = self.highlight;
        let lasso = self.lasso;
        let opacity = self.opacity;
        let a11y = self.a11y;

        // 无障碍语义在绘制前建立（笔画顺序 = 节点顺序），高亮笔画标记为选中。
        let mut semantics = a11y.as_ref().map(|_| layer.semantics());
        if let Some(semantics) = semantics.as_mut() {
            for &index in &highlight {
                if let Some(node) = semantics.get_mut(index as u64 + 1) {
                    node.set_selected(true);
                }
            }
        }

        let element = canvas(
            |_bounds, _window, _cx| {},
            move |_bounds, _state, window, _cx| {
                // 先画普通笔画，再画高亮笔画，保证选中项在最上层。
                for (index, stroke) in layer.strokes.iter().enumerate() {
                    if highlight.contains(&index) {
                        continue;
                    }
                    paint_stroke(
                        window,
                        &stroke.points,
                        stroke.style.width,
                        stroke.style.effective_color().opacity(opacity),
                    );
                }
                for &index in &highlight {
                    if let Some(stroke) = layer.strokes.get(index) {
                        paint_stroke(
                            window,
                            &stroke.points,
                            stroke.style.width,
                            accent.opacity(opacity),
                        );
                    }
                }
                if let Some(pending) = layer.pending() {
                    paint_stroke(
                        window,
                        &pending.points,
                        pending.style.width,
                        pending.style.effective_color().opacity(opacity),
                    );
                }
                if let Some(path) = polygon_path(&lasso, LASSO_STROKE_WIDTH) {
                    window.paint_path(path, accent);
                }
            },
        )
        .size_full();

        match (a11y, semantics) {
            (Some((id, label)), Some(semantics)) => element
                .id(id)
                .a11y(Role::Canvas, semantics)
                .a11y_label(label),
            _ => element,
        }
    }
}

/// 单条笔画的语义节点：包围盒按笔宽外扩（单点 / 细笔画的零尺寸盒也能被命中与排序）。
fn stroke_semantics(index: usize, stroke: &Stroke) -> Option<CanvasNode> {
    let bounds = stroke.bounds()?;
    let pad = stroke.style.width * 0.5;
    let bounds = Bounds::new(
        point(bounds.origin.x - pad, bounds.origin.y - pad),
        size(
            bounds.size.width + stroke.style.width,
            bounds.size.height + stroke.style.width,
        ),
    );
    let tool = if stroke.style.is_highlighter() {
        "荧光笔"
    } else {
        "钢笔"
    };
    Some(
        CanvasNode::new(
            index as u64 + 1,
            Role::GraphicsObject,
            format!("笔画 {}", index + 1),
            bounds,
        )
        .with_value(format!("{} 个点", stroke.points.len()))
        .with_description(format!("{tool} · 宽 {:.0}px", stroke.style.width.as_f32())),
    )
}

/// 把采样点平滑成路径：中点二次贝塞尔（每段以中点为界，控制点取原采样点）。
pub fn stroke_path(points: &[InkPoint], width: Pixels) -> Option<Path<Pixels>> {
    if points.is_empty() {
        return None;
    }
    let options = StrokeOptions::default()
        .with_line_width(width.0)
        .with_line_cap(LineCap::Round)
        .with_line_join(LineJoin::Round);
    let mut builder = PathBuilder::stroke(width).with_style(PathStyle::Stroke(options));
    let first = points[0].position;
    builder.move_to(first);
    match points.len() {
        // 单点：画一个极短线段，配合圆头渲染成圆点。
        1 => builder.line_to(point(px(first.x.0 + 0.01), first.y)),
        2 => builder.line_to(points[1].position),
        _ => {
            // 中点二次贝塞尔平滑：以每个中间采样点为控制点，画到与下一点的中点。
            for i in 1..points.len() - 1 {
                let control = points[i].position;
                let next = points[i + 1].position;
                builder.curve_to(midpoint(control, next), control);
            }
            builder.line_to(points[points.len() - 1].position);
        }
    }
    builder.build().ok()
}

/// Catmull-Rom 曲线平滑路径（三次贝塞尔拟合）。
///
/// 与 [`stroke_path`] 的中点二次贝塞尔近似相比，Catmull-Rom 穿过全部采样点
/// （控制点 `p1 + (p2 - p0) / 6`、`p2 - (p3 - p1) / 6`），曲线更贴合原始笔迹；
/// 两端按端点复制处理（`p0 = p1` / `p3 = p2`）。
pub fn catmull_rom_path(points: &[InkPoint], width: Pixels) -> Option<Path<Pixels>> {
    if points.is_empty() {
        return None;
    }
    let options = StrokeOptions::default()
        .with_line_width(width.0)
        .with_line_cap(LineCap::Round)
        .with_line_join(LineJoin::Round);
    let mut builder = PathBuilder::stroke(width).with_style(PathStyle::Stroke(options));
    let first = points[0].position;
    builder.move_to(first);
    match points.len() {
        // 单点：画一个极短线段，配合圆头渲染成圆点。
        1 => {
            builder.line_to(point(px(first.x.0 + 0.01), first.y));
        }
        2 => {
            builder.line_to(points[1].position);
        }
        _ => {
            for i in 0..points.len() - 1 {
                // 前后控制点：首尾复制端点。
                let p0 = if i == 0 {
                    points[0].position
                } else {
                    points[i - 1].position
                };
                let p1 = points[i].position;
                let p2 = points[i + 1].position;
                let p3 = if i + 2 < points.len() {
                    points[i + 2].position
                } else {
                    p2
                };
                let c1 = point(
                    px(p1.x.0 + (p2.x.0 - p0.x.0) / 6.0),
                    px(p1.y.0 + (p2.y.0 - p0.y.0) / 6.0),
                );
                let c2 = point(
                    px(p2.x.0 - (p3.x.0 - p1.x.0) / 6.0),
                    px(p2.y.0 - (p3.y.0 - p1.y.0) / 6.0),
                );
                builder.cubic_bezier_to(p2, c1, c2);
            }
        }
    }
    builder.build().ok()
}

/// 压感曲线拟合：把原始压感按幂曲线整形。
///
/// `gamma < 1` 让轻压区更饱满（笔画更粗），`gamma > 1` 让重压区更突出；
/// 非有限值按满压（1.0）处理，曲线结果夹取到 0..=1。
pub fn smooth_pressure(pressure: f32, gamma: f32) -> f32 {
    if !pressure.is_finite() {
        return 1.0;
    }
    let pressure = pressure.clamp(0.0, 1.0);
    if !gamma.is_finite() || gamma <= 0.0 {
        return pressure;
    }
    pressure.powf(gamma).clamp(0.0, 1.0)
}

/// 按压感缩放的笔画宽度：`pressure` 0..=1 线性映射到基础宽度的
/// [`MIN_PRESSURE_SCALE`]..=1 倍（非有限值按满压处理）。
pub fn pressure_width(base: Pixels, pressure: f32) -> Pixels {
    let pressure = if pressure.is_finite() {
        pressure.clamp(0.0, 1.0)
    } else {
        1.0
    };
    base * (MIN_PRESSURE_SCALE + (1.0 - MIN_PRESSURE_SCALE) * pressure)
}

/// 逐段路径：每段按两端压感均值缩放宽度（圆头圆角衔接）。
///
/// 压感全为 `1.0`（鼠标输入）或点数不超过 2 时退化为单条平滑的 [`stroke_path`]，
/// 不产生额外开销；带压感时按线段拆分，相邻段靠圆头自然衔接。
pub fn stroke_paths(points: &[InkPoint], width: Pixels) -> Vec<Path<Pixels>> {
    if points.is_empty() {
        return Vec::new();
    }
    let uniform = points
        .iter()
        .all(|point| (point.pressure - 1.0).abs() < f32::EPSILON);
    if points.len() <= 2 || uniform {
        return stroke_path(points, width).into_iter().collect();
    }
    let mut paths = Vec::with_capacity(points.len() - 1);
    for pair in points.windows(2) {
        let segment = pressure_width(width, (pair[0].pressure + pair[1].pressure) * 0.5);
        let options = StrokeOptions::default()
            .with_line_width(segment.0)
            .with_line_cap(LineCap::Round)
            .with_line_join(LineJoin::Round);
        let mut builder = PathBuilder::stroke(segment).with_style(PathStyle::Stroke(options));
        builder.move_to(pair[0].position);
        builder.line_to(pair[1].position);
        if let Ok(path) = builder.build() {
            paths.push(path);
        }
    }
    paths
}

/// 绘制一条笔画（带压感时按段绘制，否则一条平滑路径）。
///
/// 半透明笔画（荧光笔、整层不透明度）仍走等宽单路径：逐段圆头在接缝处叠加会加深颜色，
/// 出现「串珠」；等宽荧光笔也更接近真实马克笔。
fn paint_stroke(window: &mut Window, points: &[InkPoint], width: Pixels, color: Hsla) {
    let paths: Vec<Path<Pixels>> = if color.a >= 1.0 {
        stroke_paths(points, width)
    } else {
        stroke_path(points, width).into_iter().collect()
    };
    for path in paths {
        window.paint_path(path, color);
    }
}

/// 构造套索多边形路径（闭合、虚线）。
fn polygon_path(polygon: &[Point<Pixels>], width: Pixels) -> Option<Path<Pixels>> {
    if polygon.len() < 2 {
        return None;
    }
    let mut builder = PathBuilder::stroke(width).dash_array(&[px(4.0), px(3.0)]);
    builder.move_to(polygon[0]);
    for p in &polygon[1..] {
        builder.line_to(*p);
    }
    builder.line_to(polygon[0]);
    builder.build().ok()
}

/// 点到线段的最短距离（像素）。
pub fn point_segment_distance(p: Point<Pixels>, a: Point<Pixels>, b: Point<Pixels>) -> f32 {
    let abx = b.x.0 - a.x.0;
    let aby = b.y.0 - a.y.0;
    let len2 = abx * abx + aby * aby;
    let t = if len2 <= f32::EPSILON {
        0.0
    } else {
        (((p.x.0 - a.x.0) * abx + (p.y.0 - a.y.0) * aby) / len2).clamp(0.0, 1.0)
    };
    let cx = a.x.0 + abx * t;
    let cy = a.y.0 + aby * t;
    let dx = p.x.0 - cx;
    let dy = p.y.0 - cy;
    (dx * dx + dy * dy).sqrt()
}

/// 射线法判断点是否在多边形内。
pub fn point_in_polygon(p: Point<Pixels>, polygon: &[Point<Pixels>]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = (polygon[i].x.0, polygon[i].y.0);
        let (xj, yj) = (polygon[j].x.0, polygon[j].y.0);
        if (yi > p.y.0) != (yj > p.y.0) {
            let x_at = (xj - xi) * (p.y.0 - yi) / (yj - yi) + xi;
            if p.x.0 < x_at {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// 多边形包围盒。
pub fn polygon_bounds(polygon: &[Point<Pixels>]) -> Option<Bounds<Pixels>> {
    bounds_of(polygon.iter().copied())
}

/// Douglas–Peucker 抽稀：保留首尾与偏离超过 `tolerance` 的关键点。
pub fn simplify_points(points: &[InkPoint], tolerance: Pixels) -> Vec<InkPoint> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let tol = tolerance.0.max(0.0);
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut stack = vec![(0usize, points.len() - 1)];
    while let Some((first, last)) = stack.pop() {
        if last <= first + 1 {
            continue;
        }
        let a = points[first].position;
        let b = points[last].position;
        let mut max_distance = 0.0f32;
        let mut farthest = first;
        for (i, point) in points.iter().enumerate().take(last).skip(first + 1) {
            let d = point_segment_distance(point.position, a, b);
            if d > max_distance {
                max_distance = d;
                farthest = i;
            }
        }
        if max_distance > tol {
            keep[farthest] = true;
            stack.push((first, farthest));
            stack.push((farthest, last));
        }
    }
    points
        .iter()
        .zip(&keep)
        .filter(|(_, keep)| **keep)
        .map(|(point, _)| *point)
        .collect()
}

/// 计算一组点的包围盒。
fn bounds_of(points: impl Iterator<Item = Point<Pixels>>) -> Option<Bounds<Pixels>> {
    let mut iter = points;
    let first = iter.next()?;
    let (mut min_x, mut min_y) = (first.x, first.y);
    let (mut max_x, mut max_y) = (first.x, first.y);
    for p in iter {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    Some(Bounds::new(
        point(min_x, min_y),
        size(max_x - min_x, max_y - min_y),
    ))
}

/// 两点距离（像素）。
fn distance(a: Point<Pixels>, b: Point<Pixels>) -> f32 {
    let dx = b.x.0 - a.x.0;
    let dy = b.y.0 - a.y.0;
    (dx * dx + dy * dy).sqrt()
}

/// 两点中点。
fn midpoint(a: Point<Pixels>, b: Point<Pixels>) -> Point<Pixels> {
    point(px((a.x.0 + b.x.0) / 2.0), px((a.y.0 + b.y.0) / 2.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hsla;

    fn pen() -> InkStyle {
        InkStyle::pen(hsla(0.6, 0.8, 0.4, 1.0), px(3.0))
    }

    fn pt(x: f32, y: f32) -> Point<Pixels> {
        point(px(x), px(y))
    }

    #[test]
    fn draw_commit_and_undo() {
        let mut layer = InkLayer::new();
        assert!(layer.is_empty());
        layer.begin_stroke(pt(0.0, 0.0), pen());
        assert!(layer.is_drawing());
        layer.extend_stroke(pt(5.0, 0.0));
        layer.extend_stroke(pt(10.0, 0.0));
        assert!(layer.end_stroke());
        assert!(!layer.is_drawing());
        assert_eq!(layer.strokes.len(), 1);
        assert_eq!(layer.strokes[0].len(), 3);
        assert!(layer.undo());
        assert!(layer.is_empty());
        assert!(!layer.undo());
    }

    #[test]
    fn min_sample_distance_filters_duplicates() {
        let mut layer = InkLayer::new();
        layer.begin_stroke(pt(0.0, 0.0), pen());
        layer.extend_stroke(pt(0.1, 0.0)); // 太近，忽略
        layer.extend_stroke(pt(2.0, 0.0));
        assert_eq!(layer.pending().unwrap().len(), 2);
    }

    #[test]
    fn cancel_discards_pending() {
        let mut layer = InkLayer::new();
        layer.begin_stroke(pt(0.0, 0.0), pen());
        layer.extend_stroke(pt(4.0, 0.0));
        layer.cancel_stroke();
        assert!(layer.is_empty());
        assert_eq!(layer.strokes.len(), 0);
    }

    #[test]
    fn erase_at_removes_nearby_strokes() {
        let mut layer = InkLayer::new();
        layer.begin_stroke(pt(0.0, 0.0), pen());
        layer.extend_stroke(pt(10.0, 0.0));
        layer.end_stroke();
        layer.begin_stroke(pt(100.0, 100.0), pen());
        layer.extend_stroke(pt(110.0, 100.0));
        layer.end_stroke();

        assert_eq!(layer.erase_at(pt(5.0, 1.0), px(3.0)), 1);
        assert_eq!(layer.strokes.len(), 1);
        assert_eq!(layer.strokes[0].points[0].position, pt(100.0, 100.0));
    }

    /// 局部擦除：圆内采样点被剪掉，笔画切成前后两段（整笔擦除不受影响）。
    #[test]
    fn erase_local_cuts_stroke_into_parts() {
        let mut layer = InkLayer::new();
        layer.begin_stroke(pt(0.0, 0.0), pen());
        layer.extend_stroke(pt(10.0, 0.0));
        layer.extend_stroke(pt(20.0, 0.0));
        layer.extend_stroke(pt(30.0, 0.0));
        layer.extend_stroke(pt(40.0, 0.0));
        layer.end_stroke();
        assert_eq!(layer.strokes.len(), 1);

        // 剪掉中段（x = 20 附近）→ 笔画变成两段。
        let removed = layer.erase_local(pt(20.0, 0.0), px(2.0));
        assert_eq!(removed, 1);
        assert_eq!(layer.strokes.len(), 2);
        assert_eq!(layer.strokes[0].points.len(), 2);
        assert_eq!(layer.strokes[1].points.len(), 2);
        // 段落的端点分别是擦除区间的前后。
        assert_eq!(
            layer.strokes[0].points.last().unwrap().position,
            pt(10.0, 0.0)
        );
        assert_eq!(layer.strokes[1].points[0].position, pt(30.0, 0.0));

        // 整笔擦除仍按笔画删除。
        assert_eq!(layer.erase_at(pt(5.0, 0.0), px(2.0)), 1);
    }
    #[test]
    fn lasso_selects_and_removes() {
        let mut layer = InkLayer::new();
        layer.begin_stroke(pt(10.0, 10.0), pen());
        layer.extend_stroke(pt(20.0, 10.0));
        layer.end_stroke();
        layer.begin_stroke(pt(80.0, 80.0), pen());
        layer.extend_stroke(pt(90.0, 80.0));
        layer.end_stroke();

        let square = [pt(0.0, 0.0), pt(40.0, 0.0), pt(40.0, 40.0), pt(0.0, 40.0)];
        assert_eq!(layer.lasso(&square), vec![0]);
        layer.remove(&[0]);
        assert_eq!(layer.strokes.len(), 1);
        assert_eq!(layer.strokes[0].points[0].position, pt(80.0, 80.0));
    }

    #[test]
    fn hit_test_finds_nearest() {
        let mut layer = InkLayer::new();
        layer.begin_stroke(pt(0.0, 0.0), pen());
        layer.extend_stroke(pt(10.0, 0.0));
        layer.end_stroke();
        assert_eq!(layer.hit_test(pt(5.0, 1.0), px(2.0)), Some(0));
        assert_eq!(layer.hit_test(pt(5.0, 10.0), px(2.0)), None);
    }

    #[test]
    fn bounds_union_of_strokes() {
        let mut layer = InkLayer::new();
        layer.begin_stroke(pt(0.0, 0.0), pen());
        layer.extend_stroke(pt(10.0, 20.0));
        layer.end_stroke();
        layer.begin_stroke(pt(-5.0, 5.0), pen());
        layer.end_stroke();
        let bounds = layer.bounds().unwrap();
        assert_eq!(bounds.origin, pt(-5.0, 0.0));
        assert_eq!(bounds.size, crate::size(px(15.0), px(20.0)));
    }

    #[test]
    fn simplify_drops_collinear_points() {
        let points: Vec<InkPoint> = (0..=10).map(|i| InkPoint::new(pt(i as f32, 0.0))).collect();
        let simplified = simplify_points(&points, px(0.5));
        assert_eq!(simplified.len(), 2);
        assert_eq!(simplified[0].position, pt(0.0, 0.0));
        assert_eq!(simplified[1].position, pt(10.0, 0.0));

        // 有拐点时保留拐点。
        let bend = vec![
            InkPoint::new(pt(0.0, 0.0)),
            InkPoint::new(pt(5.0, 0.0)),
            InkPoint::new(pt(5.0, 5.0)),
        ];
        assert_eq!(simplify_points(&bend, px(0.5)).len(), 3);
    }

    #[test]
    fn geometry_helpers() {
        assert!(
            (point_segment_distance(pt(0.0, 3.0), pt(-10.0, 0.0), pt(10.0, 0.0)) - 3.0).abs()
                < 1e-4
        );
        // 端点之外取到端点的距离。
        assert!(
            (point_segment_distance(pt(20.0, 0.0), pt(0.0, 0.0), pt(10.0, 0.0)) - 10.0).abs()
                < 1e-4
        );

        let square = [pt(0.0, 0.0), pt(10.0, 0.0), pt(10.0, 10.0), pt(0.0, 10.0)];
        assert!(point_in_polygon(pt(5.0, 5.0), &square));
        assert!(!point_in_polygon(pt(15.0, 5.0), &square));
        assert!(!point_in_polygon(pt(5.0, 5.0), &square[..2]));

        let bounds = polygon_bounds(&square).unwrap();
        assert_eq!(bounds.origin, pt(0.0, 0.0));
        assert_eq!(bounds.size, crate::size(px(10.0), px(10.0)));
        assert!(polygon_bounds(&[]).is_none());
    }

    #[test]
    fn stroke_path_and_highlighter() {
        assert!(stroke_path(&[], px(2.0)).is_none());
        let single = stroke_path(&[InkPoint::new(pt(1.0, 1.0))], px(2.0));
        assert!(single.is_some());
        let many: Vec<InkPoint> = (0..5)
            .map(|i| InkPoint::new(pt(i as f32, i as f32)))
            .collect();
        assert!(stroke_path(&many, px(2.0)).is_some());

        let base = hsla(0.2, 0.9, 0.5, 1.0);
        let highlighter = InkStyle::highlighter(base, px(12.0));
        assert!(highlighter.is_highlighter());
        assert!(highlighter.effective_color().a < 1.0);
        assert_eq!(pen().effective_color().a, 1.0);
    }

    #[test]
    fn pressure_is_clamped() {
        let p = InkPoint::with_pressure(pt(0.0, 0.0), 2.0);
        assert_eq!(p.pressure, 1.0);
        let p = InkPoint::with_pressure(pt(0.0, 0.0), -1.0);
        assert_eq!(p.pressure, 0.0);
        assert_eq!(InkPoint::new(pt(0.0, 0.0)).pressure, 1.0);
    }

    /// Catmull-Rom 平滑：曲线穿过全部采样点（含首尾端点复制处理）。
    #[test]
    fn catmull_rom_path_covers_all_points() {
        assert!(catmull_rom_path(&[], px(2.0)).is_none());
        let single = catmull_rom_path(&[InkPoint::new(pt(1.0, 1.0))], px(2.0));
        assert!(single.is_some());
        let pair = catmull_rom_path(
            &[InkPoint::new(pt(0.0, 0.0)), InkPoint::new(pt(1.0, 1.0))],
            px(2.0),
        );
        assert!(pair.is_some());
        let many: Vec<InkPoint> = (0..5)
            .map(|i| InkPoint::new(pt(i as f32, (i * i) as f32)))
            .collect();
        assert!(catmull_rom_path(&many, px(2.0)).is_some());
    }

    /// 压感曲线拟合：gamma 整形 + 越界 / 非有限值处理。
    #[test]
    fn pressure_curve_fitting() {
        // gamma = 1 时原样。
        assert!((smooth_pressure(0.5, 1.0) - 0.5).abs() < 1e-6);
        // gamma < 1 时轻压区更饱满（值更大）。
        assert!(smooth_pressure(0.5, 0.5) > 0.5);
        // gamma > 1 时轻压区更细（值更小）。
        assert!(smooth_pressure(0.5, 2.0) < 0.5);
        // 越界夹取；非有限值按满压。
        assert_eq!(smooth_pressure(2.0, 1.0), 1.0);
        assert_eq!(smooth_pressure(-1.0, 1.0), 0.0);
        assert_eq!(smooth_pressure(f32::NAN, 1.0), 1.0);
        // 非法 gamma 退化为原值。
        assert!((smooth_pressure(0.4, 0.0) - 0.4).abs() < 1e-6);
    }
    /// 压感宽度映射与逐段拆分：满压退化为单条平滑路径。
    #[test]
    fn pressure_width_and_segmented_paths() {
        // 满压 = 基础宽度，零压 = 下限，越界 / 非有限值被夹取。
        assert_eq!(pressure_width(px(10.0), 1.0), px(10.0));
        assert_eq!(pressure_width(px(10.0), 0.0), px(10.0 * MIN_PRESSURE_SCALE));
        assert_eq!(pressure_width(px(10.0), 2.0), px(10.0));
        assert_eq!(
            pressure_width(px(10.0), -3.0),
            px(10.0 * MIN_PRESSURE_SCALE)
        );
        assert_eq!(pressure_width(px(10.0), f32::NAN), px(10.0));

        let uniform: Vec<InkPoint> = (0..5)
            .map(|i| InkPoint::new(pt(i as f32 * 4.0, 0.0)))
            .collect();
        assert_eq!(stroke_paths(&uniform, px(2.0)).len(), 1);
        assert!(stroke_paths(&[], px(2.0)).is_empty());
        assert_eq!(stroke_paths(&uniform[..1], px(2.0)).len(), 1);

        // 带压感：逐段一条路径，段数 = 点数 - 1。
        let tapered: Vec<InkPoint> = (0..4)
            .map(|i| InkPoint::with_pressure(pt(i as f32 * 4.0, 0.0), 0.2 + 0.2 * i as f32))
            .collect();
        let paths = stroke_paths(&tapered, px(8.0));
        assert_eq!(paths.len(), 3);

        // 压感继续移动会保留压力值，普通移动恒为满压。
        let mut layer = InkLayer::new();
        layer.begin_stroke(pt(0.0, 0.0), pen());
        layer.extend_stroke_with_pressure(pt(5.0, 0.0), 0.25);
        layer.extend_stroke(pt(10.0, 0.0));
        let pending = layer.pending().expect("笔画应处于进行中");
        assert_eq!(pending.points[1].pressure, 0.25);
        assert_eq!(pending.points[2].pressure, 1.0);
    }

    /// 在给定图层画一条水平短线（起点 `x`）。
    fn draw_on(doc: &mut InkDocument, layer: usize, x: f32) {
        doc.set_active(layer);
        assert!(doc.begin_stroke(pt(x, 0.0), pen()));
        assert!(doc.extend_stroke(pt(x + 10.0, 0.0)));
        assert!(doc.end_stroke());
    }

    /// 新文档只有一层「图层 1」且激活。
    #[test]
    fn document_starts_with_one_layer() {
        let doc = InkDocument::new();
        assert_eq!(doc.layer_count(), 1);
        assert_eq!(doc.active_index(), 0);
        assert_eq!(doc.active().name.as_ref(), "图层 1");
        assert!(doc.active().is_editable());
        assert_eq!(doc.visible_layers().count(), 1);
        assert!(doc.bounds().is_none());
        assert_eq!(InkDocument::default().layer_count(), 1);
    }

    /// 落笔与编辑只作用在激活图层。
    #[test]
    fn drawing_and_editing_follow_active_layer() {
        let mut doc = InkDocument::new();
        let index = doc.add_layer("批注");
        assert_eq!(index, 1);
        assert_eq!(doc.active_index(), 1);

        draw_on(&mut doc, 1, 0.0);
        assert_eq!(doc.layer(0).unwrap().layer.strokes.len(), 0);
        assert_eq!(doc.layer(1).unwrap().layer.strokes.len(), 1);

        // 撤销只动激活层。
        assert!(doc.undo());
        assert!(doc.layer(1).unwrap().layer.is_empty());

        draw_on(&mut doc, 0, 20.0);
        assert_eq!(doc.layer(0).unwrap().layer.strokes.len(), 1);
        assert!(doc.clear());
        assert!(doc.layer(0).unwrap().layer.is_empty());
        assert!(!doc.clear());

        // 锁定 / 隐藏后不可落笔。
        assert!(doc.set_active(1));
        assert!(doc.set_locked(1, true));
        assert!(!doc.begin_stroke(pt(0.0, 0.0), pen()));
        assert!(!doc.extend_stroke(pt(5.0, 0.0)));
        assert!(!doc.end_stroke());
        assert!(!doc.cancel_stroke());
        assert!(!doc.undo());
        assert!(doc.set_visible(1, false));
        assert!(!doc.begin_stroke(pt(0.0, 0.0), pen()));
    }

    /// 新增 / 删除 / 移动图层时激活下标跟随。
    #[test]
    fn layer_order_removal_and_active_tracking() {
        let mut doc = InkDocument::new();
        doc.add_layer("b");
        doc.add_layer("c");
        assert_eq!(doc.active_index(), 2);

        // 把顶层 c 移到最底：激活跟随被移动的图层。
        assert!(doc.move_layer(2, 0));
        assert_eq!(doc.active_index(), 0);
        assert_eq!(doc.active().name.as_ref(), "c");

        // 把 c 移回顶层：激活跟随被移动的图层。
        assert!(doc.move_layer(0, 2));
        assert_eq!(doc.active_index(), 2);
        assert_eq!(doc.active().name.as_ref(), "c");

        // 删除激活图层后接管同下标（此时是 c）。
        assert!(doc.remove_layer(1));
        assert_eq!(doc.layer_count(), 2);
        assert_eq!(doc.active().name.as_ref(), "c");

        // 删除激活层以下的图层，激活下标左移。
        assert!(doc.remove_layer(0));
        assert_eq!(doc.active_index(), 0);
        // 最后一层不可删除。
        assert!(!doc.remove_layer(0));
        assert!(!doc.move_layer(0, 0));
        assert!(!doc.move_layer(5, 0));
        assert!(!doc.set_active(3));
    }

    /// 隐藏层不参与命中 / 擦除 / 包围盒，锁定层不参与擦除。
    #[test]
    fn hidden_and_locked_layers_are_excluded() {
        let mut doc = InkDocument::new();
        doc.add_layer("上层");
        draw_on(&mut doc, 0, 0.0);
        draw_on(&mut doc, 1, 100.0);

        // 命中自顶向下取第一个可见层。
        assert_eq!(doc.hit_test(pt(105.0, 0.0), px(2.0)), Some((1, 0)));
        assert_eq!(doc.hit_test(pt(5.0, 0.0), px(2.0)), Some((0, 0)));
        assert_eq!(doc.hit_test(pt(50.0, 50.0), px(2.0)), None);

        // 包围盒覆盖两层。
        let bounds = doc.bounds().unwrap();
        assert_eq!(bounds.origin, pt(0.0, 0.0));
        assert_eq!(bounds.size, crate::size(px(110.0), px(0.0)));

        // 隐藏上层后命中落到下层，包围盒缩回。
        assert!(doc.set_visible(1, false));
        assert_eq!(doc.hit_test(pt(105.0, 0.0), px(2.0)), None);
        assert_eq!(doc.bounds().unwrap().size, crate::size(px(10.0), px(0.0)));
        assert_eq!(doc.visible_layers().count(), 1);
        // 隐藏层的笔画不会被擦除。
        assert_eq!(doc.erase_at(pt(105.0, 0.0), px(3.0)), 0);
        assert_eq!(doc.layer(1).unwrap().layer.strokes.len(), 1);

        // 锁定层不可擦除。
        assert!(doc.set_locked(0, true));
        assert_eq!(doc.erase_at(pt(5.0, 0.0), px(3.0)), 0);
        assert_eq!(doc.layer(0).unwrap().layer.strokes.len(), 1);
        assert!(doc.set_locked(0, false));
        assert_eq!(doc.erase_at(pt(5.0, 0.0), px(3.0)), 1);
    }

    /// 图层不透明度夹取到 `0..=1`，非有限值按 0 处理。
    #[test]
    fn opacity_clamped_and_faded() {
        let mut doc = InkDocument::new();
        let base = hsla(0.2, 0.9, 0.5, 1.0);
        assert_eq!(doc.layer(0).unwrap().fade(base).a, 1.0);

        assert!(doc.set_opacity(0, 0.5));
        assert_eq!(doc.layer(0).unwrap().opacity, 0.5);
        assert!((doc.layer(0).unwrap().fade(base).a - 0.5).abs() < 1e-6);

        assert!(doc.set_opacity(0, 2.0));
        assert_eq!(doc.layer(0).unwrap().opacity, 1.0);
        assert!(doc.set_opacity(0, -1.0));
        assert_eq!(doc.layer(0).unwrap().opacity, 0.0);
        assert!(doc.set_opacity(0, 1.0));
        assert!(doc.set_opacity(0, f32::NAN));
        assert_eq!(doc.layer(0).unwrap().opacity, 0.0);

        assert!(!doc.set_opacity(9, 0.5));
        assert!(!doc.set_opacity(0, 0.0));
        assert!(!doc.set_visible(9, false));
        assert!(!doc.set_locked(9, true));
    }

    /// 套索跨图层返回 `(图层, 笔画)`，隐藏层被跳过。
    #[test]
    fn lasso_spans_visible_layers() {
        let mut doc = InkDocument::new();
        doc.add_layer("上层");
        draw_on(&mut doc, 0, 10.0);
        draw_on(&mut doc, 1, 80.0);

        let square = [pt(0.0, -5.0), pt(40.0, -5.0), pt(40.0, 5.0), pt(0.0, 5.0)];
        assert_eq!(doc.lasso(&square), vec![(0, 0)]);
        // 跨层删除：锁定层跳过。
        assert!(doc.set_locked(1, true));
        doc.remove(&[(0, 0), (1, 0)]);
        assert!(doc.layer(0).unwrap().layer.is_empty());
        assert_eq!(doc.layer(1).unwrap().layer.strokes.len(), 1);
        assert!(doc.set_locked(1, false));
        assert!(doc.lasso(&square).is_empty());

        // 隐藏层不参与套索。
        draw_on(&mut doc, 0, 10.0);
        assert_eq!(doc.lasso(&square), vec![(0, 0)]);
        assert!(doc.set_visible(0, false));
        assert!(doc.lasso(&square).is_empty());
    }

    /// 画布语义：每笔一个节点、id 稳定、点数 / 工具写入、包围盒按笔宽外扩、按阅读顺序排列。
    #[test]
    fn stroke_semantics_expose_points_and_tool() {
        let mut layer = InkLayer::new();
        layer.begin_stroke(pt(20.0, 10.0), pen());
        layer.extend_stroke(pt(30.0, 10.0));
        layer.end_stroke();
        layer.begin_stroke(
            pt(2.0, 10.0),
            InkStyle::highlighter(hsla(0.15, 0.9, 0.6, 1.0), px(12.0)),
        );
        layer.end_stroke();

        let semantics = layer.semantics();
        assert_eq!(semantics.len(), 2);
        let first = semantics.get(1).unwrap();
        assert_eq!(first.label().as_ref(), "笔画 1");
        assert_eq!(first.value().map(|value| value.as_ref()), Some("2 个点"));
        assert!(
            first
                .description()
                .is_some_and(|text| text.as_ref().contains("钢笔"))
        );
        // 包围盒按笔宽一半外扩：x 20..30、宽 3 → 18.5..31.5。
        assert_eq!(first.bounds().origin, pt(18.5, 8.5));
        assert_eq!(first.bounds().size, size(px(13.0), px(3.0)));

        // 单点荧光笔也能得到非零包围盒。
        let second = semantics.get(2).unwrap();
        assert_eq!(second.bounds().size, size(px(12.0), px(12.0)));
        assert!(
            second
                .description()
                .is_some_and(|text| text.as_ref().contains("荧光笔"))
        );

        // 阅读顺序按 x：第二笔在最左，先播报。
        assert_eq!(semantics.reading_order(), vec![2, 1]);
    }
}
