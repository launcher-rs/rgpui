//! 任务窗格（Task Pane）。
//!
//! 可停靠 / 可浮动的侧边任务窗格：默认贴在父容器左 / 右边缘，占据固定宽度，标题栏可拖动
//! （浮动态）、内缘把手可拖拽改宽、关闭按钮可隐藏；也可切到浮动态变成一块可自由移动与
//! 八向缩放的窗格（复用 [`crate::mdi`] 的缩放几何）。
//!
//! 状态由 [`TaskPaneState`] 实体持有（停靠侧 / 宽度 / 可见性 / 浮动矩形 / 拖拽会话），
//! 渲染由 [`TaskPane`] 元素承担，正文经 `body` 回调现渲染 —— 因此面板内容（属性表、
//! 样式列表、导航树…）不进入状态，可随选中对象随时变化。
//!
//! **宿主需观察状态**：`TaskPane` 只是薄壳，宽度 / 显隐 / 浮动等变化经 [`TaskPaneState`]
//! 通知，宿主视图必须 `cx.observe(&pane, |_, _, cx| cx.notify())`（或订阅 [`TaskPaneEvent`]）
//! 才能在拖拽改宽 / 切换停靠时实时重绘。
//!
//! 同一侧要放多个窗格时用 [`TaskPaneStack`] + [`TaskPaneStackState`]：按权重自上而下分配
//! 高度，相邻两格之间是可拖拽的分隔条（只改相邻两格、总高度不变），几何换算见
//! [`resize_stack_weights`]。
//!
//! ```
//! use rgpui::{div, prelude::*};
//! use rgpui::task_pane::{TaskPane, TaskPaneState};
//!
//! struct Demo {
//!     pane: Entity<TaskPaneState>,
//! }
//!
//! impl Render for Demo {
//!     fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
//!         // 浮动态需要父容器是定位祖先。
//!         div().relative().flex().flex_row().size_full().child(
//!             TaskPane::new(&self.pane)
//!                 .title("属性")
//!                 .body(|_window, _cx| div().child("正文").into_any_element()),
//!         )
//!     }
//! }
//! ```

use std::sync::Arc;

use crate::mdi::{ResizeEdge, resize_bounds, resize_edge_at, resize_edge_rects};
use crate::{
    ActiveTheme as _, AnyElement, App, Bounds, Context, CursorStyle, ElementExt as _, Entity,
    EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point,
    RenderOnce, SharedString, Size, StatefulInteractiveElement as _, Styled as _, Window, div,
    point, prelude::FluentBuilder as _, px, size,
};

/// 标题栏高度。
pub const TITLE_HEIGHT: Pixels = px(28.0);
/// 停靠态内缘拖拽把手厚度（像素）。
pub const GRIP_WIDTH: f32 = 5.0;
/// 默认宽度。
pub const DEFAULT_WIDTH: Pixels = px(260.0);
/// 最小宽度。
pub const MIN_WIDTH: Pixels = px(160.0);
/// 最大宽度。
pub const MAX_WIDTH: Pixels = px(720.0);
/// 关闭按钮宽度。
pub const CLOSE_BUTTON_WIDTH: Pixels = px(24.0);
/// 浮动窗格最小尺寸。
pub const MIN_FLOATING_SIZE: Size<Pixels> = size(px(160.0), px(120.0));
/// 浮动窗格默认与父容器边缘的留白。
pub const FLOATING_MARGIN: Pixels = px(12.0);
/// 自动隐藏收起后的轨道宽度。
pub const RAIL_WIDTH: Pixels = px(28.0);
/// 标题栏左右内边距（与 `px_2` 一致，页签几何按同一口径计算）。
pub const TITLE_PADDING: Pixels = px(8.0);

/// 停靠侧。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPaneSide {
    /// 贴在父容器左缘（内缘在右）。
    Left,
    /// 贴在父容器右缘（内缘在左）。
    Right,
}

impl TaskPaneSide {
    /// 停靠态拖拽把手所在边（左停靠 → 右缘，右停靠 → 左缘）。
    pub fn grip_edge(self) -> ResizeEdge {
        match self {
            TaskPaneSide::Left => ResizeEdge::Right,
            TaskPaneSide::Right => ResizeEdge::Left,
        }
    }
}

/// 命中结果（`None` 表示未命中窗格）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPaneHit {
    /// 未命中。
    None,
    /// 标题栏空白处（浮动态可拖动）。
    TitleBar,
    /// 页签（下标）。
    Tab(usize),
    /// 关闭按钮。
    Close,
    /// 内缘把手（停靠态改宽）。
    Grip,
    /// 缩放边 / 角（浮动态）。
    Resize(ResizeEdge),
    /// 正文。
    Body,
}

/// 任务窗格事件。
#[derive(Debug, Clone, PartialEq)]
pub enum TaskPaneEvent {
    /// 显示状态变化。
    VisibilityChanged(bool),
    /// 停靠侧变化。
    SideChanged(TaskPaneSide),
    /// 停靠宽度变化。
    WidthChanged(Pixels),
    /// 浮动 / 停靠切换。
    FloatingChanged(bool),
    /// 自动隐藏的收起 / 展开状态变化。
    Collapsed(bool),
    /// 切换页签（下标）。
    TabSelected(usize),
}

/// 把宽度夹取到 `min..=max`；非有限值回落为 `min`。
pub fn clamp_width(width: Pixels, min: Pixels, max: Pixels) -> Pixels {
    let upper = max.max(min);
    if width.as_f32().is_finite() {
        width.max(min).min(upper)
    } else {
        min
    }
}

/// 停靠态拖动内缘时的宽度换算：拖左缘取负号（向左拖变宽），右缘取正号。
pub fn resize_width(
    start: Pixels,
    edge: ResizeEdge,
    delta_x: Pixels,
    min: Pixels,
    max: Pixels,
) -> Pixels {
    let width = if edge.touches_left() {
        start - delta_x
    } else {
        start + delta_x
    };
    clamp_width(width, min, max)
}

/// 停靠态内缘把手矩形（局部坐标）。
pub fn grip_rect(rect: Bounds<Pixels>, side: TaskPaneSide) -> Bounds<Pixels> {
    let t = px(GRIP_WIDTH);
    let x = match side {
        TaskPaneSide::Left => rect.origin.x + rect.size.width - t,
        TaskPaneSide::Right => rect.origin.x,
    };
    Bounds::new(point(x, rect.origin.y), size(t, rect.size.height))
}

/// 关闭按钮矩形（标题栏右上角，局部坐标）。
pub fn close_button_rect(rect: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        point(
            rect.origin.x + rect.size.width - CLOSE_BUTTON_WIDTH,
            rect.origin.y,
        ),
        size(CLOSE_BUTTON_WIDTH, TITLE_HEIGHT),
    )
}

/// 页签矩形（标题栏内等宽平铺，右侧预留关闭按钮宽度；局部坐标）。
pub fn tab_rects(rect: Bounds<Pixels>, count: usize) -> Vec<Bounds<Pixels>> {
    let left = rect.origin.x + TITLE_PADDING;
    let right = rect.origin.x + rect.size.width - CLOSE_BUTTON_WIDTH;
    if count == 0 || right <= left {
        return Vec::new();
    }
    let width = (right - left).as_f32() / count as f32;
    (0..count)
        .map(|index| {
            Bounds::new(
                point(left + px(width * index as f32), rect.origin.y),
                size(px(width), TITLE_HEIGHT),
            )
        })
        .collect()
}

/// 浮动窗格的默认矩形：贴在父容器右下角内侧（父容器坐标）。
pub fn floating_default_bounds(parent: Bounds<Pixels>, size: Size<Pixels>) -> Bounds<Pixels> {
    let x = parent.origin.x + parent.size.width - size.width - FLOATING_MARGIN;
    let y = parent.origin.y + parent.size.height - size.height - FLOATING_MARGIN;
    Bounds::new(
        point(
            x.max(parent.origin.x + FLOATING_MARGIN),
            y.max(parent.origin.y + FLOATING_MARGIN),
        ),
        size,
    )
}

/// 拖拽会话。
#[derive(Debug, Clone, Copy)]
struct PaneDrag {
    /// 拖拽类型。
    kind: PaneDragKind,
    /// 按下时的指针位置（窗口坐标）。
    start_pointer: Point<Pixels>,
    /// 按下时的浮动矩形（父容器坐标；停靠态无意义）。
    start_floating: Bounds<Pixels>,
    /// 按下时的停靠宽度。
    start_width: Pixels,
}

/// 拖拽类型。
#[derive(Debug, Clone, Copy)]
enum PaneDragKind {
    /// 移动浮动窗格。
    Move,
    /// 改宽（停靠）或改尺寸（浮动）。
    Resize(ResizeEdge),
    /// 按住关闭按钮。
    Close,
}

/// 任务窗格状态实体。
pub struct TaskPaneState {
    /// 停靠侧。
    side: TaskPaneSide,
    /// 停靠宽度。
    width: Pixels,
    /// 最小宽度。
    min_width: Pixels,
    /// 最大宽度。
    max_width: Pixels,
    /// 是否显示。
    visible: bool,
    /// 是否浮动。
    floating: bool,
    /// 浮动矩形（父容器坐标）。
    floating_bounds: Bounds<Pixels>,
    /// 是否显示关闭按钮。
    closable: bool,
    /// 是否启用自动隐藏（停靠态鼠标移出后收起为轨道）。
    auto_hide: bool,
    /// 是否收起为轨道。
    collapsed: bool,
    /// 鼠标是否停在窗格上（由元素 `on_hover` 维护）。
    hovered: bool,
    /// 页签标题（非空时标题栏渲染为页签组）。
    tabs: Vec<SharedString>,
    /// 当前激活页签下标。
    active_tab: usize,
    /// 最近一次预绘制写入的窗格窗口矩形。
    rect: Bounds<Pixels>,
    /// 拖拽会话。
    drag: Option<PaneDrag>,
    /// 焦点句柄。
    focus_handle: FocusHandle,
}

