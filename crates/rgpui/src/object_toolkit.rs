//! 画布对象工具 —— 选区几何、缩放手柄、旋转手柄与对齐/分布。
//!
//! 面向绘图/画布编辑器的「对象操作 toolkit」：以**文档坐标**的矩形为对象边界，
//! 提供缩放手柄计算（8 向 + 旋转）、手柄命中判定、按手柄缩放、绕点旋转、对齐与分布
//! 等纯几何函数；屏幕映射复用 [`Viewport`]。另附 [`SelectionHandles`] 元素，
//! 直接把选区描边与 9 个手柄画到画布上（供应用叠在内容之上）。
//!
//! 旋转过的对象用 [`handle_point_rotated`] / [`hit_handle_rotated`] 让手柄跟着转；
//! 从空白处拖拽的多选框用 [`marquee_bounds`] + [`marquee_hits`]（相交 / 完全包含两种口径）。
//!
//! ## 用法
//!
//! ```ignore
//! use rgpui::object_toolkit::{ResizeHandle, handle_point, hit_handle, resize_bounds};
//!
//! // 鼠标按下：命中哪个手柄？
//! if let Some(handle) = hit_handle(selection, doc_point, px(6.0)) {
//!     // 拖动：按手柄缩放（保留对侧锚点）。
//!     selection = resize_bounds(selection, handle, doc_point, px(8.0));
//! }
//! ```

use crate::viewport::Viewport;
use crate::{
    ActiveTheme as _, Axis, Bounds, IntoElement, ParentElement as _, Pixels, Point, RenderOnce,
    Styled as _, Window, div, point, px, size,
};

/// 旋转手柄到选区顶边的距离。
pub const ROTATE_HANDLE_DISTANCE: Pixels = px(24.0);
/// 手柄命中判定的默认容差（半边长）。
pub const DEFAULT_HANDLE_SIZE: Pixels = px(8.0);

/// 缩放手柄方位（含旋转手柄）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResizeHandle {
    /// 左上角。
    TopLeft,
    /// 上边中点。
    Top,
    /// 右上角。
    TopRight,
    /// 右边中点。
    Right,
    /// 右下角。
    BottomRight,
    /// 下边中点。
    Bottom,
    /// 左下角。
    BottomLeft,
    /// 左边中点。
    Left,
    /// 顶部上方的旋转手柄。
    Rotate,
}

impl ResizeHandle {
    /// 八个缩放手柄（不含旋转）。
    pub const RESIZE: [ResizeHandle; 8] = [
        ResizeHandle::TopLeft,
        ResizeHandle::Top,
        ResizeHandle::TopRight,
        ResizeHandle::Right,
        ResizeHandle::BottomRight,
        ResizeHandle::Bottom,
        ResizeHandle::BottomLeft,
        ResizeHandle::Left,
    ];

    /// 全部手柄（8 缩放 + 旋转）。
    pub const ALL: [ResizeHandle; 9] = [
        ResizeHandle::TopLeft,
        ResizeHandle::Top,
        ResizeHandle::TopRight,
        ResizeHandle::Right,
        ResizeHandle::BottomRight,
        ResizeHandle::Bottom,
        ResizeHandle::BottomLeft,
        ResizeHandle::Left,
        ResizeHandle::Rotate,
    ];

    /// 是否旋转手柄。
    pub fn is_rotate(self) -> bool {
        matches!(self, ResizeHandle::Rotate)
    }

    /// 对侧手柄（旋转手柄返回自身）。
    pub fn opposite(self) -> Self {
        use ResizeHandle::*;
        match self {
            TopLeft => BottomRight,
            Top => Bottom,
            TopRight => BottomLeft,
            Right => Left,
            BottomRight => TopLeft,
            Bottom => Top,
            BottomLeft => TopRight,
            Left => Right,
            Rotate => Rotate,
        }
    }
}

/// 手柄在文档坐标中的位置。
pub fn handle_point(rect: Bounds<Pixels>, handle: ResizeHandle) -> Point<Pixels> {
    use ResizeHandle::*;
    let left = rect.origin.x;
    let top = rect.origin.y;
    let right = left + rect.size.width;
    let bottom = top + rect.size.height;
    let cx = left + rect.size.width / 2.0;
    let cy = top + rect.size.height / 2.0;
    match handle {
        TopLeft => point(left, top),
        Top => point(cx, top),
        TopRight => point(right, top),
        Right => point(right, cy),
        BottomRight => point(right, bottom),
        Bottom => point(cx, bottom),
        BottomLeft => point(left, bottom),
        Left => point(left, cy),
        Rotate => point(cx, top - ROTATE_HANDLE_DISTANCE),
    }
}

