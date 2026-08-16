use std::{cell::Cell, rc::Rc, time::Duration};

use crate::{
    Action, AnyElement, AnyView, App, AppContext, Bounds, ComponentText, Context, Display, Element,
    ElementId, GlobalElementId, Half, InspectorElementId, IntoElement, LayoutId, MouseButton,
    ParentElement, Pixels, Point, Position, Render, SharedString, Size, StatefulInteractiveElement,
    Style, StyleRefinement, Styled, Task, Window, deferred, div, point, prelude::FluentBuilder, px,
};
use crate::{
    ActiveTheme, Placement, Root, StyledExt, Transition, ease_in_out_cubic, ease_out_cubic, h_flex,
};

use super::kbd::Kbd;

/// 工具提示内容枚举，支持文本或自定义元素。
enum TooltipContext {
    Text(ComponentText),
    Element(Box<dyn Fn(&mut Window, &mut App) -> AnyElement>),
}

/// 工具提示元素，可显示文本或自定义内容，支持可选的快捷键信息。
pub struct Tooltip {
    style: StyleRefinement,
    content: TooltipContext,
    key_binding: Option<Kbd>,
    action: Option<(Box<dyn Action>, Option<SharedString>)>,
}

impl Tooltip {
    /// 使用文本内容创建 Tooltip。
    pub fn new(text: impl Into<ComponentText>) -> Self {
        Self {
            style: StyleRefinement::default(),
            content: TooltipContext::Text(text.into()),
            key_binding: None,
            action: None,
        }
    }

    /// 使用自定义元素创建 Tooltip。
    pub fn element<E, F>(builder: F) -> Self
    where
        E: IntoElement,
        F: Fn(&mut Window, &mut App) -> E + 'static,
    {
        Self {
            style: StyleRefinement::default(),
            key_binding: None,
            action: None,
            content: TooltipContext::Element(Box::new(move |window, cx| {
                builder(window, cx).into_any_element()
            })),
        }
    }

    /// 设置 Action 以显示该 action 的快捷键信息（如果存在）。
    pub fn action(mut self, action: &dyn Action, context: Option<&str>) -> Self {
        self.action = Some((action.boxed_clone(), context.map(SharedString::new)));
        self
    }

    /// 设置 Tooltip 的快捷键信息。
    pub fn key_binding(mut self, key_binding: Option<Kbd>) -> Self {
        self.key_binding = key_binding;
        self
    }

    /// 构建 tooltip 并返回为 `AnyView`。
    pub fn build(self, _: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| self).into()
    }
}

impl FluentBuilder for Tooltip {}
impl Styled for Tooltip {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl Render for Tooltip {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let key_binding = if let Some(key_binding) = &self.key_binding {
            Some(key_binding.clone())
        } else {
            if let Some((action, context)) = &self.action {
                Kbd::binding_for_action(
                    action.as_ref(),
                    context.as_ref().map(|s| s.as_ref()),
                    window,
                )
            } else {
                None
            }
        };

        div().child(
            // 包一层 child，确保左边距应用到 tooltip 上
            h_flex()
                .font_family(cx.theme().font_family.clone())
                .m_3()
                .bg(cx.theme().tokens.popover)
                .text_color(cx.theme().popover_foreground)
                .bg(cx.theme().tokens.popover)
                .border_1()
                .border_color(cx.theme().border)
                .shadow_md()
                .rounded(px(6.))
                .justify_between()
                .py_0p5()
                .px_2()
                .text_sm()
                .gap_3()
                .refine_style(&self.style)
                .map(|this| {
                    this.child(div().map(|this| match self.content {
                        TooltipContext::Text(ref text) => this.child(text.clone()),
                        TooltipContext::Element(ref builder) => this.child(builder(window, cx)),
                    }))
                })
                .when_some(key_binding, |this, kbd| {
                    this.child(
                        div()
                            .text_xs()
                            .flex_shrink_0()
                            .text_color(cx.theme().muted_foreground)
                            .child(kbd.appearance(false)),
                    )
                }),
        )
    }
}

