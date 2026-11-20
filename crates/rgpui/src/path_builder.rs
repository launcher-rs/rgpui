//! 路径构建器：矢量路径构建与「单调二次曲线轮廓」产出（环绕数路径渲染，D3）。
//!
//! 与旧实现的差异：不再用 lyon 镶嵌成三角形，而是把轮廓转成闭合的单调二次
//! 曲线链，交由 `path_winding` 分桶、GPU 逐像素计算环绕数填充。描边与虚线
//! 用 tiny-skia-path 展开成闭合轮廓后再走同一条填充管线。

use anyhow::{Error, bail};
use etagere::euclid::{Point2D, Vector2D};
use lyon_path::geom::{Angle, CubicBezierSegment, QuadraticBezierSegment};
use lyon_path::math::{Vector, vector};
use lyon_path::traits::SvgPathBuilder;
use lyon_path::{ArcFlags, Event, Polygon};
use std::sync::Arc;

pub use lyon_path::math::Transform;
pub use lyon_path::{FillRule, LineCap, LineJoin};

use crate::path_winding::PathTiles;
use crate::{PathCurve, Pixels, Point, point, px};

/// PathBuilder 的样式。
pub enum PathStyle {
    /// 描边样式。
    Stroke(StrokeOptions),
    /// 填充样式。
    Fill(FillOptions),
}

/// 描边参数。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeOptions {
    /// 描边宽度。
    pub line_width: f32,
    /// 开放子路径与虚线端点的形状。
    pub line_cap: LineCap,
    /// 拐角形状。
    pub line_join: LineJoin,
    /// 斜接长度超过线宽的多少倍后改为斜切。
    pub miter_limit: f32,
}

impl Default for StrokeOptions {
    fn default() -> Self {
        Self {
            line_width: 1.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 4.0,
        }
    }
}

impl StrokeOptions {
    /// 设置描边宽度（与旧版 lyon 同名 builder 方法对齐，调用点无须改动）。
    pub fn with_line_width(mut self, line_width: f32) -> Self {
        self.line_width = line_width;
        self
    }

    /// 设置端点形状。
    pub fn with_line_cap(mut self, line_cap: LineCap) -> Self {
        self.line_cap = line_cap;
        self
    }

    /// 设置拐角形状。
    pub fn with_line_join(mut self, line_join: LineJoin) -> Self {
        self.line_join = line_join;
        self
    }

    /// 设置斜接上限。
    pub fn with_miter_limit(mut self, miter_limit: f32) -> Self {
        self.miter_limit = miter_limit;
        self
    }
}

/// 填充参数。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FillOptions {
    /// 重叠轮廓合成填充区域的规则。
    pub fill_rule: FillRule,
}

impl Default for FillOptions {
    fn default() -> Self {
        Self {
            fill_rule: FillRule::EvenOdd,
        }
    }
}

impl FillOptions {
    /// 设置填充规则。
    pub fn with_fill_rule(mut self, fill_rule: FillRule) -> Self {
        self.fill_rule = fill_rule;
        self
    }
}

/// [`Path`] 构建器。
pub struct PathBuilder {
    raw: lyon_path::builder::WithSvg<lyon_path::BuilderImpl>,
    transform: Option<lyon_path::math::Transform>,
    /// PathBuilder 的 PathStyle。
    pub style: PathStyle,
    dash_array: Option<Vec<Pixels>>,
}

impl From<lyon_path::Builder> for PathBuilder {
    fn from(builder: lyon_path::Builder) -> Self {
        Self {
            raw: builder.with_svg(),
            ..Default::default()
        }
    }
}

impl From<lyon_path::builder::WithSvg<lyon_path::BuilderImpl>> for PathBuilder {
    fn from(raw: lyon_path::builder::WithSvg<lyon_path::BuilderImpl>) -> Self {
        Self {
            raw,
            ..Default::default()
        }
    }
}

impl From<lyon_path::math::Point> for Point<Pixels> {
    fn from(p: lyon_path::math::Point) -> Self {
        point(px(p.x), px(p.y))
    }
}

impl From<Point<Pixels>> for lyon_path::math::Point {
    fn from(p: Point<Pixels>) -> Self {
        lyon_path::math::point(p.x.0, p.y.0)
    }
}

