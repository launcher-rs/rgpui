//! 滚轮事件遮罩 - 处理嵌套滚动器的滚轮事件轴分发。

use std::cell::RefCell;
use std::rc::Rc;

use crate::{
    App, Axis, BorderStyle, Bounds, ContentMask, Edges, Element, ElementId, GlobalElementId,
    Hitbox, Hsla, InteractiveElement as _, IntoElement, IsZero as _, LayoutId, OngoingScroll,
    PaintQuad, ParentElement as _, Point, Position, ScrollHandle, ScrollWheelEvent,
    StatefulInteractiveElement as _, Style, StyleRefinement, Styled as _, Window, div, px,
    relative,
};
use crate::{Corners, Pixels};

use super::scrollable::caller_id;
use crate::{AxisExt, StyledExt as _};

/// 只消费水平滚轮增量的水平滚动视口。
///
/// RGPUI 原生的 `overflow_x_scroll` 在没有垂直溢出时会把垂直滚轮输入映射为水平滚动。
/// 该包装保持视觉裁剪与滚动偏移，同时把滚轮输入委托给 [`ScrollableMask`]，
/// 使垂直滚轮事件可以继续冒泡到父级滚动器。
pub fn horizontal_scroll_area(
    id: impl Into<ElementId>,
    scroll_handle: &ScrollHandle,
    style: &StyleRefinement,
    child: impl IntoElement,
) -> impl IntoElement {
    let id = id.into();

    // 遮罩必须是滚动元素的兄弟节点（如在 Table 中），而不是其子节点：
    // 子节点会随滚动偏移一起预绘制，导致遮罩随着内容滚动而滑离视口，
    // 露出未覆盖的部分给父级滚动器。
    div()
        .w_full()
        .relative()
        .child(
            div()
                .id(id.clone())
                .w_full()
                .refine_style(style)
                .overflow_hidden()
                .track_scroll(scroll_handle)
                .child(child),
        )
        .child(ScrollableMask::new(Axis::Horizontal, scroll_handle).id(id))
}

/// 创建一个可滚动遮罩元素，覆盖父视口并监听滚轮事件。
///
/// 鼠标滚轮滚动时，会沿 `axis` 方向移动 `scroll_handle` 的滚动偏移量。
/// 可用这个 `scroll_handle` 控制你要滚动的内容。一次只能处理一个轴向的滚动。
///
/// 轴主导的滚轮事件在捕获阶段被消费，因此遮罩会优先于在其子元素之后注册监听器的祖先滚动器
/// （如 `rgpui::list`）；由另一轴主导的事件继续传播。遮罩被遮挡时保持惰性。
///
/// 主导权按手势而非事件决定：精确（触控板）增量会锁定到手势起始的轴，
/// 滑动中段的抖动不会切换消费事件的遮罩。行增量保持逐事件比较。
///
/// 在滚动边缘两轴行为不同，与平台滚动器一致：垂直遮罩把事件交给祖先滚动器
/// （CSS `overscroll-behavior: auto` 链式传递），而水平遮罩保持消费事件——被冒泡的
/// 水平增量会被 rgpui 自己的滚轮监听器映射到垂直祖先上（见 #2468）。
pub struct ScrollableMask {
    axis: Axis,
    id: ElementId,
    scroll_handle: ScrollHandle,
    debug: Option<Hsla>,
}

impl ScrollableMask {
    /// 创建新的可滚动遮罩元素。
    #[track_caller]
    pub fn new(axis: Axis, scroll_handle: &ScrollHandle) -> Self {
        Self {
            scroll_handle: scroll_handle.clone(),
            axis,
            id: caller_id(),
            debug: None,
        }
    }

    /// 设置特定元素 id，默认为 [`std::panic::Location::caller`]。
    ///
    /// 仅当同一调用点创建多个同轴向遮罩（它们会共享手势轴锁定）时才需要。
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// 启用调试边框，显示遮罩边界。
    pub fn debug(mut self) -> Self {
        self.debug = Some(crate::yellow());
        self
    }
}