/// 命中判定：`document_point` 落在哪个手柄的 `tolerance` 容差内（旋转手柄最后判定）。
pub fn hit_handle(
    rect: Bounds<Pixels>,
    document_point: Point<Pixels>,
    tolerance: Pixels,
) -> Option<ResizeHandle> {
    ResizeHandle::ALL.into_iter().find(|handle| {
        let p = handle_point(rect, *handle);
        (p.x - document_point.x).abs() <= tolerance && (p.y - document_point.y).abs() <= tolerance
    })
}

/// 旋转后的手柄位置：把轴对齐的 [`handle_point`] 绕选区中心旋转 `angle` 弧度。
///
/// 元素旋转后手柄要跟着转，否则命中区域会和看到的框错位。
pub fn handle_point_rotated(
    rect: Bounds<Pixels>,
    angle: f32,
    handle: ResizeHandle,
) -> Point<Pixels> {
    rotate_point(handle_point(rect, handle), rect.center(), angle)
}

/// 旋转状态下的命中判定：先把指针反向旋转回选区局部坐标，再走 [`hit_handle`]。
///
/// 与 [`handle_point_rotated`] 配对使用，保证「看到的框」和「点得到的框」一致。
pub fn hit_handle_rotated(
    rect: Bounds<Pixels>,
    angle: f32,
    document_point: Point<Pixels>,
    tolerance: Pixels,
) -> Option<ResizeHandle> {
    let center = rect.center();
    hit_handle(
        rect,
        rotate_point(document_point, center, -angle),
        tolerance,
    )
}

/// 按手柄把 `pointer` 拖到的位置缩放 `rect`：保留对侧锚点，尺寸不小于 `min_size`。
///
/// 旋转手柄不改变矩形（返回原值）；中边手柄仅改一个轴向。
pub fn resize_bounds(
    rect: Bounds<Pixels>,
    handle: ResizeHandle,
    pointer: Point<Pixels>,
    min_size: Pixels,
) -> Bounds<Pixels> {
    if handle.is_rotate() {
        return rect;
    }
    use ResizeHandle::*;
    let mut left = rect.origin.x;
    let mut right = rect.origin.x + rect.size.width;
    let mut top = rect.origin.y;
    let mut bottom = rect.origin.y + rect.size.height;

    let anchor_x = match handle {
        TopLeft | Left | BottomLeft => Some(right),
        TopRight | Right | BottomRight => Some(left),
        _ => None,
    };
    let anchor_y = match handle {
        TopLeft | Top | TopRight => Some(bottom),
        BottomLeft | Bottom | BottomRight => Some(top),
        _ => None,
    };

    if let Some(ax) = anchor_x {
        let width = (pointer.x - ax).abs().max(min_size);
        if pointer.x < ax {
            left = ax - width;
            right = ax;
        } else {
            left = ax;
            right = ax + width;
        }
    }
    if let Some(ay) = anchor_y {
        let height = (pointer.y - ay).abs().max(min_size);
        if pointer.y < ay {
            top = ay - height;
            bottom = ay;
        } else {
            top = ay;
            bottom = ay + height;
        }
    }
    Bounds::new(point(left, top), size(right - left, bottom - top))
}

/// 绕过 `center` 旋转点（弧度，顺时针为正，y 轴向下）。
pub fn rotate_point(p: Point<Pixels>, center: Point<Pixels>, angle: f32) -> Point<Pixels> {
    let (sin, cos) = angle.sin_cos();
    let dx = (p.x - center.x).as_f32();
    let dy = (p.y - center.y).as_f32();
    point(
        center.x + px(dx * cos - dy * sin),
        center.y + px(dx * sin + dy * cos),
    )
}

/// 以 `center` 为心，从 `from` 转到 `to` 的角度（弧度）。
pub fn angle_between(center: Point<Pixels>, from: Point<Pixels>, to: Point<Pixels>) -> f32 {
    let from_angle = (from.y - center.y)
        .as_f32()
        .atan2((from.x - center.x).as_f32());
    let to_angle = (to.y - center.y).as_f32().atan2((to.x - center.x).as_f32());
    to_angle - from_angle
}

