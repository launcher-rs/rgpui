//! 手势检测器：将鼠标按下/移动/抬起序列识别为轻点、双击、长按、拖动（pan）与滑动（swipe）。

use crate::*;
use web_time::{Duration, Instant};

/// 滑动方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwipeDirection {
    /// 向左滑动。
    Left,
    /// 向右滑动。
    Right,
    /// 向上滑动。
    Up,
    /// 向下滑动。
    Down,
}

/// 滑动手势信息。
#[derive(Clone, Debug)]
pub struct SwipeGesture {
    /// 滑动方向。
    pub direction: SwipeDirection,
    /// 滑动速度。
    pub velocity: f32,
    /// 滑动距离。
    pub distance: f32,
}

/// 长按手势信息。
#[derive(Clone, Debug)]
pub struct LongPressGesture {
    /// 按压位置。
    pub position: Point<Pixels>,
    /// 按压时长。
    pub duration: Duration,
}

/// 轻点手势信息。
#[derive(Clone, Debug)]
pub struct TapGesture {
    /// 点击位置。
    pub position: Point<Pixels>,
    /// 连击次数。
    pub count: u32,
}

/// 拖动（pan）手势信息。
#[derive(Clone, Debug)]
pub struct PanGesture {
    /// 与上次位置相比的位移。
    pub delta: Point<Pixels>,
    /// 当前速度。
    pub velocity: Point<Pixels>,
    /// 自按下起的累计位移。
    pub total_distance: Point<Pixels>,
}

/// 手势事件。
#[derive(Clone, Debug)]
pub enum GestureEvent {
    /// 滑动。
    Swipe(SwipeGesture),
    /// 长按。
    LongPress(LongPressGesture),
    /// 轻点（含连击次数）。
    Tap(TapGesture),
    /// 拖动开始。
    PanStart(Point<Pixels>),
    /// 拖动更新。
    PanUpdate(PanGesture),
    /// 拖动结束。
    PanEnd(PanGesture),
}

/// 触发滑动所需的最小距离（像素）。
const SWIPE_MIN_DISTANCE: f32 = 50.0;
/// 触发滑动所需的最小速度。
const SWIPE_MIN_VELOCITY: f32 = 200.0;
/// 判定为长按的持续时间。
const LONG_PRESS_DURATION: Duration = Duration::from_millis(500);
/// 双击判定的最大间隔。
const DOUBLE_TAP_INTERVAL: Duration = Duration::from_millis(300);
/// 判定为拖动的最小位移阈值（像素）。
const PAN_THRESHOLD: f32 = 5.0;

/// 手势检测器状态机。
#[derive(Clone, Debug)]
pub struct GestureDetector {
    /// 按下起点与时间。
    press_start: Option<(Point<Pixels>, Instant)>,
    /// 上次鼠标位置。
    last_position: Option<Point<Pixels>>,
    /// 当前速度。
    velocity: Point<Pixels>,
    /// 自按下起的累计位移。
    total_delta: Point<Pixels>,
    /// 是否正在拖动。
    is_panning: bool,
    /// 连击计数。
    tap_count: u32,
    /// 上次轻点时间。
    last_tap_time: Option<Instant>,
    /// 上次轻点位置。
    last_tap_position: Option<Point<Pixels>>,
    /// 长按是否已触发。
    long_press_triggered: bool,
    /// 长按持续时间阈值。
    long_press_duration: Duration,
    /// 滑动最小距离阈值。
    swipe_min_distance: f32,
}

impl Default for GestureDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl GestureDetector {
    /// 创建手势检测器。
    pub fn new() -> Self {
        Self {
            press_start: None,
            last_position: None,
            velocity: point(px(0.0), px(0.0)),
            total_delta: point(px(0.0), px(0.0)),
            is_panning: false,
            tap_count: 0,
            last_tap_time: None,
            last_tap_position: None,
            long_press_triggered: false,
            long_press_duration: LONG_PRESS_DURATION,
            swipe_min_distance: SWIPE_MIN_DISTANCE,
        }
    }

    /// 设置长按持续时间阈值。
    pub fn with_long_press_duration(mut self, duration: Duration) -> Self {
        self.long_press_duration = duration;
        self
    }

    /// 设置滑动最小距离阈值。
    pub fn with_swipe_distance(mut self, distance: f32) -> Self {
        self.swipe_min_distance = distance;
        self
    }

    /// 处理鼠标按下，返回产生的手势事件。
    pub fn on_mouse_down(&mut self, position: Point<Pixels>) -> Vec<GestureEvent> {
        self.press_start = Some((position, Instant::now()));
        self.last_position = Some(position);
        self.velocity = point(px(0.0), px(0.0));
        self.total_delta = point(px(0.0), px(0.0));
        self.is_panning = false;
        self.long_press_triggered = false;
        Vec::new()
    }

