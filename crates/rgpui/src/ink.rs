//! Ink 笔迹层 —— 墨迹笔画模型、编辑操作与绘制元素。
//!
//! 面向白板 / 手写批注 / 手绘应用的**墨迹层**：以「采样点序列 + 画笔样式」为笔画
//! （[`Stroke`]），按工具（钢笔 / 荧光笔 / 橡皮 / 套索）编辑一整个 [`InkLayer`]：
//! 落笔-移动-抬笔生成笔画、按半径擦除、套索选择、撤销等；并提供**纯几何**工具函数
//! （点到线段距离、点在多边形内、Douglas–Peucker 抽稀）与把笔画平滑成 [`crate::Path`]
//! 的构建器。渲染由 [`InkLayerElement`] 承担（逐笔画 `window.paint_path`）。
//!
//! ## 用法
//!
//! ```ignore
//! use rgpui::ink::{InkLayer, InkLayerElement, InkStyle};
//! use rgpui::{hsla, px};
//!
//! let mut layer = InkLayer::new();
//! layer.begin_stroke(point, InkStyle::pen(hsla(0.6, 0.8, 0.4, 1.0), px(3.0)));
//! layer.extend_stroke(point2);
//! layer.end_stroke();
//! let element = InkLayerElement::new(layer);
//! ```
//!
//! **本版暂缓**（后续增补）：逐段压感变宽（现为等宽描边）、压感/倾角曲线拟合、
//! 平滑重采样（Catmull-Rom 已用二次贝塞尔近似）、笔迹分图层与橡皮的局部切割、
//! 触摸/数位板事件接入（现由 [`crate::Platform`] 的指针事件驱动）。

use lyon::path::{LineCap, LineJoin};

use crate::{
    ActiveTheme as _, App, Bounds, Hsla, IntoElement, Path, PathBuilder, PathStyle, Pixels, Point,
    RenderOnce, StrokeOptions, Styled as _, Window, canvas, point, px, size,
};

/// 采样最小间距：鼠标移动事件比采样更密，移动不足该距离的点被丢弃。
pub const MIN_SAMPLE_DISTANCE: Pixels = px(0.75);
/// 荧光笔的不透明度（重叠处自然加深）。
pub const HIGHLIGHTER_ALPHA: f32 = 0.35;
/// 套索选区描边的宽度。
pub const LASSO_STROKE_WIDTH: Pixels = px(1.5);

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
        if let Some(pending) = &mut self.pending {
            let far_enough = pending
                .points
                .last()
                .is_none_or(|last| distance(last.position, position) >= MIN_SAMPLE_DISTANCE.0);
            if far_enough {
                pending.points.push(InkPoint::new(position));
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

    /// 橡皮：删除与给定圆相交的笔画，返回删除数量。
    pub fn erase_at(&mut self, center: Point<Pixels>, radius: Pixels) -> usize {
        let before = self.strokes.len();
        self.strokes
            .retain(|stroke| !stroke.intersects_circle(center, radius));
        before - self.strokes.len()
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

/// 墨迹层绘制元素：逐笔画描边，可高亮选中笔画并绘制套索选区。
#[derive(IntoElement)]
pub struct InkLayerElement {
    /// 待绘制的墨迹层。
    layer: InkLayer,
    /// 高亮的笔画索引（套索选中）。
    highlight: Vec<usize>,
    /// 进行中的套索多边形。
    lasso: Vec<Point<Pixels>>,
}

impl InkLayerElement {
    /// 创建绘制元素。
    pub fn new(layer: InkLayer) -> Self {
        Self {
            layer,
            highlight: Vec::new(),
            lasso: Vec::new(),
        }
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

        canvas(
            |_bounds, _window, _cx| {},
            move |_bounds, _state, window, _cx| {
                // 先画普通笔画，再画高亮笔画，保证选中项在最上层。
                for (index, stroke) in layer.strokes.iter().enumerate() {
                    if highlight.contains(&index) {
                        continue;
                    }
                    if let Some(path) = stroke_path(&stroke.points, stroke.style.width) {
                        window.paint_path(path, stroke.style.effective_color());
                    }
                }
                for &index in &highlight {
                    if let Some(stroke) = layer.strokes.get(index)
                        && let Some(path) = stroke_path(&stroke.points, stroke.style.width)
                    {
                        window.paint_path(path, accent);
                    }
                }
                if let Some(pending) = layer.pending()
                    && let Some(path) = stroke_path(&pending.points, pending.style.width)
                {
                    window.paint_path(path, pending.style.effective_color());
                }
                if let Some(path) = polygon_path(&lasso, LASSO_STROKE_WIDTH) {
                    window.paint_path(path, accent);
                }
            },
        )
        .size_full()
    }
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
}
