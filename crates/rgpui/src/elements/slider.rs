use std::ops::Range;

use crate::AppContext as _;
use crate::AxisExt as _;
use crate::ElementExt as _;
use crate::StyledExt as _;
use crate::prelude::FluentBuilder as _;
use crate::{
    ActiveTheme, Along, App, Axis, Background, Bounds, Corners, DefiniteLength, DragMoveEvent,
    Empty, Entity, EntityId, EventEmitter, InteractiveElement, IntoElement, IsZero, MouseButton,
    MouseDownEvent, ParentElement as _, Pixels, Point, Render, RenderOnce,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div, h_flex, px, relative,
};

/// 拖动缩略图的标识。
#[derive(Clone)]
struct DragThumb((EntityId, bool));

impl Render for DragThumb {
    fn render(&mut self, _: &mut Window, _: &mut crate::Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// 拖动滑块的标识。
#[derive(Clone)]
struct DragSlider(EntityId);

impl Render for DragSlider {
    fn render(&mut self, _: &mut Window, _: &mut crate::Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// [`SliderState`] 发出的事件。
pub enum SliderEvent {
    /// 用户在拖动或点击改变滑块值时连续发出。
    Change(SliderValue),
    /// 用户拖动或点击后释放滑块时发出一次。
    Release(SliderValue),
}

/// 滑块的值，可以是单个值或值范围。
///
/// - 可从 f32 转换，视为单个值。
/// - 可从 (f32, f32) 元组转换，视为值范围。
///
/// 默认值为 `SliderValue::Single(0.0)`。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SliderValue {
    /// 单个值
    Single(f32),
    /// 值范围
    Range(f32, f32),
}

impl std::fmt::Display for SliderValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SliderValue::Single(value) => write!(f, "{}", value),
            SliderValue::Range(start, end) => write!(f, "{}..{}", start, end),
        }
    }
}

impl From<f32> for SliderValue {
    fn from(value: f32) -> Self {
        SliderValue::Single(value)
    }
}

impl From<(f32, f32)> for SliderValue {
    fn from(value: (f32, f32)) -> Self {
        SliderValue::Range(value.0, value.1)
    }
}

impl From<Range<f32>> for SliderValue {
    fn from(value: Range<f32>) -> Self {
        SliderValue::Range(value.start, value.end)
    }
}

impl Default for SliderValue {
    fn default() -> Self {
        SliderValue::Single(0.)
    }
}

impl SliderValue {
    /// 将值限制到给定范围内。
    pub fn clamp(self, min: f32, max: f32) -> Self {
        match self {
            SliderValue::Single(value) => SliderValue::Single(value.clamp(min, max)),
            SliderValue::Range(start, end) => {
                SliderValue::Range(start.clamp(min, max), end.clamp(min, max))
            }
        }
    }

    /// 是否为单个值。
    #[inline]
    pub fn is_single(&self) -> bool {
        matches!(self, SliderValue::Single(_))
    }

    /// 是否为值范围。
    #[inline]
    pub fn is_range(&self) -> bool {
        matches!(self, SliderValue::Range(_, _))
    }

    /// 获取起始值。
    pub fn start(&self) -> f32 {
        match self {
            SliderValue::Single(value) => *value,
            SliderValue::Range(start, _) => *start,
        }
    }

    /// 获取结束值。
    pub fn end(&self) -> f32 {
        match self {
            SliderValue::Single(value) => *value,
            SliderValue::Range(_, end) => *end,
        }
    }

    fn set_start(&mut self, value: f32) {
        if let SliderValue::Range(_, end) = self {
            *self = SliderValue::Range(value.min(*end), *end);
        } else {
            *self = SliderValue::Single(value);
        }
    }

    fn set_end(&mut self, value: f32) {
        if let SliderValue::Range(start, _) = self {
            *self = SliderValue::Range(*start, value.max(*start));
        } else {
            *self = SliderValue::Single(value);
        }
    }
}