    /// 处理鼠标移动，识别拖动开始与拖动更新。
    pub fn on_mouse_move(&mut self, position: Point<Pixels>) -> Vec<GestureEvent> {
        let mut events = Vec::new();

        let Some(last) = self.last_position else {
            return events;
        };
        let Some((start, _)) = self.press_start else {
            return events;
        };

        let delta = point(position.x - last.x, position.y - last.y);
        self.total_delta = point(position.x - start.x, position.y - start.y);

        self.velocity = point(delta.x * 60.0, delta.y * 60.0);

        let total_distance =
            (f32::from(self.total_delta.x).powi(2) + f32::from(self.total_delta.y).powi(2)).sqrt();

        if !self.is_panning && total_distance > PAN_THRESHOLD {
            self.is_panning = true;
            events.push(GestureEvent::PanStart(start));
        }

        if self.is_panning {
            events.push(GestureEvent::PanUpdate(PanGesture {
                delta,
                velocity: self.velocity,
                total_distance: self.total_delta,
            }));
        }

        self.last_position = Some(position);
        events
    }

    /// 处理鼠标抬起，识别拖动结束、滑动、长按或轻点（含双击）。
    pub fn on_mouse_up(&mut self, position: Point<Pixels>) -> Vec<GestureEvent> {
        let mut events = Vec::new();

        let Some((start, start_time)) = self.press_start.take() else {
            return events;
        };

        let delta = point(position.x - start.x, position.y - start.y);
        let distance = (f32::from(delta.x).powi(2) + f32::from(delta.y).powi(2)).sqrt();

        if self.is_panning {
            events.push(GestureEvent::PanEnd(PanGesture {
                delta: point(
                    position.x - self.last_position.unwrap_or(start).x,
                    position.y - self.last_position.unwrap_or(start).y,
                ),
                velocity: self.velocity,
                total_distance: delta,
            }));

            let vel_x = f32::from(self.velocity.x).abs();
            let vel_y = f32::from(self.velocity.y).abs();
            let max_vel = vel_x.max(vel_y);

            if distance >= self.swipe_min_distance && max_vel >= SWIPE_MIN_VELOCITY {
                let dx = f32::from(delta.x);
                let dy = f32::from(delta.y);
                let direction = if dx.abs() > dy.abs() {
                    if dx > 0.0 {
                        SwipeDirection::Right
                    } else {
                        SwipeDirection::Left
                    }
                } else if dy > 0.0 {
                    SwipeDirection::Down
                } else {
                    SwipeDirection::Up
                };

                events.push(GestureEvent::Swipe(SwipeGesture {
                    direction,
                    velocity: max_vel,
                    distance,
                }));
            }
        } else {
            let elapsed = start_time.elapsed();

            if elapsed >= self.long_press_duration && !self.long_press_triggered {
                events.push(GestureEvent::LongPress(LongPressGesture {
                    position,
                    duration: elapsed,
                }));
            } else {
                let is_double = self
                    .last_tap_time
                    .map(|t| t.elapsed() < DOUBLE_TAP_INTERVAL)
                    .unwrap_or(false);

                if is_double {
                    self.tap_count += 1;
                } else {
                    self.tap_count = 1;
                }

                self.last_tap_time = Some(Instant::now());
                self.last_tap_position = Some(position);

                events.push(GestureEvent::Tap(TapGesture {
                    position,
                    count: self.tap_count,
                }));
            }
        }

        self.last_position = None;
        self.is_panning = false;
        events
    }

    /// 主动检查长按是否已满足（用于定时器驱动），满足则返回长按事件。
    pub fn check_long_press(&mut self) -> Option<GestureEvent> {
        if self.long_press_triggered || self.is_panning {
            return None;
        }

        let (position, start_time) = self.press_start?;

        if start_time.elapsed() >= self.long_press_duration {
            self.long_press_triggered = true;
            Some(GestureEvent::LongPress(LongPressGesture {
                position,
                duration: start_time.elapsed(),
            }))
        } else {
            None
        }
    }

    /// 是否当前处于按下状态。
    pub fn is_pressed(&self) -> bool {
        self.press_start.is_some()
    }

    /// 是否正在拖动。
    pub fn is_panning(&self) -> bool {
        self.is_panning
    }

    /// 重置手势状态。
    pub fn reset(&mut self) {
        self.press_start = None;
        self.last_position = None;
        self.is_panning = false;
        self.long_press_triggered = false;
    }
}
