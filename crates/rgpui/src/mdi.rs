//! 多文档界面（MDI）与子窗口管理。
//!
//! 提供经典的多文档界面：一块「MDI 区域」内并列多个**子文档窗口**，每个子窗口带标题栏、
//! 最小化 / 最大化 / 关闭按钮与八向缩放边框；支持层叠 / 平铺排布、最小化图标条、
//! 任意缩放平移、z 序激活与键盘循环切换。
//!
//! 状态由 [`MdiState`] 实体持有（窗口元数据 + 几何 + 拖拽会话），渲染由
//! [`MdiArea`] 元素承担：正文内容经 `body` 回调按窗口 id 现渲染，因此状态不持有视图，
//! 应用可随时增删文档而无需重建 MDI 区域。
//!
//! **宿主需观察状态**：`MdiArea` 只是薄壳，窗口几何的变化经 `MdiState` 通知，宿主视图
//! 必须 `cx.observe(&mdi, |_, _, cx| cx.notify())`（或订阅 [`MdiEvent`]）才能在拖拽 /
//! 缩放 / 排布时实时重绘。
//!
//! ```
//! use rgpui::{div, prelude::*, px, size, Bounds, WindowBounds, WindowOptions};
//! use rgpui::mdi::{MdiArea, MdiState};
//!
//! struct Demo {
//!     mdi: Entity<MdiState>,
//! }
//!
//! impl Render for Demo {
//!     fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
//!         div().size_full().child(
//!             MdiArea::new(&self.mdi).body(|id, _window, _cx| {
//!                 div().p_3().child(format!("文档 {id}")).into_any_element()
//!             }),
//!         )
//!     }
//! }
//! ```

use std::sync::Arc;

use crate::{
    ActiveTheme as _, AnyElement, App, Bounds, Context, CursorStyle, ElementExt as _, Entity,
    EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point,
    RenderOnce, SharedString, Size, Styled as _, Window, div, point, px, size,
};

/// 标题栏高度。
pub const TITLE_BAR_HEIGHT: Pixels = px(26.0);
/// 子窗口最小尺寸。
pub const MIN_WINDOW_SIZE: Size<Pixels> = size(px(180.0), px(120.0));
/// 层叠排布的默认错开步长。
pub const CASCADE_OFFSET: Pixels = px(26.0);
/// 子窗口与 MDI 区域边缘的默认留白。
pub const MARGIN: Pixels = px(4.0);
/// 缩放命中区厚度（像素）。
pub const RESIZE_HANDLE: f32 = 6.0;
/// 最小化图标条高度。
pub const ICON_BAR_HEIGHT: Pixels = px(28.0);
/// 最小化图标宽度。
pub const ICON_WIDTH: Pixels = px(150.0);
/// 标题栏按钮宽度。
pub const TITLE_BUTTON_WIDTH: Pixels = px(28.0);
/// 拖拽时至少保留在区域内的标题栏宽度（像素）。
pub const MIN_VISIBLE: Pixels = px(60.0);

/// 子窗口状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowState {
    /// 普通（可移动 / 缩放）。
    Normal,
    /// 最小化（缩为底部图标）。
    Minimized,
    /// 最大化（铺满 MDI 区域）。
    Maximized,
}

/// 排布方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdiLayout {
    /// 层叠：从左上角按步长错开，露出每个窗口的标题栏。
    Cascade,
    /// 水平平铺：左右并排，各占满高度。
    TileHorizontal,
    /// 垂直平铺：上下堆叠，各占满宽度。
    TileVertical,
}

/// 缩放边 / 角。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    /// 左边。
    Left,
    /// 右边。
    Right,
    /// 上边。
    Top,
    /// 下边。
    Bottom,
    /// 左上角。
    TopLeft,
    /// 右上角。
    TopRight,
    /// 左下角。
    BottomLeft,
    /// 右下角。
    BottomRight,
}

impl ResizeEdge {
    /// 是否触及左边（含左上 / 左下角）。
    pub fn touches_left(self) -> bool {
        matches!(
            self,
            ResizeEdge::Left | ResizeEdge::TopLeft | ResizeEdge::BottomLeft
        )
    }

    /// 是否触及右边（含右上 / 右下角）。
    pub fn touches_right(self) -> bool {
        matches!(
            self,
            ResizeEdge::Right | ResizeEdge::TopRight | ResizeEdge::BottomRight
        )
    }

    /// 是否触及上边（含左上 / 右上角）。
    pub fn touches_top(self) -> bool {
        matches!(
            self,
            ResizeEdge::Top | ResizeEdge::TopLeft | ResizeEdge::TopRight
        )
    }

    /// 是否触及下边（含左下 / 右下角）。
    pub fn touches_bottom(self) -> bool {
        matches!(
            self,
            ResizeEdge::Bottom | ResizeEdge::BottomLeft | ResizeEdge::BottomRight
        )
    }

    /// 该边 / 角对应的光标样式。
    pub fn cursor(self) -> CursorStyle {
        match self {
            ResizeEdge::Left => CursorStyle::ResizeLeft,
            ResizeEdge::Right => CursorStyle::ResizeRight,
            ResizeEdge::Top => CursorStyle::ResizeUp,
            ResizeEdge::Bottom => CursorStyle::ResizeDown,
            ResizeEdge::TopLeft => CursorStyle::ResizeUpLeftDownRight,
            ResizeEdge::TopRight => CursorStyle::ResizeUpRightDownLeft,
            ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
            ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
        }
    }
}

/// 标题栏按钮。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowButton {
    /// 最小化。
    Minimize,
    /// 最大化 / 还原。
    Maximize,
    /// 关闭。
    Close,
}

/// 命中结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdiHit {
    /// 未命中任何窗口。
    None,
    /// 命中某个窗口的标题栏（空白处，可拖拽）。
    TitleBar(u64),
    /// 命中某个窗口的正文。
    Body(u64),
    /// 命中某个窗口的最小化图标。
    Icon(u64),
    /// 命中标题栏按钮。
    Button {
        /// 窗口 id。
        id: u64,
        /// 按钮。
        button: WindowButton,
    },
    /// 命中缩放边 / 角。
    Resize {
        /// 窗口 id。
        id: u64,
        /// 边 / 角。
        edge: ResizeEdge,
    },
}

/// MDI 事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MdiEvent {
    /// 新窗口打开。
    Opened(u64),
    /// 窗口被激活（提到最前）。
    Activated(u64),
    /// 窗口关闭。
    Closed(u64),
    /// 窗口状态变化（最小化 / 最大化 / 还原）。
    StateChanged(u64),
}

/// 子窗口能力开关（决定标题栏按钮显隐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MdiWindowFlags {
    /// 是否可关闭。
    pub closable: bool,
    /// 是否可最小化。
    pub minimizable: bool,
    /// 是否可最大化。
    pub maximizable: bool,
}

impl Default for MdiWindowFlags {
    fn default() -> Self {
        Self {
            closable: true,
            minimizable: true,
            maximizable: true,
        }
    }
}

/// 子窗口数据。
#[derive(Debug, Clone)]
pub struct MdiWindow {
    /// 唯一标识。
    pub id: u64,
    /// 标题。
    pub title: SharedString,
    /// 普通状态下的矩形（局部坐标，相对 MDI 区域原点）。
    pub bounds: Bounds<Pixels>,
    /// 当前状态。
    pub state: WindowState,
    /// 最大化 / 最小化前的矩形。
    pub restore_bounds: Bounds<Pixels>,
    /// 是否可关闭。
    pub closable: bool,
    /// 是否可最小化。
    pub minimizable: bool,
    /// 是否可最大化。
    pub maximizable: bool,
}

/// 渲染快照：单个子窗口的版式（由 [`MdiState::layout`] 下发给元素）。
#[derive(Debug, Clone, PartialEq)]
pub struct MdiFrame {
    /// 窗口 id。
    pub id: u64,
    /// 标题。
    pub title: SharedString,
    /// 窗口矩形（局部坐标；最小化时为底部图标矩形）。
    pub rect: Bounds<Pixels>,
    /// 是否已最小化。
    pub minimized: bool,
    /// 是否已最大化。
    pub maximized: bool,
    /// 是否当前激活。
    pub active: bool,
    /// 是否可关闭。
    pub closable: bool,
    /// 是否可最小化。
    pub minimizable: bool,
    /// 是否可最大化。
    pub maximizable: bool,
}