impl From<Point<Pixels>> for Vector {
    fn from(p: Point<Pixels>) -> Self {
        vector(p.x.0, p.y.0)
    }
}

impl From<Point<Pixels>> for Point2D<f32, Pixels> {
    fn from(p: Point<Pixels>) -> Self {
        Point2D::new(p.x.0, p.y.0)
    }
}

impl Default for PathBuilder {
    fn default() -> Self {
        Self {
            raw: lyon_path::Path::builder().with_svg(),
            style: PathStyle::Fill(FillOptions::default()),
            transform: None,
            dash_array: None,
        }
    }
}

impl PathBuilder {
    /// 创建新的 [`PathBuilder`] 以构建描边路径。
    pub fn stroke(width: Pixels) -> Self {
        Self {
            style: PathStyle::Stroke(StrokeOptions {
                line_width: width.0,
                ..Default::default()
            }),
            ..Self::default()
        }
    }

    /// 创建新的 [`PathBuilder`] 以构建填充路径。
    pub fn fill() -> Self {
        Self::default()
    }

    /// 设置 [`PathBuilder`] 的样式。
    pub fn with_style(self, style: PathStyle) -> Self {
        Self { style, ..self }
    }

    /// 设置 [`PathBuilder`] 的虚线数组。填充样式忽略虚线。
    ///
    /// [MDN](https://developer.mozilla.org/en-US/docs/Web/SVG/Reference/Attribute/stroke-dasharray)
    pub fn dash_array(mut self, dash_array: &[Pixels]) -> Self {
        // 如果提供奇数个值，则重复该值列表以产生偶数个值。
        // 因此，5,3,2 等效于 5,3,2,5,3,2。
        let array = if dash_array.len() % 2 == 1 {
            let mut new_dash_array = dash_array.to_vec();
            new_dash_array.extend_from_slice(dash_array);
            new_dash_array
        } else {
            dash_array.to_vec()
        };

        self.dash_array = Some(array);
        self
    }

    /// 将当前点移动到给定点。
    #[inline]
    pub fn move_to(&mut self, to: Point<Pixels>) {
        self.raw.move_to(to.into());
    }

    /// 从当前点到给定点绘制直线。
    #[inline]
    pub fn line_to(&mut self, to: Point<Pixels>) {
        self.raw.line_to(to.into());
    }

    /// 从当前点到给定点绘制曲线，使用给定的控制点。
    #[inline]
    pub fn curve_to(&mut self, to: Point<Pixels>, ctrl: Point<Pixels>) {
        self.raw.quadratic_bezier_to(ctrl.into(), to.into());
    }

    /// 向 [`Path`] 添加三次贝塞尔曲线，给定其两个控制点和终点。
    #[inline]
    pub fn cubic_bezier_to(
        &mut self,
        to: Point<Pixels>,
        control_a: Point<Pixels>,
        control_b: Point<Pixels>,
    ) {
        self.raw
            .cubic_bezier_to(control_a.into(), control_b.into(), to.into());
    }

    /// 添加椭圆弧。
    pub fn arc_to(
        &mut self,
        radii: Point<Pixels>,
        x_rotation: Pixels,
        large_arc: bool,
        sweep: bool,
        to: Point<Pixels>,
    ) {
        self.raw.arc_to(
            radii.into(),
            Angle::degrees(x_rotation.into()),
            ArcFlags { large_arc, sweep },
            to.into(),
        );
    }

    /// 等效于相对坐标中的 `arc_to`。
    pub fn relative_arc_to(
        &mut self,
        radii: Point<Pixels>,
        x_rotation: Pixels,
        large_arc: bool,
        sweep: bool,
        to: Point<Pixels>,
    ) {
        self.raw.relative_arc_to(
            radii.into(),
            Angle::degrees(x_rotation.into()),
            ArcFlags { large_arc, sweep },
            to.into(),
        );
    }

    /// 添加多边形。
    pub fn add_polygon(&mut self, points: &[Point<Pixels>], closed: bool) {
        let points = points.iter().copied().map(|p| p.into()).collect::<Vec<_>>();
        self.raw.add_polygon(Polygon {
            points: points.as_ref(),
            closed,
        });
    }

    /// 关闭当前子路径。
    #[inline]
    pub fn close(&mut self) {
        self.raw.close();
    }