impl TaskPaneState {
    /// 创建任务窗格状态（默认停靠右侧、可见）。
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            side: TaskPaneSide::Right,
            width: DEFAULT_WIDTH,
            min_width: MIN_WIDTH,
            max_width: MAX_WIDTH,
            visible: true,
            floating: false,
            floating_bounds: Bounds::default(),
            closable: true,
            auto_hide: false,
            collapsed: false,
            hovered: false,
            tabs: Vec::new(),
            active_tab: 0,
            rect: Bounds::default(),
            drag: None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// 设置初始停靠侧。
    pub fn with_side(mut self, side: TaskPaneSide) -> Self {
        self.side = side;
        self
    }

    /// 设置初始宽度（夹取到最小 / 最大之间）。
    pub fn with_width(mut self, width: Pixels) -> Self {
        self.width = clamp_width(width, self.min_width, self.max_width);
        self
    }

    /// 设置最小 / 最大宽度。
    pub fn with_width_range(mut self, min: Pixels, max: Pixels) -> Self {
        self.min_width = min.max(px(0.0));
        self.max_width = max.max(self.min_width);
        self.width = clamp_width(self.width, self.min_width, self.max_width);
        self
    }

    /// 设置初始可见性。
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// 设置是否显示关闭按钮。
    pub fn with_closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    /// 设置初始页签标题（非空时标题栏渲染为页签组，正文由应用按激活页签切换）。
    pub fn with_tabs<T: Into<SharedString>>(mut self, tabs: impl IntoIterator<Item = T>) -> Self {
        self.tabs = tabs.into_iter().map(Into::into).collect();
        self.active_tab = self.active_tab.min(self.tabs.len().saturating_sub(1));
        self
    }

    /// 当前停靠侧。
    pub fn side(&self) -> TaskPaneSide {
        self.side
    }

    /// 停靠宽度。
    pub fn width(&self) -> Pixels {
        self.width
    }

    /// 最小 / 最大宽度。
    pub fn width_range(&self) -> (Pixels, Pixels) {
        (self.min_width, self.max_width)
    }

    /// 是否可见。
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 是否浮动。
    pub fn is_floating(&self) -> bool {
        self.floating
    }

    /// 是否显示关闭按钮。
    pub fn is_closable(&self) -> bool {
        self.closable
    }

    /// 页签标题（空表示标题栏显示单个标题）。
    pub fn tabs(&self) -> &[SharedString] {
        &self.tabs
    }

    /// 当前激活页签下标。
    pub fn active_tab(&self) -> usize {
        self.active_tab
    }

    /// 当前激活页签标题（无页签时为 `None`）。
    pub fn active_tab_title(&self) -> Option<&SharedString> {
        self.tabs.get(self.active_tab)
    }

    /// 浮动矩形（父容器坐标）。
    pub fn floating_bounds(&self) -> Bounds<Pixels> {
        self.floating_bounds
    }

    /// 最近一次预绘制写入的窗格窗口矩形。
    pub fn rect(&self) -> Bounds<Pixels> {
        self.rect
    }

    /// 显示窗格。
    pub fn show(&mut self, cx: &mut Context<Self>) -> bool {
        self.set_visible(true, cx)
    }

    /// 隐藏窗格。
    pub fn hide(&mut self, cx: &mut Context<Self>) -> bool {
        self.set_visible(false, cx)
    }

    /// 切换显示 / 隐藏。
    pub fn toggle(&mut self, cx: &mut Context<Self>) -> bool {
        self.set_visible(!self.visible, cx)
    }