/// 层叠排布：从左上角按 `offset` 错开，尺寸取区域的 60% × 70%。
///
/// 错开到右下角后回绕成第二圈，保证任意数量都能落在区域内。
pub fn cascade_rects(area: Bounds<Pixels>, count: usize, offset: Pixels) -> Vec<Bounds<Pixels>> {
    if count == 0 || area.size.width <= px(0.0) || area.size.height <= px(0.0) {
        return Vec::new();
    }
    let step = offset.max(px(1.0)).as_f32();
    let width = (area.size.width * 0.6)
        .max(MIN_WINDOW_SIZE.width)
        .min(area.size.width);
    let height = (area.size.height * 0.7)
        .max(MIN_WINDOW_SIZE.height)
        .min(area.size.height);
    let per_x = ((area.size.width - width).max(px(0.0)).as_f32() / step)
        .floor()
        .max(1.0);
    let per_y = ((area.size.height - height).max(px(0.0)).as_f32() / step)
        .floor()
        .max(1.0);

    (0..count)
        .map(|index| {
            let n = index as f32;
            let dx = px(step * (n % per_x));
            let dy = px(step * (n % per_y));
            Bounds::new(area.origin + point(dx, dy), size(width, height))
        })
        .collect()
}

/// 平铺排布：`horizontal` 为真时左右并排（各占满高度），否则上下堆叠（各占满宽度）。
///
/// 区域过小时仍按均分返回（不做最小尺寸兜底），由调用方决定是否夹取。
pub fn tile_rects(
    area: Bounds<Pixels>,
    count: usize,
    horizontal: bool,
    gap: Pixels,
) -> Vec<Bounds<Pixels>> {
    if count == 0 || area.size.width <= px(0.0) || area.size.height <= px(0.0) {
        return Vec::new();
    }
    let n = count as f32;
    let total_gap = (gap * (n - 1.0).max(0.0)).as_f32();

    (0..count)
        .map(|index| {
            let i = index as f32;
            if horizontal {
                let width = ((area.size.width.as_f32() - total_gap) / n).max(0.0);
                let x = area.origin.x.as_f32() + (width + gap.as_f32()) * i;
                Bounds::new(
                    point(px(x), area.origin.y),
                    size(px(width), area.size.height),
                )
            } else {
                let height = ((area.size.height.as_f32() - total_gap) / n).max(0.0);
                let y = area.origin.y.as_f32() + (height + gap.as_f32()) * i;
                Bounds::new(
                    point(area.origin.x, px(y)),
                    size(area.size.width, px(height)),
                )
            }
        })
        .collect()
}

/// 夹取子窗口矩形：不小于 `min`，且标题栏始终可见。
///
/// 水平方向允许部分越出区域右侧，但最少保留 [`MIN_VISIBLE`] 宽度；垂直方向不允许
/// 标题栏高过区域顶部，也不允许整条标题栏掉到区域底部之下。
pub fn clamp_rect(rect: Bounds<Pixels>, area: Bounds<Pixels>, min: Size<Pixels>) -> Bounds<Pixels> {
    let width = rect.size.width.max(min.width);
    let height = rect.size.height.max(min.height);
    let min_x = area.origin.x - (width - MIN_VISIBLE);
    let max_x = area.origin.x + area.size.width - MIN_VISIBLE;
    let min_y = area.origin.y;
    let max_y = area.origin.y + area.size.height - TITLE_BAR_HEIGHT;
    let x = rect.origin.x.max(min_x).min(max_x.max(min_x));
    let y = rect.origin.y.max(min_y).min(max_y.max(min_y));
    Bounds::new(point(x, y), size(width, height))
}

/// 拖动时的边缘吸附：把矩形的边/中线吸到候选矩形（工作区或邻窗）的边/中线上。
///
/// 每个轴只取最近的一条候选线，位移必须**严格小于** `threshold` 才生效；返回吸附后的
/// 矩形（尺寸不变）。应用可在拖动过程中用它算预览位置，松手时再落到最终位置。
pub fn snap_rect(
    rect: Bounds<Pixels>,
    targets: &[Bounds<Pixels>],
    threshold: Pixels,
) -> Bounds<Pixels> {
    let left = rect.origin.x.as_f32();
    let top = rect.origin.y.as_f32();
    let width = rect.size.width.as_f32();
    let height = rect.size.height.as_f32();
    let x_lines = [left, left + width, left + width / 2.0];
    let y_lines = [top, top + height, top + height / 2.0];

    let mut best_x = threshold.as_f32();
    let mut dx = 0.0;
    let mut best_y = threshold.as_f32();
    let mut dy = 0.0;

    for target in targets {
        let t_left = target.origin.x.as_f32();
        let t_top = target.origin.y.as_f32();
        let t_width = target.size.width.as_f32();
        let t_height = target.size.height.as_f32();

        for candidate in [t_left, t_left + t_width, t_left + t_width / 2.0] {
            for line in x_lines {
                let delta = candidate - line;
                if delta.abs() < best_x {
                    best_x = delta.abs();
                    dx = delta;
                }
            }
        }
        for candidate in [t_top, t_top + t_height, t_top + t_height / 2.0] {
            for line in y_lines {
                let delta = candidate - line;
                if delta.abs() < best_y {
                    best_y = delta.abs();
                    dy = delta;
                }
            }
        }
    }

    Bounds::new(
        point(rect.origin.x + px(dx), rect.origin.y + px(dy)),
        rect.size,
    )
}

/// 最小化图标矩形：沿区域底部从左到右依次排列（序号为最小化窗口的先后次序）。
pub fn icon_rect(area: Bounds<Pixels>, index: usize) -> Bounds<Pixels> {
    let width = ICON_WIDTH.min(area.size.width);
    let x = area.origin.x + px((ICON_WIDTH.as_f32() + 2.0) * index as f32);
    let y = area.origin.y + area.size.height - ICON_BAR_HEIGHT;
    Bounds::new(point(x, y), size(width, ICON_BAR_HEIGHT))
}

/// 标题栏按钮矩形：自右向左依次为关闭 / 最大化 / 最小化。
pub fn title_button_rects(rect: Bounds<Pixels>) -> [(WindowButton, Bounds<Pixels>); 3] {
    [
        WindowButton::Close,
        WindowButton::Maximize,
        WindowButton::Minimize,
    ]
    .map(|button| {
        let index = match button {
            WindowButton::Close => 0.0,
            WindowButton::Maximize => 1.0,
            WindowButton::Minimize => 2.0,
        };
        let x = rect.origin.x + rect.size.width - TITLE_BUTTON_WIDTH * (index + 1.0);
        (
            button,
            Bounds::new(
                point(x, rect.origin.y),
                size(TITLE_BUTTON_WIDTH, TITLE_BAR_HEIGHT),
            ),
        )
    })
}

/// 命中缩放边 / 角（角优先于边）。未命中返回 `None`。
pub fn resize_edge_at(rect: Bounds<Pixels>, pointer: Point<Pixels>) -> Option<ResizeEdge> {
    if !rect.contains(&pointer) {
        return None;
    }
    let t = px(RESIZE_HANDLE);
    let left = pointer.x - rect.origin.x <= t;
    let right = rect.origin.x + rect.size.width - pointer.x <= t;
    let top = pointer.y - rect.origin.y <= t;
    let bottom = rect.origin.y + rect.size.height - pointer.y <= t;
    Some(match (left, right, top, bottom) {
        (true, _, true, _) => ResizeEdge::TopLeft,
        (_, true, true, _) => ResizeEdge::TopRight,
        (true, _, _, true) => ResizeEdge::BottomLeft,
        (_, true, _, true) => ResizeEdge::BottomRight,
        (true, _, _, _) => ResizeEdge::Left,
        (_, true, _, _) => ResizeEdge::Right,
        (_, _, true, _) => ResizeEdge::Top,
        (_, _, _, true) => ResizeEdge::Bottom,
        _ => return None,
    })
}

/// 按边 / 角缩放矩形：`delta` 为指针位移，`min` 为最小尺寸。
///
/// 拖左 / 上边时以右 / 下边为锚，够不到 `min` 时整体回退，保证不出现负尺寸。
pub fn resize_bounds(
    start: Bounds<Pixels>,
    edge: ResizeEdge,
    delta: Point<Pixels>,
    min: Size<Pixels>,
) -> Bounds<Pixels> {
    let mut left = start.origin.x;
    let mut top = start.origin.y;
    let mut right = start.origin.x + start.size.width;
    let mut bottom = start.origin.y + start.size.height;

    if edge.touches_left() {
        left += delta.x;
    }
    if edge.touches_right() {
        right += delta.x;
    }
    if edge.touches_top() {
        top += delta.y;
    }
    if edge.touches_bottom() {
        bottom += delta.y;
    }

    if right - left < min.width {
        if edge.touches_left() {
            left = right - min.width;
        } else {
            right = left + min.width;
        }
    }
    if bottom - top < min.height {
        if edge.touches_top() {
            top = bottom - min.height;
        } else {
            bottom = top + min.height;
        }
    }
    Bounds::new(point(left, top), size(right - left, bottom - top))
}