/// 宽限期：如果 tooltip 在此时间内被隐藏，则下次显示跳过延迟。
const GRACE_PERIOD: Duration = Duration::from_millis(300);
/// 没有活跃 tooltip 时，显示 tooltip 前的延迟。
const SHOW_DELAY: Duration = Duration::from_millis(500);
/// 下滑进入动画的时长。
const ENTER_DURATION: Duration = Duration::from_millis(150);
/// 切换 tooltip 时位置滑动动画的时长。
const SLIDE_DURATION: Duration = Duration::from_millis(200);
const TOOLTIP_WINDOW_MARGIN: Pixels = px(4.);

#[derive(Clone, Copy, Debug, PartialEq)]
struct TooltipOverlayPosition {
    bounds: Bounds<Pixels>,
    placement: Placement,
}

fn tooltip_overlay_position(
    trigger_bounds: Bounds<Pixels>,
    tooltip_size: Size<Pixels>,
    viewport_size: Size<Pixels>,
    margin: Pixels,
    preferred_placement: Option<Placement>,
) -> TooltipOverlayPosition {
    let right_limit = (viewport_size.width - margin).max(margin);
    let bottom_limit = (viewport_size.height - margin).max(margin);
    let available_left = (trigger_bounds.left() - margin).max(px(0.));
    let available_right = (right_limit - trigger_bounds.right()).max(px(0.));
    let available_above = (trigger_bounds.top() - margin).max(px(0.));
    let available_below = (bottom_limit - trigger_bounds.bottom()).max(px(0.));

    let placement = match preferred_placement {
        Some(Placement::Right) if tooltip_size.width <= available_right => Placement::Right,
        Some(Placement::Right) if tooltip_size.width <= available_left => Placement::Left,
        Some(Placement::Right) if available_right >= available_left => Placement::Right,
        Some(Placement::Right) => Placement::Left,
        Some(Placement::Left) if tooltip_size.width <= available_left => Placement::Left,
        Some(Placement::Left) if tooltip_size.width <= available_right => Placement::Right,
        Some(Placement::Left) if available_left >= available_right => Placement::Left,
        Some(Placement::Left) => Placement::Right,
        Some(Placement::Bottom) if tooltip_size.height <= available_below => Placement::Bottom,
        Some(Placement::Bottom) if tooltip_size.height <= available_above => Placement::Top,
        Some(Placement::Bottom) if available_below >= available_above => Placement::Bottom,
        Some(Placement::Bottom) => Placement::Top,
        Some(Placement::Top) | None if tooltip_size.height <= available_above => Placement::Top,
        Some(Placement::Top) | None if tooltip_size.height <= available_below => Placement::Bottom,
        Some(Placement::Top) | None if available_below >= available_above => Placement::Bottom,
        Some(Placement::Top) | None => Placement::Top,
    };

    let centered_x = trigger_bounds.center().x - tooltip_size.width.half();
    let centered_y = trigger_bounds.center().y - tooltip_size.height.half();
    let origin = match placement {
        Placement::Top => point(centered_x, trigger_bounds.top() - tooltip_size.height),
        Placement::Bottom => point(centered_x, trigger_bounds.bottom()),
        Placement::Left => point(trigger_bounds.left() - tooltip_size.width, centered_y),
        Placement::Right => point(trigger_bounds.right(), centered_y),
    };
    let bounds = Bounds::new(origin, tooltip_size);

    TooltipOverlayPosition {
        bounds: clamp_tooltip_bounds(bounds, viewport_size, margin),
        placement,
    }
}