    /// 设置可见性。
    pub fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) -> bool {
        if self.visible == visible {
            return false;
        }
        self.visible = visible;
        if !visible {
            self.drag = None;
        } else if self.auto_hide && !self.floating && !self.hovered {
            // 重新显示时按自动隐藏规则先收起。
            self.collapsed = true;
        }
        cx.emit(TaskPaneEvent::VisibilityChanged(visible));
        cx.notify();
        true
    }

    /// 是否启用自动隐藏。
    pub fn is_auto_hide(&self) -> bool {
        self.auto_hide
    }

    /// 设置自动隐藏（关闭时立即展开；开启时若鼠标不在窗格上则收起）。
    pub fn set_auto_hide(&mut self, auto_hide: bool, cx: &mut Context<Self>) -> bool {
        if self.auto_hide == auto_hide {
            return false;
        }
        self.auto_hide = auto_hide;
        self.auto_hide_changed(cx);
        cx.notify();
        true
    }

    /// 是否收起为轨道。
    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// 收起 / 展开（自动隐藏关闭时也可手动切换）。
    pub fn set_collapsed(&mut self, collapsed: bool, cx: &mut Context<Self>) -> bool {
        if self.collapsed == collapsed {
            return false;
        }
        self.collapsed = collapsed;
        if collapsed {
            self.drag = None;
        }
        cx.emit(TaskPaneEvent::Collapsed(collapsed));
        cx.notify();
        true
    }

    /// 切换收起 / 展开。
    pub fn toggle_collapsed(&mut self, cx: &mut Context<Self>) -> bool {
        self.set_collapsed(!self.collapsed, cx)
    }

    /// 鼠标进出窗格（元素 `on_hover` 调用）：自动隐藏时移入展开、移出收起。
    pub fn set_hovered(&mut self, hovered: bool, cx: &mut Context<Self>) -> bool {
        if self.hovered == hovered {
            return false;
        }
        self.hovered = hovered;
        if self.auto_hide && !self.floating {
            self.auto_hide_changed(cx);
            return true;
        }
        false
    }

    /// 当前实际占用的宽度（收起时是轨道宽度）。
    pub fn effective_width(&self) -> Pixels {
        if self.collapsed {
            RAIL_WIDTH
        } else {
            self.width
        }
    }

    /// 按自动隐藏规则同步收起状态（浮动 / 隐藏时始终展开）。
    fn auto_hide_changed(&mut self, cx: &mut Context<Self>) {
        let collapsed = self.auto_hide && self.visible && !self.floating && !self.hovered;
        if self.collapsed != collapsed {
            self.collapsed = collapsed;
            if collapsed {
                self.drag = None;
            }
            cx.emit(TaskPaneEvent::Collapsed(collapsed));
        }
    }

    /// 设置是否显示关闭按钮。
    pub fn set_closable(&mut self, closable: bool, cx: &mut Context<Self>) -> bool {
        if self.closable == closable {
            return false;
        }
        self.closable = closable;
        cx.notify();
        true
    }

    /// 设置页签列表（非空时标题栏渲染为页签组），激活下标越界时夹回。
    pub fn set_tabs(&mut self, tabs: Vec<SharedString>, cx: &mut Context<Self>) -> bool {
        if self.tabs == tabs {
            return false;
        }
        self.tabs = tabs;
        self.active_tab = self.active_tab.min(self.tabs.len().saturating_sub(1));
        cx.notify();
        true
    }

    /// 选中页签（越界或已是当前页签返回 `false`）。
    pub fn set_active_tab(&mut self, index: usize, cx: &mut Context<Self>) -> bool {
        if index >= self.tabs.len() || index == self.active_tab {
            return false;
        }
        self.active_tab = index;
        cx.emit(TaskPaneEvent::TabSelected(index));
        cx.notify();
        true
    }

    /// 按 `delta` 循环切换页签（`Ctrl+Tab` / `Ctrl+Shift+Tab`）。
    pub fn cycle_tab(&mut self, delta: isize, cx: &mut Context<Self>) -> bool {
        let len = self.tabs.len();
        if len == 0 {
            return false;
        }
        let next = (self.active_tab as isize + delta).rem_euclid(len as isize) as usize;
        self.set_active_tab(next, cx)
    }

    /// 页签矩形（局部坐标）。
    pub fn tab_rects(&self) -> Vec<Bounds<Pixels>> {
        tab_rects(self.local_rect(), self.tabs.len())
    }

    /// 指针落在哪个页签上（`pointer` 为窗口坐标；关闭按钮优先）。
    pub fn tab_at(&self, pointer: Point<Pixels>) -> Option<usize> {
        if !self.visible || self.collapsed {
            return None;
        }
        let rect = self.local_rect();
        let local = pointer - self.rect.origin;
        if !rect.contains(&local) || local.y - rect.origin.y > TITLE_HEIGHT {
            return None;
        }
        if self.closable && close_button_rect(rect).contains(&local) {
            return None;
        }
        self.tab_rects().iter().position(|tab| tab.contains(&local))
    }

    /// 切换停靠侧。
    pub fn set_side(&mut self, side: TaskPaneSide, cx: &mut Context<Self>) -> bool {
        if self.side == side {
            return false;
        }
        self.side = side;
        cx.emit(TaskPaneEvent::SideChanged(side));
        cx.notify();
        true
    }

    /// 设置停靠宽度（夹取到最小 / 最大之间）。
    pub fn set_width(&mut self, width: Pixels, cx: &mut Context<Self>) -> bool {
        let width = clamp_width(width, self.min_width, self.max_width);
        if self.width == width {
            return false;
        }
        self.width = width;
        cx.emit(TaskPaneEvent::WidthChanged(width));
        cx.notify();
        true
    }

    /// 浮动到给定矩形（父容器坐标）。
    pub fn float(&mut self, bounds: Bounds<Pixels>, cx: &mut Context<Self>) -> bool {
        let size = size(
            bounds.size.width.max(MIN_FLOATING_SIZE.width),
            bounds.size.height.max(MIN_FLOATING_SIZE.height),
        );
        self.floating_bounds = Bounds::new(bounds.origin, size);
        if self.floating {
            cx.notify();
            return false;
        }
        self.floating = true;
        if !self.visible {
            self.visible = true;
        }
        // 浮动态不参与自动隐藏，总是展开。
        self.expand_on_float(cx);
        cx.emit(TaskPaneEvent::FloatingChanged(true));
        cx.notify();
        true
    }

    /// 回到停靠态。
    pub fn dock(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.floating {
            return false;
        }
        self.floating = false;
        self.drag = None;
        // 停靠后重新套用自动隐藏（鼠标不在窗格上则收起）。
        self.auto_hide_changed(cx);
        cx.emit(TaskPaneEvent::FloatingChanged(false));
        cx.notify();
        true
    }

    /// 浮动态展开并清除收起状态。
    fn expand_on_float(&mut self, cx: &mut Context<Self>) {
        if self.collapsed {
            self.collapsed = false;
            cx.emit(TaskPaneEvent::Collapsed(false));
        }
    }

    /// 在浮动与停靠之间切换（浮动时贴在 `parent` 右下角内侧）。
    pub fn toggle_float(&mut self, parent: Bounds<Pixels>, cx: &mut Context<Self>) -> bool {
        if self.floating {
            self.dock(cx)
        } else {
            let bounds = floating_default_bounds(parent, size(self.width, DEFAULT_WIDTH));
            self.float(bounds, cx)
        }
    }

    /// 由视图预绘制同步窗格矩形（一般无需手动调用）。
    pub fn set_rect(&mut self, rect: Bounds<Pixels>, cx: &mut Context<Self>) {
        if rect == self.rect {
            return;
        }
        self.rect = rect;
        cx.notify();
    }

    /// 焦点击柄。
    pub fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }

    /// 命中测试（`pointer` 为窗口坐标）。
    pub fn hit_test(&self, pointer: Point<Pixels>) -> TaskPaneHit {
        if !self.visible {
            return TaskPaneHit::None;
        }
        let local = pointer - self.rect.origin;
        let rect = self.local_rect();
        if !rect.contains(&local) {
            return TaskPaneHit::None;
        }
        // 收起为轨道：整条按标题栏处理（悬停即展开，不提供按钮 / 缩放）。
        if self.collapsed {
            return TaskPaneHit::TitleBar;
        }
        if self.floating {
            if let Some(edge) = resize_edge_at(rect, local) {
                return TaskPaneHit::Resize(edge);
            }
        } else {
            let grip = grip_rect(rect, self.side);
            if grip.contains(&local) {
                return TaskPaneHit::Grip;
            }
        }
        if self.closable && close_button_rect(rect).contains(&local) {
            return TaskPaneHit::Close;
        }
        if let Some(index) = self.tab_at(pointer) {
            return TaskPaneHit::Tab(index);
        }
        if local.y - rect.origin.y <= TITLE_HEIGHT {
            return TaskPaneHit::TitleBar;
        }
        TaskPaneHit::Body
    }

    /// 指针按下（`pointer` 为窗口坐标）。
    pub fn on_pointer_down(&mut self, pointer: Point<Pixels>) {
        let kind = match self.hit_test(pointer) {
            TaskPaneHit::None | TaskPaneHit::Body | TaskPaneHit::Tab(_) => {
                self.drag = None;
                return;
            }
            TaskPaneHit::Close => PaneDragKind::Close,
            TaskPaneHit::Grip => PaneDragKind::Resize(self.side.grip_edge()),
            TaskPaneHit::Resize(edge) => PaneDragKind::Resize(edge),
            TaskPaneHit::TitleBar => {
                if self.floating {
                    PaneDragKind::Move
                } else {
                    self.drag = None;
                    return;
                }
            }
        };
        self.drag = Some(PaneDrag {
            kind,
            start_pointer: pointer,
            start_floating: self.floating_bounds,
            start_width: self.width,
        });
    }

    /// 指针移动（`pointer` 为窗口坐标）。
    pub fn on_pointer_move(&mut self, pointer: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(drag) = self.drag else {
            return;
        };
        let delta = pointer - drag.start_pointer;
        match drag.kind {
            PaneDragKind::Move => {
                let bounds =
                    Bounds::new(drag.start_floating.origin + delta, drag.start_floating.size);
                if bounds == self.floating_bounds {
                    return;
                }
                self.floating_bounds = bounds;
                cx.notify();
            }
            PaneDragKind::Resize(edge) => {
                if self.floating {
                    let bounds = resize_bounds(drag.start_floating, edge, delta, MIN_FLOATING_SIZE);
                    if bounds == self.floating_bounds {
                        return;
                    }
                    self.floating_bounds = bounds;
                    cx.notify();
                } else {
                    let width = resize_width(
                        drag.start_width,
                        edge,
                        delta.x,
                        self.min_width,
                        self.max_width,
                    );
                    if width == self.width {
                        return;
                    }
                    self.width = width;
                    cx.emit(TaskPaneEvent::WidthChanged(width));
                    cx.notify();
                }
            }
            PaneDragKind::Close => {}
        }
    }

    /// 指针释放（`pointer` 为窗口坐标）：仍命中关闭按钮时隐藏。
    pub fn on_pointer_up(&mut self, pointer: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        if matches!(drag.kind, PaneDragKind::Close) && self.hit_test(pointer) == TaskPaneHit::Close
        {
            self.hide(cx);
        }
    }

    /// 键盘：`Esc` 取消当前拖拽，`Ctrl+Tab` / `Ctrl+Shift+Tab` 循环切换页签。
    pub fn on_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.to_ascii_lowercase();
        let modifiers = event.keystroke.modifiers;
        if key == "tab" && modifiers.control {
            if self.cycle_tab(if modifiers.shift { -1 } else { 1 }, cx) {
                cx.stop_propagation();
            }
            return;
        }
        if key == "escape" && self.drag.take().is_some() {
            cx.stop_propagation();
            cx.notify();
        }
    }

    /// 窗格局部矩形（原点为窗格自身左上角）。
    fn local_rect(&self) -> Bounds<Pixels> {
        Bounds::new(
            point(px(0.0), px(0.0)),
            size(self.rect.size.width, self.rect.size.height),
        )
    }
}

impl Focusable for TaskPaneState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.focus_handle(cx)
    }
}

impl EventEmitter<TaskPaneEvent> for TaskPaneState {}

/// 任务窗格元素。
///
/// 停靠态是普通流式子元素（固定宽度、撑满高度、内缘一条把手）；浮动态是
/// `absolute` 定位的子元素，因此**父容器必须是定位祖先**（`relative()`）。
#[derive(IntoElement)]
pub struct TaskPane {
    /// 状态实体。
    state: Entity<TaskPaneState>,
    /// 标题。
    title: SharedString,
    /// 正文渲染回调。
    body: Arc<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>,
}

impl TaskPane {
    /// 创建任务窗格元素（正文默认为空白）。
    pub fn new(state: &Entity<TaskPaneState>) -> Self {
        Self {
            state: state.clone(),
            title: "".into(),
            body: Arc::new(|_window, _cx| div().into_any_element()),
        }
    }

    /// 设置标题栏文案。
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// 设置正文渲染回调。
    pub fn body<F>(mut self, body: F) -> Self
    where
        F: Fn(&mut Window, &mut App) -> AnyElement + 'static,
    {
        self.body = Arc::new(body);
        self
    }
}

impl RenderOnce for TaskPane {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self { state, title, body } = self;
        let theme = cx.theme();
        let (visible, floating, closable, collapsed, tabs, active_tab, tab_rects) = state
            .read_with(cx, |state, _| {
                (
                    state.visible,
                    state.floating,
                    state.closable,
                    state.collapsed,
                    state.tabs.clone(),
                    state.active_tab,
                    state.tab_rects(),
                )
            });
        if !visible {
            return div().into_any_element();
        }
        let (side, width, bounds) = state.read_with(cx, |state, _| {
            (state.side, state.effective_width(), state.floating_bounds)
        });

        let background = theme.tokens.popover;
        let border = theme.border;
        let title_background = theme.secondary;
        let foreground = theme.foreground;
        let title_text = theme.muted_foreground;
        let hover = theme.muted;
        // 轨道态竖排标题在标题栏消费 `title` 之前留一份（有页签时优先显示激活页签）。
        let rail_title = tabs
            .get(active_tab)
            .cloned()
            .unwrap_or_else(|| title.clone());
        let accent = theme.primary;

        let local = Bounds::new(
            point(px(0.0), px(0.0)),
            size(bounds.size.width, bounds.size.height),
        );