/// 拖拽会话。
#[derive(Debug, Clone, Copy)]
struct DragSession {
    /// 目标窗口。
    id: u64,
    /// 拖拽类型。
    kind: DragKind,
    /// 按下时的指针位置（局部坐标）。
    start_pointer: Point<Pixels>,
    /// 按下时的窗口矩形。
    start_rect: Bounds<Pixels>,
}

/// 拖拽类型。
#[derive(Debug, Clone, Copy)]
enum DragKind {
    /// 移动窗口。
    Move,
    /// 缩放窗口。
    Resize(ResizeEdge),
    /// 按住标题栏按钮（释放时判定是否命中同一按钮）。
    Button(WindowButton),
}

/// MDI 状态实体：窗口集合（按 z 序，末尾在最前）、激活窗口、区域与拖拽会话。
pub struct MdiState {
    /// 子窗口（后 → 前）。
    windows: Vec<MdiWindow>,
    /// 当前激活窗口。
    active: Option<u64>,
    /// 下一个窗口 id。
    next_id: u64,
    /// MDI 区域（窗口坐标，由视图预绘制同步）。
    area: Bounds<Pixels>,
    /// 子窗口最小尺寸。
    min_size: Size<Pixels>,
    /// 层叠步长。
    cascade_offset: Pixels,
    /// 平铺间隔。
    tile_gap: Pixels,
    /// 尚未取得区域时挂起的排布请求。
    pending_layout: Option<MdiLayout>,
    /// 拖拽会话。
    drag: Option<DragSession>,
    /// 焦点句柄。
    focus_handle: FocusHandle,
}

impl MdiState {
    /// 创建 MDI 状态。
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            windows: Vec::new(),
            active: None,
            next_id: 1,
            area: Bounds::default(),
            min_size: MIN_WINDOW_SIZE,
            cascade_offset: CASCADE_OFFSET,
            tile_gap: px(6.0),
            pending_layout: None,
            drag: None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// 设置子窗口最小尺寸。
    pub fn with_min_size(mut self, min: Size<Pixels>) -> Self {
        self.min_size = min;
        self
    }

    /// 设置层叠步长。
    pub fn with_cascade_offset(mut self, offset: Pixels) -> Self {
        self.cascade_offset = offset.max(px(1.0));
        self
    }

    /// 设置平铺间隔。
    pub fn with_tile_gap(mut self, gap: Pixels) -> Self {
        self.tile_gap = gap.max(px(0.0));
        self
    }

    /// 全部子窗口（后 → 前）。
    pub fn windows(&self) -> &[MdiWindow] {
        &self.windows
    }

    /// 窗口数量。
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    /// 是否没有窗口。
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// 当前激活窗口 id。
    pub fn active(&self) -> Option<u64> {
        self.active
    }

    /// 按 id 取窗口。
    pub fn get(&self, id: u64) -> Option<&MdiWindow> {
        self.windows.iter().find(|window| window.id == id)
    }

    /// MDI 区域（窗口坐标）。
    pub fn area(&self) -> Bounds<Pixels> {
        self.area
    }

    /// 某个窗口的当前矩形（局部坐标）：最大化铺满区域、最小化落在底部图标条。
    pub fn rect_of(&self, id: u64) -> Option<Bounds<Pixels>> {
        self.get(id).and_then(|window| self.rect_in(window))
    }

    /// 渲染快照（后 → 前，末尾在最上层）。
    pub fn layout(&self) -> Vec<MdiFrame> {
        self.windows
            .iter()
            .map(|window| MdiFrame {
                id: window.id,
                title: window.title.clone(),
                rect: self.rect_in(window).unwrap_or_default(),
                minimized: window.state == WindowState::Minimized,
                maximized: window.state == WindowState::Maximized,
                active: self.active == Some(window.id),
                closable: window.closable,
                minimizable: window.minimizable,
                maximizable: window.maximizable,
            })
            .collect()
    }

    /// 打开一个新窗口并激活，返回其 id。
    pub fn open(&mut self, title: impl Into<SharedString>, cx: &mut Context<Self>) -> u64 {
        self.open_with(title, MdiWindowFlags::default(), cx)
    }

    /// 按能力开关打开一个新窗口并激活，返回其 id。
    pub fn open_with(
        &mut self,
        title: impl Into<SharedString>,
        flags: MdiWindowFlags,
        cx: &mut Context<Self>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let bounds = self.default_bounds();
        self.windows.push(MdiWindow {
            id,
            title: title.into(),
            bounds,
            state: WindowState::Normal,
            restore_bounds: bounds,
            closable: flags.closable,
            minimizable: flags.minimizable,
            maximizable: flags.maximizable,
        });
        self.active = Some(id);
        cx.emit(MdiEvent::Opened(id));
        cx.notify();
        id
    }

    /// 关闭窗口，返回是否存在该窗口。
    pub fn close(&mut self, id: u64, cx: &mut Context<Self>) -> bool {
        let Some(index) = self.windows.iter().position(|window| window.id == id) else {
            return false;
        };
        self.windows.remove(index);
        if self.active == Some(id) {
            self.active = self.windows.last().map(|window| window.id);
        }
        if self.drag.map(|drag| drag.id) == Some(id) {
            self.drag = None;
        }
        cx.emit(MdiEvent::Closed(id));
        cx.notify();
        true
    }

    /// 关闭当前激活窗口。
    pub fn close_active(&mut self, cx: &mut Context<Self>) -> bool {
        match self.active {
            Some(id) => self.close(id, cx),
            None => false,
        }
    }

    /// 关闭全部窗口。
    pub fn close_all(&mut self, cx: &mut Context<Self>) {
        while let Some(window) = self.windows.last() {
            let id = window.id;
            self.close(id, cx);
        }
    }

    /// 激活窗口（提到最前），返回是否发生变化。
    pub fn activate(&mut self, id: u64, cx: &mut Context<Self>) -> bool {
        let Some(index) = self.windows.iter().position(|window| window.id == id) else {
            return false;
        };
        let already_front = index + 1 == self.windows.len();
        if !already_front {
            let window = self.windows.remove(index);
            self.windows.push(window);
        }
        let active_changed = self.active != Some(id);
        self.active = Some(id);
        if active_changed {
            cx.emit(MdiEvent::Activated(id));
        }
        if !already_front || active_changed {
            cx.notify();
        }
        !already_front || active_changed
    }

    /// 循环切换窗口：`forward` 为真时切换到下一个，否则上一个。
    pub fn cycle(&mut self, forward: bool, cx: &mut Context<Self>) -> Option<u64> {
        if self.windows.is_empty() {
            return None;
        }
        let len = self.windows.len();
        let current = self
            .active
            .and_then(|id| self.windows.iter().position(|window| window.id == id));
        let next = match current {
            Some(index) if forward => (index + 1) % len,
            Some(index) => (index + len - 1) % len,
            None => 0,
        };
        let id = self.windows[next].id;
        self.activate(id, cx);
        Some(id)
    }

    /// 设置窗口矩形（夹取到可见范围）。
    pub fn set_bounds(&mut self, id: u64, bounds: Bounds<Pixels>, cx: &mut Context<Self>) -> bool {
        let rect = clamp_rect(bounds, self.local_area(), self.min_size);
        let Some(window) = self.windows.iter_mut().find(|window| window.id == id) else {
            return false;
        };
        window.state = WindowState::Normal;
        if window.bounds == rect {
            return false;
        }
        window.bounds = rect;
        window.restore_bounds = rect;
        cx.notify();
        true
    }

    /// 相对移动窗口。
    pub fn move_by(&mut self, id: u64, delta: Point<Pixels>, cx: &mut Context<Self>) -> bool {
        let Some(window) = self.get(id) else {
            return false;
        };
        let bounds = Bounds::new(window.bounds.origin + delta, window.bounds.size);
        self.set_bounds(id, bounds, cx)
    }

    /// 让活动窗口吸附到工作区边缘与其他普通窗口的边缘（`threshold` 内）。
    ///
    /// 典型用法是拖动结束时调一次；拖动过程中可以先用 [`snap_rect`] 算预览位置。
    /// 返回窗口位置是否真的发生了变化。
    pub fn snap_active(&mut self, threshold: Pixels, cx: &mut Context<Self>) -> bool {
        let Some(active) = self.active() else {
            return false;
        };
        let Some(rect) = self.rect_of(active) else {
            return false;
        };

        // 候选线：工作区边界 + 其余普通窗口（最小化/最大化窗口不参与）
        let mut targets = vec![self.local_area()];
        targets.extend(
            self.windows
                .iter()
                .filter(|window| window.id != active && window.state == WindowState::Normal)
                .map(|window| window.bounds),
        );

        let snapped = snap_rect(rect, &targets, threshold);
        if snapped == rect {
            return false;
        }
        self.set_bounds(active, snapped, cx)
    }