    /// 对路径应用变换。
    #[inline]
    pub fn transform(&mut self, transform: Transform) {
        self.transform = Some(transform);
    }

    /// 对路径应用平移。
    #[inline]
    pub fn translate(&mut self, to: Point<Pixels>) {
        if let Some(transform) = self.transform {
            self.transform = Some(transform.then_translate(Vector2D::new(to.x.0, to.y.0)));
        } else {
            self.transform = Some(Transform::translation(to.x.0, to.y.0))
        }
    }

    /// 对路径应用缩放。
    #[inline]
    pub fn scale(&mut self, scale: f32) {
        if let Some(transform) = self.transform {
            self.transform = Some(transform.then_scale(scale, scale));
        } else {
            self.transform = Some(Transform::scale(scale, scale));
        }
    }

    /// 对路径应用旋转。
    ///
    /// `angle` 是以度为单位的值，范围为 0.0 到 360.0。
    #[inline]
    pub fn rotate(&mut self, angle: f32) {
        let radians = angle.to_radians();
        if let Some(transform) = self.transform {
            self.transform = Some(transform.then_rotate(Angle::radians(radians)));
        } else {
            self.transform = Some(Transform::rotation(Angle::radians(radians)));
        }
    }

    /// 构建为 [`Path`]。
    pub fn build(self) -> Result<Path, Error> {
        let (curves, fill_rule) = self.outline()?;
        Ok(Path::from_curves(curves, fill_rule))
    }

    /// 待填充的闭合轮廓曲线与填充规则。
    pub(crate) fn outline(self) -> Result<(Vec<PathCurve>, FillRule), Error> {
        let path = if let Some(transform) = self.transform {
            self.raw.build().transformed(&transform)
        } else {
            self.raw.build()
        };

        Ok(match self.style {
            PathStyle::Stroke(options) => (
                Self::stroke_outline(&path, &options, self.dash_array.as_deref())?,
                FillRule::NonZero,
            ),
            PathStyle::Fill(options) => (Self::fill_outline(&path)?, options.fill_rule),
        })
    }

    /// 描边：用 tiny-skia 把路径展开成闭合轮廓（虚线先切再描），再转成单调曲线。
    fn stroke_outline(
        path: &lyon_path::Path,
        options: &StrokeOptions,
        dash_array: Option<&[Pixels]>,
    ) -> Result<Vec<PathCurve>, Error> {
        let outline = skia_path(path)
            .map(|skia_path| match dash_array {
                Some(dash_array) => {
                    let array = dash_array.iter().map(|length| length.0).collect();
                    match tiny_skia_path::StrokeDash::new(array, 0.0) {
                        Some(dash) => skia_path
                            .dash(&dash, STROKE_RESOLUTION_SCALE)
                            .unwrap_or(skia_path),
                        // 非法虚线数组按无虚线描边（与 SVG 一致）。
                        None => skia_path,
                    }
                }
                None => skia_path,
            })
            .and_then(|skia_path| {
                skia_path.stroke(
                    &tiny_skia_path::Stroke {
                        width: options.line_width,
                        miter_limit: options.miter_limit,
                        line_cap: match options.line_cap {
                            LineCap::Butt => tiny_skia_path::LineCap::Butt,
                            LineCap::Square => tiny_skia_path::LineCap::Square,
                            LineCap::Round => tiny_skia_path::LineCap::Round,
                        },
                        line_join: match options.line_join {
                            LineJoin::Miter => tiny_skia_path::LineJoin::Miter,
                            LineJoin::MiterClip => tiny_skia_path::LineJoin::MiterClip,
                            LineJoin::Round => tiny_skia_path::LineJoin::Round,
                            LineJoin::Bevel => tiny_skia_path::LineJoin::Bevel,
                        },
                        dash: None,
                    },
                    STROKE_RESOLUTION_SCALE,
                )
            });
        let Some(outline) = outline else {
            return Ok(Vec::new());
        };

        let mut output = Outline::default();
        let at = |p: tiny_skia_path::Point| point(p.x, p.y);
        for segment in outline.segments() {
            match segment {
                tiny_skia_path::PathSegment::MoveTo(p) => output.move_to(at(p)),
                tiny_skia_path::PathSegment::LineTo(p) => output.line_to(at(p)),
                tiny_skia_path::PathSegment::QuadTo(ctrl, p) => output.curve_to(at(p), at(ctrl)),
                tiny_skia_path::PathSegment::CubicTo(ctrl1, ctrl2, p) => {
                    output.cubic_to(at(p), at(ctrl1), at(ctrl2))
                }
                tiny_skia_path::PathSegment::Close => output.close(),
            }
        }
        output.into_curves()
    }