        // 停靠态内缘把手 / 浮动态八向缩放块（只设光标，点击由容器命中测试统一处理）。
        let handles: Vec<AnyElement> = if collapsed {
            Vec::new()
        } else if floating {
            resize_edge_rects(local)
                .into_iter()
                .map(|(edge, rect)| {
                    div()
                        .absolute()
                        .left(rect.origin.x)
                        .top(rect.origin.y)
                        .w(rect.size.width)
                        .h(rect.size.height)
                        .cursor(edge.cursor())
                        .into_any_element()
                })
                .collect()
        } else {
            let grip = grip_rect(local, side);
            vec![
                div()
                    .absolute()
                    .left(grip.origin.x)
                    .top(grip.origin.y)
                    .w(grip.size.width)
                    .h(grip.size.height)
                    .cursor(CursorStyle::ResizeLeftRight)
                    .into_any_element(),
            ]
        };

        let close = (closable && !collapsed).then(|| {
            let rect = close_button_rect(local);
            div()
                .id(("task-pane-close", state.entity_id()))
                .absolute()
                .left(rect.origin.x)
                .top(rect.origin.y)
                .w(rect.size.width)
                .h(rect.size.height)
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(title_text)
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .child("✕")
        });

        let title_bar = div()
            .flex()
            .flex_row()
            .items_center()
            .flex_none()
            .h(TITLE_HEIGHT)
            .px_2()
            .pr(CLOSE_BUTTON_WIDTH)
            .bg(title_background)
            .border_b_1()
            .border_color(border)
            // 有页签时标题栏只作背景，页签单元格覆盖在它之上。
            .when(tabs.is_empty(), |this| {
                this.child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .text_sm()
                        .text_color(foreground)
                        .child(title.clone()),
                )
            });

        // 页签：几何与 `TaskPaneState::tab_at` 同源，点击由容器命中测试统一分发。
        let tab_cells: Vec<AnyElement> = tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                let Some(rect) = tab_rects.get(index).copied() else {
                    return div().into_any_element();
                };
                let active = index == active_tab;
                div()
                    .absolute()
                    .left(rect.origin.x)
                    .top(rect.origin.y)
                    .w(rect.size.width)
                    .h(rect.size.height)
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .text_xs()
                    .text_color(if active { foreground } else { title_text })
                    .cursor_pointer()
                    .when(active, |this| {
                        this.bg(background).border_b_2().border_color(accent)
                    })
                    .hover(move |style| style.bg(hover))
                    .child(tab.clone())
                    .into_any_element()
            })
            .collect();

        let inner = if collapsed {
            // 轨道态：竖排标题 + 展开提示，不渲染正文与按钮。
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .py_2()
                .size_full()
                .overflow_hidden()
                .bg(background)
                .text_xs()
                .child(
                    div()
                        .text_color(title_text)
                        .child(if side == TaskPaneSide::Left {
                            "›"
                        } else {
                            "‹"
                        }),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .text_color(foreground)
                        .children(rail_title.chars().map(|ch| div().child(ch.to_string()))),
                )
                // 手动收起（自动隐藏关闭）时点轨道展开；自动隐藏开启时鼠标移入即展开。
                .id(("task-pane-rail", state.entity_id()))
                .on_click(window.listener_for(&state, |state, _event, _window, cx| {
                    state.set_collapsed(false, cx);
                }))
                .into_any_element()
        } else {
            let content = body(window, cx);
            div()
                .flex()
                .flex_col()
                .size_full()
                .overflow_hidden()
                .bg(background)
                .child(title_bar)
                .child(div().flex_1().overflow_hidden().child(content))
                .children(tab_cells)
                .children(handles)
                .children(close)
                .into_any_element()
        };

        let root = if floating {
            div()
                .id(("task-pane", state.entity_id()))
                .key_context("TaskPane")
                .track_focus(&state.read(cx).focus_handle.clone())
                .absolute()
                .left(bounds.origin.x)
                .top(bounds.origin.y)
                .w(bounds.size.width)
                .h(bounds.size.height)
                .rounded_sm()
                .border_1()
                .border_color(border)
                .shadow_lg()
        } else {
            let root = div()
                .id(("task-pane", state.entity_id()))
                .key_context("TaskPane")
                .track_focus(&state.read(cx).focus_handle.clone())
                .relative()
                .flex_none()
                .w(width)
                .h_full()
                .border_color(border);
            match side {
                TaskPaneSide::Left => root.border_r_1(),
                TaskPaneSide::Right => root.border_l_1(),
            }
        };

        root.on_prepaint({
            let state = state.clone();
            move |bounds, _window, cx| {
                state.update(cx, |state, cx| state.set_rect(bounds, cx));
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
                // 页签点击即切换（不走拖拽会话）。
                if let TaskPaneHit::Tab(index) = state.hit_test(event.position) {
                    state.set_active_tab(index, cx);
                    return;
                }
                state.on_pointer_down(event.position);
            }),
        )
        .on_mouse_move(
            window.listener_for(&state, |state, event: &MouseMoveEvent, _window, cx| {
                state.on_pointer_move(event.position, cx);
            }),
        )
        .capture_any_mouse_up(window.listener_for(
            &state,
            |state, event: &MouseUpEvent, _window, cx| {
                state.on_pointer_up(event.position, cx);
            },
        ))
        .on_hover(
            window.listener_for(&state, |state, hovered: &bool, _window, cx| {
                state.set_hovered(*hovered, cx);
            }),
        )
        .child(inner)
        .into_any_element()
    }
}

/// 堆叠分隔条高度（像素）。
pub const STACK_SEPARATOR_HEIGHT: Pixels = px(4.0);

/// 单个窗格的最小权重（相对值；拖拽时不会把某一格压到 0）。
pub const MIN_STACK_WEIGHT: f32 = 0.05;

/// 规范化单个权重：非有限值或非正数回退到 `1.0`，过小的值夹到
/// [`MIN_STACK_WEIGHT`]。
pub fn normalize_stack_weight(weight: f32) -> f32 {
    if weight.is_finite() && weight > 0.0 {
        weight.max(MIN_STACK_WEIGHT)
    } else {
        1.0
    }
}

/// 就地调整相邻两个窗格的权重（保持两者之和不变），返回是否发生变化。
///
/// `delta` 是指针的纵向位移，`usable` 是去掉分隔条后的可用高度：总权重不变时窗格
/// 高度与权重成正比，因此 `delta` 像素正好换算成 `delta * 总权重 / usable`。
/// 两格都夹在 [`MIN_STACK_WEIGHT`] 以上（两格都极窄时按对半分）。
pub fn resize_stack_weights(
    weights: &mut [f32],
    index: usize,
    delta: Pixels,
    usable: Pixels,
) -> bool {
    if index + 1 >= weights.len() {
        return false;
    }
    let usable = usable.as_f32();
    let delta = delta.as_f32();
    if !usable.is_finite() || usable <= 0.0 || !delta.is_finite() || delta == 0.0 {
        return false;
    }
    let total: f32 = weights
        .iter()
        .map(|weight| normalize_stack_weight(*weight))
        .sum();
    let above = normalize_stack_weight(weights[index]);
    let below = normalize_stack_weight(weights[index + 1]);
    let pair = above + below;
    let min = MIN_STACK_WEIGHT.min(pair * 0.5);
    let next_above = (above + delta * total / usable).clamp(min, pair - min);
    let next_below = pair - next_above;
    if (next_above - weights[index]).abs() < 1e-4 && (next_below - weights[index + 1]).abs() < 1e-4
    {
        return false;
    }
    weights[index] = next_above;
    weights[index + 1] = next_below;
    true
}

/// 同侧多个任务窗格的垂直堆叠状态（各格按权重分配高度）。
///
/// 权重只表示相对比例：高度 = 可用高度 × 权重 / 权重之和。拖拽分隔条时只改相邻两格，
/// 且保持两者之和不变，因此其他窗格与总高度都不动。
pub struct TaskPaneStackState {
    /// 各格权重（与子元素顺序一致；缺失时按 `1.0` 处理）。
    weights: Vec<f32>,
    /// 堆叠矩形（窗口坐标，预绘制写入）。
    rect: Bounds<Pixels>,
    /// 子元素数量（预绘制写入，用于扣除分隔条）。
    slots: usize,
    /// 正在拖拽的分隔条（其上方子元素下标）。
    dragging: Option<usize>,
    /// 拖拽起点指针 y（窗口坐标）。
    drag_origin_y: f32,
    /// 拖拽开始时上下两格的权重。
    drag_start: (f32, f32),
}