/// 一组矩形的最小外接矩形（空集返回 `None`）。
pub fn union_bounds(rects: &[Bounds<Pixels>]) -> Option<Bounds<Pixels>> {
    let mut iter = rects.iter();
    let first = iter.next()?;
    let (mut left, mut top) = (first.origin.x, first.origin.y);
    let (mut right, mut bottom) = (left + first.size.width, top + first.size.height);
    for rect in iter {
        left = left.min(rect.origin.x);
        top = top.min(rect.origin.y);
        right = right.max(rect.origin.x + rect.size.width);
        bottom = bottom.max(rect.origin.y + rect.size.height);
    }
    Some(Bounds::new(
        point(left, top),
        size(right - left, bottom - top),
    ))
}

/// 框选的矩形：把按下点与当前点整理成左上角 + 尺寸（向左上拖也成立）。
pub fn marquee_bounds(start: Point<Pixels>, current: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        point(start.x.min(current.x), start.y.min(current.y)),
        size((current.x - start.x).abs(), (current.y - start.y).abs()),
    )
}

/// 框选判定口径。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarqueeMode {
    /// 与框选矩形相交即选中（默认，符合从空白处拖拽的直觉）。
    #[default]
    Intersect,
    /// 必须被框选矩形完全包住才选中（严格口径，适合「框住才动」的场景）。
    Contain,
}

/// 框选命中的对象索引（保持输入顺序）。
pub fn marquee_hits(
    objects: &[Bounds<Pixels>],
    marquee: Bounds<Pixels>,
    mode: MarqueeMode,
) -> Vec<usize> {
    objects
        .iter()
        .enumerate()
        .filter(|(_, bounds)| match mode {
            MarqueeMode::Intersect => bounds.intersects(&marquee),
            MarqueeMode::Contain => {
                marquee.origin.x <= bounds.origin.x
                    && marquee.origin.y <= bounds.origin.y
                    && bounds.origin.x + bounds.size.width <= marquee.origin.x + marquee.size.width
                    && bounds.origin.y + bounds.size.height
                        <= marquee.origin.y + marquee.size.height
            }
        })
        .map(|(index, _)| index)
        .collect()
}

/// 对齐方式（相对选区的外接矩形）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AlignMode {
    /// 左对齐。
    Left,
    /// 水平居中。
    CenterHorizontal,
    /// 右对齐。
    Right,
    /// 顶对齐。
    Top,
    /// 垂直居中。
    Middle,
    /// 底对齐。
    Bottom,
}

/// 把每个矩形按 `mode` 对齐到选区外接矩形的对应边/中线上。
pub fn align(rects: &[Bounds<Pixels>], mode: AlignMode) -> Vec<Bounds<Pixels>> {
    let Some(union) = union_bounds(rects) else {
        return Vec::new();
    };
    let center = point(
        union.origin.x + union.size.width / 2.0,
        union.origin.y + union.size.height / 2.0,
    );
    rects
        .iter()
        .map(|rect| {
            let mut origin = rect.origin;
            match mode {
                AlignMode::Left => origin.x = union.origin.x,
                AlignMode::CenterHorizontal => origin.x = center.x - rect.size.width / 2.0,
                AlignMode::Right => origin.x = union.origin.x + union.size.width - rect.size.width,
                AlignMode::Top => origin.y = union.origin.y,
                AlignMode::Middle => origin.y = center.y - rect.size.height / 2.0,
                AlignMode::Bottom => {
                    origin.y = union.origin.y + union.size.height - rect.size.height
                }
            }
            Bounds::new(origin, rect.size)
        })
        .collect()
}