/// 滑块的刻度模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SliderScale {
    /// 线性刻度，值在滑块范围内均匀变化。这是默认模式。
    #[default]
    Linear,
    /// 对数刻度，值之间的距离呈指数增长。
    ///
    /// 适用于值范围较大、低值处变化更显著的参数。常见示例：
    ///
    /// - 音量控制（人耳听觉是对数感知）
    /// - 频率控制（音符遵循对数刻度）
    /// - 缩放级别
    /// - 任何需要在低值处进行更精细控制的参数
    ///
    /// # 例如
    ///
    /// ```
    /// use rgpui::slider::{SliderState, SliderScale};
    ///
    /// let slider = SliderState::new()
    ///     .min(1.0)    // 对数刻度必须大于 0
    ///     .max(1000.0)
    ///     .scale(SliderScale::Logarithmic);
    /// ```
    Logarithmic,
}

impl SliderScale {
    /// 是否为线性刻度。
    #[inline]
    pub fn is_linear(&self) -> bool {
        matches!(self, SliderScale::Linear)
    }

    /// 是否为对数刻度。
    #[inline]
    pub fn is_logarithmic(&self) -> bool {
        matches!(self, SliderScale::Logarithmic)
    }
}

/// [`Slider`] 的状态。
pub struct SliderState {
    /// 最小值
    min: f32,
    /// 最大值
    max: f32,
    /// 步长
    step: f32,
    /// 当前值
    value: SliderValue,
    /// 当为单个值模式时，只使用 `end`，start 始终为 0.0。
    percentage: Range<f32>,
    /// 渲染后滑块的边界。
    bounds: Bounds<Pixels>,
    /// 刻度模式
    scale: SliderScale,
    /// 记录用户是否正在与滑块交互，以便仅在真实按下/拖动后发出 Release 事件。
    dragging: bool,
}

impl SliderState {
    /// 创建新的 [`SliderState`]。
    pub fn new() -> Self {
        Self {
            min: 0.0,
            max: 100.0,
            step: 1.0,
            value: SliderValue::default(),
            percentage: (0.0..0.0),
            bounds: Bounds::default(),
            scale: SliderScale::default(),
            dragging: false,
        }
    }

    /// 设置滑块的最小值，默认：0.0
    pub fn min(mut self, min: f32) -> Self {
        if self.scale.is_logarithmic() {
            assert!(
                min > 0.0,
                "`min` must be greater than 0 for SliderScale::Logarithmic"
            );
            assert!(
                min < self.max,
                "`min` must be less than `max` for Logarithmic scale"
            );
        }
        self.min = min;
        self.update_thumb_pos();
        self
    }

    /// 设置滑块的最大值，默认：100.0
    pub fn max(mut self, max: f32) -> Self {
        if self.scale.is_logarithmic() {
            assert!(
                max > self.min,
                "`max` must be greater than `min` for Logarithmic scale"
            );
        }
        self.max = max;
        self.update_thumb_pos();
        self
    }

    /// 设置滑块的步长，默认：1.0
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// 设置滑块的刻度模式，默认：[`SliderScale::Linear`]。
    pub fn scale(mut self, scale: SliderScale) -> Self {
        if scale.is_logarithmic() {
            assert!(
                self.min > 0.0,
                "`min` must be greater than 0 for Logarithmic scale"
            );
            assert!(
                self.max > self.min,
                "`max` must be greater than `min` for Logarithmic scale"
            );
        }
        self.scale = scale;
        self.update_thumb_pos();
        self
    }

    /// 设置滑块的默认值，默认：0.0
    pub fn default_value(mut self, value: impl Into<SliderValue>) -> Self {
        self.value = value.into();
        self.update_thumb_pos();
        self
    }

    /// 设置滑块的值。
    pub fn set_value(
        &mut self,
        value: impl Into<SliderValue>,
        _: &mut Window,
        cx: &mut crate::Context<Self>,
    ) {
        self.value = value.into();
        self.update_thumb_pos();
        cx.notify();
    }

    /// 获取滑块的值。
    pub fn value(&self) -> SliderValue {
        self.value
    }