impl TaskPaneStackState {
    /// 创建空堆叠状态。
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            weights: Vec::new(),
            rect: Bounds::default(),
            slots: 0,
            dragging: None,
            drag_origin_y: 0.0,
            drag_start: (1.0, 1.0),
        }
    }

    /// 设置初始权重（非法值按 [`normalize_stack_weight`] 规范化）。
    pub fn with_weights(mut self, weights: impl IntoIterator<Item = f32>) -> Self {
        self.weights = weights.into_iter().map(normalize_stack_weight).collect();
        self
    }

    /// 当前权重切片。
    pub fn weights(&self) -> &[f32] {
        &self.weights
    }

    /// 下标处的权重（缺失或非法时回退到 `1.0`）。
    pub fn weight(&self, index: usize) -> f32 {
        self.weights
            .get(index)
            .copied()
            .map(normalize_stack_weight)
            .unwrap_or(1.0)
    }

    /// 整体替换权重。
    pub fn set_weights(&mut self, weights: impl IntoIterator<Item = f32>, cx: &mut Context<Self>) {
        self.weights = weights.into_iter().map(normalize_stack_weight).collect();
        cx.notify();
    }

    /// 设置单格权重（下标超出时补 `1.0`），返回是否变化。
    pub fn set_weight(&mut self, index: usize, weight: f32, cx: &mut Context<Self>) -> bool {
        let weight = normalize_stack_weight(weight);
        if self.weights.len() <= index {
            self.weights.resize(index + 1, 1.0);
        }
        if (self.weights[index] - weight).abs() < 1e-6 {
            return false;
        }
        self.weights[index] = weight;
        cx.notify();
        true
    }

    /// 堆叠矩形（窗口坐标；预绘制前为默认值）。
    pub fn rect(&self) -> Bounds<Pixels> {
        self.rect
    }

    /// 子元素数量（预绘制前为 0）。
    pub fn slots(&self) -> usize {
        self.slots
    }

    /// 去掉分隔条后的可用高度。
    pub fn usable_height(&self) -> Pixels {
        let separators = self.slots.saturating_sub(1) as f32;
        (self.rect.size.height - STACK_SEPARATOR_HEIGHT * separators).max(px(0.0))
    }

    /// 正在拖拽的分隔条（其上方子元素下标）。
    pub fn dragging(&self) -> Option<usize> {
        self.dragging
    }

    /// 开始拖拽下标 `index` 与 `index + 1` 之间的分隔条。
    pub fn begin_drag(&mut self, index: usize, pointer_y: Pixels, cx: &mut Context<Self>) {
        self.dragging = Some(index);
        self.drag_origin_y = pointer_y.as_f32();
        self.drag_start = (self.weight(index), self.weight(index + 1));
        cx.notify();
    }

    /// 拖拽中：始终以「起点权重 + 总位移」重算，避免逐帧累积误差。
    pub fn update_drag(&mut self, pointer_y: Pixels, cx: &mut Context<Self>) {
        let Some(index) = self.dragging else {
            return;
        };
        while self.weights.len() <= index + 1 {
            self.weights.push(1.0);
        }
        self.weights[index] = self.drag_start.0;
        self.weights[index + 1] = self.drag_start.1;
        let delta = px(pointer_y.as_f32() - self.drag_origin_y);
        let usable = self.usable_height();
        if resize_stack_weights(&mut self.weights, index, delta, usable) {
            cx.notify();
        }
    }

    /// 结束拖拽。
    pub fn end_drag(&mut self, cx: &mut Context<Self>) {
        if self.dragging.take().is_some() {
            cx.notify();
        }
    }

    /// 预绘制回写矩形与子元素数量（由堆叠元素调用）。
    fn sync_layout(&mut self, rect: Bounds<Pixels>, slots: usize) {
        self.rect = rect;
        self.slots = slots;
    }
}

/// 同侧多个任务窗格的垂直堆叠元素。
///
/// 子元素按顺序自上而下排列，高度按 [`TaskPaneStackState`] 的权重分配，相邻两格之间是
/// 可拖拽的分隔条。子元素通常是 [`TaskPane`]；**不可见的窗格不要加入**（元素会占位，
/// 应在构建子元素列表时过滤掉）。
///
/// ```
/// use rgpui::{div, prelude::*};
/// use rgpui::task_pane::{TaskPane, TaskPaneStack, TaskPaneStackState, TaskPaneState};
///
/// struct Demo {
///     stack: Entity<TaskPaneStackState>,
///     explorer: Entity<TaskPaneState>,
///     properties: Entity<TaskPaneState>,
/// }
///
/// impl Render for Demo {
///     fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
///         div().relative().flex().flex_row().size_full().child(
///             TaskPaneStack::new(&self.stack)
///                 .child(TaskPane::new(&self.explorer).title("资源管理器"))
///                 .child(TaskPane::new(&self.properties).title("属性")),
///         )
///     }
/// }
/// ```
#[derive(IntoElement)]
pub struct TaskPaneStack {
    /// 堆叠状态实体。
    state: Entity<TaskPaneStackState>,
    /// 子元素（自上而下）。
    children: Vec<AnyElement>,
}

impl TaskPaneStack {
    /// 使用给定堆叠状态创建元素。
    pub fn new(state: &Entity<TaskPaneStackState>) -> Self {
        Self {
            state: state.clone(),
            children: Vec::new(),
        }
    }

    /// 追加一个子元素（通常是 [`TaskPane`]）。
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    /// 追加若干子元素。
    pub fn children(mut self, children: impl IntoIterator<Item = impl IntoElement>) -> Self {
        for child in children {
            self = self.child(child);
        }
        self
    }
}

impl RenderOnce for TaskPaneStack {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let border = theme.border;
        let accent = theme.primary;
        let state = self.state;
        let slots = self.children.len();
        let weights: Vec<f32> = (0..slots)
            .map(|index| state.read(cx).weight(index))
            .collect();
        let dragging = state.read(cx).dragging;

        let mut column = div()
            .id(("task-pane-stack", state.entity_id()))
            .relative()
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .on_prepaint({
                let state = state.clone();
                move |bounds, _window, cx| {
                    state.update(cx, |state, _| state.sync_layout(bounds, slots));
                }
            });

        for (index, child) in self.children.into_iter().enumerate() {
            if index > 0 {
                // 分隔条：拖动时按纵向位移调整上下两格的权重。
                column = column.child(
                    div()
                        .flex_none()
                        .h(STACK_SEPARATOR_HEIGHT)
                        .w_full()
                        .bg(if dragging == Some(index - 1) {
                            accent
                        } else {
                            border
                        })
                        .cursor_row_resize()
                        .on_mouse_down(
                            MouseButton::Left,
                            window.listener_for(
                                &state,
                                move |state, event: &MouseDownEvent, _window, cx| {
                                    state.begin_drag(index - 1, event.position.y, cx);
                                },
                            ),
                        )
                        .on_mouse_move(window.listener_for(
                            &state,
                            |state, event: &MouseMoveEvent, _window, cx| {
                                state.update_drag(event.position.y, cx);
                            },
                        ))
                        .capture_any_mouse_up(window.listener_for(
                            &state,
                            |state, _event: &MouseUpEvent, _window, cx| {
                                state.end_drag(cx);
                            },
                        )),
                );
            }
            column = column.child(
                div()
                    .flex_grow(weights[index])
                    .flex_basis(px(0.0))
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .child(child),
            );
        }