/// 沿 `axis` 等间距分布（保持两端位置，至少 3 个元素才生效）。
pub fn distribute(rects: &[Bounds<Pixels>], axis: Axis) -> Vec<Bounds<Pixels>> {
    if rects.len() < 3 {
        return rects.to_vec();
    }
    let horizontal = matches!(axis, Axis::Horizontal);
    let start_of = |rect: &Bounds<Pixels>| {
        if horizontal {
            rect.origin.x
        } else {
            rect.origin.y
        }
    };
    let size_of = |rect: &Bounds<Pixels>| {
        if horizontal {
            rect.size.width
        } else {
            rect.size.height
        }
    };

    let mut order: Vec<usize> = (0..rects.len()).collect();
    order.sort_by(|a, b| {
        start_of(&rects[*a])
            .as_f32()
            .total_cmp(&start_of(&rects[*b]).as_f32())
    });

    let first_start = start_of(&rects[order[0]]).as_f32();
    // `rects.len() >= 3` 已在函数入口保证，`order` 与它等长。
    let last = &rects[order[order.len() - 1]];
    let last_end = start_of(last).as_f32() + size_of(last).as_f32();
    let total_size: f32 = order.iter().map(|i| size_of(&rects[*i]).as_f32()).sum();
    let gap = (last_end - first_start - total_size) / (order.len() - 1) as f32;

    let mut result = rects.to_vec();
    let mut cursor = first_start;
    for index in order {
        let rect = &rects[index];
        let mut origin = rect.origin;
        if horizontal {
            origin.x = px(cursor);
        } else {
            origin.y = px(cursor);
        }
        result[index] = Bounds::new(origin, rect.size);
        cursor += size_of(rect).as_f32() + gap;
    }
    result
}

/// 选区手柄覆盖层：选区描边 + 9 个手柄（8 缩放 + 旋转），按 [`Viewport`] 映射到屏幕。
#[derive(IntoElement)]
pub struct SelectionHandles {
    /// 选区（文档坐标）。
    selection: Bounds<Pixels>,
    /// 屏幕 ↔ 文档坐标映射。
    viewport: Viewport,
    /// 手柄边长（屏幕像素）。
    handle_size: Pixels,
    /// 是否绘制旋转手柄。
    rotate: bool,
}

impl SelectionHandles {
    /// 创建手柄覆盖层。
    pub fn new(selection: Bounds<Pixels>, viewport: Viewport) -> Self {
        Self {
            selection,
            viewport,
            handle_size: DEFAULT_HANDLE_SIZE,
            rotate: true,
        }
    }

    /// 手柄边长（屏幕像素，默认 [`DEFAULT_HANDLE_SIZE`]）。
    pub fn handle_size(mut self, size: Pixels) -> Self {
        self.handle_size = size;
        self
    }

    /// 是否绘制旋转手柄（默认 `true`）。
    pub fn rotate(mut self, show: bool) -> Self {
        self.rotate = show;
        self
    }
}

impl RenderOnce for SelectionHandles {
    fn render(self, _window: &mut Window, cx: &mut crate::App) -> impl IntoElement {
        let theme = cx.theme();
        let accent = theme.primary;
        let surface = theme.background;

        let origin = self.viewport.document_to_screen(self.selection.origin);
        let selection_size = self.viewport.document_size_to_screen(self.selection.size);

        let handles = if self.rotate {
            ResizeHandle::ALL.as_slice()
        } else {
            ResizeHandle::RESIZE.as_slice()
        };

        let mut root = div().relative().size_full().child(
            div()
                .absolute()
                .left(origin.x)
                .top(origin.y)
                .w(selection_size.width)
                .h(selection_size.height)
                .border_1()
                .border_color(accent),
        );

        for handle in handles {
            let document = handle_point(self.selection, *handle);
            let screen = self.viewport.document_to_screen(document);
            root = root.child(
                div()
                    .absolute()
                    .left(screen.x - self.handle_size / 2.0)
                    .top(screen.y - self.handle_size / 2.0)
                    .w(self.handle_size)
                    .h(self.handle_size)
                    .border_1()
                    .border_color(accent)
                    .bg(surface),
            );
        }

        root
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{
        AlignMode, MarqueeMode, ResizeHandle, align, angle_between, distribute, handle_point,
        handle_point_rotated, hit_handle, hit_handle_rotated, marquee_bounds, marquee_hits,
        resize_bounds, rotate_point, union_bounds,
    };
    use crate::{Axis, Bounds, Pixels, point, px, size};

    /// 构造矩形。
    fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
    }

    /// 手柄位置：四角/四边中点/旋转手柄（顶部上方 24）。
    #[test]
    fn handle_points_are_on_rect_edges() {
        let r = rect(10.0, 20.0, 100.0, 60.0);
        assert_eq!(
            handle_point(r, ResizeHandle::TopLeft),
            point(px(10.0), px(20.0))
        );
        assert_eq!(
            handle_point(r, ResizeHandle::Top),
            point(px(60.0), px(20.0))
        );
        assert_eq!(
            handle_point(r, ResizeHandle::BottomRight),
            point(px(110.0), px(80.0))
        );
        assert_eq!(
            handle_point(r, ResizeHandle::Left),
            point(px(10.0), px(50.0))
        );
        assert_eq!(
            handle_point(r, ResizeHandle::Rotate),
            point(px(60.0), px(-4.0))
        );
    }