fn clamp_tooltip_bounds(
    mut bounds: Bounds<Pixels>,
    viewport_size: Size<Pixels>,
    margin: Pixels,
) -> Bounds<Pixels> {
    let right_limit = (viewport_size.width - margin).max(margin);
    let bottom_limit = (viewport_size.height - margin).max(margin);

    if bounds.right() > right_limit {
        bounds.origin.x -= bounds.right() - right_limit;
    }
    if bounds.left() < margin {
        bounds.origin.x = margin;
    }

    if bounds.bottom() > bottom_limit {
        bounds.origin.y -= bounds.bottom() - bottom_limit;
    }
    if bounds.top() < margin {
        bounds.origin.y = margin;
    }

    bounds
}

struct TooltipOverlayPositioner {
    trigger_bounds: Bounds<Pixels>,
    preferred_placement: Option<Placement>,
    children: Vec<AnyElement>,
}

struct TooltipOverlayPositionerState {
    child_layout_ids: Vec<LayoutId>,
}

fn tooltip_overlay_positioner(
    trigger_bounds: Bounds<Pixels>,
    preferred_placement: Option<Placement>,
) -> TooltipOverlayPositioner {
    TooltipOverlayPositioner {
        trigger_bounds,
        preferred_placement,
        children: Vec::new(),
    }
}

impl ParentElement for TooltipOverlayPositioner {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Element for TooltipOverlayPositioner {
    type RequestLayoutState = TooltipOverlayPositionerState;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let child_layout_ids = self
            .children
            .iter_mut()
            .map(|child| child.request_layout(window, cx))
            .collect::<Vec<_>>();

        let layout_id = window.request_layout(
            Style {
                position: Position::Absolute,
                display: Display::Flex,
                ..Style::default()
            },
            child_layout_ids.iter().copied(),
            cx,
        );

        (
            layout_id,
            TooltipOverlayPositionerState { child_layout_ids },
        )
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if request_layout.child_layout_ids.is_empty() {
            return;
        }

        let mut child_min: Point<Pixels> = point(Pixels::MAX, Pixels::MAX);
        let mut child_max = Point::default();
        for child_layout_id in &request_layout.child_layout_ids {
            let child_bounds = window.layout_bounds(*child_layout_id);
            child_min = child_min.min(&child_bounds.origin);
            child_max = child_max.max(&child_bounds.bottom_right());
        }

        let tooltip_size: Size<Pixels> = (child_max - child_min).into();
        let client_inset = window.client_inset().unwrap_or(px(0.));
        let tooltip_position = tooltip_overlay_position(
            self.trigger_bounds,
            tooltip_size,
            window.viewport_size(),
            TOOLTIP_WINDOW_MARGIN + client_inset,
            self.preferred_placement,
        );

        let offset = tooltip_position.bounds.origin - bounds.origin;
        let offset = point(offset.x.round(), offset.y.round());

        window.with_element_offset(offset, |window| {
            for child in &mut self.children {
                child.prepaint(window, cx);
            }
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        for child in &mut self.children {
            child.paint(window, cx);
        }
    }
}

impl IntoElement for TooltipOverlayPositioner {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// 受管 tooltip 的内容。
#[derive(Clone)]
pub(crate) struct TooltipContent {
    pub build: Rc<dyn Fn(&mut Window, &mut App) -> AnyView>,
    pub trigger_bounds: Bounds<Pixels>,
    pub preferred_placement: Option<Placement>,
}

/// 管理 tooltip 生命周期：延迟、宽限期、动画和渲染。
///
/// 每个窗口的 [`Root`] 中有一个实例。组件通过
/// [`ManagedTooltipExt::managed_tooltip`] 注册悬停事件，该方法会调用此覆盖层。
pub struct TooltipOverlay {
    content: Option<TooltipContent>,
    prev_trigger_bounds: Option<Bounds<Pixels>>,
    epoch: usize,
    had_recent_tooltip: bool,
    animation_epoch: usize,
    is_switching: bool,

    _show_task: Option<Task<()>>,
    _hide_task: Option<Task<()>>,
}

impl TooltipOverlay {
    /// 创建新的 TooltipOverlay。
    pub fn new() -> Self {
        Self {
            content: None,
            prev_trigger_bounds: None,
            epoch: 0,
            had_recent_tooltip: false,
            animation_epoch: 0,
            is_switching: false,
            _show_task: None,
            _hide_task: None,
        }
    }