    /// 把 lyon 路径转成填充轮廓（每个子路径闭合）。
    fn fill_outline(path: &lyon_path::Path) -> Result<Vec<PathCurve>, Error> {
        let mut output = Outline::default();
        let at = |p: lyon_path::math::Point| point(p.x, p.y);
        for event in path.iter() {
            match event {
                Event::Begin { at: p } => output.move_to(at(p)),
                Event::Line { to, .. } => output.line_to(at(to)),
                Event::Quadratic { ctrl, to, .. } => output.curve_to(at(to), at(ctrl)),
                Event::Cubic {
                    ctrl1, ctrl2, to, ..
                } => output.cubic_to(at(to), at(ctrl1), at(ctrl2)),
                Event::End { .. } => output.close(),
            }
        }
        output.into_curves()
    }
}

/// 一条填充路径：闭合的单调曲线轮廓（逻辑像素）。由 [`PathBuilder`] 构建。
#[derive(Clone, Debug)]
pub struct Path {
    /// 闭合轮廓的单调曲线（逻辑像素）。
    curves: Arc<[PathCurve]>,
    /// 是否按奇偶规则填充。
    even_odd: bool,
}

impl Path {
    /// 由曲线与填充规则构建路径。
    pub(crate) fn from_curves(curves: Vec<PathCurve>, fill_rule: FillRule) -> Self {
        Self {
            curves: curves.into(),
            even_odd: matches!(fill_rule, FillRule::EvenOdd),
        }
    }

    /// 是否按奇偶规则填充。
    pub(crate) fn even_odd(&self) -> bool {
        self.even_odd
    }

    /// 按给定显示比例把曲线分桶成设备像素瓦片，供绘制使用。
    pub(crate) fn bin(&self, scale: f32) -> PathTiles {
        crate::path_winding::bin(&self.curves, self.even_odd, scale)
    }
}

/// [`PathBuilder`] 收集的路径轮廓：闭合的单调曲线链。
#[derive(Default)]
pub(crate) struct Outline {
    curves: Vec<PathCurve>,
    start: Point<f32>,
    current: Point<f32>,
    non_finite: bool,
}

impl Outline {
    /// 这些点全部有限时才能继续加入线段。
    fn accepts(&mut self, points: &[Point<f32>]) -> bool {
        self.non_finite |= !points
            .iter()
            .all(|point| point.x.is_finite() && point.y.is_finite());
        !self.non_finite
    }

    /// 关闭当前轮廓并在给定点开新轮廓。
    pub(crate) fn move_to(&mut self, to: Point<f32>) {
        if !self.accepts(&[to]) {
            return;
        }
        self.close();
        self.start = to;
        self.current = to;
    }

    pub(crate) fn line_to(&mut self, to: Point<f32>) {
        if self.accepts(&[to]) && to != self.current {
            self.curves.push(PathCurve::line(self.current, to));
            self.current = to;
        }
    }

    pub(crate) fn curve_to(&mut self, to: Point<f32>, ctrl: Point<f32>) {
        if !self.accepts(&[to, ctrl]) || (to == self.current && ctrl == self.current) {
            return;
        }
        PathCurve::push_monotone(&mut self.curves, self.current, ctrl, to);
        self.current = to;
    }

    pub(crate) fn cubic_to(&mut self, to: Point<f32>, ctrl1: Point<f32>, ctrl2: Point<f32>) {
        /// 最大误差（逻辑像素）。
        const TOLERANCE: f32 = 0.25;
        if !self.accepts(&[to, ctrl1, ctrl2]) {
            return;
        }
        let lyon_point = |p: Point<f32>| lyon_path::math::point(p.x, p.y);
        let cubic = CubicBezierSegment {
            from: lyon_point(self.current),
            ctrl1: lyon_point(ctrl1),
            ctrl2: lyon_point(ctrl2),
            to: lyon_point(to),
        };
        cubic.for_each_quadratic_bezier(TOLERANCE, &mut |quadratic| {
            self.curve_to(
                point(quadratic.to.x, quadratic.to.y),
                point(quadratic.ctrl.x, quadratic.ctrl.y),
            );
        });
    }