impl IntoElement for ScrollableMask {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ScrollableMask {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    // 需要 id 在帧间保持手势的轴锁定。轴向后缀使同一滚动器的两个遮罩在共享 id 时区分开。
    fn id(&self) -> Option<ElementId> {
        let axis = match self.axis {
            Axis::Horizontal => "horizontal",
            Axis::Vertical => "vertical",
        };

        Some((self.id.clone(), axis).into())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&crate::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        // 相对表格视图设置布局样式以获得相同尺寸。
        style.position = Position::Absolute;
        style.flex_grow = 1.0;
        style.flex_shrink = 1.0;
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();

        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&crate::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        _: &mut App,
    ) -> Self::PrepaintState {
        // 将 y 移到 bounds 高度之上以覆盖父视图。
        let cover_bounds = Bounds {
            origin: Point {
                x: bounds.origin.x,
                y: bounds.origin.y - bounds.size.height,
            },
            size: bounds.size,
        };

        window.insert_hitbox(cover_bounds, crate::HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _: Option<&crate::InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        _: &mut App,
    ) {
        let is_horizontal = self.axis.is_horizontal();
        let line_height = window.line_height();
        let bounds = hitbox.bounds;
        let ongoing_scroll = global_id
            .map(|global_id| {
                window.with_element_state::<Rc<RefCell<OngoingScroll>>, _>(global_id, |state, _| {
                    let state = state.unwrap_or_default();
                    (state.clone(), state)
                })
            })
            .unwrap_or_default();

        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            if let Some(color) = self.debug {
                window.paint_quad(PaintQuad {
                    bounds,
                    border_widths: Edges::all(px(1.0)),
                    border_color: color,
                    background: crate::transparent_white().into(),
                    corner_radii: Corners::all(px(0.)),
                    border_style: BorderStyle::default(),
                });
            }

            window.on_mouse_event({
                let view_id = window.current_view();
                let scroll_handle = self.scroll_handle.clone();
                let hitbox_id = hitbox.id;
                let ongoing_scroll = ongoing_scroll.clone();

                move |event: &ScrollWheelEvent, phase, window, cx| {
                    // 在捕获阶段处理：祖先滚动器（如 `rgpui::list`）在其子元素绘制后
                    // 才注册滚轮监听器，因此在冒泡阶段（反向注册顺序）它们会先运行，
                    // 并在本遮罩能阻止传播之前消费触控板手势的垂直分量。
                    //
                    // `should_handle_scroll`（而非裸 bounds 检查）使遮罩在遮挡时保持惰性，
                    // 例如在打开的对话框或上下文菜单之下。
                    if !(phase.capture() && hitbox_id.should_handle_scroll(window)) {
                        return;
                    }

                    let mut offset = scroll_handle.offset();
                    let mut delta = event.delta.pixel_delta(line_height);

                    // 把手势锁定到起始轴，使对角线触控板滑动不会从事件间切换消费它的遮罩。
                    // 行增量不携带触摸相位。
                    if event.delta.precise() {
                        ongoing_scroll
                            .borrow_mut()
                            .filter(&mut delta, event.touch_phase);
                    }

                    // 限制同一时刻只单向滚动。
                    // 使用 MacBook 触控板时可能同时获得 x 和 y 增量，
                    // 只允许增量更大的那个滚动方向。
                    if !delta.x.is_zero() && !delta.y.is_zero() {
                        if delta.x.abs() > delta.y.abs() {
                            delta.y = px(0.);
                        } else {
                            delta.x = px(0.);
                        }
                    }

                    if !is_horizontal {
                        // 当前偏移也必须被钳制：冒泡事件之后，滚动元素自身的监听器
                        // 会把共享偏移推到边缘之外未钳制（div 只在预绘制时钳制），
                        // 这个瞬时过度滚动会被误读为"有滚动空间"。
                        let axis_max = scroll_handle.max_offset().y.max(px(0.));
                        let current = offset.y.clamp(-axis_max, px(0.));
                        let new_offset = (current + delta.y).clamp(-axis_max, px(0.));
                        if new_offset == current {
                            // 在边缘或没有溢出：冒泡给父级。
                            return;
                        }

                        offset.y = new_offset;
                        scroll_handle.set_offset(offset);
                        cx.notify(view_id);
                        cx.stop_propagation();
                        return;
                    }

                    offset.x += delta.x;

                    // 注意：`set_offset` 不钳制（钳制发生在 div 的预绘制中），
                    // 因此任何非零的水平主导增量都能通过此守卫——即使在滚动边缘，
                    // 事件也会被消费而不是变成父级滚动。
                    if offset != scroll_handle.offset() {
                        scroll_handle.set_offset(offset);
                        cx.notify(view_id);
                        cx.stop_propagation();
                    }
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Context, IntoElement, ListAlignment, ListState, Render, ScrollDelta, ScrollWheelEvent,
        TestAppContext, VisualTestContext, Window, div, list, point, px,
    };

    struct HorizontalScrollAreaTest {
        scroll_handle: ScrollHandle,
    }

    impl Render for HorizontalScrollAreaTest {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().w(px(100.)).h(px(40.)).child(horizontal_scroll_area(
                "horizontal-scroll-area",
                &self.scroll_handle,
                &Default::default(),
                div().w(px(300.)).h(px(40.)),
            ))
        }
    }

    #[crate::test]
    fn horizontal_scroll_area_ignores_vertical_wheel(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let scroll_handle = ScrollHandle::new();
        let (_, cx) = cx.add_window_view({
            let scroll_handle = scroll_handle.clone();
            move |_, _| HorizontalScrollAreaTest {
                scroll_handle: scroll_handle.clone(),
            }
        });
        let cx: &mut VisualTestContext = cx;
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });

        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-40.))),
            ..Default::default()
        });

        assert_eq!(scroll_handle.offset().x, px(0.));
    }

    /// 复现 markdown 表格场景：滚动区域位于 `rgpui::list` 项内。
    /// list 在项绘制后才注册滚轮监听器，因此在冒泡阶段（反向注册顺序）
    /// list 先运行并消费每次触控板滑动的 `delta.y`。
    struct ListWithHorizontalAreaTest {
        scroll_handle: ScrollHandle,
        list_state: ListState,
        occluded: bool,
    }

    impl Render for ListWithHorizontalAreaTest {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let scroll_handle = self.scroll_handle.clone();
            let mut root = div().w(px(100.)).h(px(100.)).child(
                list(self.list_state.clone(), move |ix, _, _| {
                    if ix == 0 {
                        horizontal_scroll_area(
                            "horizontal-scroll-area",
                            &scroll_handle,
                            &Default::default(),
                            div().w(px(300.)).h(px(40.)),
                        )
                        .into_any_element()
                    } else {
                        div().w(px(100.)).h(px(40.)).into_any_element()
                    }
                })
                .w_full()
                .h_full(),
            );
            if self.occluded {
                // 列表上方的覆盖层，如打开的对话框或菜单。
                root = root.child(div().absolute().top_0().left_0().size_full().occlude());
            }
            root
        }
    }

    fn setup_list_test<'a>(
        cx: &'a mut TestAppContext,
        scroll_handle: &ScrollHandle,
        list_state: &ListState,
        occluded: bool,
    ) -> &'a mut VisualTestContext {
        cx.update(crate::theme::init);
        let (_, cx) = cx.add_window_view({
            let scroll_handle = scroll_handle.clone();
            let list_state = list_state.clone();
            move |_, _| ListWithHorizontalAreaTest {
                scroll_handle: scroll_handle.clone(),
                list_state: list_state.clone(),
                occluded,
            }
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
        cx
    }

    #[crate::test]
    fn horizontal_scroll_area_in_list_keeps_horizontal_dominant_wheel(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let scroll_handle = ScrollHandle::new();
        let list_state = ListState::new(10, ListAlignment::Top, px(0.));
        let cx = setup_list_test(cx, &scroll_handle, &list_state, false);

        // 触控板滑动很少是纯轴向的：水平主导带一点垂直分量。
        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(-40.), px(-10.))),
            ..Default::default()
        });

        // 区域消费水平增量...
        assert_eq!(scroll_handle.offset().x, px(-40.));
        // ...外部 list 必须不垂直滚动。
        let scroll_top = list_state.logical_scroll_top();
        assert_eq!((scroll_top.item_ix, scroll_top.offset_in_item), (0, px(0.)));
    }

    #[crate::test]
    fn horizontal_scroll_area_in_list_bubbles_vertical_dominant_wheel(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let scroll_handle = ScrollHandle::new();
        let list_state = ListState::new(10, ListAlignment::Top, px(0.));
        let cx = setup_list_test(cx, &scroll_handle, &list_state, false);

        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(-10.), px(-40.))),
            ..Default::default()
        });

        // 垂直主导：list 滚动，区域不滚动。
        assert_eq!(scroll_handle.offset().x, px(0.));
        let scroll_top = list_state.logical_scroll_top();
        assert_ne!((scroll_top.item_ix, scroll_top.offset_in_item), (0, px(0.)));
    }

    #[crate::test]
    fn horizontal_scroll_area_covers_viewport_after_scrolled(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let scroll_handle = ScrollHandle::new();
        let list_state = ListState::new(10, ListAlignment::Top, px(0.));
        let cx = setup_list_test(cx, &scroll_handle, &list_state, false);

        // 滚动区域到中间，然后重绘。
        scroll_handle.set_offset(point(px(-150.), px(0.)));
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });

        // 在视口右侧滑动。遮罩必须仍然覆盖它——不能随滚动内容一起滑离。
        cx.simulate_event(ScrollWheelEvent {
            position: point(px(90.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(-40.), px(-10.))),
            ..Default::default()
        });

        assert_eq!(scroll_handle.offset().x, px(-190.));
        let scroll_top = list_state.logical_scroll_top();
        assert_eq!((scroll_top.item_ix, scroll_top.offset_in_item), (0, px(0.)));
    }

    #[crate::test]
    fn horizontal_scroll_area_traps_wheel_at_edge(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let scroll_handle = ScrollHandle::new();
        let list_state = ListState::new(10, ListAlignment::Top, px(0.));
        let cx = setup_list_test(cx, &scroll_handle, &list_state, false);

        // 滚动区域到右边缘（300 - 100 = 200）。
        scroll_handle.set_offset(point(px(-200.), px(0.)));
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });

        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(-40.), px(-10.))),
            ..Default::default()
        });

        // 水平遮罩即使在边缘也消费事件：被冒泡的水平增量会被轴映射到垂直 list 上（#2468）。
        let scroll_top = list_state.logical_scroll_top();
        assert_eq!((scroll_top.item_ix, scroll_top.offset_in_item), (0, px(0.)));
    }

    #[crate::test]
    fn horizontal_scroll_area_ignores_wheel_when_occluded(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let scroll_handle = ScrollHandle::new();
        let list_state = ListState::new(10, ListAlignment::Top, px(0.));
        let cx = setup_list_test(cx, &scroll_handle, &list_state, true);

        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(-40.), px(-10.))),
            ..Default::default()
        });

        // 覆盖层（对话框、上下文菜单）遮挡区域：区域必须不在其下滚动。
        assert_eq!(scroll_handle.offset().x, px(0.));
    }

    #[crate::test]
    fn horizontal_scroll_area_uses_horizontal_wheel(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let scroll_handle = ScrollHandle::new();
        let (_, cx) = cx.add_window_view({
            let scroll_handle = scroll_handle.clone();
            move |_, _| HorizontalScrollAreaTest {
                scroll_handle: scroll_handle.clone(),
            }
        });
        let cx: &mut VisualTestContext = cx;
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });

        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(-40.), px(0.))),
            ..Default::default()
        });

        assert_eq!(scroll_handle.offset().x, px(-40.));
    }

    fn setup_horizontal_area_test<'a>(
        cx: &'a mut TestAppContext,
        scroll_handle: &ScrollHandle,
    ) -> &'a mut VisualTestContext {
        cx.update(crate::theme::init);
        let (_, cx) = cx.add_window_view({
            let scroll_handle = scroll_handle.clone();
            move |_, _| HorizontalScrollAreaTest {
                scroll_handle: scroll_handle.clone(),
            }
        });
        let cx: &mut VisualTestContext = cx;
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
        cx
    }

    #[crate::test]
    fn horizontal_mask_keeps_axis_lock_within_a_gesture(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let scroll_handle = ScrollHandle::new();
        let cx = setup_horizontal_area_test(cx, &scroll_handle);

        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(-40.), px(-10.))),
            ..Default::default()
        });
        assert_eq!(scroll_handle.offset().x, px(-40.));

        // 同一手势，现在偏向垂直但在解锁比例内：锁定保持，水平偏移继续移动。
        // 单独比较该事件会清零 `delta.x` 并把滚动器停在 -40。
        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(-10.), px(-15.))),
            ..Default::default()
        });
        assert_eq!(scroll_handle.offset().x, px(-50.));
    }

    #[crate::test]
    fn horizontal_mask_releases_axis_lock_on_a_strong_turn(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let scroll_handle = ScrollHandle::new();
        let cx = setup_horizontal_area_test(cx, &scroll_handle);

        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(-40.), px(-10.))),
            ..Default::default()
        });
        assert_eq!(scroll_handle.offset().x, px(-40.));

        // 超过解锁比例后手势不再是水平，事件停止驱动该滚动器。
        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(-10.), px(-25.))),
            ..Default::default()
        });
        assert_eq!(scroll_handle.offset().x, px(-40.));
    }

    /// 复现 DataTable 场景：嵌套在外部垂直滚动器内的垂直可滚动元素。
    struct NestedVerticalScrollTest {
        outer_handle: ScrollHandle,
        inner_handle: ScrollHandle,
        inner_content_height: Pixels,
    }

    impl Render for NestedVerticalScrollTest {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .id("outer")
                .w(px(100.))
                .h(px(100.))
                .overflow_y_scroll()
                .track_scroll(&self.outer_handle)
                .child(
                    div()
                        .relative()
                        .w_full()
                        .h(px(60.))
                        .child(
                            div()
                                .id("inner")
                                .size_full()
                                .overflow_y_scroll()
                                .track_scroll(&self.inner_handle)
                                .child(div().w_full().h(self.inner_content_height)),
                        )
                        .child(ScrollableMask::new(Axis::Vertical, &self.inner_handle)),
                )
                .child(div().w_full().h(px(400.)))
        }
    }

    fn setup_nested_vertical_test<'a>(
        cx: &'a mut TestAppContext,
        outer_handle: &ScrollHandle,
        inner_handle: &ScrollHandle,
        inner_content_height: Pixels,
    ) -> &'a mut VisualTestContext {
        cx.update(crate::theme::init);
        let (_, cx) = cx.add_window_view({
            let outer_handle = outer_handle.clone();
            let inner_handle = inner_handle.clone();
            move |_, _| NestedVerticalScrollTest {
                outer_handle: outer_handle.clone(),
                inner_handle: inner_handle.clone(),
                inner_content_height,
            }
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
        cx
    }

    #[crate::test]
    fn vertical_mask_consumes_wheel_when_scrollable(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let outer_handle = ScrollHandle::new();
        let inner_handle = ScrollHandle::new();
        let cx = setup_nested_vertical_test(cx, &outer_handle, &inner_handle, px(300.));

        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-40.))),
            ..Default::default()
        });

        // 内部滚动器消费事件；外部滚动器必须不动。
        assert_eq!(inner_handle.offset().y, px(-40.));
        assert_eq!(outer_handle.offset().y, px(0.));
    }

    #[crate::test]
    fn vertical_mask_hands_off_to_parent_at_edge(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let outer_handle = ScrollHandle::new();
        let inner_handle = ScrollHandle::new();
        let cx = setup_nested_vertical_test(cx, &outer_handle, &inner_handle, px(300.));

        // 内部元素滚动到底边缘（300 - 60 = 240）。
        inner_handle.set_offset(point(px(0.), px(-240.)));
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });

        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-40.))),
            ..Default::default()
        });

        // 在边缘事件冒泡：外部滚动器接管。
        assert_eq!(outer_handle.offset().y, px(-40.));
        // 内部偏移在下一次预绘制时被钳制回边缘。
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
        assert_eq!(inner_handle.offset().y, px(-240.));
    }

    #[crate::test]
    fn vertical_mask_bubbles_when_no_overflow(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let outer_handle = ScrollHandle::new();
        let inner_handle = ScrollHandle::new();
        // 内部内容 (40) 适配其 60px 视口：没有可滚动的内容。
        let cx = setup_nested_vertical_test(cx, &outer_handle, &inner_handle, px(40.));

        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-40.))),
            ..Default::default()
        });

        assert_eq!(outer_handle.offset().y, px(-40.));
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
        assert_eq!(inner_handle.offset().y, px(0.));
    }

    #[crate::test]
    fn vertical_mask_ignores_transient_overscroll(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let outer_handle = ScrollHandle::new();
        let inner_handle = ScrollHandle::new();
        let cx = setup_nested_vertical_test(cx, &outer_handle, &inner_handle, px(300.));

        inner_handle.set_offset(point(px(0.), px(-240.)));
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });

        // 边缘连续两个滚轮事件且中间没有重绘：第一个事件把内部偏移推到
        // 边缘之外未钳制，这不能被误读为"有滚动空间"而吞掉第二个事件。
        for _ in 0..2 {
            cx.simulate_event(ScrollWheelEvent {
                position: point(px(10.), px(10.)),
                delta: ScrollDelta::Pixels(point(px(0.), px(-40.))),
                ..Default::default()
            });
        }

        assert_eq!(outer_handle.offset().y, px(-80.));
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
        assert_eq!(inner_handle.offset().y, px(-240.));
    }

    /// 垂直遮罩嵌套在 `rgpui::list` 祖先中：list 在其项绘制后注册滚轮监听器，
    /// 因此只有捕获阶段的遮罩能阻止它消费同一事件。
    struct ListWithVerticalAreaTest {
        scroll_handle: ScrollHandle,
        list_state: ListState,
    }

    impl Render for ListWithVerticalAreaTest {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let scroll_handle = self.scroll_handle.clone();
            div().w(px(100.)).h(px(100.)).child(
                list(self.list_state.clone(), move |ix, _, _| {
                    if ix == 0 {
                        div()
                            .relative()
                            .w_full()
                            .h(px(60.))
                            .child(
                                div()
                                    .id("inner")
                                    .size_full()
                                    .overflow_y_scroll()
                                    .track_scroll(&scroll_handle)
                                    .child(div().w_full().h(px(300.))),
                            )
                            .child(ScrollableMask::new(Axis::Vertical, &scroll_handle))
                            .into_any_element()
                    } else {
                        div().w(px(100.)).h(px(40.)).into_any_element()
                    }
                })
                .w_full()
                .h_full(),
            )
        }
    }

    #[crate::test]
    fn vertical_mask_in_list_consumes_wheel_when_scrollable(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let scroll_handle = ScrollHandle::new();
        let list_state = ListState::new(10, ListAlignment::Top, px(0.));
        let (_, cx) = cx.add_window_view({
            let scroll_handle = scroll_handle.clone();
            let list_state = list_state.clone();
            move |_, _| ListWithVerticalAreaTest {
                scroll_handle: scroll_handle.clone(),
                list_state: list_state.clone(),
            }
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });

        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-40.))),
            ..Default::default()
        });

        // 内部滚动器消费事件；list 必须不滚动。
        assert_eq!(scroll_handle.offset().y, px(-40.));
        let scroll_top = list_state.logical_scroll_top();
        assert_eq!((scroll_top.item_ix, scroll_top.offset_in_item), (0, px(0.)));
    }
}