    /// 最小化窗口。
    pub fn minimize(&mut self, id: u64, cx: &mut Context<Self>) -> bool {
        self.set_state(id, WindowState::Minimized, cx)
    }

    /// 最大化窗口。
    pub fn maximize(&mut self, id: u64, cx: &mut Context<Self>) -> bool {
        self.set_state(id, WindowState::Maximized, cx)
    }

    /// 还原窗口（最大化 / 最小化 → 普通）。
    pub fn restore(&mut self, id: u64, cx: &mut Context<Self>) -> bool {
        self.set_state(id, WindowState::Normal, cx)
    }

    /// 在最大化与还原之间切换。
    pub fn toggle_maximize(&mut self, id: u64, cx: &mut Context<Self>) -> bool {
        match self.get(id).map(|window| window.state) {
            Some(WindowState::Maximized) => self.restore(id, cx),
            Some(_) => self.maximize(id, cx),
            None => false,
        }
    }

    /// 按给定方式排布所有非最小化窗口（并把最大化的窗口还原）。
    pub fn arrange(&mut self, layout: MdiLayout, cx: &mut Context<Self>) -> bool {
        let changed = self.apply_layout(layout);
        if changed {
            cx.notify();
        }
        changed
    }

    /// 由视图预绘制同步 MDI 区域；首次取得区域时自动层叠排布已有窗口。
    pub fn set_area(&mut self, area: Bounds<Pixels>, cx: &mut Context<Self>) {
        let first = self.area.size.width <= px(0.0) || self.area.size.height <= px(0.0);
        if first && area == self.area {
            return;
        }
        if !first && area == self.area {
            return;
        }
        self.area = area;

        let mut changed = false;
        if first && !self.windows.is_empty() {
            let layout = self.pending_layout.take().unwrap_or(MdiLayout::Cascade);
            changed = self.apply_layout(layout);
        } else {
            let local = self.local_area();
            let min = self.min_size;
            for window in &mut self.windows {
                if window.state != WindowState::Normal {
                    continue;
                }
                let clamped = clamp_rect(window.bounds, local, min);
                if clamped != window.bounds {
                    window.bounds = clamped;
                    changed = true;
                }
            }
        }
        if changed {
            cx.notify();
        }
    }

    /// 命中测试（`pointer` 为局部坐标）。
    pub fn hit_test(&self, pointer: Point<Pixels>) -> MdiHit {
        for window in self.windows.iter().rev() {
            let Some(rect) = self.rect_in(window) else {
                continue;
            };
            if !rect.contains(&pointer) {
                continue;
            }
            if window.state == WindowState::Minimized {
                return MdiHit::Icon(window.id);
            }
            if window.state == WindowState::Normal
                && let Some(edge) = resize_edge_at(rect, pointer)
            {
                return MdiHit::Resize {
                    id: window.id,
                    edge,
                };
            }
            for (button, button_rect) in title_button_rects(rect) {
                if button_rect.contains(&pointer) && Self::button_enabled(window, button) {
                    return MdiHit::Button {
                        id: window.id,
                        button,
                    };
                }
            }
            if pointer.y - rect.origin.y <= TITLE_BAR_HEIGHT {
                return MdiHit::TitleBar(window.id);
            }
            return MdiHit::Body(window.id);
        }
        MdiHit::None
    }

    /// 指针按下（`local` 为局部坐标）。
    pub fn on_pointer_down(
        &mut self,
        local: Point<Pixels>,
        click_count: usize,
        cx: &mut Context<Self>,
    ) {
        let hit = self.hit_test(local);
        match hit {
            MdiHit::None => {
                self.drag = None;
            }
            MdiHit::TitleBar(id) => {
                self.activate(id, cx);
                if click_count >= 2 {
                    self.drag = None;
                    self.toggle_maximize(id, cx);
                    return;
                }
                if let Some(rect) = self.rect_of(id) {
                    self.drag = Some(DragSession {
                        id,
                        kind: DragKind::Move,
                        start_pointer: local,
                        start_rect: rect,
                    });
                }
            }
            MdiHit::Body(id) => {
                self.drag = None;
                self.activate(id, cx);
            }
            MdiHit::Icon(id) => {
                self.drag = None;
                self.restore(id, cx);
                self.activate(id, cx);
            }
            MdiHit::Button { id, button } => {
                self.activate(id, cx);
                if let Some(rect) = self.rect_of(id) {
                    self.drag = Some(DragSession {
                        id,
                        kind: DragKind::Button(button),
                        start_pointer: local,
                        start_rect: rect,
                    });
                }
            }
            MdiHit::Resize { id, edge } => {
                self.activate(id, cx);
                if let Some(rect) = self.rect_of(id) {
                    self.drag = Some(DragSession {
                        id,
                        kind: DragKind::Resize(edge),
                        start_pointer: local,
                        start_rect: rect,
                    });
                }
            }
        }
        cx.notify();
    }

    /// 指针移动（`local` 为局部坐标）。
    pub fn on_pointer_move(&mut self, local: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(drag) = self.drag else {
            return;
        };
        let delta = local - drag.start_pointer;
        match drag.kind {
            DragKind::Move => {
                let origin = drag.start_rect.origin + delta;
                let rect = clamp_rect(
                    Bounds::new(origin, drag.start_rect.size),
                    self.local_area(),
                    self.min_size,
                );
                if self.apply_bounds(drag.id, rect) {
                    cx.notify();
                }
            }
            DragKind::Resize(edge) => {
                let rect = resize_bounds(drag.start_rect, edge, delta, self.min_size);
                let rect = clamp_rect(rect, self.local_area(), self.min_size);
                if self.apply_bounds(drag.id, rect) {
                    cx.notify();
                }
            }
            DragKind::Button(_) => {}
        }
    }

    /// 指针释放（`local` 为局部坐标）：命中同一标题栏按钮时执行对应操作。
    pub fn on_pointer_up(&mut self, local: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        if let DragKind::Button(button) = drag.kind
            && self.hit_test(local)
                == (MdiHit::Button {
                    id: drag.id,
                    button,
                })
        {
            match button {
                WindowButton::Close => self.close(drag.id, cx),
                WindowButton::Minimize => self.minimize(drag.id, cx),
                WindowButton::Maximize => self.toggle_maximize(drag.id, cx),
            };
        }
        cx.notify();
    }