    fn next_epoch(&mut self) -> usize {
        self.epoch += 1;
        self.epoch
    }

    /// 请求显示 tooltip。如果已有其他 tooltip 活跃或最近刚隐藏，
    /// 则立即显示并带滑动动画，否则启动延迟。
    pub(crate) fn request_show(
        &mut self,
        content: TooltipContent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 取消任何待处理的隐藏
        self._hide_task = None;

        let was_visible = self.content.is_some();
        let in_grace = self.had_recent_tooltip;

        if was_visible || in_grace {
            // 切换：立即显示并带滑动动画
            self.prev_trigger_bounds = self.content.as_ref().map(|c| c.trigger_bounds);
            self.content = Some(content);
            self._show_task = None;
            self.is_switching = was_visible;
            self.animation_epoch += 1;
            cx.notify();
        } else {
            // 新建：延迟后显示并带下滑动画
            let epoch = self.next_epoch();
            let content = content.clone();
            self._show_task = Some(cx.spawn_in(window, async move |this, cx| {
                cx.background_executor().timer(SHOW_DELAY).await;
                let _ = this.update_in(cx, |this, _, cx| {
                    if this.epoch != epoch {
                        return;
                    }

                    this.content = Some(content);
                    this.prev_trigger_bounds = None;
                    this.is_switching = false;
                    this.animation_epoch += 1;
                    cx.notify();
                });
            }));
        }
    }

    /// 请求隐藏当前 tooltip。启动短暂的宽限期，使移动到另一个
    /// 带 tooltip 的元素时感觉更即时。
    pub(crate) fn request_hide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 取消任何待处理的显示
        self._show_task = None;

        if self.content.is_none() {
            return;
        }

        let epoch = self.next_epoch();
        self.had_recent_tooltip = true;

        self._hide_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(GRACE_PERIOD).await;
            let _ = this.update_in(cx, |this, _, cx| {
                if this.epoch != epoch {
                    return;
                }
                this.content = None;
                this.prev_trigger_bounds = None;
                this.had_recent_tooltip = false;
                cx.notify();
            });
        }));
    }

    /// 立即隐藏 tooltip 并清除状态。
    pub(crate) fn hide(&mut self, cx: &mut Context<Self>) {
        if self.clear_state() {
            cx.notify();
        }
    }

    fn clear_state(&mut self) -> bool {
        let changed = self.content.is_some()
            || self.prev_trigger_bounds.is_some()
            || self.had_recent_tooltip
            || self.is_switching
            || self._show_task.is_some()
            || self._hide_task.is_some();

        self.content = None;
        self.prev_trigger_bounds = None;
        self.had_recent_tooltip = false;
        self.is_switching = false;
        self._show_task = None;
        self._hide_task = None;

        changed
    }
}

impl Render for TooltipOverlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(content) = self.content.as_ref() else {
            return div().into_any_element();
        };

        let content_view = (content.build)(window, cx);
        let trigger_bounds = content.trigger_bounds;
        let preferred_placement = content.preferred_placement;
        let animation_epoch = self.animation_epoch;
        let is_switching = self.is_switching;
        let prev_trigger_bounds = self.prev_trigger_bounds;

        deferred(
            tooltip_overlay_positioner(trigger_bounds, preferred_placement).child(
                div().child(content_view).map(|el| {
                    if is_switching {
                        let Some(prev_bounds) = prev_trigger_bounds else {
                            return el.into_any_element();
                        };

                        let is_same_y =
                            (trigger_bounds.origin.y - prev_bounds.origin.y).abs() < px(10.);
                        if !is_same_y {
                            // 如果新触发器在不同的 Y 层级，不做水平滑动以避免奇怪的对角线移动。
                            return el.into_any_element();
                        }

                        let dx = trigger_bounds.center().x - prev_bounds.center().x;

                        Transition::new(SLIDE_DURATION)
                            .ease(ease_in_out_cubic)
                            .slide_x(-dx, px(0.))
                            .apply(
                                el,
                                ElementId::NamedInteger(
                                    "tooltip-slide".into(),
                                    animation_epoch as u64,
                                ),
                            )
                            .into_any_element()
                    } else {
                        // 新 tooltip：下滑 + 淡入
                        Transition::new(ENTER_DURATION)
                            .ease(ease_out_cubic)
                            .slide_y(px(4.), px(0.))
                            .fade(0.0, 1.0)
                            .apply(
                                el,
                                ElementId::NamedInteger(
                                    "tooltip-enter".into(),
                                    animation_epoch as u64,
                                ),
                            )
                            .into_any_element()
                    }
                }),
            ),
        )
        .with_priority(2)
        .into_any_element()
    }
}