    /// 用直线闭合当前轮廓回到起点。
    pub(crate) fn close(&mut self) {
        let start = self.start;
        self.line_to(start);
    }

    /// 取曲线，所有轮廓均已闭合；含非有限坐标时整条路径失败。
    pub(crate) fn into_curves(mut self) -> Result<Vec<PathCurve>, Error> {
        if self.non_finite {
            bail!("路径含非有限坐标点");
        }
        self.close();
        Ok(self.curves)
    }
}

impl PathCurve {
    fn new(p0: Point<f32>, ctrl: Point<f32>, p1: Point<f32>, sign: f32) -> Self {
        Self {
            p0,
            p1,
            a: point(p0.x - 2.0 * ctrl.x + p1.x, p0.y - 2.0 * ctrl.y + p1.y),
            b: point(2.0 * (ctrl.x - p0.x), 2.0 * (ctrl.y - p0.y)),
            sign,
            pad: 0,
        }
    }

    /// 一条直线：二次项精确为零。
    fn line(from: Point<f32>, to: Point<f32>) -> Self {
        let (p0, p1, sign) = downward(from, to);
        Self {
            p0,
            p1,
            a: point(0.0, 0.0),
            b: point(p1.x - p0.x, p1.y - p0.y),
            sign,
            pad: 0,
        }
    }

    /// 在极值点拆分二次曲线，产出 xy 单调的曲线段。
    fn push_monotone(curves: &mut Vec<Self>, p0: Point<f32>, ctrl: Point<f32>, p1: Point<f32>) {
        let curve = QuadraticBezierSegment {
            from: lyon_path::math::point(p0.x, p0.y),
            ctrl: lyon_path::math::point(ctrl.x, ctrl.y),
            to: lyon_path::math::point(p1.x, p1.y),
        };
        // 不用 `split_range`：它会把舍入推出端点包围盒的控制点原样保留，
        // 这里把控制点钳回端点盒。
        curve.for_each_monotonic(&mut |monotone| {
            let from = point(monotone.from.x, monotone.from.y);
            let to = point(monotone.to.x, monotone.to.y);
            if from == to {
                return;
            }
            // 水平曲线保留：向上的射线要与它交叉。
            let (p0, p1, sign) = downward(from, to);
            let ctrl = point(
                monotone.ctrl.x.clamp(p0.x.min(p1.x), p0.x.max(p1.x)),
                monotone.ctrl.y.clamp(p0.y, p1.y),
            );
            curves.push(Self::new(p0, ctrl, p1, sign));
        });
    }
}

/// 端点统一成「自上而下」方向，返回 (p0, p1, 符号)。
fn downward(from: Point<f32>, to: Point<f32>) -> (Point<f32>, Point<f32>, f32) {
    if from.y > to.y {
        (to, from, -1.0)
    } else {
        (from, to, 1.0)
    }
}

/// 描边在逻辑像素中展开。tiny-skia 把偏移曲线保持在
/// `0.25 / STROKE_RESOLUTION_SCALE` 精度内（2x 下为四分之一设备像素），并按
/// `0.5 / STROKE_RESOLUTION_SCALE` 精度展平曲线以测量虚线。圆头/圆角忽略该
/// 常量：其弧线按 0.25 逻辑像素近似。
const STROKE_RESOLUTION_SCALE: f32 = 2.0;

/// 空几何返回 [`None`]。
fn skia_path(path: &lyon_path::Path) -> Option<tiny_skia_path::Path> {
    let mut builder = tiny_skia_path::PathBuilder::new();
    for event in path.iter() {
        match event {
            Event::Begin { at } => builder.move_to(at.x, at.y),
            Event::Line { to, .. } => builder.line_to(to.x, to.y),
            Event::Quadratic { ctrl, to, .. } => builder.quad_to(ctrl.x, ctrl.y, to.x, to.y),
            Event::Cubic {
                ctrl1, ctrl2, to, ..
            } => builder.cubic_to(ctrl1.x, ctrl1.y, ctrl2.x, ctrl2.y, to.x, to.y),
            Event::End { close, .. } => {
                if close {
                    builder.close();
                }
            }
        }
    }
    builder.finish()
}
