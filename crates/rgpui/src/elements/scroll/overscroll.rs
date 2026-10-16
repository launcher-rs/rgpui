//! 越界回弹（overscroll）：滚到边缘后继续滚/拖，内容跟手拉伸，松手回弹。
//!
//! 设计（借 gpui-kit `scroll_bounce` 思路，不搬体量）：逻辑位保持钳制
//! （`Div` 每 prepaint 硬钳制，`scrollable_mask` 的瞬时超界断言也不动），
//! 只做视觉位移——包一层带 `relative` 偏移的 `div`。物理直接用现有的
//! [`ScrollPhysics`](crate::scroll_physics::ScrollPhysics)（每轴一个，边界
//! `[0, 0]`，位置即视觉位移），该模块至此终于有了生产调用点。
//!
//! v1 约束：固定 60fps 步长回弹（与帧率无关的精确弹簧以后再说）；触摸与滚轮
//! 走同一条 `ScrollWheelEvent` 路径（触摸以带 `touch_phase` 的滚轮事件呈现），
//! 不做手势方向锁与动量抑制；`reduce_motion` 开启时跳过拉伸（装饰性位移）。

use std::{cell::RefCell, panic::Location, rc::Rc};

use crate::scroll_physics::ScrollPhysics;
use crate::{
    App, ElementId, EntityId, InteractiveElement, IntoElement, ParentElement, Pixels, Point,
    RenderOnce, ScrollHandle, Styled, TouchPhase, Window, div, point, px,
};

use super::ScrollbarAxis;

/// 单轴最大视觉拉伸（橡胶带钳制上限，超出部分直接丢掉）。
const MAX_PULL: f32 = 96.0;
/// 拉伸阻力：滚轮/触摸增量进入视觉位移的比例。
const PULL_RESISTANCE: f32 = 0.35;
/// 回弹步进（固定步长，见模块头 v1 约束）。
const SETTLE_DT: f32 = 1.0 / 60.0;

/// 越界回弹状态（`Rc<RefCell>`：滚轮监听里只有 `&mut App`，用内部可变性；
/// 模板抄 `scrollbar.rs` 的 `ScrollbarState`）。
#[derive(Debug, Clone, Default)]
struct OverscrollState(Rc<RefCell<OverscrollInner>>);

#[derive(Debug)]
struct OverscrollInner {
    /// 每轴一个物理模型（边界 `[0, 0]`，位置即该轴视觉位移）。
    x: ScrollPhysics,
    /// 每轴一个物理模型（边界 `[0, 0]`，位置即该轴视觉位移）。
    y: ScrollPhysics,
    /// 上次滚轮事件后内层逻辑偏移（钳制口径；没动即到边，见 [`OverscrollState::pull`]）。
    last_offset: Point<Pixels>,
}

impl Default for OverscrollInner {
    fn default() -> Self {
        Self {
            x: ScrollPhysics::new().with_bounds(0.0, 0.0),
            y: ScrollPhysics::new().with_bounds(0.0, 0.0),
            last_offset: point(px(0.), px(0.)),
        }
    }
}

impl OverscrollState {
    /// 当前视觉位移（两轴物理位置拼成点）。
    fn visual(&self) -> Point<Pixels> {
        let inner = self.0.borrow();
        point(px(inner.x.position()), px(inner.y.position()))
    }

    /// 是否还有残留位移或速度（是即 render 里推进一帧并请求下一帧）。
    fn settling(&self) -> bool {
        let inner = self.0.borrow();
        inner.x.is_overscrolled()
            || inner.x.is_moving()
            || inner.y.is_overscrolled()
            || inner.y.is_moving()
    }

    /// 清空位移与速度（`reduce_motion` / `Cancelled` 时瞬时归位，不要动画）。
    fn clear(&self) {
        let mut inner = self.0.borrow_mut();
        inner.x.set_position(0.0);
        inner.x.stop();
        inner.y.set_position(0.0);
        inner.y.stop();
    }

    /// 推进一帧回弹。
    fn settle(&self) {
        let mut inner = self.0.borrow_mut();
        inner.x.tick(SETTLE_DT);
        inner.y.tick(SETTLE_DT);
    }