    /// 将 0.0 到 1.0 之间的值转换为最小值和最大值之间的值，取决于刻度模式。
    fn percentage_to_value(&self, percentage: f32) -> f32 {
        match self.scale {
            SliderScale::Linear => self.min + (self.max - self.min) * percentage,
            SliderScale::Logarithmic => {
                let base = self.max / self.min;
                (base.powf(percentage) * self.min).clamp(self.min, self.max)
            }
        }
    }

    /// 将最小值和最大值之间的值转换为 0.0 到 1.0 之间的值，取决于刻度模式。
    fn value_to_percentage(&self, value: f32) -> f32 {
        match self.scale {
            SliderScale::Linear => {
                let range = self.max - self.min;
                if range <= 0.0 {
                    0.0
                } else {
                    (value - self.min) / range
                }
            }
            SliderScale::Logarithmic => {
                let base = self.max / self.min;
                (value / self.min).log(base).clamp(0.0, 1.0)
            }
        }
    }

    fn update_thumb_pos(&mut self) {
        match self.value {
            SliderValue::Single(value) => {
                let percentage = self.value_to_percentage(value.clamp(self.min, self.max));
                self.percentage = 0.0..percentage;
            }
            SliderValue::Range(start, end) => {
                let clamped_start = start.clamp(self.min, self.max);
                let clamped_end = end.clamp(self.min, self.max);
                self.percentage =
                    self.value_to_percentage(clamped_start)..self.value_to_percentage(clamped_end);
            }
        }
    }

    /// 根据鼠标位置更新值。
    fn update_value_by_position(
        &mut self,
        axis: Axis,
        position: Point<Pixels>,
        is_start: bool,
        _: &mut Window,
        cx: &mut crate::Context<Self>,
    ) {
        self.dragging = true;
        let bounds = self.bounds;
        let step = self.step;

        let inner_pos = if axis.is_horizontal() {
            position.x - bounds.left()
        } else {
            bounds.bottom() - position.y
        };
        let total_size = bounds.size.along(axis);
        let percentage = inner_pos.clamp(px(0.), total_size) / total_size;

        let percentage = if is_start {
            percentage.clamp(0.0, self.percentage.end)
        } else {
            percentage.clamp(self.percentage.start, 1.0)
        };

        let value = self.percentage_to_value(percentage);
        let value = (value / step).round() * step;

        if is_start {
            self.percentage.start = percentage;
            self.value.set_start(value);
        } else {
            self.percentage.end = percentage;
            self.value.set_end(value);
        }
        cx.emit(SliderEvent::Change(self.value));
        cx.notify();
    }

    /// 如果用户正在与滑块交互则发出 [`SliderEvent::Release`]。在滑块内外鼠标抬起时调用。
    fn handle_release(&mut self, cx: &mut crate::Context<Self>) {
        if !self.dragging {
            return;
        }
        self.dragging = false;
        cx.emit(SliderEvent::Release(self.value));
    }
}

impl EventEmitter<SliderEvent> for SliderState {}

/// 滑块（Slider）元素。
#[derive(IntoElement)]
pub struct Slider {
    /// 状态实体
    state: Entity<SliderState>,
    /// 布局方向
    axis: Axis,
    /// 样式精炼
    style: StyleRefinement,
    /// 是否禁用
    disabled: bool,
}

impl Slider {
    /// 创建绑定到 [`SliderState`] 的新 [`Slider`] 元素。
    pub fn new(state: &Entity<SliderState>) -> Self {
        Self {
            axis: Axis::Horizontal,
            state: state.clone(),
            style: StyleRefinement::default(),
            disabled: false,
        }
    }

    /// 作为水平滑块。
    pub fn horizontal(mut self) -> Self {
        self.axis = Axis::Horizontal;
        self
    }

    /// 作为垂直滑块。
    pub fn vertical(mut self) -> Self {
        self.axis = Axis::Vertical;
        self
    }