    /// 命中判定在容差内命中对应手柄，容差外为 `None`。
    #[test]
    fn hit_handle_respects_tolerance() {
        let r = rect(0.0, 0.0, 100.0, 100.0);
        assert_eq!(
            hit_handle(r, point(px(2.0), px(3.0)), px(6.0)),
            Some(ResizeHandle::TopLeft)
        );
        assert_eq!(
            hit_handle(r, point(px(100.0), px(50.0)), px(6.0)),
            Some(ResizeHandle::Right)
        );
        assert_eq!(hit_handle(r, point(px(50.0), px(50.0)), px(6.0)), None);
    }

    /// 按角/边手柄缩放保留对侧锚点，且尺寸不小于最小值。
    #[test]
    fn resize_keeps_opposite_anchor_and_min_size() {
        let r = rect(0.0, 0.0, 100.0, 100.0);
        // 拖右下角到 (150, 130)：左上角不动。
        let resized = resize_bounds(
            r,
            ResizeHandle::BottomRight,
            point(px(150.0), px(130.0)),
            px(8.0),
        );
        assert_eq!(resized.origin, point(px(0.0), px(0.0)));
        assert_eq!(resized.size, size(px(150.0), px(130.0)));
        // 拖左上角到 (-50, -20)：右下角不动。
        let resized = resize_bounds(
            r,
            ResizeHandle::TopLeft,
            point(px(-50.0), px(-20.0)),
            px(8.0),
        );
        assert_eq!(resized.origin, point(px(-50.0), px(-20.0)));
        assert_eq!(resized.size, size(px(150.0), px(120.0)));
        // 拖右边越过锚点、穿过左侧：夹到最小宽度，仍锚定左边。
        let resized = resize_bounds(r, ResizeHandle::Right, point(px(-40.0), px(0.0)), px(8.0));
        assert_eq!(resized.origin, point(px(-40.0), px(0.0)));
        assert_eq!(resized.size, size(px(40.0), px(100.0)));
        // 旋转手柄不改矩形。
        assert_eq!(
            resize_bounds(r, ResizeHandle::Rotate, point(px(5.0), px(5.0)), px(8.0)),
            r
        );
    }

    /// 对侧手柄与旋转手柄自反。
    #[test]
    fn opposite_handle_pairs() {
        assert_eq!(ResizeHandle::TopLeft.opposite(), ResizeHandle::BottomRight);
        assert_eq!(ResizeHandle::Bottom.opposite(), ResizeHandle::Top);
        assert_eq!(ResizeHandle::Rotate.opposite(), ResizeHandle::Rotate);
        assert!(ResizeHandle::Rotate.is_rotate());
    }

    /// 绕点旋转：90° 顺时针把右侧点转到下方。
    #[test]
    fn rotate_point_quarter_turn() {
        let center = point(px(0.0), px(0.0));
        let rotated = rotate_point(
            point(px(10.0), px(0.0)),
            center,
            std::f32::consts::FRAC_PI_2,
        );
        assert!((rotated.x.as_f32()).abs() < 0.001);
        assert!((rotated.y.as_f32() - 10.0).abs() < 0.001);
    }

    /// 角度差：从右侧转到下方为 +90°。
    #[test]
    fn angle_between_measures_rotation() {
        let center = point(px(0.0), px(0.0));
        let angle = angle_between(center, point(px(10.0), px(0.0)), point(px(0.0), px(10.0)));
        assert!((angle - std::f32::consts::FRAC_PI_2).abs() < 0.001);
    }

    /// 框选矩形把两端点整理成左上角 + 正尺寸，向左上拖结果一致。
    #[test]
    fn marquee_bounds_normalizes_drag_direction() {
        let forward = marquee_bounds(point(px(10.0), px(20.0)), point(px(60.0), px(80.0)));
        let backward = marquee_bounds(point(px(60.0), px(80.0)), point(px(10.0), px(20.0)));

        assert_eq!(forward, backward);
        assert_eq!(forward.origin, point(px(10.0), px(20.0)));
        assert_eq!(forward.size, size(px(50.0), px(60.0)));
        // 原地点击：退化成零尺寸矩形
        let zero = marquee_bounds(point(px(5.0), px(5.0)), point(px(5.0), px(5.0)));
        assert!(zero.is_empty());
    }