    /// 处理一次滚轮/触摸增量，返回视觉位移是否有变化（有才需重绘）。
    ///
    /// 本监听挂在冒泡阶段，内层滚动区先消费：逻辑偏移没动说明到边（或内容
    /// 未溢出），且增量指向界外时才拉伸；逻辑动了说明滚回去了，视觉清零。
    fn pull(&self, delta: Point<Pixels>, handle: &ScrollHandle, axis: ScrollbarAxis) -> bool {
        let mut inner = self.0.borrow_mut();
        let offset = handle.offset();
        let max = handle.max_offset();
        // 内层消费与钳制之间有一帧时间差：事件时刻的偏移可能是瞬时超界值
        // （下次 prepaint 才钳回），到边判定必须看钳制后的逻辑位（与绘制同口径）。
        let clamped = point(
            offset.x.clamp(-max.x, px(0.)),
            offset.y.clamp(-max.y, px(0.)),
        );
        let moved = clamped != inner.last_offset;
        inner.last_offset = clamped;

        let mut changed = false;
        if axis.has_horizontal() {
            changed |= Self::pull_axis(&mut inner.x, delta.x, clamped.x, max.x, moved);
        }
        if axis.has_vertical() {
            changed |= Self::pull_axis(&mut inner.y, delta.y, clamped.y, max.y, moved);
        }
        changed
    }

    /// 单轴拉伸：内容未溢出（`max <= 0`）或增量未指向界外时不动。
    fn pull_axis(
        physics: &mut ScrollPhysics,
        delta: Pixels,
        offset: Pixels,
        max: Pixels,
        moved: bool,
    ) -> bool {
        if moved {
            // 逻辑滚回去了：视觉清零（有残留才算变化）。
            if physics.position() != 0.0 {
                physics.set_position(0.0);
                physics.stop();
                return true;
            }
            return false;
        }
        if max <= px(0.) {
            return false;
        }
        // 偏移约定：`offset` 落在 `[-max, 0]`；顶部界外为正增量，底部为负。
        let outward = (offset >= px(0.) && delta > px(0.)) || (offset <= -max && delta < px(0.));
        if !outward {
            return false;
        }
        physics.apply_delta(delta.0 * PULL_RESISTANCE);
        physics.set_position(physics.position().clamp(-MAX_PULL, MAX_PULL));
        true
    }
}

/// 越界回弹包装器：把滚动区包一层，到边后内容跟手拉伸、松手回弹。
///
/// 主入口是 [`Scrollable::overscroll`](super::Scrollable::overscroll)；
/// 裸 `track_scroll` 的 `div` 也可直接包。包装根固定 `size_full`（滚动区
/// 语义），不要塞进自适应高度的祖先里。
#[derive(IntoElement)]
pub struct Overscroll<E: IntoElement + 'static> {
    id: ElementId,
    child: E,
    handle: ScrollHandle,
    axis: ScrollbarAxis,
}

impl<E: IntoElement + 'static> Overscroll<E> {
    /// 创建回弹包装（`handle` 为内层滚动区的跟踪句柄，用于到边判定）。
    #[track_caller]
    pub fn new(child: E, handle: &ScrollHandle, axis: impl Into<ScrollbarAxis>) -> Self {
        Self {
            id: ElementId::CodeLocation(*Location::caller()),
            child,
            handle: handle.clone(),
            axis: axis.into(),
        }
    }
}