/// 内部受管 tooltip trait。
pub(crate) trait ManagedTooltipExt:
    StatefulInteractiveElement + crate::ElementExt + Sized
{
    fn managed_tooltip(
        self,
        build_tooltip: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
    ) -> Self {
        self.managed_tooltip_with_placement(None, build_tooltip)
    }

    fn managed_tooltip_with_placement(
        self,
        preferred_placement: Option<Placement>,
        build_tooltip: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
    ) -> Self {
        let build_tooltip = Rc::new(build_tooltip);
        let trigger_bounds_cell: Rc<Cell<Bounds<Pixels>>> = Rc::new(Cell::new(Bounds::default()));
        let bounds_writer = trigger_bounds_cell.clone();

        self.on_prepaint(move |bounds, _, _| {
            bounds_writer.set(bounds);
        })
        .on_hover({
            let trigger_bounds_cell = trigger_bounds_cell.clone();
            let build_tooltip = build_tooltip.clone();
            move |hovered, window, cx| {
                if let Some(overlay) = Root::tooltip_overlay(window, cx) {
                    if *hovered {
                        let bounds = trigger_bounds_cell.get();
                        overlay.update(cx, |o: &mut TooltipOverlay, cx| {
                            o.request_show(
                                TooltipContent {
                                    build: build_tooltip.clone(),
                                    trigger_bounds: bounds,
                                    preferred_placement,
                                },
                                window,
                                cx,
                            );
                        });
                    } else {
                        overlay.update(cx, |o: &mut TooltipOverlay, cx| {
                            o.request_hide(window, cx);
                        });
                    }
                }
            }
        })
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            if let Some(overlay) = Root::tooltip_overlay(window, cx) {
                overlay.update(cx, |overlay, cx| {
                    overlay.hide(cx);
                });
            }
        })
    }
}

impl<E: StatefulInteractiveElement + crate::ElementExt> ManagedTooltipExt for E {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::size;

    fn test_content(bounds: Bounds<Pixels>) -> TooltipContent {
        TooltipContent {
            build: Rc::new(|window, cx| Tooltip::new("Test tooltip").build(window, cx)),
            trigger_bounds: bounds,
            preferred_placement: None,
        }
    }

    fn test_bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    fn test_size(width: f32, height: f32) -> Size<Pixels> {
        size(px(width), px(height))
    }

    #[test]
    fn tooltip_overlay_clear_state_resets_active_tooltip() {
        let mut overlay = TooltipOverlay::new();

        overlay.content = Some(test_content(test_bounds(10., 10., 40., 20.)));
        overlay.prev_trigger_bounds = Some(test_bounds(0., 0., 40., 20.));
        overlay.had_recent_tooltip = true;
        overlay.is_switching = true;
        overlay._show_task = Some(Task::ready(()));

        assert!(overlay.clear_state());
        assert!(overlay.content.is_none());
        assert!(overlay.prev_trigger_bounds.is_none());
        assert!(!overlay.had_recent_tooltip);
        assert!(!overlay.is_switching);
        assert!(overlay._show_task.is_none());
        assert!(overlay._hide_task.is_none());
    }