    /// 框选两种口径：相交即选 vs 必须完全包住。
    #[test]
    fn marquee_hits_supports_intersect_and_contain() {
        let objects = [
            rect(0.0, 0.0, 40.0, 40.0),
            rect(30.0, 30.0, 40.0, 40.0),
            rect(200.0, 200.0, 10.0, 10.0),
        ];
        let marquee = rect(20.0, 20.0, 30.0, 30.0);

        // 0 与 1 都和框相交，2 完全在外面
        assert_eq!(
            marquee_hits(&objects, marquee, MarqueeMode::Intersect),
            vec![0, 1]
        );
        // 没有对象被完全包住
        assert!(marquee_hits(&objects, marquee, MarqueeMode::Contain).is_empty());
        // 放大框后两者都能完全包住（2 仍在外）
        let wide = rect(0.0, 0.0, 100.0, 100.0);
        assert_eq!(
            marquee_hits(&objects, wide, MarqueeMode::Contain),
            vec![0, 1]
        );
    }

    /// 旋转后手柄跟着转：位置绕中心旋转，命中判定同步跟随。
    #[test]
    fn rotated_handles_follow_selection() {
        let selection = rect(0.0, 0.0, 100.0, 50.0);
        let angle = std::f32::consts::FRAC_PI_2;

        // 不旋转时与轴对齐版本完全一致
        assert_eq!(
            handle_point_rotated(selection, 0.0, ResizeHandle::TopLeft),
            handle_point(selection, ResizeHandle::TopLeft)
        );

        // 转 90° 后左上角离开原位，转到 (75, -25)
        let rotated = handle_point_rotated(selection, angle, ResizeHandle::TopLeft);
        assert!((rotated.x.as_f32() - 75.0).abs() < 0.001);
        assert!((rotated.y.as_f32() + 25.0).abs() < 0.001);

        // 在新位置上点得到同一个手柄；用轴对齐判定则点不到
        assert_eq!(
            hit_handle_rotated(selection, angle, rotated, px(1.0)),
            Some(ResizeHandle::TopLeft)
        );
        assert_eq!(hit_handle(selection, rotated, px(1.0)), None);
    }

    /// 外接矩形。
    #[test]
    fn union_covers_all_rects() {
        let rects = [rect(0.0, 0.0, 10.0, 10.0), rect(20.0, 30.0, 10.0, 10.0)];
        let union = union_bounds(&rects).unwrap();
        assert_eq!(union.origin, point(px(0.0), px(0.0)));
        assert_eq!(union.size, size(px(30.0), px(40.0)));
        assert!(union_bounds(&[]).is_none());
    }

    /// 对齐：左/水平居中/顶。
    #[test]
    fn align_moves_to_union_edges() {
        let rects = [rect(0.0, 0.0, 10.0, 10.0), rect(20.0, 30.0, 30.0, 30.0)];
        let left = align(&rects, AlignMode::Left);
        assert_eq!(left[0].origin.x, px(0.0));
        assert_eq!(left[1].origin.x, px(0.0));
        let h_center = align(&rects, AlignMode::CenterHorizontal);
        // 外接宽 50，中心 25：小矩形中心 25 → x=20。
        assert_eq!(h_center[0].origin.x, px(20.0));
        let top = align(&rects, AlignMode::Top);
        assert_eq!(top[1].origin.y, px(0.0));
    }

    /// 分布：三个等宽矩形沿水平方向等间距。
    #[test]
    fn distribute_equalizes_gaps() {
        let rects = [
            rect(0.0, 0.0, 10.0, 10.0),
            rect(20.0, 0.0, 10.0, 10.0),
            rect(100.0, 0.0, 10.0, 10.0),
        ];
        let out = distribute(&rects, Axis::Horizontal);
        // 两端不动，中间一个居中于 [10, 100] 的空隙。
        assert_eq!(out[0].origin.x, px(0.0));
        assert_eq!(out[2].origin.x, px(100.0));
        // 总跨度 110，三块各 10 → 两段空隙各 40 → 中间块起点 50。
        assert_eq!(out[1].origin.x, px(50.0));
        // 少于 3 个原样返回。
        assert_eq!(
            distribute(&rects[..2], Axis::Horizontal),
            rects[..2].to_vec()
        );
    }
}