impl<E> RenderOnce for Overscroll<E>
where
    E: IntoElement + 'static,
{
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window
            .use_keyed_state(self.id.clone(), cx, |_, _| OverscrollState::default())
            .read(cx)
            .clone();
        let view: EntityId = window.current_view();

        // 回弹推进：有残留即收敛一帧并请求下一帧；收敛完自动停。
        if state.settling() {
            state.settle();
            window.request_animation_frame();
        }
        let visual = state.visual();

        let handle = self.handle.clone();
        let axis = self.axis;
        div()
            .id(self.id)
            .size_full()
            .relative()
            .left(visual.x)
            .top(visual.y)
            .on_scroll_wheel(move |event, window, cx| {
                if event.touch_phase == TouchPhase::Cancelled {
                    state.clear();
                    cx.notify(view);
                    return;
                }
                if cx.reduce_motion() {
                    state.clear();
                    return;
                }
                let delta = event.delta.pixel_delta(window.line_height());
                if state.pull(delta, &handle, axis) {
                    cx.notify(view);
                }
            })
            .child(self.child)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::scroll::scrollable::ScrollableElement;
    use crate::{
        Context, Div, Render, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext,
        point, px,
    };

    /// 强制重绘一帧（`scrollable.rs` 测试同款）。
    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    }

    /// 推进 `n` 帧（含下一帧回调的交付，即 `request_animation_frame` 链）。
    fn pump_frames(cx: &mut VisualTestContext, n: usize) {
        for _ in 0..n {
            cx.update(|window, cx| {
                window.simulate_next_frame(cx);
            });
            draw(cx);
        }
    }

    /// 在给定位置发送滚轮事件并重绘。
    fn scroll(cx: &mut VisualTestContext, x: f32, y: f32, dx: f32, dy: f32) {
        cx.simulate_event(ScrollWheelEvent {
            position: point(px(x), px(y)),
            delta: ScrollDelta::Pixels(point(px(dx), px(dy))),
            ..Default::default()
        });
        draw(cx);
    }

    /// 固定高度的内容行（`flex_shrink_0`，可滚动）。
    fn row(selector: &'static str, height: f32) -> Div {
        div()
            .h(px(height))
            .flex_shrink_0()
            .debug_selector(move || selector.to_string())
    }

    /// 200x100 视口 + 150 内容（可滚 50px）的回弹测试视图。
    struct BounceTest;
    impl Render for BounceTest {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().w(px(200.)).h(px(100.)).child(
                crate::v_flex()
                    .size_full()
                    .overflow_y_scrollbar()
                    .overscroll(true)
                    .child(row("bounce-first", 50.))
                    .child(row("bounce-second", 50.))
                    .child(row("bounce-third", 50.)),
            )
        }
    }

    /// 顶部继续下拉：逻辑位不动（仍为 0），内容视觉下移；多帧后回弹归位。
    #[crate::test]
    fn overscroll_pulls_and_settles(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let (_, cx) = cx.add_window_view(|_, _| BounceTest);
        let cx: &mut VisualTestContext = cx;
        draw(cx);

        let initial_y = cx.debug_bounds("bounce-first").unwrap().origin.y;
        // 顶部 outward（正增量）：拉伸。
        scroll(cx, 10., 10., 0., 50.);
        let pulled_y = cx.debug_bounds("bounce-first").unwrap().origin.y;
        assert!(
            pulled_y > initial_y,
            "到边后继续滚，内容应视觉下移（{pulled_y:?} > {initial_y:?}）"
        );
        assert!(
            pulled_y - initial_y <= px(MAX_PULL + 1.0),
            "拉伸应钳制在上限内（{pulled_y:?} - {initial_y:?}）"
        );

        // 无新事件时逐帧回弹，最终归位。
        pump_frames(cx, 300);
        let settled_y = cx.debug_bounds("bounce-first").unwrap().origin.y;
        assert!(
            (settled_y - initial_y).abs() < px(1.0),
            "回弹后应归位（{settled_y:?} ≈ {initial_y:?}）"
        );
    }

    /// 未开启时行为不变：到边滚轮不产生视觉位移。
    #[crate::test]
    fn overscroll_disabled_by_default(cx: &mut TestAppContext) {
        struct PlainTest;
        impl Render for PlainTest {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().w(px(200.)).h(px(100.)).child(
                    crate::v_flex()
                        .size_full()
                        .overflow_y_scrollbar()
                        .child(row("plain-first", 50.))
                        .child(row("plain-second", 50.))
                        .child(row("plain-third", 50.)),
                )
            }
        }

        cx.update(crate::theme::init);
        let (_, cx) = cx.add_window_view(|_, _| PlainTest);
        let cx: &mut VisualTestContext = cx;
        draw(cx);

        let initial_y = cx.debug_bounds("plain-first").unwrap().origin.y;
        scroll(cx, 10., 10., 0., 50.);
        let after_y = cx.debug_bounds("plain-first").unwrap().origin.y;
        assert_eq!(after_y, initial_y, "默认关闭时到边滚轮无视觉位移");
    }

    /// 正常滚动不受影响：界内滚轮照常走逻辑位，无视觉残留。
    #[crate::test]
    fn overscroll_keeps_normal_scroll(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);
        let (_, cx) = cx.add_window_view(|_, _| BounceTest);
        let cx: &mut VisualTestContext = cx;
        draw(cx);

        let initial_y = cx.debug_bounds("bounce-first").unwrap().origin.y;
        // 顶部 inward（负增量）：逻辑消费，内容上移。
        scroll(cx, 10., 10., 0., -30.);
        let scrolled_y = cx.debug_bounds("bounce-first").unwrap().origin.y;
        assert!(
            scrolled_y < initial_y,
            "界内滚轮应正常滚动（{scrolled_y:?} < {initial_y:?}）"
        );
        pump_frames(cx, 60);
        // 逻辑位移保持（回弹只收视觉位移，不碰逻辑位）。
        let kept_y = cx.debug_bounds("bounce-first").unwrap().origin.y;
        assert!(
            (kept_y - scrolled_y).abs() < px(1.0),
            "正常滚动不应被回弹吃掉（{kept_y:?} ≈ {scrolled_y:?}）"
        );
    }
}