    /// 设置滑块的禁用状态，默认：false
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    #[allow(clippy::too_many_arguments)]
    fn render_thumb(
        &self,
        start: DefiniteLength,
        is_start: bool,
        bar_color: Background,
        thumb_bg: Background,
        radius: Corners<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl crate::IntoElement {
        let entity_id = self.state.entity_id();
        let axis = self.axis;
        let id = ("slider-thumb", is_start as u32);

        if self.disabled {
            return div().id(id);
        }

        div()
            .id(id)
            .absolute()
            .when(axis.is_horizontal(), |this| {
                this.top(px(-5.)).left(start).ml(-px(8.))
            })
            .when(axis.is_vertical(), |this| {
                this.bottom(start).left(px(-5.)).mb(-px(8.))
            })
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .corner_radii(radius)
            .bg(bar_color.opacity(0.5))
            .when(cx.theme().shadow, |this| this.shadow_md())
            .size_4()
            .p(px(1.))
            .child(
                div()
                    .flex_shrink_0()
                    .size_full()
                    .corner_radii(radius)
                    .bg(thumb_bg),
            )
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .on_drag(DragThumb((entity_id, is_start)), |drag, _, _, cx| {
                cx.stop_propagation();
                cx.new(|_| drag.clone())
            })
            .on_drag_move(window.listener_for(
                &self.state,
                move |view, e: &DragMoveEvent<DragThumb>, window, cx| {
                    match e.drag(cx) {
                        DragThumb((id, is_start)) => {
                            if *id != entity_id {
                                return;
                            }

                            // 根据鼠标位置设置值
                            view.update_value_by_position(
                                axis,
                                e.event.position,
                                *is_start,
                                window,
                                cx,
                            )
                        }
                    }
                },
            ))
    }
}

impl Styled for Slider {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let axis = self.axis;
        let entity_id = self.state.entity_id();
        let state = self.state.read(cx);
        let is_range = state.value().is_range();
        let percentage = state.percentage.clone();
        let bar_start = relative(percentage.start);
        let bar_end = relative(1. - percentage.end);
        let rem_size = window.rem_size();

        let bar_color = self
            .style
            .background
            .clone()
            .and_then(|bg| bg.color())
            .unwrap_or(cx.theme().tokens.slider_bar.into());
        let thumb_bg: Background = self
            .style
            .text
            .color
            .map(Into::into)
            .unwrap_or_else(|| cx.theme().tokens.slider_thumb.into());
        let corner_radii = self.style.corner_radii.clone();
        let default_radius = px(999.);
        let mut radius = Corners {
            top_left: corner_radii
                .top_left
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or(default_radius),
            top_right: corner_radii
                .top_right
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or(default_radius),
            bottom_left: corner_radii
                .bottom_left
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or(default_radius),
            bottom_right: corner_radii
                .bottom_right
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or(default_radius),
        };
        if cx.theme().radius.is_zero() {
            radius.top_left = px(0.);
            radius.top_right = px(0.);
            radius.bottom_left = px(0.);
            radius.bottom_right = px(0.);
        }