        column
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{
        CLOSE_BUTTON_WIDTH, DEFAULT_WIDTH, MAX_WIDTH, MIN_FLOATING_SIZE, MIN_STACK_WEIGHT,
        MIN_WIDTH, RAIL_WIDTH, STACK_SEPARATOR_HEIGHT, TITLE_HEIGHT, TITLE_PADDING, TaskPane,
        TaskPaneEvent, TaskPaneHit, TaskPaneSide, TaskPaneStack, TaskPaneStackState, TaskPaneState,
        clamp_width, close_button_rect, floating_default_bounds, grip_rect, normalize_stack_weight,
        resize_stack_weights, resize_width, tab_rects,
    };
    use crate::mdi::ResizeEdge;
    use crate::{
        AppContext as _, Bounds, Context, Entity, IntoElement, ParentElement as _, Pixels, Point,
        Render, Styled as _, TestAppContext, Window, div, point, px, size,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    /// 宽度夹取与内缘拖拽换算。
    #[test]
    fn clamp_and_resize_width() {
        assert_eq!(clamp_width(px(300.0), px(160.0), px(720.0)), px(300.0));
        assert_eq!(clamp_width(px(10.0), px(160.0), px(720.0)), px(160.0));
        assert_eq!(clamp_width(px(9_999.0), px(160.0), px(720.0)), px(720.0));
        // 非有限值回落为最小值。
        assert_eq!(clamp_width(px(f32::NAN), px(160.0), px(720.0)), px(160.0));

        // 拖右缘：向右变宽。
        assert_eq!(
            resize_width(px(260.0), ResizeEdge::Right, px(40.0), px(160.0), px(720.0)),
            px(300.0)
        );
        // 拖左缘：向左变宽（取负号）。
        assert_eq!(
            resize_width(px(260.0), ResizeEdge::Left, px(-40.0), px(160.0), px(720.0)),
            px(300.0)
        );
        // 收过头按最小宽度兜底。
        assert_eq!(
            resize_width(px(260.0), ResizeEdge::Left, px(999.0), px(160.0), px(720.0)),
            px(160.0)
        );
    }

    /// 把手 / 关闭按钮 / 浮动默认矩形。
    #[test]
    fn grip_close_and_floating_rects() {
        let rect = Bounds::new(point(px(0.0), px(0.0)), size(px(260.0), px(500.0)));
        // 左停靠：把手在右缘。
        let left = grip_rect(rect, TaskPaneSide::Left);
        assert_eq!(left.origin.x + left.size.width, px(260.0));
        assert_eq!(left.size.height, px(500.0));
        // 右停靠：把手在左缘，且拖拽边相反。
        let right = grip_rect(rect, TaskPaneSide::Right);
        assert_eq!(right.origin.x, px(0.0));
        assert_eq!(TaskPaneSide::Left.grip_edge(), ResizeEdge::Right);
        assert_eq!(TaskPaneSide::Right.grip_edge(), ResizeEdge::Left);

        // 关闭按钮贴标题栏右上角。
        let close = close_button_rect(rect);
        assert_eq!(close.origin.x + close.size.width, px(260.0));
        assert_eq!(close.size, size(CLOSE_BUTTON_WIDTH, TITLE_HEIGHT));

        // 浮动默认贴父容器右下角内侧，且不越出左上角。
        let parent = Bounds::new(point(px(0.0), px(0.0)), size(px(1000.0), px(700.0)));
        let floating = floating_default_bounds(parent, size(px(300.0), px(400.0)));
        assert_eq!(
            floating.origin,
            point(px(1000.0 - 300.0 - 12.0), px(700.0 - 400.0 - 12.0))
        );
        let tiny_parent = Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        let floating = floating_default_bounds(tiny_parent, size(px(300.0), px(400.0)));
        assert_eq!(floating.origin, point(px(12.0), px(12.0)));
    }

    /// 事件收集宿主（无窗口即可驱动状态）。
    struct Probe {
        /// 窗格状态。
        state: Entity<TaskPaneState>,
        /// 事件日志。
        log: Rc<RefCell<Vec<TaskPaneEvent>>>,
    }

    impl Probe {
        /// 创建宿主并订阅窗格事件。
        fn new(cx: &mut Context<Self>) -> Self {
            let state = cx.new(TaskPaneState::new);
            let log: Rc<RefCell<Vec<TaskPaneEvent>>> = Rc::new(RefCell::new(Vec::new()));
            let sink = log.clone();
            cx.subscribe(&state, move |_this, _state, event: &TaskPaneEvent, _cx| {
                sink.borrow_mut().push(event.clone());
            })
            .detach();
            Self { state, log }
        }
    }

    /// 显示 / 隐藏 / 改宽 / 换侧都抛事件。
    #[rgpui::test]
    fn visibility_side_and_width_events(cx: &mut TestAppContext) {
        let probe = cx.update(|cx| cx.new(Probe::new));
        let state = probe.read_with(cx, |probe, _| probe.state.clone());

        // 默认：右侧停靠、可见、默认宽度。
        assert!(state.read_with(cx, |state, _| state.is_visible()));
        assert_eq!(
            state.read_with(cx, |state, _| state.side()),
            TaskPaneSide::Right
        );
        assert_eq!(state.read_with(cx, |state, _| state.width()), DEFAULT_WIDTH);

        // 切换显示：隐藏一次为真，重复隐藏为假。
        assert!(cx.update(|cx| state.update(cx, |state, cx| state.toggle(cx))));
        assert!(!state.read_with(cx, |state, _| state.is_visible()));
        assert!(!cx.update(|cx| state.update(cx, |state, cx| state.hide(cx))));
        assert!(cx.update(|cx| state.update(cx, |state, cx| state.show(cx))));

        // 宽度夹取。
        cx.update(|cx| state.update(cx, |state, cx| state.set_width(px(9_999.0), cx)));
        assert_eq!(state.read_with(cx, |state, _| state.width()), MAX_WIDTH);
        cx.update(|cx| state.update(cx, |state, cx| state.set_width(px(1.0), cx)));
        assert_eq!(state.read_with(cx, |state, _| state.width()), MIN_WIDTH);
        // 同值不抛事件。
        assert!(!cx.update(|cx| state.update(cx, |state, cx| state.set_width(MIN_WIDTH, cx))));

        // 换侧。
        assert!(
            cx.update(|cx| state.update(cx, |state, cx| state.set_side(TaskPaneSide::Left, cx)))
        );
        assert!(
            !cx.update(|cx| state.update(cx, |state, cx| state.set_side(TaskPaneSide::Left, cx)))
        );

        let log = probe.read_with(cx, |probe, _| probe.log.borrow().clone());
        assert_eq!(log[0], TaskPaneEvent::VisibilityChanged(false));
        assert_eq!(log[1], TaskPaneEvent::VisibilityChanged(true));
        assert!(log.contains(&TaskPaneEvent::WidthChanged(MAX_WIDTH)));
        assert!(log.contains(&TaskPaneEvent::WidthChanged(MIN_WIDTH)));
        assert_eq!(
            log.last(),
            Some(&TaskPaneEvent::SideChanged(TaskPaneSide::Left))
        );
    }

    /// 浮动 / 停靠切换维护浮动矩形与事件。
    #[rgpui::test]
    fn float_and_dock(cx: &mut TestAppContext) {
        let probe = cx.update(|cx| cx.new(Probe::new));
        let state = probe.read_with(cx, |probe, _| probe.state.clone());

        let parent = Bounds::new(point(px(0.0), px(0.0)), size(px(1000.0), px(700.0)));
        let bounds = Bounds::new(point(px(120.0), px(140.0)), size(px(320.0), px(420.0)));
        assert!(cx.update(|cx| state.update(cx, |state, cx| state.float(bounds, cx))));
        assert!(state.read_with(cx, |state, _| state.is_floating()));
        assert_eq!(
            state.read_with(cx, |state, _| state.floating_bounds()),
            bounds
        );
        // 已浮动时重复浮动只更新矩形。
        let moved = Bounds::new(point(px(200.0), px(200.0)), size(px(320.0), px(420.0)));
        assert!(!cx.update(|cx| state.update(cx, |state, cx| state.float(moved, cx))));
        assert_eq!(
            state.read_with(cx, |state, _| state.floating_bounds()),
            moved
        );
        // 过小的浮动矩形会抬到最小尺寸。
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.float(
                    Bounds::new(point(px(10.0), px(10.0)), size(px(20.0), px(20.0))),
                    cx,
                )
            })
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.floating_bounds().size),
            MIN_FLOATING_SIZE
        );

        // 回到停靠态。
        assert!(cx.update(|cx| state.update(cx, |state, cx| state.dock(cx))));
        assert!(!state.read_with(cx, |state, _| state.is_floating()));
        assert!(!cx.update(|cx| state.update(cx, |state, cx| state.dock(cx))));
        // 再切换：浮到父容器右下角内侧。
        assert!(cx.update(|cx| { state.update(cx, |state, cx| state.toggle_float(parent, cx)) }));
        let floating = state.read_with(cx, |state, _| state.floating_bounds());
        assert!(parent.contains(&floating.origin));
        assert!(floating.origin.x + floating.size.width <= parent.size.width);

        let log = probe.read_with(cx, |probe, _| probe.log.borrow().clone());
        assert!(log.contains(&TaskPaneEvent::FloatingChanged(true)));
        assert!(log.contains(&TaskPaneEvent::FloatingChanged(false)));
    }

    /// 停靠态：拖动内缘把手改宽（左停靠为右缘，右停靠为左缘）。
    #[rgpui::test]
    fn docked_grip_drag_resizes_width(cx: &mut TestAppContext) {
        let probe = cx.update(|cx| cx.new(Probe::new));
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        // 伪造成窗口原点处的窗格矩形。
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.set_rect(
                    Bounds::new(point(px(0.0), px(0.0)), size(px(260.0), px(500.0))),
                    cx,
                )
            })
        });

        // 右停靠 → 把手在左缘。
        let grip = point(px(2.0), px(250.0));
        assert_eq!(
            state.read_with(cx, |state, _| state.hit_test(grip)),
            TaskPaneHit::Grip
        );
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.on_pointer_down(grip);
                // 向左拖 40px → 变宽 40px。
                state.on_pointer_move(point(px(-38.0), px(250.0)), cx);
            })
        });
        assert_eq!(state.read_with(cx, |state, _| state.width()), px(300.0));
        // 收过头按最小宽度兜底。
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.on_pointer_move(point(px(900.0), px(250.0)), cx);
                state.on_pointer_up(point(px(900.0), px(250.0)), cx);
            })
        });
        assert_eq!(state.read_with(cx, |state, _| state.width()), MIN_WIDTH);

        // 左停靠 → 把手在右缘，向右拖变宽。
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.set_side(TaskPaneSide::Left, cx);
                state.set_width(px(200.0), cx);
                state.set_rect(
                    Bounds::new(point(px(0.0), px(0.0)), size(px(200.0), px(500.0))),
                    cx,
                )
            })
        });
        let grip = point(px(198.0), px(250.0));
        assert_eq!(
            state.read_with(cx, |state, _| state.hit_test(grip)),
            TaskPaneHit::Grip
        );
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.on_pointer_down(grip);
                state.on_pointer_move(point(px(258.0), px(250.0)), cx);
                state.on_pointer_up(point(px(258.0), px(250.0)), cx);
            })
        });
        assert_eq!(state.read_with(cx, |state, _| state.width()), px(260.0));
    }

    /// 浮动态：标题栏拖动平移、右下角拖拽缩放。
    #[rgpui::test]
    fn floating_drag_moves_and_resizes(cx: &mut TestAppContext) {
        let probe = cx.update(|cx| cx.new(Probe::new));
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        // 父容器原点视为 (0,0)：窗口矩形即浮动矩形。
        let bounds = Bounds::new(point(px(100.0), px(100.0)), size(px(300.0), px(400.0)));
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.float(bounds, cx);
                state.set_rect(bounds, cx);
            })
        });

        // 标题栏拖动：整体平移。
        let title = point(px(110.0), px(108.0));
        assert_eq!(
            state.read_with(cx, |state, _| state.hit_test(title)),
            TaskPaneHit::TitleBar
        );
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.on_pointer_down(title);
                state.on_pointer_move(point(px(140.0), px(128.0)), cx);
                state.on_pointer_up(point(px(140.0), px(128.0)), cx);
            })
        });
        let moved = state.read_with(cx, |state, _| state.floating_bounds());
        assert_eq!(moved.origin, point(px(130.0), px(120.0)));
        assert_eq!(moved.size, bounds.size);
        // 跟随新位置刷新矩形（真实运行中由预绘制同步）。
        cx.update(|cx| state.update(cx, |state, cx| state.set_rect(moved, cx)));

        // 右下角拖拽缩放。
        let corner = point(
            moved.origin.x + moved.size.width - px(2.0),
            moved.origin.y + moved.size.height - px(2.0),
        );
        assert_eq!(
            state.read_with(cx, |state, _| state.hit_test(corner)),
            TaskPaneHit::Resize(ResizeEdge::BottomRight)
        );
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.on_pointer_down(corner);
                state.on_pointer_move(point(corner.x + px(50.0), corner.y + px(30.0)), cx);
                state.on_pointer_up(point(corner.x + px(50.0), corner.y + px(30.0)), cx);
            })
        });
        let resized = state.read_with(cx, |state, _| state.floating_bounds());
        assert_eq!(
            resized.size,
            size(moved.size.width + px(50.0), moved.size.height + px(30.0))
        );
    }

    /// 关闭按钮：按下并在按钮上释放才隐藏。
    #[rgpui::test]
    fn close_button_hides_pane(cx: &mut TestAppContext) {
        let probe = cx.update(|cx| cx.new(Probe::new));
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        let rect = Bounds::new(point(px(0.0), px(0.0)), size(px(260.0), px(500.0)));
        cx.update(|cx| state.update(cx, |state, cx| state.set_rect(rect, cx)));

        let close = close_button_rect(rect).center();
        assert_eq!(
            state.read_with(cx, |state, _| state.hit_test(close)),
            TaskPaneHit::Close
        );
        // 按下后滑出按钮再释放 → 不关闭。
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.on_pointer_down(close);
                state.on_pointer_up(point(px(120.0), px(200.0)), cx);
            })
        });
        assert!(state.read_with(cx, |state, _| state.is_visible()));

        // 正常点击 → 隐藏。
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.on_pointer_down(close);
                state.on_pointer_up(close, cx);
            })
        });
        assert!(!state.read_with(cx, |state, _| state.is_visible()));
        // 隐藏后不再命中。
        assert_eq!(
            state.read_with(cx, |state, _| state.hit_test(close)),
            TaskPaneHit::None
        );
        // 关闭按钮可禁用。
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.show(cx);
                state.set_closable(false, cx);
            })
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.hit_test(close)),
            TaskPaneHit::TitleBar
        );
    }

    /// 渲染宿主。
    struct RenderProbe {
        /// 窗格状态。
        state: Entity<TaskPaneState>,
    }

    impl Render for RenderProbe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().relative().flex().flex_row().size_full().child(
                TaskPane::new(&self.state)
                    .title("属性")
                    .body(|_window, _cx| div().child("正文").into_any_element()),
            )
        }
    }

    /// 真实绘制：停靠态占位并回写窗格矩形；隐藏后不再占位。
    #[rgpui::test]
    fn renders_and_tracks_rect(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|_window, cx| RenderProbe {
            state: cx.new(TaskPaneState::new),
        });
        let state = probe.read_with(&*cx, |probe, _| probe.state.clone());
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        let rect = state.read_with(&*cx, |state, _| state.rect());
        assert!(rect.size.width > px(0.0) && rect.size.height > px(0.0));

        // 隐藏后再绘制一帧不 panic，且矩形不再更新为有宽高。
        cx.update(|_, cx| state.update(cx, |state, cx| state.hide(cx)));
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        assert!(!state.read_with(&*cx, |state, _| state.is_visible()));

        // 浮动态渲染一帧（父容器已是定位祖先）。
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.float(
                    Bounds::new(point(px(20.0), px(20.0)), size(px(280.0), px(360.0))),
                    cx,
                )
            })
        });
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        assert!(state.read_with(&*cx, |state, _| state.is_floating()));
    }

    /// 自动隐藏：悬停展开、移出收起；浮动与关闭自动隐藏时保持展开。
    #[rgpui::test]
    fn auto_hide_collapses_and_hover_expands(cx: &mut TestAppContext) {
        let probe = cx.update(|cx| cx.new(Probe::new));
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        let rect = Bounds::new(point(px(0.0), px(0.0)), size(px(260.0), px(500.0)));
        cx.update(|cx| state.update(cx, |state, cx| state.set_rect(rect, cx)));

        // 默认展开、未启用自动隐藏。
        assert!(!state.read_with(cx, |state, _| state.is_auto_hide()));
        assert!(!state.read_with(cx, |state, _| state.is_collapsed()));
        assert_eq!(
            state.read_with(cx, |state, _| state.effective_width()),
            DEFAULT_WIDTH
        );

        // 启用后鼠标不在窗格上 → 收起为轨道。
        assert!(cx.update(|cx| state.update(cx, |state, cx| state.set_auto_hide(true, cx))));
        assert!(state.read_with(cx, |state, _| state.is_collapsed()));
        assert_eq!(
            state.read_with(cx, |state, _| state.effective_width()),
            RAIL_WIDTH
        );
        // 收起态整条按标题栏命中，不提供关闭 / 缩放。
        assert_eq!(
            state.read_with(cx, |state, _| state.hit_test(point(px(10.0), px(10.0)))),
            TaskPaneHit::TitleBar
        );
        assert_eq!(
            state.read_with(cx, |state, _| state
                .hit_test(close_button_rect(rect).center())),
            TaskPaneHit::TitleBar
        );

        // 悬停展开、移出收起；重复设置同一状态不返回变化。
        assert!(cx.update(|cx| state.update(cx, |state, cx| state.set_hovered(true, cx))));
        assert!(!state.read_with(cx, |state, _| state.is_collapsed()));
        assert!(!cx.update(|cx| state.update(cx, |state, cx| state.set_hovered(true, cx))));
        assert!(cx.update(|cx| state.update(cx, |state, cx| state.set_hovered(false, cx))));
        assert!(state.read_with(cx, |state, _| state.is_collapsed()));

        // 浮动时忽略自动隐藏，回到停靠后重新收起。
        cx.update(|cx| state.update(cx, |state, cx| state.float(rect, cx)));
        assert!(!state.read_with(cx, |state, _| state.is_collapsed()));
        assert_eq!(
            state.read_with(cx, |state, _| state.effective_width()),
            DEFAULT_WIDTH
        );
        cx.update(|cx| state.update(cx, |state, cx| state.dock(cx)));
        assert!(state.read_with(cx, |state, _| state.is_collapsed()));

        // 关闭自动隐藏 → 立即展开，悬停不再改变收起状态。
        assert!(cx.update(|cx| state.update(cx, |state, cx| state.set_auto_hide(false, cx))));
        assert!(!state.read_with(cx, |state, _| state.is_collapsed()));
        assert!(!cx.update(|cx| state.update(cx, |state, cx| state.set_hovered(true, cx))));
        assert!(!state.read_with(cx, |state, _| state.is_collapsed()));

        // 关闭自动隐藏后手动收起：悬停不会展开，需要显式点击（`set_collapsed(false)`）。
        assert!(cx.update(|cx| state.update(cx, |state, cx| state.set_collapsed(true, cx))));
        assert!(state.read_with(cx, |state, _| state.is_collapsed()));
        assert!(!cx.update(|cx| state.update(cx, |state, cx| state.set_hovered(false, cx))));
        assert!(state.read_with(cx, |state, _| state.is_collapsed()));
        assert!(cx.update(|cx| state.update(cx, |state, cx| state.set_collapsed(false, cx))));
        assert!(!state.read_with(cx, |state, _| state.is_collapsed()));

        // 收起 / 展开事件成对出现。
        let log = probe.read_with(cx, |probe, _| probe.log.borrow().clone());
        assert!(log.contains(&TaskPaneEvent::Collapsed(true)));
        assert!(log.contains(&TaskPaneEvent::Collapsed(false)));
    }

    /// 页签几何：等宽平铺标题栏、右侧预留关闭按钮。
    #[test]
    fn tab_rects_tile_title_bar() {
        let rect = Bounds::new(point(px(0.0), px(0.0)), size(px(260.0), px(400.0)));
        let rects = tab_rects(rect, 3);
        assert_eq!(rects.len(), 3);
        // 左起 8px，右到 260-24=236，三格等宽。
        assert_eq!(rects[0].origin.x, TITLE_PADDING);
        let expected = (236.0 - 8.0) / 3.0;
        assert!((rects[0].size.width.as_f32() - expected).abs() < 1e-4);
        assert_eq!(rects[2].origin.x + rects[2].size.width, px(236.0));
        assert_eq!(rects[1].origin.y, px(0.0));
        assert_eq!(rects[1].size.height, TITLE_HEIGHT);

        // 没有页签 / 宽度不够时不产出矩形。
        assert!(tab_rects(rect, 0).is_empty());
        let narrow = Bounds::new(point(px(0.0), px(0.0)), size(px(20.0), px(400.0)));
        assert!(tab_rects(narrow, 2).is_empty());
    }

    /// 页签选择：点击命中、事件、循环切换、越界与关闭按钮优先。
    #[rgpui::test]
    fn select_tabs_by_hit_test(cx: &mut TestAppContext) {
        let probe = cx.update(|cx| cx.new(Probe::new));
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        let rect = Bounds::new(point(px(100.0), px(100.0)), size(px(300.0), px(500.0)));
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.set_rect(rect, cx);
                state.set_tabs(vec!["大纲".into(), "属性".into(), "历史".into()], cx);
            })
        });

        // 等宽三格：108 + 89.33 的步长（窗口坐标再加 100 偏移）。
        let width = (300.0 - 8.0 - 24.0) / 3.0;
        let centers: Vec<Point<Pixels>> = (0..3)
            .map(|index| point(px(100.0 + 8.0 + width * (index as f32 + 0.5)), px(110.0)))
            .collect();
        for (index, center) in centers.iter().enumerate() {
            assert_eq!(
                state.read_with(cx, |state, _| state.hit_test(*center)),
                TaskPaneHit::Tab(index)
            );
            assert_eq!(
                state.read_with(cx, |state, _| state.tab_at(*center)),
                Some(index)
            );
        }

        // 关闭按钮优先于页签，非标题栏区域不算页签。
        let close = close_button_rect(rect);
        assert_eq!(
            state.read_with(cx, |state, _| state.hit_test(close.center())),
            TaskPaneHit::Close
        );
        assert_eq!(
            state.read_with(cx, |state, _| state
                .tab_at(point(close.center().x, px(300.0)))),
            None
        );
        assert_eq!(
            state.read_with(cx, |state, _| state.tab_at(point(px(300.0), px(300.0)))),
            None
        );

        // 点击第二个页签：切换 + 事件；重复选择不返回变化。
        assert!(cx.update(|cx| state.update(cx, |state, cx| state.set_active_tab(1, cx))));
        assert!(!cx.update(|cx| state.update(cx, |state, cx| state.set_active_tab(1, cx))));
        assert!(!cx.update(|cx| state.update(cx, |state, cx| state.set_active_tab(9, cx))));
        assert_eq!(state.read_with(cx, |state, _| state.active_tab()), 1);
        assert_eq!(
            state.read_with(cx, |state, _| state.active_tab_title().cloned()),
            Some("属性".into())
        );

        // 循环切换：从 1 前进到 2，再前进回绕到 0，后退回到 2。
        assert!(cx.update(|cx| state.update(cx, |state, cx| state.cycle_tab(1, cx))));
        assert_eq!(state.read_with(cx, |state, _| state.active_tab()), 2);
        assert!(cx.update(|cx| state.update(cx, |state, cx| state.cycle_tab(1, cx))));
        assert_eq!(state.read_with(cx, |state, _| state.active_tab()), 0);
        assert!(cx.update(|cx| state.update(cx, |state, cx| state.cycle_tab(-1, cx))));
        assert_eq!(state.read_with(cx, |state, _| state.active_tab()), 2);

        // 缩减页签时激活下标夹回；同列表不算变化。
        assert!(
            cx.update(|cx| state.update(cx, |state, cx| state.set_tabs(vec!["大纲".into()], cx)))
        );
        assert_eq!(state.read_with(cx, |state, _| state.active_tab()), 0);
        assert!(
            !cx.update(|cx| state.update(cx, |state, cx| state.set_tabs(vec!["大纲".into()], cx)))
        );
        // 无页签时循环切换无事发生。
        assert!(cx.update(|cx| state.update(cx, |state, cx| state.set_tabs(Vec::new(), cx))));
        assert!(!cx.update(|cx| state.update(cx, |state, cx| state.cycle_tab(1, cx))));

        let log = probe.read_with(cx, |probe, _| probe.log.borrow().clone());
        assert!(log.contains(&TaskPaneEvent::TabSelected(1)));
        assert!(log.contains(&TaskPaneEvent::TabSelected(2)));
    }

    /// 堆叠权重换算：位移按可用高度折算，两格之和不变、夹到下限、其他格不动。
    #[test]
    fn resize_stack_weights_moves_boundary() {
        let mut weights = vec![1.0, 1.0, 2.0];
        // 可用高度 200px、总权重 4：向下拖 30px → 上格 +30*4/200 = +0.6。
        assert!(resize_stack_weights(&mut weights, 0, px(30.0), px(200.0)));
        assert!((weights[0] - 1.6).abs() < 1e-4);
        assert!((weights[1] - 0.4).abs() < 1e-4);
        assert!((weights[2] - 2.0).abs() < 1e-4);
        // 两格之和保持不变。
        assert!((weights[0] + weights[1] - 2.0).abs() < 1e-4);

        // 拖过头：下格夹到最小权重，上格吃掉剩余的。
        assert!(resize_stack_weights(&mut weights, 0, px(1000.0), px(200.0)));
        assert!((weights[1] - MIN_STACK_WEIGHT).abs() < 1e-6);
        assert!((weights[0] - (2.0 - MIN_STACK_WEIGHT)).abs() < 1e-6);

        // 位移为 0、下标越界、可用高度为 0 都不改动。
        assert!(!resize_stack_weights(&mut weights, 0, px(0.0), px(200.0)));
        assert!(!resize_stack_weights(&mut weights, 2, px(10.0), px(200.0)));
        assert!(!resize_stack_weights(&mut weights, 0, px(10.0), px(0.0)));

        // 非法权重先规范化，再参与换算。
        assert_eq!(normalize_stack_weight(f32::NAN), 1.0);
        assert_eq!(normalize_stack_weight(-2.0), 1.0);
        assert_eq!(normalize_stack_weight(0.001), MIN_STACK_WEIGHT);
        let mut dirty = vec![f32::NAN, 1.0];
        assert!(resize_stack_weights(&mut dirty, 0, px(10.0), px(100.0)));
        assert!(dirty[0] > 1.0);
    }

    /// 堆叠状态：权重规范化、分隔条拖拽（按总位移重算）与结束清理。
    #[rgpui::test]
    fn stack_state_resizes_with_drag(cx: &mut TestAppContext) {
        let state = cx.new(|cx| TaskPaneStackState::new(cx));
        let (usable, dragging) = cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.set_weights([1.0, 1.0], cx);
                // 视口 104px、两格一条分隔条 → 可用 100px。
                state.rect = Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(104.0)));
                state.slots = 2;
                state.begin_drag(0, px(100.0), cx);
                (state.usable_height(), state.dragging())
            })
        });
        assert_eq!(usable, px(100.0));
        assert_eq!(dragging, Some(0));

        // 向下拖 30px：上格 1.6、下格 0.4。
        cx.update(|cx| state.update(cx, |state, cx| state.update_drag(px(130.0), cx)));
        let (above, below) = state.read_with(cx, |state, _| (state.weight(0), state.weight(1)));
        assert!((above - 1.6).abs() < 1e-4);
        assert!((below - 0.4).abs() < 1e-4);

        // 再拖到极远仍从起点算起，不累积误差：下格夹到最小权重。
        cx.update(|cx| state.update(cx, |state, cx| state.update_drag(px(1000.0), cx)));
        assert!((state.read_with(cx, |state, _| state.weight(1)) - MIN_STACK_WEIGHT).abs() < 1e-6);

        cx.update(|cx| state.update(cx, |state, cx| state.end_drag(cx)));
        assert_eq!(state.read_with(cx, |state, _| state.dragging()), None);

        // 缺失下标回退 1.0；set_weight 越界补 1.0。
        assert_eq!(state.read_with(cx, |state, _| state.weight(9)), 1.0);
        assert!(cx.update(|cx| state.update(cx, |state, cx| state.set_weight(4, 3.0, cx))));
        assert_eq!(state.read_with(cx, |state, _| state.weight(4)), 3.0);
        assert_eq!(state.read_with(cx, |state, _| state.weight(3)), 1.0);
        // 非正数回退 1.0；过小权重夹到下限。
        cx.update(|cx| state.update(cx, |state, cx| state.set_weight(0, 0.0, cx)));
        assert_eq!(state.read_with(cx, |state, _| state.weight(0)), 1.0);
        cx.update(|cx| state.update(cx, |state, cx| state.set_weight(0, 0.001, cx)));
        assert_eq!(
            state.read_with(cx, |state, _| state.weight(0)),
            MIN_STACK_WEIGHT
        );
    }

    /// 真实绘制：按权重分配高度，分隔条占位，两格矩形依次下移。
    #[rgpui::test]
    fn stack_renders_weighted_panes(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);

        struct StackProbe {
            stack: Entity<TaskPaneStackState>,
            first: Entity<TaskPaneState>,
            second: Entity<TaskPaneState>,
        }

        impl Render for StackProbe {
            fn render(
                &mut self,
                _window: &mut Window,
                _cx: &mut Context<Self>,
            ) -> impl IntoElement {
                div().w(px(240.0)).h(px(304.0)).child(
                    TaskPaneStack::new(&self.stack)
                        .child(TaskPane::new(&self.first).title("一"))
                        .child(TaskPane::new(&self.second).title("二")),
                )
            }
        }

        let (probe, cx) = cx.add_window_view(|_window, cx| StackProbe {
            stack: cx.new(|cx| TaskPaneStackState::new(cx).with_weights([2.0, 1.0])),
            first: cx.new(TaskPaneState::new),
            second: cx.new(TaskPaneState::new),
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });

        let (first, second, slots, usable) = probe.read_with(&*cx, |probe, cx| {
            (
                probe.first.read(cx).rect(),
                probe.second.read(cx).rect(),
                probe.stack.read(cx).slots(),
                probe.stack.read(cx).usable_height(),
            )
        });
        assert_eq!(slots, 2);
        assert_eq!(usable, px(300.0));
        // 2:1 → 200px / 100px。
        assert!((first.size.height - px(200.0)).abs() < px(1.0));
        assert!((second.size.height - px(100.0)).abs() < px(1.0));
        // 第二格紧接在第一格 + 分隔条之后。
        assert!(
            (second.origin.y - (first.origin.y + first.size.height + STACK_SEPARATOR_HEIGHT)).abs()
                < px(1.0)
        );
    }
}