    #[test]
    fn tooltip_overlay_position_prefers_above_when_space_allows() {
        let trigger_bounds = test_bounds(100., 80., 80., 24.);
        let position = tooltip_overlay_position(
            trigger_bounds,
            test_size(120., 30.),
            test_size(300., 200.),
            TOOLTIP_WINDOW_MARGIN,
            None,
        );

        assert_eq!(position.placement, Placement::Top);
        assert_eq!(position.bounds.origin.x, px(80.));
        assert_eq!(position.bounds.origin.y, px(50.));
        assert_eq!(position.bounds.bottom(), trigger_bounds.top());
    }

    #[test]
    fn tooltip_overlay_position_flips_below_near_top_edge() {
        let trigger_bounds = test_bounds(24., 4., 120., 32.);
        let position = tooltip_overlay_position(
            trigger_bounds,
            test_size(240., 32.),
            test_size(520., 260.),
            TOOLTIP_WINDOW_MARGIN,
            None,
        );

        assert_eq!(position.placement, Placement::Bottom);
        assert_eq!(position.bounds.top(), trigger_bounds.bottom());
        assert!(position.bounds.top() >= trigger_bounds.bottom());
    }

    #[test]
    fn tooltip_overlay_position_clamps_horizontal_edges() {
        let trigger_bounds = test_bounds(4., 80., 24., 24.);
        let position = tooltip_overlay_position(
            trigger_bounds,
            test_size(120., 30.),
            test_size(300., 200.),
            TOOLTIP_WINDOW_MARGIN,
            None,
        );

        assert_eq!(position.placement, Placement::Top);
        assert_eq!(position.bounds.left(), TOOLTIP_WINDOW_MARGIN);
    }

    #[test]
    fn tooltip_overlay_position_uses_larger_side_when_neither_side_fits() {
        let trigger_bounds = test_bounds(120., 20., 40., 20.);
        let position = tooltip_overlay_position(
            trigger_bounds,
            test_size(160., 120.),
            test_size(300., 100.),
            TOOLTIP_WINDOW_MARGIN,
            None,
        );

        assert_eq!(position.placement, Placement::Bottom);
        assert_eq!(position.bounds.top(), TOOLTIP_WINDOW_MARGIN);
        assert_eq!(position.bounds.left(), px(60.));
    }

    #[test]
    fn tooltip_overlay_position_places_tooltip_to_the_right() {
        let trigger_bounds = test_bounds(20., 60., 32., 32.);
        let position = tooltip_overlay_position(
            trigger_bounds,
            test_size(120., 30.),
            test_size(300., 200.),
            TOOLTIP_WINDOW_MARGIN,
            Some(Placement::Right),
        );

        assert_eq!(position.placement, Placement::Right);
        assert_eq!(position.bounds.left(), trigger_bounds.right());
        assert_eq!(position.bounds.center().y, trigger_bounds.center().y);
    }

    #[test]
    fn tooltip_overlay_position_flips_left_near_right_edge() {
        let trigger_bounds = test_bounds(260., 60., 32., 32.);
        let position = tooltip_overlay_position(
            trigger_bounds,
            test_size(120., 30.),
            test_size(300., 200.),
            TOOLTIP_WINDOW_MARGIN,
            Some(Placement::Right),
        );

        assert_eq!(position.placement, Placement::Left);
        assert_eq!(position.bounds.right(), trigger_bounds.left());
    }

    #[test]
    fn tooltip_overlay_position_clamps_vertical_edges_for_right_placement() {
        let trigger_bounds = test_bounds(20., 2., 32., 20.);
        let position = tooltip_overlay_position(
            trigger_bounds,
            test_size(120., 40.),
            test_size(300., 200.),
            TOOLTIP_WINDOW_MARGIN,
            Some(Placement::Right),
        );

        assert_eq!(position.placement, Placement::Right);
        assert_eq!(position.bounds.top(), TOOLTIP_WINDOW_MARGIN);
        assert_eq!(position.bounds.left(), trigger_bounds.right());
    }
}