    /// 键盘：`Ctrl+Tab`/`Ctrl+F6` 循环切换、`Ctrl+F4` 关闭、`Ctrl+M` 最小化、`Esc` 取消拖拽。
    pub fn on_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.to_ascii_lowercase();
        let modifiers = event.keystroke.modifiers;
        match key.as_str() {
            "tab" | "f6" if modifiers.control => {
                self.cycle(!modifiers.shift, cx);
            }
            "f4" if modifiers.control => {
                self.close_active(cx);
            }
            "m" if modifiers.control => {
                if let Some(id) = self.active {
                    self.minimize(id, cx);
                }
            }
            "escape" => {
                self.drag = None;
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    /// 焦点击柄。
    pub fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }

    /// 区域（局部坐标）：原点固定为 `(0, 0)`。
    fn local_area(&self) -> Bounds<Pixels> {
        Bounds::new(
            point(px(0.0), px(0.0)),
            size(self.area.size.width, self.area.size.height),
        )
    }

    /// 子窗口可用区域：MDI 区域四周留出 [`MARGIN`]。
    fn content_area(&self) -> Bounds<Pixels> {
        let local = self.local_area();
        Bounds::new(
            local.origin + point(MARGIN, MARGIN),
            size(
                (local.size.width - MARGIN * 2.0).max(px(0.0)),
                (local.size.height - MARGIN * 2.0).max(px(0.0)),
            ),
        )
    }

    /// 某个窗口的当前矩形。
    fn rect_in(&self, window: &MdiWindow) -> Option<Bounds<Pixels>> {
        match window.state {
            WindowState::Normal => Some(window.bounds),
            WindowState::Maximized => Some(self.local_area()),
            WindowState::Minimized => {
                let index = self.icon_index(window.id)?;
                Some(icon_rect(self.local_area(), index))
            }
        }
    }

    /// 窗口在最小化图标条中的序号。
    fn icon_index(&self, id: u64) -> Option<usize> {
        self.windows
            .iter()
            .filter(|window| window.state == WindowState::Minimized)
            .position(|window| window.id == id)
    }

    /// 新窗口的默认矩形：已知区域时按层叠位；否则用最小尺寸兜底。
    fn default_bounds(&self) -> Bounds<Pixels> {
        let fallback = Bounds::new(
            point(px(0.0), px(0.0)),
            size(
                MIN_WINDOW_SIZE.width.max(px(360.0)),
                MIN_WINDOW_SIZE.height.max(px(280.0)),
            ),
        );
        if self.area.size.width <= px(0.0) || self.area.size.height <= px(0.0) {
            return fallback;
        }
        let count = self.windows.len() + 1;
        cascade_rects(self.content_area(), count, self.cascade_offset)
            .get(count - 1)
            .copied()
            .unwrap_or(fallback)
    }

    /// 直接写入矩形（不做夹取），用于拖拽路径。
    fn apply_bounds(&mut self, id: u64, rect: Bounds<Pixels>) -> bool {
        let Some(window) = self.windows.iter_mut().find(|window| window.id == id) else {
            return false;
        };
        if window.bounds == rect {
            return false;
        }
        window.bounds = rect;
        window.restore_bounds = rect;
        true
    }

    /// 切换窗口状态。
    fn set_state(&mut self, id: u64, state: WindowState, cx: &mut Context<Self>) -> bool {
        let local = self.local_area();
        let min = self.min_size;
        let Some(window) = self.windows.iter_mut().find(|window| window.id == id) else {
            return false;
        };
        if window.state == state {
            return false;
        }
        match state {
            WindowState::Normal => {
                if window.state == WindowState::Maximized {
                    window.bounds = clamp_rect(window.restore_bounds, local, min);
                }
            }
            WindowState::Maximized | WindowState::Minimized => {
                if window.state == WindowState::Normal {
                    window.restore_bounds = window.bounds;
                }
            }
        }
        window.state = state;
        cx.emit(MdiEvent::StateChanged(id));
        cx.notify();
        true
    }

    /// 施加排布；区域未知时挂起。返回是否真正改动了几何。
    fn apply_layout(&mut self, layout: MdiLayout) -> bool {
        if self.area.size.width <= px(0.0) || self.area.size.height <= px(0.0) {
            self.pending_layout = Some(layout);
            return false;
        }
        let area = self.content_area();
        let ids: Vec<u64> = self
            .windows
            .iter()
            .filter(|window| window.state != WindowState::Minimized)
            .map(|window| window.id)
            .collect();
        if ids.is_empty() {
            return false;
        }
        let rects = match layout {
            MdiLayout::Cascade => cascade_rects(area, ids.len(), self.cascade_offset),
            MdiLayout::TileHorizontal => tile_rects(area, ids.len(), true, self.tile_gap),
            MdiLayout::TileVertical => tile_rects(area, ids.len(), false, self.tile_gap),
        };
        let mut changed = false;
        for (id, rect) in ids.into_iter().zip(rects) {
            if let Some(window) = self.windows.iter_mut().find(|window| window.id == id) {
                if window.state == WindowState::Maximized || window.bounds != rect {
                    changed = true;
                }
                window.state = WindowState::Normal;
                window.bounds = rect;
                window.restore_bounds = rect;
            }
        }
        self.pending_layout = None;
        changed
    }

    /// 按钮是否对该窗口可用。
    fn button_enabled(window: &MdiWindow, button: WindowButton) -> bool {
        match button {
            WindowButton::Close => window.closable,
            WindowButton::Minimize => window.minimizable,
            WindowButton::Maximize => window.maximizable,
        }
    }
}

impl Focusable for MdiState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.focus_handle(cx)
    }
}

impl EventEmitter<MdiEvent> for MdiState {}

/// MDI 区域元素：渲染子窗口框架（标题栏 + 正文 + 按钮 + 缩放边），并把指针 / 键盘
/// 事件转发给 [`MdiState`]。
///
/// 本元素只是薄壳，宿主视图需观察 [`MdiState`]（`cx.observe`）或订阅 [`MdiEvent`]，
/// 才能在拖拽 / 排布等几何变化时实时重绘。
#[derive(IntoElement)]
pub struct MdiArea {
    /// 状态实体。
    state: Entity<MdiState>,
    /// 正文渲染回调（按窗口 id 现渲染）。
    body: Arc<dyn Fn(u64, &mut Window, &mut App) -> AnyElement + 'static>,
}

impl MdiArea {
    /// 使用给定状态创建 MDI 区域（正文默认为空白）。
    pub fn new(state: &Entity<MdiState>) -> Self {
        Self {
            state: state.clone(),
            body: Arc::new(|_id, _window, _cx| div().into_any_element()),
        }
    }

    /// 设置正文渲染回调。
    pub fn body<F>(mut self, body: F) -> Self
    where
        F: Fn(u64, &mut Window, &mut App) -> AnyElement + 'static,
    {
        self.body = Arc::new(body);
        self
    }
}

