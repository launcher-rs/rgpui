//! 触控手势识别词表。
//!
//! RGPUI 在 rgpui 核心的单一可移植竞技场中识别来自原始
//! [`TouchEvent`](crate::TouchEvent) 的手势：识别器竞争进行中的触摸，
//! 获胜者认领，失败者被取消。识别出的手势尽可能通过*已有*的语义事件呈现，
//! 点击变为 [`ClickEvent::Touch`](crate::ClickEvent)，平移变为携带
//! [`TouchPhase`](crate::TouchPhase) 的 [`ScrollWheelEvent`](crate::ScrollWheelEvent)，
//! 捏合变为 [`PinchEvent`](crate::PinchEvent)，
//! 因此针对 `on_click` 和滚动容器编写的组件在移动端无需修改即可工作。

use std::time::Duration;
use web_time::Instant;

use crate::{Axis, IsZero as _, Pixels, Point, TouchPhase, px};

/// 跟踪一次滚动手势中各事件的主导方向轴。
///
/// 手势由可用的触摸阶段分隔，对于只发出 [`TouchPhase::Moved`] 的平台，
/// 使用超时作为回退方案。
#[derive(Clone, Copy, Debug, Default)]
pub struct OngoingScroll {
    last_event: Option<Instant>,
    axis: Option<Axis>,
}

const SCROLL_EVENT_SEPARATION: Duration = Duration::from_millis(28);

impl OngoingScroll {
    /// 将给定的增量过滤为当前滚动手势的主导方向轴。
    pub fn filter(&mut self, delta: &mut Point<Pixels>, touch_phase: TouchPhase) {
        self.filter_at(delta, touch_phase, Instant::now())
    }

    fn filter_at(&mut self, delta: &mut Point<Pixels>, touch_phase: TouchPhase, now: Instant) {
        const UNLOCK_PERCENT: f32 = 1.9;
        const UNLOCK_LOWER_BOUND: Pixels = px(6.);

        if matches!(touch_phase, TouchPhase::Ended | TouchPhase::Cancelled) {
            self.last_event = None;
            self.axis = None;
            return;
        }

        let x = delta.x.abs();
        let y = delta.y.abs();
        if x.is_zero() && y.is_zero() {
            if touch_phase == TouchPhase::Started {
                self.last_event = None;
                self.axis = None;
            }
            return;
        }

        let starts_new_gesture = touch_phase == TouchPhase::Started
            || self
                .last_event
                .is_none_or(|last_event| now.duration_since(last_event) >= SCROLL_EVENT_SEPARATION);
        let mut axis = self.axis;
        if starts_new_gesture {
            axis = if x <= y {
                Some(Axis::Vertical)
            } else {
                Some(Axis::Horizontal)
            };
        } else if x.max(y) >= UNLOCK_LOWER_BOUND {
            match axis {
                Some(Axis::Vertical) if x > y && x >= y * UNLOCK_PERCENT => {
                    axis = None;
                }
                Some(Axis::Horizontal) if y > x && y >= x * UNLOCK_PERCENT => {
                    axis = None;
                }
                _ => {}
            }
        }

        self.last_event = Some(now);
        self.axis = axis;
        match axis {
            Some(Axis::Vertical) => delta.x = Pixels::ZERO,
            Some(Axis::Horizontal) => delta.y = Pixels::ZERO,
            None => {}
        }
    }
}

/// 手势识别器使用的体验常量。尽力提供，取决于各平台的支持，
/// 默认使用 RGPUI 自身（iOS 风格）的值
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GestureTuning {
    /// 触摸在不再是潜在点击并变为平移/拖动之前可以移动的距离。
    pub touch_slop: Pixels,
    /// 点击之间累积点击计数的最大间隔。
    pub multi_tap_interval: Duration,
    /// 点击之间累积点击计数的最大距离。
    pub multi_tap_slop: Pixels,
    /// 触摸必须保持在 [`Self::touch_slop`] 范围内多久才会被识别为长按。
    pub long_press_duration: Duration,
    /// 在快速滑动后应用于滚动动量的每毫秒衰减因子。
    /// （`UIScrollView` 使用每毫秒 `0.998` 作为其正常减速率。）
    pub momentum_decay_per_ms: f32,
    /// 启动滚动动量所需的最小释放速度（像素/秒）。
    pub min_fling_velocity: f32,
}

impl Default for GestureTuning {
    fn default() -> Self {
        Self {
            touch_slop: px(8.),
            multi_tap_interval: Duration::from_millis(400),
            multi_tap_slop: px(16.),
            long_press_duration: Duration::from_millis(500),
            momentum_decay_per_ms: 0.998,
            min_fling_velocity: 50.,
        }
    }
}

/// 参与识别的手势类型集合。
///
/// 由 [`PlatformGestures::native_recognizers`] 使用，用于声明平台原生识别哪些手势，
/// 而不是留给 rgpui 核心的可移植识别器处理。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GestureKinds {
    /// 点击（和多次点击），通过 [`ClickEvent::Touch`](crate::ClickEvent) 呈现。
    pub tap: bool,
    /// 长按，通过 [`LongPressEvent`] 呈现。
    pub long_press: bool,
    /// 平移/滚动（包括快速滑动动量），通过
    /// [`ScrollWheelEvent`](crate::ScrollWheelEvent) 呈现。
    pub pan: bool,
    /// 捏合缩放，通过 [`PinchEvent`](crate::PinchEvent) 呈现。
    pub pinch: bool,
}

impl GestureKinds {
    /// 无手势；rgpui 核心的可移植识别器处理所有手势。
    pub const NONE: Self = Self {
        tap: false,
        long_press: false,
        pan: false,
        pinch: false,
    };

    /// 所有手势类型。
    pub const ALL: Self = Self {
        tap: true,
        long_press: true,
        pan: true,
        pinch: true,
    };
}

/// 长按手势，移动端的右键菜单触发器。
///
/// 裸长按通过 [`ClickEvent`](crate::ClickEvent) 以 `long_press: true` 呈现，
/// 与右键点击一起传递给辅助点击监听器。此事件是需要手势本身的元素的原始钩子
/// （例如长按开始拖动）；注册 API 与手势竞技场一起提供。
#[derive(Clone, Debug, Default)]
pub struct LongPressEvent {
    /// 被识别为长按的触摸位置。
    pub position: Point<Pixels>,
}

/// 平台手势识别服务。
///
/// 如果你的移动平台支持原生手势识别，使用此 trait 将其与 RGPUI 共享。
pub trait PlatformGestures {
    /// 此平台上可移植识别器的体验常量。
    fn tuning(&self) -> GestureTuning {
        GestureTuning::default()
    }

    /// 此平台原生识别的手势类型。
    fn native_recognizers(&self) -> GestureKinds {
        GestureKinds::NONE
    }
}

/// 一个空操作的 [`PlatformGestures`] 实现：无原生识别器，使用默认调优。
/// 适用于桌面平台和测试。
pub struct NullPlatformGestures;

impl PlatformGestures for NullPlatformGestures {}