        div()
            .id(("slider", self.state.entity_id()))
            .flex()
            .flex_1()
            .items_center()
            .justify_center()
            .when(axis.is_vertical(), |this| this.h(px(120.)))
            .when(axis.is_horizontal(), |this| this.w_full())
            .refine_style(&self.style)
            .bg(cx.theme().transparent)
            .text_color(cx.theme().foreground)
            .when(!self.disabled, |this| {
                this.on_mouse_up(
                    MouseButton::Left,
                    window.listener_for(&self.state, |state, _, _, cx| {
                        state.handle_release(cx);
                    }),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    window.listener_for(&self.state, |state, _, _, cx| {
                        state.handle_release(cx);
                    }),
                )
            })
            .child(
                h_flex()
                    .id("slider-bar-container")
                    .when(!self.disabled, |this| {
                        this.on_mouse_down(
                            MouseButton::Left,
                            window.listener_for(
                                &self.state,
                                move |state, e: &MouseDownEvent, window, cx| {
                                    let mut is_start = false;
                                    if is_range {
                                        let bar_size = state.bounds.size.along(axis);
                                        let inner_pos = if axis.is_horizontal() {
                                            e.position.x - state.bounds.left()
                                        } else {
                                            state.bounds.bottom() - e.position.y
                                        };
                                        let center = ((percentage.end - percentage.start) / 2.0
                                            + percentage.start)
                                            * bar_size;
                                        is_start = inner_pos < center;
                                    }

                                    state.update_value_by_position(
                                        axis, e.position, is_start, window, cx,
                                    )
                                },
                            ),
                        )
                    })
                    .when(!self.disabled && !is_range, |this| {
                        this.on_drag(DragSlider(entity_id), |drag, _, _, cx| {
                            cx.stop_propagation();
                            cx.new(|_| drag.clone())
                        })
                        .on_drag_move(window.listener_for(
                            &self.state,
                            move |view, e: &DragMoveEvent<DragSlider>, window, cx| match e.drag(cx)
                            {
                                DragSlider(id) => {
                                    if *id != entity_id {
                                        return;
                                    }

                                    view.update_value_by_position(
                                        axis,
                                        e.event.position,
                                        false,
                                        window,
                                        cx,
                                    )
                                }
                            },
                        ))
                    })
                    .when(axis.is_horizontal(), |this| {
                        this.items_center().h_6().w_full()
                    })
                    .when(axis.is_vertical(), |this| {
                        this.justify_center().w_6().h_full()
                    })
                    .flex_shrink_0()
                    .child(
                        div()
                            .id("slider-bar")
                            .relative()
                            .when(axis.is_horizontal(), |this| this.w_full().h_1p5())
                            .when(axis.is_vertical(), |this| this.h_full().w_1p5())
                            .bg(bar_color.opacity(0.2))
                            .active(|this| this.bg(bar_color.opacity(0.4)))
                            .corner_radii(radius)
                            .child(
                                div()
                                    .absolute()
                                    .when(axis.is_horizontal(), |this| {
                                        this.h_full().left(bar_start).right(bar_end)
                                    })
                                    .when(axis.is_vertical(), |this| {
                                        this.w_full().bottom(bar_start).top(bar_end)
                                    })
                                    .bg(bar_color)
                                    .when(!cx.theme().radius.is_zero(), |this| this.rounded_full()),
                            )
                            .when(is_range, |this| {
                                this.child(self.render_thumb(
                                    relative(percentage.start),
                                    true,
                                    bar_color,
                                    thumb_bg,
                                    radius,
                                    window,
                                    cx,
                                ))
                            })
                            .child(self.render_thumb(
                                relative(percentage.end),
                                false,
                                bar_color,
                                thumb_bg,
                                radius,
                                window,
                                cx,
                            ))
                            .on_prepaint({
                                let state = self.state.clone();
                                move |bounds, _, cx| state.update(cx, |r, _| r.bounds = bounds)
                            }),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 SliderValue 转换
    #[test]
    fn test_slider_value_from() {
        assert_eq!(SliderValue::from(5.0), SliderValue::Single(5.0));
        assert_eq!(SliderValue::from((1.0, 3.0)), SliderValue::Range(1.0, 3.0));
        assert_eq!(SliderValue::from(2.0..4.0), SliderValue::Range(2.0, 4.0));
    }

    /// 测试 SliderValue clamp
    #[test]
    fn test_slider_value_clamp() {
        let v = SliderValue::Single(50.0).clamp(0.0, 10.0);
        assert_eq!(v, SliderValue::Single(10.0));
        let r = SliderValue::Range(5.0, 20.0).clamp(0.0, 10.0);
        assert_eq!(r, SliderValue::Range(5.0, 10.0));
    }

    /// 测试 SliderState 默认值与构造
    #[test]
    fn test_slider_state() {
        let state = SliderState::new().min(0.0).max(10.0).default_value(5.0);
        assert_eq!(state.value(), SliderValue::Single(5.0));
        assert_eq!(state.min, 0.0);
        assert_eq!(state.max, 10.0);
    }
}