impl RenderOnce for MdiArea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self { state, body } = self;
        let focus_handle = state.read(cx).focus_handle(cx);
        let frames = state.read(cx).layout();
        let theme = cx.theme();
        let border = theme.border;
        let primary = theme.primary;
        let muted = theme.muted;
        let secondary = theme.secondary;
        let foreground = theme.foreground;
        let muted_foreground = theme.muted_foreground;
        let surface = theme.tokens.popover;

        let children: Vec<AnyElement> = frames
            .into_iter()
            .map(|frame| {
                let active_border = if frame.active { primary } else { border };
                let title_bg = if frame.active { secondary } else { muted };
                let title_fg = if frame.active {
                    foreground
                } else {
                    muted_foreground
                };
                if frame.minimized {
                    return div()
                        .id(("mdi-icon", frame.id))
                        .absolute()
                        .left(frame.rect.origin.x)
                        .top(frame.rect.origin.y)
                        .w(frame.rect.size.width)
                        .h(frame.rect.size.height)
                        .flex()
                        .items_center()
                        .gap_1()
                        .px_2()
                        .rounded_sm()
                        .bg(title_bg)
                        .border_1()
                        .border_color(active_border)
                        .text_xs()
                        .text_color(title_fg)
                        .overflow_hidden()
                        .cursor_pointer()
                        .child(div().flex_1().overflow_hidden().child(frame.title))
                        .into_any_element();
                }

                // 标题栏按钮（视觉；点击由容器命中测试统一处理）。
                let buttons: Vec<AnyElement> = title_button_rects(frame.rect)
                    .into_iter()
                    .filter(|(button, _)| match button {
                        WindowButton::Close => frame.closable,
                        WindowButton::Minimize => frame.minimizable,
                        WindowButton::Maximize => frame.maximizable,
                    })
                    .map(|(button, rect)| {
                        let (glyph, element_id) = match (button, frame.maximized) {
                            (WindowButton::Close, _) => ("✕", "mdi-button-close"),
                            (WindowButton::Minimize, _) => ("—", "mdi-button-minimize"),
                            (WindowButton::Maximize, true) => ("❐", "mdi-button-restore"),
                            (WindowButton::Maximize, false) => ("□", "mdi-button-maximize"),
                        };
                        div()
                            .id((element_id, frame.id))
                            .absolute()
                            .left(rect.origin.x - frame.rect.origin.x)
                            .top(rect.origin.y - frame.rect.origin.y)
                            .w(rect.size.width)
                            .h(rect.size.height)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(title_fg)
                            .cursor_pointer()
                            .hover(move |style| style.bg(muted))
                            .child(glyph)
                            .into_any_element()
                    })
                    .collect();

                // 八向缩放边框（最大化时不可缩放）。
                let handles: Vec<AnyElement> = if frame.maximized {
                    Vec::new()
                } else {
                    resize_edge_rects(frame.rect)
                        .into_iter()
                        .map(|(edge, rect)| {
                            div()
                                .absolute()
                                .left(rect.origin.x - frame.rect.origin.x)
                                .top(rect.origin.y - frame.rect.origin.y)
                                .w(rect.size.width)
                                .h(rect.size.height)
                                .cursor(edge.cursor())
                                .into_any_element()
                        })
                        .collect()
                };

                let content = body(frame.id, window, cx);

                div()
                    .id(("mdi-window", frame.id))
                    .absolute()
                    .left(frame.rect.origin.x)
                    .top(frame.rect.origin.y)
                    .w(frame.rect.size.width)
                    .h(frame.rect.size.height)
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded_sm()
                    .bg(surface)
                    .border_1()
                    .border_color(active_border)
                    .shadow_lg()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .flex_none()
                            .h(TITLE_BAR_HEIGHT)
                            .px_2()
                            .bg(title_bg)
                            .border_b_1()
                            .border_color(border)
                            .child(
                                div()
                                    .flex_1()
                                    .overflow_hidden()
                                    .text_sm()
                                    .text_color(title_fg)
                                    .child(frame.title),
                            ),
                    )
                    .child(div().flex_1().overflow_hidden().child(content))
                    .children(buttons)
                    .children(handles)
                    .into_any_element()
            })
            .collect();

        div()
            .id(("mdi-area", state.entity_id()))
            .key_context("Mdi")
            .track_focus(&focus_handle)
            .relative()
            .overflow_hidden()
            .size_full()
            .bg(muted)
            .on_prepaint({
                let state = state.clone();
                move |bounds, _window, cx| {
                    state.update(cx, |state, cx| state.set_area(bounds, cx));
                }
            })
            .on_key_down(
                window.listener_for(&state, |state, event: &KeyDownEvent, _window, cx| {
                    state.on_key_down(event, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Left,
                window.listener_for(&state, |state, event: &MouseDownEvent, _window, cx| {
                    let local = event.position - state.area().origin;
                    state.on_pointer_down(local, event.click_count, cx);
                }),
            )
            .on_mouse_move(window.listener_for(
                &state,
                |state, event: &MouseMoveEvent, _window, cx| {
                    let local = event.position - state.area().origin;
                    state.on_pointer_move(local, cx);
                },
            ))
            .capture_any_mouse_up(window.listener_for(
                &state,
                |state, event: &MouseUpEvent, _window, cx| {
                    let local = event.position - state.area().origin;
                    state.on_pointer_up(local, cx);
                },
            ))
            .children(children)
    }
}

/// 缩放边 / 角命中矩形（局部坐标）：四条边 + 四个角。
pub fn resize_edge_rects(rect: Bounds<Pixels>) -> Vec<(ResizeEdge, Bounds<Pixels>)> {
    let t = px(RESIZE_HANDLE);
    let (x, y) = (rect.origin.x, rect.origin.y);
    let (w, h) = (rect.size.width, rect.size.height);
    let inner_w = (w - t * 2.0).max(px(0.0));
    let inner_h = (h - t * 2.0).max(px(0.0));
    vec![
        (ResizeEdge::TopLeft, Bounds::new(point(x, y), size(t, t))),
        (
            ResizeEdge::TopRight,
            Bounds::new(point(x + w - t, y), size(t, t)),
        ),
        (
            ResizeEdge::BottomLeft,
            Bounds::new(point(x, y + h - t), size(t, t)),
        ),
        (
            ResizeEdge::BottomRight,
            Bounds::new(point(x + w - t, y + h - t), size(t, t)),
        ),
        (
            ResizeEdge::Left,
            Bounds::new(point(x, y + t), size(t, inner_h)),
        ),
        (
            ResizeEdge::Right,
            Bounds::new(point(x + w - t, y + t), size(t, inner_h)),
        ),
        (
            ResizeEdge::Top,
            Bounds::new(point(x + t, y), size(inner_w, t)),
        ),
        (
            ResizeEdge::Bottom,
            Bounds::new(point(x + t, y + h - t), size(inner_w, t)),
        ),
    ]
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{
        CASCADE_OFFSET, MIN_WINDOW_SIZE, MdiArea, MdiEvent, MdiHit, MdiLayout, MdiState, MdiWindow,
        MdiWindowFlags, ResizeEdge, TITLE_BAR_HEIGHT, TITLE_BUTTON_WIDTH, WindowButton,
        WindowState, cascade_rects, clamp_rect, icon_rect, resize_bounds, resize_edge_at,
        snap_rect, tile_rects, title_button_rects,
    };
    use crate::{
        AppContext as _, Bounds, Context, Entity, IntoElement, KeyDownEvent, Keystroke, Modifiers,
        ParentElement as _, Pixels, Point, Render, Styled as _, TestAppContext, Window, div, point,
        px, size,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    /// 800×600 的区域。
    fn area() -> Bounds<Pixels> {
        Bounds::new(point(px(0.0), px(0.0)), size(px(800.0), px(600.0)))
    }

    /// 某个窗口的局部原点。
    fn origin_of(window: &MdiWindow) -> Point<Pixels> {
        window.bounds.origin
    }

    /// 读取宿主实际区域（首次绘制后由预绘制写入真实窗口尺寸）。
    fn host_area(state: &Entity<MdiState>, cx: &mut crate::VisualTestContext) -> Bounds<Pixels> {
        state.read_with(&*cx, |state, _| state.area())
    }

    /// 层叠与平铺都落在区域内，且平铺正好铺满。
    #[test]
    fn cascade_and_tile_fit_area() {
        let area = area();
        let cascade = cascade_rects(area, 5, CASCADE_OFFSET);
        assert_eq!(cascade.len(), 5);
        for rect in &cascade {
            assert!(area.contains(&rect.origin));
            assert!(rect.size.width <= area.size.width);
            assert!(rect.size.height <= area.size.height);
        }
        // 每个窗口相对前一个错开一个步长（未回绕时）。
        assert_eq!(cascade[1].origin.x - cascade[0].origin.x, CASCADE_OFFSET);
        assert_ne!(cascade[0].origin, cascade[3].origin);

        // 水平平铺：等宽、铺满高度、总宽 + 间隔 = 区域宽。
        let tiled = tile_rects(area, 4, true, px(0.0));
        assert_eq!(tiled.len(), 4);
        assert_eq!(tiled[0].size.width, px(200.0));
        assert_eq!(tiled[0].size.height, area.size.height);
        assert_eq!(tiled[3].origin.x + tiled[3].size.width, area.size.width);

        // 垂直平铺：等高、铺满宽度。
        let stacked = tile_rects(area, 2, false, px(10.0));
        assert_eq!(stacked[0].size.width, area.size.width);
        assert_eq!(stacked[0].size.height, px(295.0));
        assert_eq!(stacked[1].origin.y, px(305.0));

        // 空集合与零尺寸区域返回空。
        assert!(tile_rects(area, 0, true, px(0.0)).is_empty());
        assert!(cascade_rects(Bounds::default(), 3, CASCADE_OFFSET).is_empty());
    }

    /// 夹取保证最小尺寸与标题栏可见。
    #[test]
    fn clamp_keeps_min_size_and_title_bar_visible() {
        let area = area();
        let tiny = Bounds::new(point(px(10.0), px(10.0)), size(px(20.0), px(20.0)));
        let clamped = clamp_rect(tiny, area, MIN_WINDOW_SIZE);
        assert_eq!(clamped.size, MIN_WINDOW_SIZE);
        assert_eq!(clamped.origin, point(px(10.0), px(10.0)));

        // 左上角越界：被推回区域顶部，且最少保留 MIN_VISIBLE 宽度可见。
        let off = Bounds::new(point(px(-999.0), px(-999.0)), size(px(200.0), px(150.0)));
        let clamped = clamp_rect(off, area, MIN_WINDOW_SIZE);
        assert_eq!(clamped.origin.y, px(0.0));
        assert_eq!(clamped.origin.x, px(-140.0));

        // 右下角越界，但不允许标题栏掉出底部。
        let far = Bounds::new(point(px(900.0), px(900.0)), size(px(200.0), px(150.0)));
        let clamped = clamp_rect(far, area, MIN_WINDOW_SIZE);
        assert_eq!(clamped.origin.x, px(740.0));
        assert_eq!(clamped.origin.y, px(600.0) - TITLE_BAR_HEIGHT);
    }

    /// 缩放边 / 角判定与最小尺寸约束。
    #[test]
    fn resize_edges_and_min_size() {
        let rect = Bounds::new(point(px(100.0), px(100.0)), size(px(200.0), px(150.0)));
        assert_eq!(
            resize_edge_at(rect, point(px(101.0), px(175.0))),
            Some(ResizeEdge::Left)
        );
        assert_eq!(
            resize_edge_at(rect, point(px(299.0), px(100.5))),
            Some(ResizeEdge::TopRight)
        );
        assert_eq!(resize_edge_at(rect, point(px(200.0), px(200.0))), None);
        assert_eq!(resize_edge_at(rect, point(px(0.0), px(0.0))), None);

        // 右 + 下：变大。
        let grown = resize_bounds(
            rect,
            ResizeEdge::BottomRight,
            point(px(50.0), px(25.0)),
            MIN_WINDOW_SIZE,
        );
        assert_eq!(grown.size, size(px(250.0), px(175.0)));
        // 左 + 上：向负方向收小；宽度收过最小尺寸时以右边为锚回退。
        let shrunk = resize_bounds(
            rect,
            ResizeEdge::TopLeft,
            point(px(30.0), px(20.0)),
            MIN_WINDOW_SIZE,
        );
        assert_eq!(shrunk.origin, point(px(120.0), px(120.0)));
        assert_eq!(shrunk.size, size(MIN_WINDOW_SIZE.width, px(130.0)));
        // 收过头时按最小尺寸回退，锚点保持在右 / 下边。
        let floored = resize_bounds(
            rect,
            ResizeEdge::Left,
            point(px(500.0), px(0.0)),
            MIN_WINDOW_SIZE,
        );
        assert_eq!(floored.origin.x, px(300.0) - MIN_WINDOW_SIZE.width);
        assert_eq!(floored.size.width, MIN_WINDOW_SIZE.width);
    }

    /// 标题栏按钮自右向左为关闭 / 最大化 / 最小化。
    #[test]
    fn title_buttons_are_right_aligned() {
        let rect = Bounds::new(point(px(0.0), px(0.0)), size(px(300.0), px(200.0)));
        let buttons = title_button_rects(rect);
        assert_eq!(buttons[0].0, WindowButton::Close);
        assert_eq!(buttons[1].0, WindowButton::Maximize);
        assert_eq!(buttons[2].0, WindowButton::Minimize);
        assert_eq!(buttons[0].1.origin.x + TITLE_BUTTON_WIDTH, px(300.0));
        assert_eq!(
            buttons[1].1.origin.x + TITLE_BUTTON_WIDTH,
            buttons[0].1.origin.x
        );
        assert_eq!(buttons[0].1.size.height, TITLE_BAR_HEIGHT);

        // 最小化图标自左向右排列。
        let first = icon_rect(area(), 0);
        let second = icon_rect(area(), 1);
        assert!(second.origin.x > first.origin.x);
        assert_eq!(first.origin.y + first.size.height, px(600.0));
    }

    /// 渲染 MDI 区域的测试宿主。
    struct Probe {
        /// MDI 状态。
        state: Entity<MdiState>,
    }

    impl Render for Probe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                MdiArea::new(&self.state)
                    .body(|id, _window, _cx| div().child(format!("doc {id}")).into_any_element()),
            )
        }
    }

    /// 构建宿主并收集事件。
    fn setup(
        cx: &mut TestAppContext,
    ) -> (
        Entity<MdiState>,
        Rc<RefCell<Vec<MdiEvent>>>,
        &mut crate::VisualTestContext,
    ) {
        let events: Rc<RefCell<Vec<MdiEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = events.clone();
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(move |window, cx| {
            let state = cx.new(MdiState::new);
            cx.subscribe_in(
                &state,
                window,
                move |_this, _state, event: &MdiEvent, _window, _cx| {
                    sink.borrow_mut().push(event.clone());
                },
            )
            .detach();
            Probe { state }
        });
        let state = probe.read_with(&*cx, |probe, _| probe.state.clone());
        (state, events, cx)
    }

    /// 打开 / 激活 / 关闭维护 z 序与激活窗口，并抛出事件。
    #[rgpui::test]
    fn open_activate_close_updates_z_order(cx: &mut TestAppContext) {
        let (state, events, cx) = setup(cx);

        let first = cx.update(|_, cx| state.update(cx, |state, cx| state.open("一", cx)));
        let second = cx.update(|_, cx| state.update(cx, |state, cx| state.open("二", cx)));
        assert_eq!(state.read_with(cx, |state, _| state.len()), 2);
        assert_eq!(state.read_with(cx, |state, _| state.active()), Some(second));
        // 后开者在前。
        assert_eq!(
            state.read_with(cx, |state, _| state.windows()[1].id),
            second
        );

        // 激活第一个：提到最前。
        cx.update(|_, cx| state.update(cx, |state, cx| state.activate(first, cx)));
        assert_eq!(
            state.read_with(cx, |state, _| state.windows().last().map(|w| w.id)),
            Some(first)
        );
        assert_eq!(state.read_with(cx, |state, _| state.active()), Some(first));

        // 循环切换：再次激活第二个。
        let next = cx.update(|_, cx| state.update(cx, |state, cx| state.cycle(true, cx)));
        assert_eq!(next, Some(second));

        // 关闭第二个后激活窗口回落到剩下的一个。
        cx.update(|_, cx| state.update(cx, |state, cx| state.close(second, cx)));
        assert_eq!(state.read_with(cx, |state, _| state.len()), 1);
        assert_eq!(state.read_with(cx, |state, _| state.active()), Some(first));
        // 关闭不存在的窗口返回 false。
        assert!(!cx.update(|_, cx| state.update(cx, |state, cx| state.close(99, cx))));

        let log = events.borrow();
        assert!(log.contains(&MdiEvent::Opened(first)));
        assert!(log.contains(&MdiEvent::Opened(second)));
        assert!(log.contains(&MdiEvent::Activated(first)));
        assert!(log.contains(&MdiEvent::Closed(second)));
    }

    /// 最小化 / 最大化 / 还原的矩形语义。
    #[rgpui::test]
    fn minimize_maximize_restore(cx: &mut TestAppContext) {
        let (state, _events, cx) = setup(cx);
        let area = host_area(&state, cx);
        assert!(area.size.width > px(0.0));
        let id = cx.update(|_, cx| state.update(cx, |state, cx| state.open("文档", cx)));

        // 首次预绘制后区域已知：窗口落在层叠位。
        let normal = state.read_with(cx, |state, _| state.rect_of(id).unwrap());
        assert!(area.contains(&normal.origin));

        cx.update(|_, cx| state.update(cx, |state, cx| state.maximize(id, cx)));
        assert_eq!(
            state.read_with(cx, |state, _| state.get(id).unwrap().state),
            WindowState::Maximized
        );
        assert_eq!(
            state.read_with(cx, |state, _| state.rect_of(id).unwrap()),
            area
        );

        // 最大化 → 最小化 → 还原：回到最初的普通矩形。
        cx.update(|_, cx| state.update(cx, |state, cx| state.minimize(id, cx)));
        assert_eq!(
            state.read_with(cx, |state, _| state.rect_of(id).unwrap()),
            icon_rect(area, 0)
        );
        cx.update(|_, cx| state.update(cx, |state, cx| state.restore(id, cx)));
        assert_eq!(
            state.read_with(cx, |state, _| state.rect_of(id).unwrap()),
            normal
        );

        // 双击切换最大化，再切换回来。
        cx.update(|_, cx| state.update(cx, |state, cx| state.toggle_maximize(id, cx)));
        assert_eq!(
            state.read_with(cx, |state, _| state.get(id).unwrap().state),
            WindowState::Maximized
        );
        cx.update(|_, cx| state.update(cx, |state, cx| state.toggle_maximize(id, cx)));
        assert_eq!(
            state.read_with(cx, |state, _| state.rect_of(id).unwrap()),
            normal
        );
    }

    /// 能力开关会禁用标题栏按钮。
    #[rgpui::test]
    fn flags_disable_title_buttons(cx: &mut TestAppContext) {
        let (state, _events, cx) = setup(cx);
        let id = cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.open_with(
                    "只读",
                    MdiWindowFlags {
                        closable: false,
                        minimizable: false,
                        maximizable: false,
                    },
                    cx,
                )
            })
        });

        let rect = state.read_with(cx, |state, _| state.rect_of(id).unwrap());
        let close = title_button_rects(rect)[0].1;
        let point_in_close = close.center();
        assert_eq!(
            state.read_with(cx, |state, _| state.hit_test(point_in_close)),
            MdiHit::TitleBar(id)
        );
    }

    /// 指针按下 / 移动 / 释放：拖拽移动、缩放、按钮点击。
    #[rgpui::test]
    fn pointer_drag_resize_and_buttons(cx: &mut TestAppContext) {
        let (state, _events, cx) = setup(cx);
        let id = cx.update(|_, cx| state.update(cx, |state, cx| state.open("文档", cx)));
        let start = state.read_with(cx, |state, _| state.rect_of(id).unwrap());

        // 标题栏按下 → 命中标题栏；拖拽后窗口平移（夹取在可见范围内）。
        let title = point(start.origin.x + px(10.0), start.origin.y + px(8.0));
        assert_eq!(
            state.read_with(cx, |state, _| state.hit_test(title)),
            MdiHit::TitleBar(id)
        );
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.on_pointer_down(title, 1, cx);
                state.on_pointer_move(point(title.x + px(40.0), title.y + px(20.0)), cx);
                state.on_pointer_up(title, cx);
            });
        });
        let moved = state.read_with(cx, |state, _| state.rect_of(id).unwrap());
        assert_eq!(
            moved.origin,
            point(start.origin.x + px(40.0), start.origin.y + px(20.0))
        );

        // 右下角内缘拖拽缩放（外缘正好落在矩形边界外）。
        let corner = point(
            moved.origin.x + moved.size.width - px(2.0),
            moved.origin.y + moved.size.height - px(2.0),
        );
        assert!(matches!(
            state.read_with(cx, |state, _| state.hit_test(corner)),
            MdiHit::Resize { id: hit, edge: ResizeEdge::BottomRight } if hit == id
        ));
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.on_pointer_down(corner, 1, cx);
                state.on_pointer_move(point(corner.x + px(60.0), corner.y + px(40.0)), cx);
                state.on_pointer_up(corner, cx);
            });
        });
        let resized = state.read_with(cx, |state, _| state.rect_of(id).unwrap());
        assert_eq!(
            resized.size,
            size(moved.size.width + px(60.0), moved.size.height + px(40.0))
        );

        // 关闭按钮：按下 + 在按钮上释放 → 关闭。
        let close = title_button_rects(resized)[0].1;
        let point_in_close = close.center();
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.on_pointer_down(point_in_close, 1, cx);
                state.on_pointer_up(point_in_close, cx);
            });
        });
        assert_eq!(state.read_with(cx, |state, _| state.len()), 0);
    }

    /// 排布：平铺 / 层叠写在区域内，最小化窗口不参与。
    #[rgpui::test]
    fn arrange_layouts_within_area(cx: &mut TestAppContext) {
        let (state, _events, cx) = setup(cx);
        let area = host_area(&state, cx);
        let a = cx.update(|_, cx| state.update(cx, |state, cx| state.open("一", cx)));
        let b = cx.update(|_, cx| state.update(cx, |state, cx| state.open("二", cx)));

        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.arrange(MdiLayout::TileHorizontal, cx))
        });
        let left = state.read_with(cx, |state, _| state.rect_of(a).unwrap());
        let right = state.read_with(cx, |state, _| state.rect_of(b).unwrap());
        assert_eq!(left.size.width, right.size.width);
        assert_eq!(left.size.height, right.size.height);
        assert!(left.origin.x < right.origin.x);
        assert!(area.contains(&left.origin) && area.contains(&right.origin));

        // 层叠：第二个窗口相对第一个错开。
        cx.update(|_, cx| state.update(cx, |state, cx| state.arrange(MdiLayout::Cascade, cx)));
        let left = state.read_with(cx, |state, _| state.rect_of(a).unwrap());
        let right = state.read_with(cx, |state, _| state.rect_of(b).unwrap());
        assert!(right.origin.x > left.origin.x && right.origin.y > left.origin.y);

        // 最小化的窗口不参与排布，保持图标位置。
        cx.update(|_, cx| state.update(cx, |state, cx| state.minimize(a, cx)));
        cx.update(|_, cx| state.update(cx, |state, cx| state.arrange(MdiLayout::TileVertical, cx)));
        assert_eq!(
            state.read_with(cx, |state, _| state.rect_of(a).unwrap()),
            icon_rect(area, 0)
        );
        assert_eq!(
            state.read_with(cx, |state, _| state.get(b).unwrap().state),
            WindowState::Normal
        );
    }

    /// 尚无区域时排布请求挂起，取得区域后一次性铺开。
    #[rgpui::test]
    fn arrange_waits_for_area(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let state = cx.update(|cx| cx.new(MdiState::new));
        let a = cx.update(|cx| state.update(cx, |state, cx| state.open("一", cx)));
        let b = cx.update(|cx| state.update(cx, |state, cx| state.open("二", cx)));

        // 无窗口 → 无区域：排布请求挂起（不 panic、几何不变）。
        let changed = cx.update(|cx| {
            state.update(cx, |state, cx| state.arrange(MdiLayout::TileHorizontal, cx))
        });
        assert!(!changed);
        assert_eq!(
            state.read_with(cx, |state, _| state.area()),
            Bounds::default()
        );

        // 取得区域后按挂起的方式铺开。
        cx.update(|cx| state.update(cx, |state, cx| state.set_area(area(), cx)));
        let left = state.read_with(cx, |state, _| state.rect_of(a).unwrap());
        let right = state.read_with(cx, |state, _| state.rect_of(b).unwrap());
        assert_eq!(left.size.width, right.size.width);
        assert!(left.origin.x < right.origin.x);
        assert_eq!(left.size.height, area().size.height - super::MARGIN * 2.0);
    }

    /// 键盘：Ctrl+Tab 循环、Ctrl+F4 关闭、Ctrl+M 最小化。
    #[rgpui::test]
    fn keyboard_shortcuts(cx: &mut TestAppContext) {
        let (state, _events, cx) = setup(cx);
        let area = host_area(&state, cx);
        let a = cx.update(|_, cx| state.update(cx, |state, cx| state.open("一", cx)));
        let b = cx.update(|_, cx| state.update(cx, |state, cx| state.open("二", cx)));

        let key = |name: &str, control: bool, shift: bool| KeyDownEvent {
            keystroke: Keystroke {
                modifiers: Modifiers {
                    control,
                    shift,
                    ..Default::default()
                },
                key: name.to_string(),
                key_char: None,
            },
            is_held: false,
            prefer_character_input: false,
        };

        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.on_key_down(&key("tab", true, false), cx)
            })
        });
        assert_eq!(state.read_with(cx, |state, _| state.active()), Some(a));
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.on_key_down(&key("tab", true, true), cx)
            })
        });
        assert_eq!(state.read_with(cx, |state, _| state.active()), Some(b));

        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.on_key_down(&key("m", true, false), cx)
            })
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.get(b).unwrap().state),
            WindowState::Minimized
        );

        // 最小化图标命中后还原。
        let icon = icon_rect(area, 0).center();
        assert_eq!(
            state.read_with(cx, |state, _| state.hit_test(icon)),
            MdiHit::Icon(b)
        );
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.on_pointer_down(icon, 1, cx);
                state.on_pointer_up(icon, cx);
            })
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.get(b).unwrap().state),
            WindowState::Normal
        );

        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.on_key_down(&key("f4", true, false), cx)
            })
        });
        assert_eq!(state.read_with(cx, |state, _| state.len()), 1);
    }

    /// `move_by` 与 `set_bounds` 会夹取到可见范围。
    #[rgpui::test]
    fn move_and_set_bounds_clamp(cx: &mut TestAppContext) {
        let (state, _events, cx) = setup(cx);
        let id = cx.update(|_, cx| state.update(cx, |state, cx| state.open("文档", cx)));

        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.move_by(id, point(px(-10_000.0), px(-10_000.0)), cx)
            })
        });
        let rect = state.read_with(cx, |state, _| state.rect_of(id).unwrap());
        assert_eq!(rect.origin.y, px(0.0));
        assert_eq!(rect.origin.x, px(60.0) - rect.size.width);

        // 直接用 set_bounds 也能落位（并夹取最小尺寸）。
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.set_bounds(
                    id,
                    Bounds::new(point(px(40.0), px(50.0)), size(px(10.0), px(10.0))),
                    cx,
                )
            })
        });
        let rect = state.read_with(cx, |state, _| state.rect_of(id).unwrap());
        assert_eq!(rect.origin, point(px(40.0), px(50.0)));
        assert_eq!(rect.size, MIN_WINDOW_SIZE);

        // 挪动后标题栏仍在区域内。
        let moved = state.read_with(cx, |state, _| origin_of(state.get(id).unwrap()));
        assert_eq!(moved, point(px(40.0), px(50.0)));
    }

    /// 吸附：阈值内吸到最近的边/中线，超阈值保持原样。
    #[test]
    fn snap_rect_snaps_within_threshold() {
        let area = area();
        let window = Bounds::new(point(px(100.0), px(100.0)), size(px(200.0), px(100.0)));

        // 左边缘离工作区左边界 3px → 吸到 0
        let near_left = Bounds::new(point(px(3.0), px(100.0)), window.size);
        assert_eq!(snap_rect(near_left, &[area], px(4.0)).origin.x, px(0.0));

        // 右边缘离工作区右边界 3px → 吸到 600
        let near_right = Bounds::new(point(px(597.0), px(100.0)), window.size);
        assert_eq!(snap_rect(near_right, &[area], px(4.0)).origin.x, px(600.0));

        // 距离 5px 超过阈值 4px → 不动
        let far = Bounds::new(point(px(5.0), px(100.0)), window.size);
        assert_eq!(snap_rect(far, &[area], px(4.0)), far);

        // 不在任何候选线附近 → 原样返回
        assert_eq!(snap_rect(window, &[area], px(4.0)), window);
    }

    /// 活动窗口吸附到工作区边缘，无位移时不产生变化。
    #[rgpui::test]
    fn snap_active_pulls_window_to_edge(cx: &mut TestAppContext) {
        let (state, _events, cx) = setup(cx);
        let id = cx.update(|_, cx| state.update(cx, |state, cx| state.open("一", cx)));
        let rect = state.read_with(cx, |state, _| state.rect_of(id).unwrap());

        // 挪到离左边界 3px 处再吸附
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.set_bounds(
                    id,
                    Bounds::new(point(px(3.0), rect.origin.y), rect.size),
                    cx,
                )
            })
        });
        let moved = cx.update(|_, cx| state.update(cx, |state, cx| state.snap_active(px(4.0), cx)));
        assert!(moved);
        assert_eq!(
            state
                .read_with(cx, |state, _| state.rect_of(id).unwrap())
                .origin
                .x,
            px(0.0)
        );

        // 已经贴边：再吸附一次没有位移
        let again = cx.update(|_, cx| state.update(cx, |state, cx| state.snap_active(px(4.0), cx)));
        assert!(!again);
    }
}
