//! 动画时间线 —— 多轨关键帧编辑器（标尺 + 轨道 + 关键帧 + 播放头）。
//!
//! 面向动效 / 音视频 / 关键帧动画的**时间线编辑**场景：时间标尺、多条轨道、每条
//! 轨道若干关键帧、可拖拽的播放头、按插值方式绘制的值曲线。缩略的纯数据模型
//! （[`Timeline`] / [`Track`] / [`Keyframe`]）负责关键帧存储与求值（线性 / 保持 /
//! 平滑），控件（[`TimelineState`] / [`TimelineView`]）负责标尺刻度、命中测试、
//! 拖拽（水平改时间、垂直改值）与播放头。
//!
//! 轨道可直接转成运行时的进度式动画：`Track::to_keyframe_animation` 把「绝对秒 +
//! 关键帧」映射成 [`KeyframeAnimation`](crate::animation::KeyframeAnimation)，编辑完
//! 即可交给元素动画播放。
//!
//! ## 用法
//!
//! ```ignore
//! use rgpui::anim_timeline::{Timeline, TimelineState, TimelineView, Track, Keyframe};
//!
//! let timeline = Timeline::from_tracks(vec![
//!     Track::with_keyframes("位置", vec![Keyframe::new(0.0, 0.0), Keyframe::new(2.0, 200.0)]),
//!     Track::with_keyframes("透明度", vec![Keyframe::new(0.0, 0.0), Keyframe::new(1.0, 1.0)]),
//! ]);
//! let state = cx.new(|cx| TimelineState::new(timeline, cx));
//!
//! // 视图树中：
//! TimelineView::new(&state)
//! ```

use lyon::path::{LineCap, LineJoin};

use crate::{
    ActiveTheme as _, AnyElement, App, Bounds, Context, ElementExt as _, Entity, EventEmitter,
    FocusHandle, Focusable, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Path, PathBuilder, PathStyle,
    Pixels, Point, Render, RenderOnce, ScrollDelta, ScrollWheelEvent, SharedString,
    StatefulInteractiveElement as _, StrokeOptions, Styled as _, Window, canvas, div, point,
    prelude::FluentBuilder as _, px,
};

use crate::animation::KeyframeAnimation;

/// 标尺高度（像素）。
pub const RULER_HEIGHT: Pixels = px(24.0);
/// 每条轨道行高（像素）。
pub const TRACK_HEIGHT: Pixels = px(24.0);
/// 折叠后的轨道行高（像素）。
pub const COLLAPSED_TRACK_HEIGHT: Pixels = px(12.0);
/// 左侧轨道名列宽度（像素）。
pub const LABEL_WIDTH: Pixels = px(96.0);
/// 关键帧命中半径（像素）。
pub const KEYFRAME_HIT_RADIUS: Pixels = px(7.0);
/// 关键帧方块边长（像素）。
pub const KEYFRAME_SIZE: Pixels = px(8.0);
/// 默认缩放（每秒像素）。
pub const DEFAULT_PX_PER_SECOND: f32 = 80.0;
/// 最小缩放（每秒像素）。
pub const MIN_PX_PER_SECOND: f32 = 8.0;
/// 最大缩放（每秒像素）。
pub const MAX_PX_PER_SECOND: f32 = 800.0;
/// 默认吸附帧率。
pub const DEFAULT_FPS: f32 = 30.0;
/// 标尺刻度之间要求的最小像素距离。
pub const MIN_TICK_SPACING: f32 = 56.0;
/// 标尺刻度数量上限（防止极端缩放下渲染爆炸）。
pub const MAX_TICKS: usize = 200;
/// 值曲线在轨道行内的上下留白（像素）。
pub const VALUE_PADDING: f32 = 3.0;
/// 值曲线描边宽度（像素）。
pub const CURVE_STROKE_WIDTH: Pixels = px(1.5);
/// 值曲线采样间距（像素，越小越平滑、开销越大）。
pub const CURVE_SAMPLE_SPACING: f32 = 6.0;
/// 值曲线采样点数量上限（防止极度缩放下渲染爆炸）。
pub const MAX_CURVE_SAMPLES: usize = 512;
/// 值域留白比例：曲线上下各留最大值与最小值之差的 10%。
pub const VALUE_RANGE_PADDING: f32 = 0.1;

/// 单个关键帧：时间（秒）+ 值。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframe {
    /// 时间（秒，非负）。
    pub time: f32,
    /// 值。
    pub value: f32,
}

impl Keyframe {
    /// 创建关键帧（时间会被夹取为非负）。
    pub fn new(time: f32, value: f32) -> Self {
        Self {
            time: if time.is_finite() { time.max(0.0) } else { 0.0 },
            value,
        }
    }
}

/// 轨道的关键帧插值方式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Interpolation {
    /// 线性插值。
    #[default]
    Linear,
    /// 保持前值直到下一关键帧（阶跃）。
    Hold,
    /// 平滑插值（smoothstep）。
    Smooth,
    /// 缓入（二次缓入：起步慢、收尾快）。
    EaseIn,
    /// 缓出（二次缓出：起步快、收尾慢）。
    EaseOut,
    /// 缓入缓出（三次缓动：两端慢、中段快）。
    EaseInOut,
}

impl Interpolation {
    /// 把局部进度 `t`（0~1）映射为插值系数。
    pub fn ease(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Interpolation::Linear => t,
            Interpolation::Hold => 0.0,
            Interpolation::Smooth => t * t * (3.0 - 2.0 * t),
            Interpolation::EaseIn => t * t,
            Interpolation::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Interpolation::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
        }
    }

    /// 插值方式的中文名（UI 展示用）。
    pub fn label(self) -> &'static str {
        match self {
            Interpolation::Linear => "线性",
            Interpolation::Hold => "阶跃",
            Interpolation::Smooth => "平滑",
            Interpolation::EaseIn => "缓入",
            Interpolation::EaseOut => "缓出",
            Interpolation::EaseInOut => "缓入缓出",
        }
    }
}

/// 一条轨道：名称 + 关键帧（按时间升序、时间唯一）+ 插值方式 + 折叠状态。
#[derive(Clone, Debug)]
pub struct Track {
    /// 轨道名称。
    pub name: SharedString,
    /// 关键帧列表。
    pub keyframes: Vec<Keyframe>,
    /// 插值方式。
    pub interpolation: Interpolation,
    /// 是否折叠（行高缩为 [`COLLAPSED_TRACK_HEIGHT`]，不绘制关键帧）。
    pub collapsed: bool,
}

impl Track {
    /// 创建空轨道。
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            keyframes: Vec::new(),
            interpolation: Interpolation::Linear,
            collapsed: false,
        }
    }

    /// 用给定关键帧创建轨道（自动排序并合并同一时间的重复帧）。
    pub fn with_keyframes(name: impl Into<SharedString>, keyframes: Vec<Keyframe>) -> Self {
        let mut track = Self::new(name);
        track.keyframes = keyframes;
        track.normalize();
        track
    }

    /// 设置插值方式。
    pub fn with_interpolation(mut self, interpolation: Interpolation) -> Self {
        self.interpolation = interpolation;
        self
    }

    /// 设置初始折叠状态。
    pub fn with_collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    /// 设置折叠状态，返回是否变化。
    pub fn set_collapsed(&mut self, collapsed: bool) -> bool {
        if self.collapsed == collapsed {
            return false;
        }
        self.collapsed = collapsed;
        true
    }

    /// 当前行高：折叠时为 [`COLLAPSED_TRACK_HEIGHT`]，否则 [`TRACK_HEIGHT`]。
    pub fn row_height(&self) -> Pixels {
        if self.collapsed {
            COLLAPSED_TRACK_HEIGHT
        } else {
            TRACK_HEIGHT
        }
    }

    /// 关键帧数量。
    pub fn len(&self) -> usize {
        self.keyframes.len()
    }

    /// 是否没有关键帧。
    pub fn is_empty(&self) -> bool {
        self.keyframes.is_empty()
    }

    /// 按时间升序排序，并合并同一时间的重复帧（保留最后一个）。
    pub fn normalize(&mut self) {
        self.keyframes.retain(|kf| kf.time.is_finite());
        self.keyframes.sort_by(|a, b| {
            a.time
                .partial_cmp(&b.time)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut deduped: Vec<Keyframe> = Vec::with_capacity(self.keyframes.len());
        for kf in self.keyframes.drain(..) {
            match deduped.last_mut() {
                Some(last) if (last.time - kf.time).abs() < 1e-6 => *last = kf,
                _ => deduped.push(kf),
            }
        }
        self.keyframes = deduped;
    }

    /// 写入关键帧：同一时间覆盖，返回 `true` 表示新增（否则为覆盖）。
    pub fn set_keyframe(&mut self, time: f32, value: f32) -> bool {
        let keyframe = Keyframe::new(time, value);
        if let Some(slot) = self
            .keyframes
            .iter_mut()
            .find(|kf| (kf.time - keyframe.time).abs() < 1e-6)
        {
            *slot = keyframe;
            return false;
        }
        self.keyframes.push(keyframe);
        self.normalize();
        true
    }

    /// 删除下标处的关键帧。
    pub fn remove_keyframe(&mut self, index: usize) -> Option<Keyframe> {
        if index < self.keyframes.len() {
            Some(self.keyframes.remove(index))
        } else {
            None
        }
    }

    /// 命中 `time` 附近（`tolerance` 秒内）最近的关键帧下标。
    pub fn keyframe_index_at(&self, time: f32, tolerance: f32) -> Option<usize> {
        let tolerance = tolerance.max(0.0);
        let mut best: Option<(usize, f32)> = None;
        for (index, keyframe) in self.keyframes.iter().enumerate() {
            let distance = (keyframe.time - time).abs();
            if distance <= tolerance
                && best.is_none_or(|(_, best_distance)| distance < best_distance)
            {
                best = Some((index, distance));
            }
        }
        best.map(|(index, _)| index)
    }

    /// 把下标处的关键帧移动到新时间并重新排序，返回移动后的下标。
    pub fn move_keyframe(&mut self, index: usize, time: f32) -> Option<usize> {
        if index >= self.keyframes.len() {
            return None;
        }
        let mut keyframe = self.keyframes.remove(index);
        keyframe.time = if time.is_finite() { time.max(0.0) } else { 0.0 };
        let moved_time = keyframe.time;
        self.keyframes.push(keyframe);
        self.normalize();
        self.keyframes
            .iter()
            .position(|kf| (kf.time - moved_time).abs() < 1e-6)
    }

    /// 修改下标处关键帧的值（不改时间，因此不需要重新排序），返回是否变化。
    ///
    /// 非有限值（`NaN` / 无穷）被忽略，避免污染值域与曲线采样。
    pub fn set_keyframe_value(&mut self, index: usize, value: f32) -> bool {
        if !value.is_finite() {
            return false;
        }
        let Some(keyframe) = self.keyframes.get_mut(index) else {
            return false;
        };
        if keyframe.value == value {
            return false;
        }
        keyframe.value = value;
        true
    }

    /// 曲线绘制用的值域 `(最小值, 最大值)`，上下各留 [`VALUE_RANGE_PADDING`] 比例。
    ///
    /// 空轨道（或全是非有限值）退化为 `(0.0, 1.0)`；所有值相同时上下各留 `0.5`，
    /// 让水平曲线落在行中间而不是贴边。
    pub fn value_range(&self) -> (f32, f32) {
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for keyframe in &self.keyframes {
            if keyframe.value.is_finite() {
                min = min.min(keyframe.value);
                max = max.max(keyframe.value);
            }
        }
        if !min.is_finite() || !max.is_finite() {
            return (0.0, 1.0);
        }
        let span = max - min;
        let pad = if span > f32::EPSILON {
            span * VALUE_RANGE_PADDING
        } else {
            0.5
        };
        (min - pad, max + pad)
    }

    /// 轨道时长（最后一个关键帧的时间；空轨道为 0）。
    pub fn duration(&self) -> f32 {
        self.keyframes.last().map_or(0.0, |kf| kf.time)
    }

    /// 求 `time` 处的插值（首帧之前保持首值，末帧之后保持末值）。
    pub fn value_at(&self, time: f32) -> f32 {
        let Some(first) = self.keyframes.first().copied() else {
            return 0.0;
        };
        if !time.is_finite() || time <= first.time {
            return first.value;
        }
        let last = *self.keyframes.last().unwrap_or(&first);
        if time >= last.time {
            return last.value;
        }
        for pair in self.keyframes.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            if time <= to.time {
                let span = to.time - from.time;
                if span <= f32::EPSILON {
                    return to.value;
                }
                let t = (time - from.time) / span;
                return from.value + (to.value - from.value) * self.interpolation.ease(t);
            }
        }
        last.value
    }

    /// 转成进度式 [`KeyframeAnimation`]：`duration` 秒映射到进度 0~1。
    ///
    /// 端点值被显式写入，因此覆盖 [`KeyframeAnimation::new`] 自带的 `(0, 0)`/`(1, 1)`。
    pub fn to_keyframe_animation(&self, id: impl Into<String>, duration: f32) -> KeyframeAnimation {
        let duration = if duration.is_finite() && duration > 0.0 {
            duration
        } else {
            self.duration().max(f32::EPSILON)
        };
        let mut animation = KeyframeAnimation::new(id).at(0.0, self.value_at(0.0));
        for keyframe in &self.keyframes {
            animation = animation.at((keyframe.time / duration).clamp(0.0, 1.0), keyframe.value);
        }
        animation.at(1.0, self.value_at(duration))
    }
}

/// 多轨时间线数据模型。
#[derive(Clone, Debug, Default)]
pub struct Timeline {
    /// 轨道列表（顺序即显示顺序）。
    tracks: Vec<Track>,
}

impl Timeline {
    /// 创建空时间线。
    pub fn new() -> Self {
        Self::default()
    }

    /// 用给定轨道创建时间线。
    pub fn from_tracks(tracks: Vec<Track>) -> Self {
        Self { tracks }
    }

    /// 全部轨道。
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// 全部轨道（可变）。
    pub fn tracks_mut(&mut self) -> &mut Vec<Track> {
        &mut self.tracks
    }

    /// 下标处的轨道。
    pub fn track(&self, index: usize) -> Option<&Track> {
        self.tracks.get(index)
    }

    /// 下标处的轨道（可变）。
    pub fn track_mut(&mut self, index: usize) -> Option<&mut Track> {
        self.tracks.get_mut(index)
    }

    /// 追加一条轨道，返回其下标。
    pub fn add_track(&mut self, track: Track) -> usize {
        self.tracks.push(track);
        self.tracks.len() - 1
    }

    /// 删除下标处的轨道。
    pub fn remove_track(&mut self, index: usize) -> Option<Track> {
        if index < self.tracks.len() {
            Some(self.tracks.remove(index))
        } else {
            None
        }
    }

    /// 调整轨道顺序（`to` 为移除后的目标下标，越界自动夹取），返回是否移动。
    pub fn move_track(&mut self, from: usize, to: usize) -> bool {
        if from >= self.tracks.len() {
            return false;
        }
        let to = to.min(self.tracks.len() - 1);
        if from == to {
            return false;
        }
        let track = self.tracks.remove(from);
        self.tracks.insert(to, track);
        true
    }

    /// 轨道数量。
    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    /// 是否没有轨道。
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    /// 总时长（所有轨道时长的最大值）。
    pub fn duration(&self) -> f32 {
        self.tracks.iter().map(Track::duration).fold(0.0, f32::max)
    }

    /// 求 `time` 处所有轨道的值（顺序与轨道一致）。
    pub fn values_at(&self, time: f32) -> Vec<f32> {
        self.tracks
            .iter()
            .map(|track| track.value_at(time))
            .collect()
    }
}

/// 秒 → 内容区局部像素（`x = (time - scroll) * px_per_second`）。
pub fn time_to_x(time: f32, px_per_second: f32, scroll: f32) -> f32 {
    (time - scroll) * px_per_second
}

/// 内容区局部像素 → 秒。
pub fn x_to_time(x: f32, px_per_second: f32, scroll: f32) -> f32 {
    if px_per_second > 0.0 && px_per_second.is_finite() {
        scroll + x / px_per_second
    } else {
        scroll
    }
}

/// 吸附到帧网格（`fps <= 0` 时不吸附）。
pub fn snap_time(time: f32, fps: f32) -> f32 {
    if fps > 0.0 && fps.is_finite() {
        (time * fps).round() / fps
    } else {
        time
    }
}

/// 夹取每秒像素到 [`MIN_PX_PER_SECOND`]..=[`MAX_PX_PER_SECOND`]。
pub fn clamp_px_per_second(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(MIN_PX_PER_SECOND, MAX_PX_PER_SECOND)
    } else {
        DEFAULT_PX_PER_SECOND
    }
}

/// 依据每秒像素选一个「整齐」的标尺步长（秒），保证刻度间隔不小于 `min_px` 像素。
pub fn ruler_step(px_per_second: f32, min_px: f32) -> f32 {
    if px_per_second.is_nan() || px_per_second <= 0.0 || !px_per_second.is_finite() {
        return 1.0;
    }
    let raw = (min_px / px_per_second).max(1e-4);
    let magnitude = 10f32.powf(raw.log10().floor());
    for factor in [1.0, 2.0, 5.0, 10.0] {
        let step = factor * magnitude;
        if step >= raw {
            return step;
        }
    }
    10.0 * magnitude
}

/// 格式化时间标签：小于 60 秒用 `1.20s`，否则用 `1:02.5`。
pub fn format_time(time: f32) -> String {
    let time = if time.is_finite() { time.max(0.0) } else { 0.0 };
    if time < 60.0 {
        format!("{time:.2}s")
    } else {
        let minutes = (time / 60.0).floor();
        let seconds = time - minutes * 60.0;
        format!("{minutes:.0}:{seconds:04.1}")
    }
}

/// 值 → 轨道行内纵坐标（0 在顶部，`height` 在底部），超出值域时夹到两端。
pub fn value_to_y(value: f32, range: (f32, f32), height: Pixels) -> f32 {
    let height = if height.as_f32().is_finite() {
        height.as_f32().max(0.0)
    } else {
        0.0
    };
    let usable = (height - VALUE_PADDING * 2.0).max(0.0);
    let (min, max) = range;
    let span = max - min;
    let t = if span.is_finite() && span > f32::EPSILON && value.is_finite() {
        ((value - min) / span).clamp(0.0, 1.0)
    } else {
        0.5
    };
    VALUE_PADDING + (1.0 - t) * usable
}

/// 轨道行内纵坐标 → 值（[`value_to_y`] 的逆映射，越界夹到值域两端）。
pub fn y_to_value(y: f32, range: (f32, f32), height: Pixels) -> f32 {
    let height = if height.as_f32().is_finite() {
        height.as_f32().max(0.0)
    } else {
        0.0
    };
    let usable = (height - VALUE_PADDING * 2.0).max(0.0);
    let (min, max) = range;
    let t = if usable > f32::EPSILON && y.is_finite() {
        (1.0 - (y - VALUE_PADDING) / usable).clamp(0.0, 1.0)
    } else {
        0.5
    };
    min + (max - min) * t
}

/// 采样一条轨道的值曲线，返回轨道行内局部坐标（`x` 已含缩放与滚动）。
///
/// 逐段按 [`CURVE_SAMPLE_SPACING`] 采样，段尾补一个 `to.time` 处的点：`Hold`
/// 因此得到一条竖直跳变，而不是一段斜坡；采样总量受 [`MAX_CURVE_SAMPLES`] 限制。
pub fn curve_points(
    track: &Track,
    range: (f32, f32),
    px_per_second: f32,
    scroll: f32,
    height: Pixels,
) -> Vec<Point<Pixels>> {
    let Some(first) = track.keyframes.first().copied() else {
        return Vec::new();
    };
    if track.keyframes.len() < 2 || !px_per_second.is_finite() || px_per_second <= 0.0 {
        return vec![point(
            px(time_to_x(first.time, px_per_second, scroll)),
            px(value_to_y(first.value, range, height)),
        )];
    }
    // 每段的采样步数（按像素间距折算），再按总量上限等比缩减。
    let mut steps: Vec<usize> = track
        .keyframes
        .windows(2)
        .map(|pair| {
            let span = (pair[1].time - pair[0].time).max(0.0) * px_per_second;
            ((span / CURVE_SAMPLE_SPACING).ceil() as usize).clamp(1, MAX_CURVE_SAMPLES)
        })
        .collect();
    let total: usize = steps.iter().sum();
    if total > MAX_CURVE_SAMPLES {
        let factor = MAX_CURVE_SAMPLES as f32 / total as f32;
        for step in &mut steps {
            *step = ((*step as f32 * factor).ceil() as usize).max(1);
        }
    }
    let mut points = Vec::with_capacity(total + 1);
    for (pair, steps) in track.keyframes.windows(2).zip(steps) {
        let (from, to) = (pair[0], pair[1]);
        let span = to.time - from.time;
        for step in 0..steps {
            // 末个采样点正好落在段尾：`Hold` 在 `to.time` 处先保持前值，
            // 再由下面补的端点跳到新值，形成竖直跳变。
            let progress = if steps > 1 {
                step as f32 / (steps - 1) as f32
            } else {
                1.0
            };
            let time = from.time + span * progress;
            let value = from.value + (to.value - from.value) * track.interpolation.ease(progress);
            points.push(point(
                px(time_to_x(time, px_per_second, scroll)),
                px(value_to_y(value, range, height)),
            ));
        }
    }
    points.push(point(
        px(time_to_x(
            track.keyframes.last().map_or(first.time, |kf| kf.time),
            px_per_second,
            scroll,
        )),
        px(value_to_y(
            track.keyframes.last().map_or(first.value, |kf| kf.value),
            range,
            height,
        )),
    ));
    points
}

/// 把采样点连成一条圆头折线路径（点不足 2 个时返回 `None`）。
fn polyline_path(points: &[Point<Pixels>]) -> Option<Path<Pixels>> {
    if points.len() < 2 {
        return None;
    }
    let options = StrokeOptions::default()
        .with_line_width(CURVE_STROKE_WIDTH.0)
        .with_line_cap(LineCap::Round)
        .with_line_join(LineJoin::Round);
    let mut builder =
        PathBuilder::stroke(CURVE_STROKE_WIDTH).with_style(PathStyle::Stroke(options));
    builder.move_to(points[0]);
    for point in &points[1..] {
        builder.line_to(*point);
    }
    builder.build().ok()
}

/// 命中测试结果（坐标为时间线局部像素）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimelineHit {
    /// 标尺区域（拖拽播放头）。
    Ruler,
    /// 轨道行内的某个关键帧。
    Keyframe {
        /// 轨道下标。
        track: usize,
        /// 关键帧下标。
        index: usize,
    },
    /// 轨道行的空白处。
    Track {
        /// 轨道下标。
        track: usize,
    },
    /// 标签列或行外空白。
    None,
}

/// 时间线抛出的事件（应用经 `subscribe` 接收）。
#[derive(Clone, Debug, PartialEq)]
pub enum TimelineEvent {
    /// 播放头时间变化（拖拽标尺 / 键盘 / 应用设置）。
    TimeChanged(f32),
    /// 选中变化（轨道 / 关键帧）。
    SelectionChanged {
        /// 选中的轨道。
        track: Option<usize>,
        /// 选中的关键帧。
        keyframe: Option<usize>,
    },
    /// 关键帧被拖动（时间与值都可能变化；`index` 为移动后的下标）。
    KeyframeMoved {
        /// 轨道下标。
        track: usize,
        /// 移动后的关键帧下标。
        index: usize,
        /// 新时间（秒，已吸附）。
        time: f32,
        /// 新值（纵向拖拽后的值）。
        value: f32,
    },
    /// 请求在（轨道, 时间）处新增关键帧（双击空白处）。
    KeyframeRequested {
        /// 轨道下标。
        track: usize,
        /// 请求时间（秒，已吸附）。
        time: f32,
    },
    /// 关键帧被删除。
    KeyframeRemoved {
        /// 轨道下标。
        track: usize,
        /// 被删除的关键帧下标。
        index: usize,
    },
    /// 播放 / 暂停状态切换。
    PlaybackChanged(bool),
    /// 轨道顺序变化（`to` 为移动后的下标）。
    TrackMoved {
        /// 原下标。
        from: usize,
        /// 移动后的下标。
        to: usize,
    },
    /// 轨道折叠 / 展开。
    TrackCollapsed {
        /// 轨道下标。
        index: usize,
        /// 是否折叠。
        collapsed: bool,
    },
    /// 轨道插值方式变化。
    TrackInterpolation {
        /// 轨道下标。
        index: usize,
        /// 新的插值方式。
        interpolation: Interpolation,
    },
}

/// 时间线控件状态：数据模型 + 播放头 + 缩放 / 滚动 / 选择。
pub struct TimelineState {
    /// 数据模型。
    timeline: Timeline,
    /// 播放头时间（秒）。
    time: f32,
    /// 缩放（每秒像素）。
    px_per_second: f32,
    /// 水平滚动偏移（秒）。
    scroll: f32,
    /// 垂直滚动偏移（像素，轨道区）。
    vscroll: f32,
    /// 吸附帧率（`<= 0` 表示关闭吸附）。
    fps: f32,
    /// 是否播放中（推进由应用调用 [`Self::advance`] 驱动）。
    playing: bool,
    /// 是否循环播放。
    looping: bool,
    /// 选中的轨道。
    selected_track: Option<usize>,
    /// 选中的关键帧。
    selected_keyframe: Option<usize>,
    /// 是否正在拖拽播放头。
    scrubbing: bool,
    /// 正在拖拽的关键帧（轨道, 关键帧）。
    dragging: Option<(usize, usize)>,
    /// 拖拽关键帧时冻结的值域（避免值域随拖动缩放抖动）。
    drag_range: (f32, f32),
    /// 按下时指针值 → 关键帧值的偏移（保证抓取不跳变）。
    drag_value_offset: f32,
    /// 最近一次预绘制的根元素边界。
    bounds: Bounds<Pixels>,
    /// 焦点句柄。
    focus_handle: FocusHandle,
}

impl TimelineState {
    /// 创建时间线状态。
    pub fn new(timeline: Timeline, cx: &mut Context<Self>) -> Self {
        Self {
            timeline,
            time: 0.0,
            px_per_second: DEFAULT_PX_PER_SECOND,
            scroll: 0.0,
            vscroll: 0.0,
            fps: DEFAULT_FPS,
            playing: false,
            looping: true,
            selected_track: None,
            selected_keyframe: None,
            scrubbing: false,
            dragging: None,
            drag_range: (0.0, 1.0),
            drag_value_offset: 0.0,
            bounds: Bounds::default(),
            focus_handle: cx.focus_handle(),
        }
    }

    /// 数据模型。
    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    /// 数据模型（可变；修改后调用 `cx.notify()`）。
    pub fn timeline_mut(&mut self) -> &mut Timeline {
        &mut self.timeline
    }

    /// 播放头时间（秒）。
    pub fn time(&self) -> f32 {
        self.time
    }

    /// 总时长（秒）。
    pub fn duration(&self) -> f32 {
        self.timeline.duration()
    }

    /// 设置播放头时间（夹取为非负，吸附到帧网格）并抛出 `TimeChanged`。
    pub fn set_time(&mut self, time: f32, cx: &mut Context<Self>) {
        let time = self.snapped(time.max(0.0));
        if (self.time - time).abs() > 1e-6 {
            self.time = time;
            cx.emit(TimelineEvent::TimeChanged(time));
            cx.notify();
        }
    }

    /// 推进播放头 `dt` 秒（播放时由应用每帧调用）。
    ///
    /// 越过末尾时：循环则回绕到 0，否则停在末尾并停止播放。
    pub fn advance(&mut self, dt: f32, cx: &mut Context<Self>) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let duration = self.duration();
        let mut time = self.time + dt;
        if duration > 0.0 && time > duration {
            if self.looping {
                time = time.rem_euclid(duration);
            } else {
                time = duration;
                self.playing = false;
                cx.emit(TimelineEvent::PlaybackChanged(false));
            }
        }
        let time = self.snapped(time.max(0.0));
        if (self.time - time).abs() > 1e-6 {
            self.time = time;
            cx.emit(TimelineEvent::TimeChanged(time));
            cx.notify();
        }
    }

    /// 是否播放中。
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// 开始播放（若当前不在播放中）。
    pub fn play(&mut self, cx: &mut Context<Self>) {
        if !self.playing {
            self.playing = true;
            cx.emit(TimelineEvent::PlaybackChanged(true));
            cx.notify();
        }
    }

    /// 暂停播放。
    pub fn pause(&mut self, cx: &mut Context<Self>) {
        if self.playing {
            self.playing = false;
            cx.emit(TimelineEvent::PlaybackChanged(false));
            cx.notify();
        }
    }

    /// 在播放 / 暂停之间切换。
    pub fn toggle_play(&mut self, cx: &mut Context<Self>) {
        if self.playing {
            self.pause(cx);
        } else {
            self.play(cx);
        }
    }

    /// 是否循环播放。
    pub fn is_looping(&self) -> bool {
        self.looping
    }

    /// 设置是否循环播放。
    pub fn set_looping(&mut self, looping: bool, cx: &mut Context<Self>) {
        if self.looping != looping {
            self.looping = looping;
            cx.notify();
        }
    }

    /// 缩放（每秒像素）。
    pub fn px_per_second(&self) -> f32 {
        self.px_per_second
    }

    /// 设置缩放（夹取到 [`MIN_PX_PER_SECOND`]..=[`MAX_PX_PER_SECOND`]）。
    pub fn set_px_per_second(&mut self, value: f32, cx: &mut Context<Self>) {
        let value = clamp_px_per_second(value);
        if (self.px_per_second - value).abs() > f32::EPSILON {
            self.px_per_second = value;
            cx.notify();
        }
    }

    /// 按倍率缩放。
    pub fn zoom_by(&mut self, factor: f32, cx: &mut Context<Self>) {
        self.set_px_per_second(self.px_per_second * factor, cx);
    }

    /// 水平滚动偏移（秒）。
    pub fn scroll(&self) -> f32 {
        self.scroll
    }

    /// 设置水平滚动偏移（夹取为非负）。
    pub fn set_scroll(&mut self, scroll: f32, cx: &mut Context<Self>) {
        let scroll = if scroll.is_finite() {
            scroll.max(0.0)
        } else {
            0.0
        };
        if (self.scroll - scroll).abs() > 1e-6 {
            self.scroll = scroll;
            cx.notify();
        }
    }

    /// 垂直滚动偏移（像素，轨道区）。
    pub fn vscroll(&self) -> f32 {
        self.vscroll
    }

    /// 纵向可滚动范围（轨道总高超出可视高度的部分）。
    pub fn max_vscroll(&self) -> f32 {
        let visible = (self.bounds.size.height - RULER_HEIGHT).as_f32().max(0.0);
        (self.tracks_height() - visible).max(0.0)
    }

    /// 设置垂直滚动偏移（夹取到 `0..=max_vscroll()`）。
    pub fn set_vscroll(&mut self, vscroll: f32, cx: &mut Context<Self>) {
        let vscroll = if vscroll.is_finite() {
            vscroll.clamp(0.0, self.max_vscroll())
        } else {
            0.0
        };
        if (self.vscroll - vscroll).abs() > 0.5 {
            self.vscroll = vscroll;
            cx.notify();
        }
    }

    /// 轨道区内容总高（折叠轨道按 [`COLLAPSED_TRACK_HEIGHT`] 计）。
    pub fn tracks_height(&self) -> f32 {
        self.timeline
            .tracks()
            .iter()
            .map(|track| track.row_height().as_f32())
            .sum()
    }

    /// 轨道 `index` 顶部在轨道区内容坐标里的位置（未减 `vscroll`）。
    pub fn track_top(&self, index: usize) -> f32 {
        self.timeline
            .tracks()
            .iter()
            .take(index)
            .map(|track| track.row_height().as_f32())
            .sum()
    }

    /// 把某条轨道滚入可视区（已可见时不动）。
    pub fn scroll_to_track(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(track) = self.timeline.track(index) else {
            return;
        };
        let visible = (self.bounds.size.height - RULER_HEIGHT).as_f32().max(0.0);
        if visible <= 0.0 {
            return;
        }
        let height = track.row_height().as_f32();
        let top = self.track_top(index);
        let bottom = top + height;
        let next = if top < self.vscroll {
            top
        } else if bottom > self.vscroll + visible {
            bottom - visible
        } else {
            return;
        };
        self.set_vscroll(next, cx);
    }

    /// 折叠 / 展开轨道（折叠后行高缩小、不绘制关键帧），返回是否变化。
    pub fn toggle_track_collapsed(&mut self, index: usize, cx: &mut Context<Self>) -> bool {
        let Some(track) = self.timeline.track_mut(index) else {
            return false;
        };
        let collapsed = !track.collapsed;
        if !track.set_collapsed(collapsed) {
            return false;
        }
        // 行高变化后把纵向偏移夹回可滚动范围。
        let max = self.max_vscroll();
        self.vscroll = self.vscroll.min(max);
        cx.emit(TimelineEvent::TrackCollapsed { index, collapsed });
        cx.notify();
        true
    }

    /// 设置轨道的插值方式（值曲线形状），返回是否变化。
    pub fn set_track_interpolation(
        &mut self,
        index: usize,
        interpolation: Interpolation,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(track) = self.timeline.track_mut(index) else {
            return false;
        };
        if track.interpolation == interpolation {
            return false;
        }
        track.interpolation = interpolation;
        cx.emit(TimelineEvent::TrackInterpolation {
            index,
            interpolation,
        });
        cx.notify();
        true
    }

    /// 调整轨道顺序，选中项与拖动中的关键帧跟随移动。
    pub fn move_track(&mut self, from: usize, to: usize, cx: &mut Context<Self>) -> bool {
        if !self.timeline.move_track(from, to) {
            return false;
        }
        let to = to.min(self.timeline.len().saturating_sub(1));
        if let Some(selected) = self.selected_track {
            self.selected_track = Some(if selected == from {
                to
            } else if from < selected && to >= selected {
                selected - 1
            } else if from > selected && to <= selected {
                selected + 1
            } else {
                selected
            });
        }
        if let Some((track, keyframe)) = self.dragging
            && track == from
        {
            self.dragging = Some((to, keyframe));
        }
        cx.emit(TimelineEvent::TrackMoved { from, to });
        cx.notify();
        true
    }

    /// 吸附帧率（`<= 0` 表示关闭吸附）。
    pub fn fps(&self) -> f32 {
        self.fps
    }

    /// 设置吸附帧率。
    pub fn set_fps(&mut self, fps: f32, cx: &mut Context<Self>) {
        let fps = if fps.is_finite() && fps > 0.0 {
            fps
        } else {
            0.0
        };
        if (self.fps - fps).abs() > f32::EPSILON {
            self.fps = fps;
            cx.notify();
        }
    }

    /// 是否开启吸附。
    pub fn is_snapping(&self) -> bool {
        self.fps > 0.0
    }

    /// 按当前帧率吸附时间。
    pub fn snapped(&self, time: f32) -> f32 {
        snap_time(time, self.fps)
    }

    /// 选中的（轨道, 关键帧）。
    pub fn selection(&self) -> (Option<usize>, Option<usize>) {
        (self.selected_track, self.selected_keyframe)
    }

    /// 选中某个关键帧（越界则清空选择）。
    pub fn select_keyframe(&mut self, track: usize, index: usize, cx: &mut Context<Self>) {
        let valid = self.timeline.track(track).is_some_and(|t| index < t.len());
        let (track, index) = if valid {
            (Some(track), Some(index))
        } else {
            (None, None)
        };
        if (self.selected_track, self.selected_keyframe) != (track, index) {
            self.selected_track = track;
            self.selected_keyframe = index;
            cx.emit(TimelineEvent::SelectionChanged {
                track,
                keyframe: index,
            });
            cx.notify();
        }
    }

    /// 清空选择。
    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.select_keyframe(usize::MAX, usize::MAX, cx);
    }

    /// 删除当前选中的关键帧并抛出 `KeyframeRemoved`。
    pub fn remove_selected_keyframe(&mut self, cx: &mut Context<Self>) {
        let (Some(track), Some(index)) = (self.selected_track, self.selected_keyframe) else {
            return;
        };
        let removed = self
            .timeline
            .track_mut(track)
            .and_then(|t| t.remove_keyframe(index));
        if removed.is_some() {
            cx.emit(TimelineEvent::KeyframeRemoved { track, index });
            self.selected_track = None;
            self.selected_keyframe = None;
            cx.notify();
        }
    }

    /// 聚焦时间线。
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
    }

    /// 命中测试：把局部坐标（相对时间线左上角）映射到标尺 / 关键帧 / 轨道空白。
    pub fn hit_test(&self, local: Point<Pixels>) -> TimelineHit {
        let x = local.x - LABEL_WIDTH;
        if x < px(0.0) || local.x < px(0.0) || local.y < px(0.0) {
            return TimelineHit::None;
        }
        if local.y < RULER_HEIGHT {
            return TimelineHit::Ruler;
        }
        // 轨道区随垂直滚动偏移平移：先换算成内容坐标，再按各行实际高度找行。
        let y = (local.y - RULER_HEIGHT).as_f32() + self.vscroll;
        if !y.is_finite() || y < 0.0 {
            return TimelineHit::None;
        }
        let mut top = 0.0;
        let mut track = None;
        for (index, candidate) in self.timeline.tracks().iter().enumerate() {
            let height = candidate.row_height().as_f32();
            if y < top + height {
                track = Some(index);
                break;
            }
            top += height;
        }
        let Some(track) = track else {
            return TimelineHit::None;
        };
        let timeline_track = &self.timeline.tracks()[track];
        if timeline_track.collapsed {
            // 折叠轨道不绘制关键帧，整行都算轨道空白。
            return TimelineHit::Track { track };
        }
        let time = x_to_time(x.as_f32(), self.px_per_second, self.scroll);
        let tolerance = KEYFRAME_HIT_RADIUS.as_f32() / self.px_per_second;
        match timeline_track.keyframe_index_at(time, tolerance) {
            Some(index) => TimelineHit::Keyframe { track, index },
            None => TimelineHit::Track { track },
        }
    }

    /// 轨道某一行内的局部纵坐标（去掉标尺高度、加上纵向滚动、减去行顶）。
    fn row_y(&self, track: usize, local: Point<Pixels>) -> f32 {
        (local.y - RULER_HEIGHT).as_f32() + self.vscroll - self.track_top(track)
    }

    /// 轨道绘制用值域：正在拖拽该轨道时用冻结值域，否则按当前关键帧计算。
    fn range_for(&self, index: usize, track: &Track) -> (f32, f32) {
        if self
            .dragging
            .is_some_and(|(drag_track, _)| drag_track == index)
        {
            self.drag_range
        } else {
            track.value_range()
        }
    }

    /// 轨道行内纵坐标 → 关键帧值（按冻结值域与抓取偏移换算）。
    fn value_at_y(&self, y: f32) -> f32 {
        y_to_value(y, self.drag_range, TRACK_HEIGHT) + self.drag_value_offset
    }

    /// 指针按下：标尺开始拖拽播放头；轨道上选中 / 拖拽关键帧；双击空白请求新增。
    fn on_pointer_down(
        &mut self,
        local: Point<Pixels>,
        click_count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus(window, cx);
        match self.hit_test(local) {
            TimelineHit::Ruler => {
                self.scrubbing = true;
                let time = x_to_time(
                    (local.x - LABEL_WIDTH).as_f32(),
                    self.px_per_second,
                    self.scroll,
                );
                self.set_time(time, cx);
            }
            TimelineHit::Keyframe { track, index } => {
                self.select_keyframe(track, index, cx);
                // 冻结值域并记录抓取偏移：纵向拖拽时曲线跟着指针走、不因值域缩放而抖动。
                if let Some(model) = self.timeline.track(track) {
                    self.drag_range = model.value_range();
                    let pointer_value =
                        y_to_value(self.row_y(track, local), self.drag_range, TRACK_HEIGHT);
                    let value = model
                        .keyframes
                        .get(index)
                        .map_or(pointer_value, |keyframe| keyframe.value);
                    self.drag_value_offset = value - pointer_value;
                }
                self.dragging = Some((track, index));
            }
            TimelineHit::Track { track } => {
                self.clear_selection(cx);
                if click_count >= 2 {
                    let time = x_to_time(
                        (local.x - LABEL_WIDTH).as_f32(),
                        self.px_per_second,
                        self.scroll,
                    );
                    cx.emit(TimelineEvent::KeyframeRequested {
                        track,
                        time: self.snapped(time.max(0.0)),
                    });
                } else {
                    self.selected_track = Some(track);
                }
            }
            // 左侧轨道名列的点击（折叠箭头 / 名称）不清空选择。
            TimelineHit::None => {
                if local.x >= LABEL_WIDTH {
                    self.clear_selection(cx);
                }
            }
        }
    }

    /// 指针移动：拖拽播放头 / 拖拽关键帧（水平改时间、垂直改值）。
    fn on_pointer_move(&mut self, local: Point<Pixels>, cx: &mut Context<Self>) {
        let x = (local.x - LABEL_WIDTH).as_f32();
        if self.scrubbing {
            let time = x_to_time(x, self.px_per_second, self.scroll);
            self.set_time(time, cx);
        } else if let Some((track, index)) = self.dragging {
            let time = self.snapped(x_to_time(x, self.px_per_second, self.scroll).max(0.0));
            let value = self.value_at_y(self.row_y(track, local));
            let Some(model) = self.timeline.track_mut(track) else {
                return;
            };
            let Some(new_index) = model.move_keyframe(index, time) else {
                return;
            };
            model.set_keyframe_value(new_index, value);
            let value = model
                .keyframes
                .get(new_index)
                .map_or(value, |keyframe| keyframe.value);
            self.selected_track = Some(track);
            self.selected_keyframe = Some(new_index);
            self.dragging = Some((track, new_index));
            cx.emit(TimelineEvent::KeyframeMoved {
                track,
                index: new_index,
                time,
                value,
            });
            cx.notify();
        }
    }

    /// 指针释放：结束拖拽并清掉冻结的值域。
    fn on_pointer_up(&mut self, cx: &mut Context<Self>) {
        if self.scrubbing || self.dragging.is_some() {
            self.scrubbing = false;
            self.dragging = None;
            self.drag_range = (0.0, 1.0);
            self.drag_value_offset = 0.0;
            cx.notify();
        }
    }

    /// 滚轮：`Ctrl/Cmd` 缩放；`Shift` 纵向滚动轨道；其余水平平移时间轴。
    fn on_wheel(&mut self, event: &ScrollWheelEvent, local: Point<Pixels>, cx: &mut Context<Self>) {
        let delta = match event.delta {
            ScrollDelta::Pixels(delta) => delta,
            ScrollDelta::Lines(delta) => point(px(delta.x * 16.0), px(delta.y * 16.0)),
        };
        if event.modifiers.control || event.modifiers.platform {
            let anchor_x = (local.x - LABEL_WIDTH).as_f32();
            let anchor_time = x_to_time(anchor_x, self.px_per_second, self.scroll);
            let factor = if delta.y > px(0.0) { 1.1 } else { 1.0 / 1.1 };
            let next = clamp_px_per_second(self.px_per_second * factor);
            if (next - self.px_per_second).abs() > f32::EPSILON {
                self.px_per_second = next;
                // 保持指针下的时间不动。
                self.scroll = (anchor_time - anchor_x / next).max(0.0);
                cx.notify();
            }
            return;
        }
        // 横向平移是时间线的主操作（与视频编辑器一致），纵向轨道滚动走 `Shift + 滚轮`。
        if event.modifiers.shift {
            if delta.y != px(0.0) {
                self.set_vscroll(self.vscroll + delta.y.as_f32(), cx);
            }
            return;
        }
        let pan = if delta.x != px(0.0) { delta.x } else { delta.y };
        let scroll = self.scroll - pan.as_f32() / self.px_per_second;
        self.set_scroll(scroll, cx);
    }

    /// 键盘：空格播放/暂停、方向键移动播放头、Home/End 首尾、Delete 删除关键帧、± 缩放。
    fn on_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.to_ascii_lowercase();
        let modifiers = event.keystroke.modifiers;
        let step = if modifiers.shift {
            1.0
        } else if self.is_snapping() {
            1.0 / self.fps
        } else {
            0.1
        };

        match key.as_str() {
            "space" => self.toggle_play(cx),
            "left" => self.set_time(self.time - step, cx),
            "right" => self.set_time(self.time + step, cx),
            "home" => self.set_time(0.0, cx),
            "end" => self.set_time(self.duration(), cx),
            "delete" | "backspace" => self.remove_selected_keyframe(cx),
            "=" | "+" => self.zoom_by(1.25, cx),
            "-" | "_" => self.zoom_by(0.8, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    /// 可见时间跨度（秒）。
    fn visible_span(&self) -> f32 {
        let width = (self.bounds.size.width - LABEL_WIDTH).as_f32();
        if width <= 0.0 {
            0.0
        } else {
            width / self.px_per_second
        }
    }

    /// 渲染标尺刻度。
    fn render_ruler(&self, border: Hsla, muted_foreground: Hsla) -> AnyElement {
        let span = self.visible_span();
        let step = ruler_step(self.px_per_second, MIN_TICK_SPACING);
        let mut ticks: Vec<AnyElement> = Vec::new();
        if step > 0.0 && span > 0.0 {
            let count = ((span / step).floor() as usize + 1).min(MAX_TICKS);
            let start = (self.scroll / step).floor() * step;
            for index in 0..count {
                let time = start + index as f32 * step;
                let x = time_to_x(time, self.px_per_second, self.scroll).round();
                ticks.push(
                    div()
                        .absolute()
                        .left(px(x))
                        .top_0()
                        .bottom_0()
                        .child(div().w(px(1.0)).h(px(6.0)).bg(border))
                        .child(
                            div()
                                .absolute()
                                .left(px(3.0))
                                .top(px(6.0))
                                .text_xs()
                                .text_color(muted_foreground)
                                .child(format_time(time)),
                        )
                        .into_any_element(),
                );
            }
        }

        div()
            .absolute()
            .left(LABEL_WIDTH)
            .top_0()
            .right_0()
            .h(RULER_HEIGHT)
            .overflow_hidden()
            .border_b_1()
            .border_color(border)
            .children(ticks)
            .into_any_element()
    }

    /// 渲染一条轨道行。
    fn render_track(
        &self,
        index: usize,
        selected_bg: Hsla,
        border: Hsla,
        accent: Hsla,
        muted_foreground: Hsla,
    ) -> AnyElement {
        let selected = self.selected_track == Some(index);
        let track = &self.timeline.tracks()[index];
        let is_empty = track.is_empty();
        let collapsed = track.collapsed;
        let height = track.row_height();
        let top = self.track_top(index);

        let mut markers: Vec<AnyElement> = Vec::with_capacity(track.len());
        if !collapsed {
            let range = self.range_for(index, track);
            let max_top = (TRACK_HEIGHT.as_f32() - KEYFRAME_SIZE.as_f32()).max(0.0);
            for (kf_index, keyframe) in track.keyframes.iter().enumerate() {
                let x = time_to_x(keyframe.time, self.px_per_second, self.scroll);
                let is_selected =
                    self.selected_track == Some(index) && self.selected_keyframe == Some(kf_index);
                // 方块压在值曲线上（而不是行中间），纵向拖拽时能直接看到值变化。
                let top = (value_to_y(keyframe.value, range, TRACK_HEIGHT)
                    - KEYFRAME_SIZE.as_f32() * 0.5)
                    .clamp(0.0, max_top);
                markers.push(
                    div()
                        .absolute()
                        .left(px(x - KEYFRAME_SIZE.as_f32() * 0.5))
                        .top(px(top))
                        .w(KEYFRAME_SIZE)
                        .h(KEYFRAME_SIZE)
                        .rounded_sm()
                        .bg(if is_selected {
                            accent
                        } else {
                            muted_foreground
                        })
                        .border_2()
                        .border_color(if is_selected { accent } else { border })
                        .into_any_element(),
                );
            }
        }

        // 值曲线：按插值方式采样成折线，画在关键帧方块下面。
        let curve = (!collapsed && track.len() >= 2).then(|| {
            let points = curve_points(
                track,
                self.range_for(index, track),
                self.px_per_second,
                self.scroll,
                TRACK_HEIGHT,
            );
            canvas(
                move |_bounds, _window, _cx| polyline_path(&points),
                move |_bounds, path, window, _cx| {
                    if let Some(path) = path {
                        window.paint_path(path, accent.opacity(0.8));
                    }
                },
            )
            .absolute()
            .size_full()
            .into_any_element()
        });

        div()
            .absolute()
            .left(LABEL_WIDTH)
            .top(px(top - self.vscroll))
            .right_0()
            .h(height)
            .overflow_hidden()
            .border_b_1()
            .border_color(border)
            .when(selected, |this| this.bg(selected_bg))
            .when(is_empty && !collapsed, |this| {
                this.child(
                    div()
                        .absolute()
                        .left_2()
                        .top_0()
                        .bottom_0()
                        .flex()
                        .items_center()
                        .text_xs()
                        .text_color(muted_foreground)
                        .child("双击添加关键帧"),
                )
            })
            .children(curve)
            .children(markers)
            .into_any_element()
    }
}

impl EventEmitter<TimelineEvent> for TimelineState {}

impl Focusable for TimelineState {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TimelineState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let border = theme.border;
        let accent = theme.primary;
        let muted_foreground = theme.muted_foreground;
        let selected_bg = theme.secondary;
        let background = theme.background;
        let state = cx.entity();

        let ruler = self.render_ruler(border, muted_foreground);
        let tracks: Vec<AnyElement> = (0..self.timeline.len())
            .map(|index| self.render_track(index, selected_bg, border, accent, muted_foreground))
            .collect();
        let labels: Vec<AnyElement> = self
            .timeline
            .tracks()
            .iter()
            .enumerate()
            .map(|(index, track)| {
                let height = track.row_height();
                div()
                    .absolute()
                    .left_0()
                    .top(px(self.track_top(index) - self.vscroll))
                    .w(LABEL_WIDTH)
                    .h(height)
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .overflow_hidden()
                    .border_b_1()
                    .border_color(border)
                    .text_xs()
                    .child(
                        // 折叠箭头：点按切换该轨道折叠（不选择关键帧）。
                        div()
                            .id(("timeline-collapse", index))
                            .flex_none()
                            .w(px(14.0))
                            .flex()
                            .justify_center()
                            .text_color(muted_foreground)
                            .cursor_pointer()
                            .child(if track.collapsed { "▸" } else { "▾" })
                            .on_click(window.listener_for(
                                &state,
                                move |state, _event, _window, cx| {
                                    state.toggle_track_collapsed(index, cx);
                                },
                            )),
                    )
                    .child(track.name.clone())
                    .into_any_element()
            })
            .collect();

        // 轨道区裁剪容器：滚动时行不会盖住标尺或越出底部。
        let lane = div()
            .absolute()
            .left_0()
            .top(RULER_HEIGHT)
            .right_0()
            .bottom_0()
            .overflow_hidden()
            .children(tracks)
            .children(labels)
            .into_any_element();

        // 播放头：整条竖线 + 标尺上的手柄。
        let playhead_x = time_to_x(self.time, self.px_per_second, self.scroll);
        let playhead = div()
            .absolute()
            .left(LABEL_WIDTH + px(playhead_x))
            .top_0()
            .bottom_0()
            .w(px(2.0))
            .bg(accent)
            .into_any_element();
        let handle = div()
            .absolute()
            .left(LABEL_WIDTH + px(playhead_x - 5.0))
            .top(px(4.0))
            .w(px(10.0))
            .h(px(10.0))
            .rounded_sm()
            .bg(accent)
            .into_any_element();

        div()
            .id(("anim-timeline-body", cx.entity().entity_id()))
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(background)
            .on_prepaint({
                let state = cx.entity();
                move |bounds, _window, cx| {
                    state.update(cx, |state, _| {
                        state.bounds = bounds;
                        // 视口变小后把偏移夹回可滚动范围。
                        state.vscroll = state.vscroll.min(state.max_vscroll());
                    });
                }
            })
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .w(LABEL_WIDTH)
                    .h(RULER_HEIGHT)
                    .border_b_1()
                    .border_color(border),
            )
            .child(ruler)
            .child(lane)
            .child(playhead)
            .child(handle)
    }
}

/// 时间线元素（薄包装：按键上下文、焦点与指针事件）。
#[derive(IntoElement)]
pub struct TimelineView {
    state: Entity<TimelineState>,
}

impl TimelineView {
    /// 使用给定 [`TimelineState`] 创建时间线元素。
    pub fn new(state: &Entity<TimelineState>) -> Self {
        Self {
            state: state.clone(),
        }
    }
}

impl RenderOnce for TimelineView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus_handle = self.state.read(cx).focus_handle(cx);
        let entity_id = self.state.entity_id();

        div()
            .id(("anim-timeline", entity_id))
            .key_context("AnimTimeline")
            .track_focus(&focus_handle)
            .size_full()
            .on_key_down(window.listener_for(
                &self.state,
                |state, event: &KeyDownEvent, _window, cx| {
                    state.on_key_down(event, cx);
                },
            ))
            .on_mouse_down(
                MouseButton::Left,
                window.listener_for(&self.state, |state, event: &MouseDownEvent, window, cx| {
                    let local = event.position - state.bounds.origin;
                    state.on_pointer_down(local, event.click_count, window, cx);
                }),
            )
            .on_mouse_move(window.listener_for(
                &self.state,
                |state, event: &MouseMoveEvent, _window, cx| {
                    let local = event.position - state.bounds.origin;
                    state.on_pointer_move(local, cx);
                },
            ))
            .on_scroll_wheel(window.listener_for(
                &self.state,
                |state, event: &ScrollWheelEvent, _window, cx| {
                    let local = event.position - state.bounds.origin;
                    state.on_wheel(event, local, cx);
                },
            ))
            .capture_any_mouse_up(window.listener_for(
                &self.state,
                |state, _event: &MouseUpEvent, _window, cx| {
                    state.on_pointer_up(cx);
                },
            ))
            .child(self.state)
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{
        COLLAPSED_TRACK_HEIGHT, DEFAULT_PX_PER_SECOND, Interpolation, Keyframe, MAX_PX_PER_SECOND,
        MIN_PX_PER_SECOND, RULER_HEIGHT, TRACK_HEIGHT, Timeline, TimelineEvent, TimelineHit,
        TimelineState, TimelineView, Track, VALUE_PADDING, clamp_px_per_second, curve_points,
        format_time, ruler_step, snap_time, time_to_x, value_to_y, x_to_time, y_to_value,
    };
    use crate::{
        AppContext as _, Bounds, Context, Entity, IntoElement, ParentElement as _, Render,
        Styled as _, Window, div, point, px, size,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    /// 两条轨道的示例时间线。
    fn demo_timeline() -> Timeline {
        Timeline::from_tracks(vec![
            Track::with_keyframes(
                "位置",
                vec![Keyframe::new(0.0, 0.0), Keyframe::new(2.0, 200.0)],
            ),
            Track::with_keyframes(
                "透明度",
                vec![
                    Keyframe::new(0.0, 0.0),
                    Keyframe::new(1.0, 1.0),
                    Keyframe::new(2.0, 0.5),
                ],
            ),
        ])
    }

    /// 渲染时间线的测试宿主。
    struct Probe {
        state: Entity<TimelineState>,
    }

    impl Render for Probe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(TimelineView::new(&self.state))
        }
    }

    /// 构建宿主并收集时间线事件。
    fn setup(
        cx: &mut crate::TestAppContext,
    ) -> (
        Entity<TimelineState>,
        Rc<RefCell<Vec<TimelineEvent>>>,
        &mut crate::VisualTestContext,
    ) {
        let events: Rc<RefCell<Vec<TimelineEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = events.clone();
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(move |window, cx| {
            let state = cx.new(|cx| TimelineState::new(demo_timeline(), cx));
            cx.subscribe_in(
                &state,
                window,
                move |_this, _state, event: &TimelineEvent, _window, _cx| {
                    sink.borrow_mut().push(event.clone());
                },
            )
            .detach();
            Probe { state }
        });
        let state = probe.read_with(&*cx, |probe, _| probe.state.clone());
        (state, events, cx)
    }

    /// `normalize` 排序并合并同一时间的关键帧。
    #[test]
    fn track_normalizes_and_dedupes() {
        let mut track = Track::with_keyframes(
            "t",
            vec![
                Keyframe::new(2.0, 20.0),
                Keyframe::new(0.0, 0.0),
                Keyframe::new(2.0, 25.0),
                Keyframe::new(1.0, 10.0),
            ],
        );
        assert_eq!(track.len(), 3);
        assert_eq!(track.keyframes[0], Keyframe::new(0.0, 0.0));
        assert_eq!(track.keyframes[1], Keyframe::new(1.0, 10.0));
        // 同一时间的重复帧保留最后一个。
        assert_eq!(track.keyframes[2], Keyframe::new(2.0, 25.0));
        assert_eq!(track.duration(), 2.0);

        // 覆盖已有时间返回 false，新增返回 true。
        assert!(!track.set_keyframe(1.0, 11.0));
        assert_eq!(track.value_at(1.0), 11.0);
        assert!(track.set_keyframe(1.5, 15.0));
        assert_eq!(track.len(), 4);
    }

    /// 三种插值方式求值。
    #[test]
    fn track_interpolation_modes() {
        let linear = Track::with_keyframes(
            "linear",
            vec![Keyframe::new(0.0, 0.0), Keyframe::new(1.0, 10.0)],
        );
        assert_eq!(linear.value_at(0.5), 5.0);
        // 首帧之前 / 末帧之后保持端点值。
        assert_eq!(linear.value_at(-1.0), 0.0);
        assert_eq!(linear.value_at(5.0), 10.0);

        let hold = Track::with_keyframes(
            "hold",
            vec![Keyframe::new(0.0, 0.0), Keyframe::new(1.0, 10.0)],
        )
        .with_interpolation(Interpolation::Hold);
        assert_eq!(hold.value_at(0.5), 0.0);
        assert_eq!(hold.value_at(1.0), 10.0);

        let smooth = Track::with_keyframes(
            "smooth",
            vec![Keyframe::new(0.0, 0.0), Keyframe::new(1.0, 10.0)],
        )
        .with_interpolation(Interpolation::Smooth);
        // smoothstep(0.5) = 0.5
        assert!((smooth.value_at(0.5) - 5.0).abs() < 1e-4);
        assert!((smooth.value_at(0.25) - 10.0 * 0.15625).abs() < 1e-3);

        // 空轨道恒为 0。
        assert_eq!(Track::new("empty").value_at(0.5), 0.0);
    }

    /// 缓动插值：曲线形状与端点行为。
    #[test]
    fn easing_interpolation_curves() {
        // 缓入（二次）：中点 0.25；缓出：0.75；缓入缓出：0.5。
        assert!((Interpolation::EaseIn.ease(0.5) - 0.25).abs() < 1e-5);
        assert!((Interpolation::EaseOut.ease(0.5) - 0.75).abs() < 1e-5);
        assert!((Interpolation::EaseInOut.ease(0.5) - 0.5).abs() < 1e-5);
        // 缓入缓出在四分之一处为 4 * 0.25³ = 0.0625。
        assert!((Interpolation::EaseInOut.ease(0.25) - 0.0625).abs() < 1e-5);
        // 所有插值：两端固定、中间单调。
        for interpolation in [
            Interpolation::Linear,
            Interpolation::Smooth,
            Interpolation::EaseIn,
            Interpolation::EaseOut,
            Interpolation::EaseInOut,
        ] {
            assert_eq!(interpolation.ease(0.0), 0.0);
            assert_eq!(interpolation.ease(1.0), 1.0);
            assert!(interpolation.ease(0.5) <= 1.0);
            // 越界进度被夹取。
            assert_eq!(interpolation.ease(-1.0), 0.0);
            assert_eq!(interpolation.ease(2.0), 1.0);
        }
        // 中文名（UI 展示用）。
        assert_eq!(Interpolation::EaseInOut.label(), "缓入缓出");
    }

    /// 设置轨道插值：轨道变化 + 抛出 `TrackInterpolation`；相同值不算变化。
    #[rgpui::test]
    fn set_track_interpolation_updates_track(cx: &mut crate::TestAppContext) {
        let (state, events, mut cx) = setup(cx);
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                assert!(state.set_track_interpolation(0, Interpolation::EaseInOut, cx));
                // 相同值不再变更。
                assert!(!state.set_track_interpolation(0, Interpolation::EaseInOut, cx));
                // 越界轨道忽略。
                assert!(!state.set_track_interpolation(99, Interpolation::Linear, cx));
            })
        });
        let interpolation = cx.update(|_, cx| {
            state.read_with(cx, |state, _| {
                state.timeline().track(0).map(|t| t.interpolation)
            })
        });
        assert_eq!(interpolation, Some(Interpolation::EaseInOut));
        let events = events.borrow().clone();
        assert!(events.iter().any(|event| matches!(
            event,
            TimelineEvent::TrackInterpolation {
                index: 0,
                interpolation: Interpolation::EaseInOut
            }
        )));
    }
    /// 命中测试与关键帧移动 / 删除。
    #[test]
    fn keyframe_hit_move_and_remove() {
        let mut track = Track::with_keyframes(
            "t",
            vec![
                Keyframe::new(0.0, 0.0),
                Keyframe::new(1.0, 10.0),
                Keyframe::new(2.0, 5.0),
            ],
        );
        // 容差内命中最近帧，容差外不命中。
        assert_eq!(track.keyframe_index_at(1.05, 0.1), Some(1));
        assert_eq!(track.keyframe_index_at(1.5, 0.1), None);

        // 把第一个关键帧移到 1.5，重新排序后下标为 1。
        let moved = track.move_keyframe(0, 1.5);
        assert_eq!(moved, Some(1));
        assert_eq!(track.keyframes[0].time, 1.0);
        assert_eq!(track.keyframes[1].time, 1.5);
        assert_eq!(track.keyframes[1].value, 0.0);

        // 越界删除返回 None。
        assert!(track.remove_keyframe(9).is_none());
        assert_eq!(track.remove_keyframe(0).map(|kf| kf.time), Some(1.0));
        assert_eq!(track.len(), 2);
    }

    /// 关键帧改值（不改时间）、值域留白：空轨道 / 单值 / 越界。
    #[test]
    fn keyframe_value_edit_and_range() {
        let mut track =
            Track::with_keyframes("t", vec![Keyframe::new(0.0, 0.0), Keyframe::new(1.0, 10.0)]);
        // 值域上下各留 10%。
        assert_eq!(track.value_range(), (-1.0, 11.0));

        assert!(track.set_keyframe_value(1, 4.0));
        assert_eq!(track.keyframes[1].value, 4.0);
        // 时间不变，排序也不受影响。
        assert_eq!(track.keyframes[1].time, 1.0);
        assert_eq!(track.value_range(), (-0.4, 4.4));

        // 值相同不算变化；越界下标与非有限值被忽略。
        assert!(!track.set_keyframe_value(1, 4.0));
        assert!(!track.set_keyframe_value(9, 1.0));
        assert!(!track.set_keyframe_value(1, f32::NAN));
        assert_eq!(track.keyframes[1].value, 4.0);

        // 所有值相同：上下各留 0.5，曲线落在行中间。
        let flat =
            Track::with_keyframes("f", vec![Keyframe::new(0.0, 2.0), Keyframe::new(1.0, 2.0)]);
        assert_eq!(flat.value_range(), (1.5, 2.5));
        // 空轨道退化为 (0, 1)。
        assert_eq!(Track::new("e").value_range(), (0.0, 1.0));
    }

    /// 值 ↔ 行内纵坐标：留白、中点、往返一致与越界夹取。
    #[test]
    fn value_geometry_roundtrips() {
        let range = (-1.0, 11.0);
        let height = TRACK_HEIGHT;

        // 值域两端正好落在上下留白处。
        assert!((value_to_y(11.0, range, height) - VALUE_PADDING).abs() < 1e-4);
        assert!((value_to_y(-1.0, range, height) - (24.0 - VALUE_PADDING)).abs() < 1e-4);
        // 中间值落在可用高度中点（3 + 18/2 = 12）。
        let mid = value_to_y(5.0, range, height);
        assert!((mid - 12.0).abs() < 1e-4);
        assert!((y_to_value(mid, range, height) - 5.0).abs() < 1e-4);

        // 越界值 / 越界坐标都夹到两端。
        assert_eq!(value_to_y(100.0, range, height), VALUE_PADDING);
        assert_eq!(y_to_value(-100.0, range, height), 11.0);
        assert_eq!(y_to_value(100.0, range, height), -1.0);
    }

    /// 值曲线采样：首尾点对齐关键帧，单帧 / 空轨道退化，`Hold` 有竖直跳变。
    #[test]
    fn curve_samples_follow_interpolation() {
        let track =
            Track::with_keyframes("t", vec![Keyframe::new(0.0, 0.0), Keyframe::new(1.0, 10.0)]);
        let range = track.value_range();
        let points = curve_points(&track, range, 80.0, 0.0, TRACK_HEIGHT);
        assert!(points.len() > 2);
        assert!((points[0].x.as_f32()).abs() < 1e-4);
        assert!((points[0].y.as_f32() - value_to_y(0.0, range, TRACK_HEIGHT)).abs() < 1e-4);
        let last = points[points.len() - 1];
        assert!((last.x.as_f32() - 80.0).abs() < 1e-4);
        assert!((last.y.as_f32() - value_to_y(10.0, range, TRACK_HEIGHT)).abs() < 1e-4);

        // 单关键帧只有一个点；空轨道没有点。
        let single = Track::with_keyframes("s", vec![Keyframe::new(0.5, 1.0)]);
        assert_eq!(
            curve_points(&single, single.value_range(), 80.0, 0.0, TRACK_HEIGHT).len(),
            1
        );
        assert!(curve_points(&Track::new("e"), (0.0, 1.0), 80.0, 0.0, TRACK_HEIGHT).is_empty());

        // Hold：段尾竖直跳变（两点同 x、不同 y），跳变前保持首值。
        let hold =
            Track::with_keyframes("h", vec![Keyframe::new(0.0, 0.0), Keyframe::new(1.0, 10.0)])
                .with_interpolation(Interpolation::Hold);
        let range = hold.value_range();
        let points = curve_points(&hold, range, 80.0, 0.0, TRACK_HEIGHT);
        let jump = points.windows(2).find(|pair| {
            (pair[0].x.as_f32() - pair[1].x.as_f32()).abs() < 1e-4
                && (pair[0].y.as_f32() - pair[1].y.as_f32()).abs() > 1.0
        });
        assert!(jump.is_some(), "Hold 曲线应有竖直跳变");
        for point in &points[..points.len() - 1] {
            assert!((point.y.as_f32() - value_to_y(0.0, range, TRACK_HEIGHT)).abs() < 1e-3);
        }
    }

    /// 时间 ↔ 像素换算、吸附、缩放夹取与标尺步长。
    #[test]
    fn time_mapping_and_helpers() {
        // scroll = 1s, 100px/s：t=2s → x=100px。
        assert_eq!(time_to_x(2.0, 100.0, 1.0), 100.0);
        assert_eq!(x_to_time(100.0, 100.0, 1.0), 2.0);
        // 非法缩放时退化为 scroll。
        assert_eq!(x_to_time(50.0, 0.0, 3.0), 3.0);

        // 30fps 吸附：0.04s → 1/30 ≈ 0.0333。
        assert!((snap_time(0.04, 30.0) - 1.0 / 30.0).abs() < 1e-6);
        assert_eq!(snap_time(0.04, 0.0), 0.04);

        assert_eq!(clamp_px_per_second(1.0), MIN_PX_PER_SECOND);
        assert_eq!(clamp_px_per_second(9999.0), MAX_PX_PER_SECOND);
        assert_eq!(clamp_px_per_second(f32::NAN), DEFAULT_PX_PER_SECOND);

        // 整齐步长：100px/s 下要求至少 56px，即 0.56s → 取 1s。
        assert_eq!(ruler_step(100.0, 56.0), 1.0);
        // 10px/s：0.56 → 5.6s → 取 10s。
        assert_eq!(ruler_step(10.0, 56.0), 10.0);
        // 1000px/s：0.056 → 取 0.1s。
        assert!((ruler_step(1000.0, 56.0) - 0.1).abs() < 1e-6);

        assert_eq!(format_time(1.2), "1.20s");
        assert_eq!(format_time(62.5), "1:02.5");
    }

    /// 总时长与某时刻的所有轨道值。
    #[test]
    fn timeline_duration_and_values() {
        let timeline = demo_timeline();
        assert_eq!(timeline.duration(), 2.0);
        assert_eq!(timeline.values_at(1.0), vec![100.0, 1.0]);
        assert_eq!(timeline.len(), 2);

        let mut timeline = timeline;
        let index = timeline.add_track(Track::new("x"));
        assert_eq!(index, 2);
        assert_eq!(timeline.duration(), 2.0);
        assert!(timeline.remove_track(2).is_some());
        assert!(timeline.remove_track(9).is_none());
    }

    /// 轨道转进度式关键帧动画。
    #[test]
    fn track_converts_to_keyframe_animation() {
        let track =
            Track::with_keyframes("t", vec![Keyframe::new(0.0, 0.0), Keyframe::new(2.0, 10.0)]);
        let animation = track.to_keyframe_animation("t", 2.0);
        assert!((animation.interpolate(0.5) - 5.0).abs() < 1e-4);

        // 未显式给时长时用轨道时长。
        let animation = track.to_keyframe_animation("t", 0.0);
        assert!((animation.interpolate(1.0) - 10.0).abs() < 1e-4);
    }

    /// 播放头设置 / 推进 / 循环 / 播放状态。
    #[rgpui::test]
    fn playhead_advance_and_playback(cx: &mut crate::TestAppContext) {
        let (state, events, mut cx) = setup(cx);

        cx.update(|_, cx| state.update(cx, |state, cx| state.set_time(1.0, cx)));
        assert_eq!(state.read_with(cx, |state, _| state.time()), 1.0);

        // 越过末尾时循环回绕。
        cx.update(|_, cx| state.update(cx, |state, cx| state.advance(1.5, cx)));
        let time = state.read_with(cx, |state, _| state.time());
        assert!((time - 0.5).abs() < 1e-6, "循环回绕得到 {time}");

        // 关闭循环后停在末尾并停止播放。
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.set_looping(false, cx);
                state.play(cx);
                state.set_time(1.9, cx);
                state.advance(1.0, cx);
            })
        });
        assert_eq!(state.read_with(cx, |state, _| state.time()), 2.0);
        assert!(!state.read_with(cx, |state, _| state.is_playing()));

        cx.update(|_, cx| state.update(cx, |state, cx| state.toggle_play(cx)));
        assert!(state.read_with(cx, |state, _| state.is_playing()));

        let events = events.borrow();
        assert!(events.contains(&TimelineEvent::PlaybackChanged(true)));
        assert!(events.contains(&TimelineEvent::PlaybackChanged(false)));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, TimelineEvent::TimeChanged(_)))
        );
    }

    /// 缩放 / 滚动 / 吸附与选中 / 删除关键帧。
    #[rgpui::test]
    fn zoom_scroll_snap_and_keyframe_selection(cx: &mut crate::TestAppContext) {
        let (state, events, mut cx) = setup(cx);

        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.set_px_per_second(9999.0, cx);
                state.set_scroll(-5.0, cx);
                state.set_fps(60.0, cx);
            })
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.px_per_second()),
            MAX_PX_PER_SECOND
        );
        assert_eq!(state.read_with(cx, |state, _| state.scroll()), 0.0);
        assert!(state.read_with(cx, |state, _| state.is_snapping()));

        cx.update(|_, cx| state.update(cx, |state, cx| state.zoom_by(0.5, cx)));
        assert!(state.read_with(cx, |state, _| state.px_per_second()) < MAX_PX_PER_SECOND);

        cx.update(|_, cx| state.update(cx, |state, cx| state.select_keyframe(1, 2, cx)));
        assert_eq!(
            state.read_with(cx, |state, _| state.selection()),
            (Some(1), Some(2))
        );

        // 越界选择被清空。
        cx.update(|_, cx| state.update(cx, |state, cx| state.select_keyframe(9, 0, cx)));
        assert_eq!(
            state.read_with(cx, |state, _| state.selection()),
            (None, None)
        );

        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.select_keyframe(1, 1, cx);
                state.remove_selected_keyframe(cx);
            })
        });
        let count = state.read_with(cx, |state, _| state.timeline().track(1).unwrap().len());
        assert_eq!(count, 2);
        assert!(
            events.borrow().iter().any(|event| matches!(
                event,
                TimelineEvent::KeyframeRemoved { track: 1, index: 1 }
            ))
        );
    }

    /// 纵向拖拽关键帧改值：按下不跳变、时间同步吸附、释放清空拖拽态。
    #[rgpui::test]
    fn vertical_drag_updates_keyframe_value(cx: &mut crate::TestAppContext) {
        let (state, events, mut cx) = setup(cx);

        let (before, grabbed, moved_value, moved_time) = cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                state.bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(600.0), px(200.0)));
                let track = state.timeline().track(0).expect("轨道存在");
                let range = track.value_range();
                let row_top = RULER_HEIGHT.as_f32();
                let keyframe_y = row_top + value_to_y(200.0, range, TRACK_HEIGHT);
                // 在关键帧中心下方 2px 按下（命中只看时间），抓取偏移应被记住。
                let grab = point(px(256.0), px(keyframe_y + 2.0));
                state.on_pointer_down(grab, 1, window, cx);
                let before = state.timeline().track(0).expect("轨道存在").keyframes[1].value;
                // 指针不动（仍在按下点）：值应原样保持，不因抓取偏移跳变。
                state.on_pointer_move(grab, cx);
                let grabbed = state.timeline().track(0).expect("轨道存在").keyframes[1].value;
                state.on_pointer_up(cx);
                // 再抓住关键帧中心（偏移为 0），拖到行底。
                state.on_pointer_down(point(px(256.0), px(keyframe_y)), 1, window, cx);
                state.on_pointer_move(
                    point(
                        px(96.0 + 2.5 * DEFAULT_PX_PER_SECOND),
                        px(row_top + TRACK_HEIGHT.as_f32() - VALUE_PADDING),
                    ),
                    cx,
                );
                let track = state.timeline().track(0).expect("轨道存在");
                (
                    before,
                    grabbed,
                    track.keyframes[1].value,
                    track.keyframes[1].time,
                )
            })
        });

        assert_eq!(before, 200.0);
        assert!(
            (grabbed - 200.0).abs() < 1e-3,
            "抓取偏移应抵消，实际 {grabbed}"
        );
        // 第一轨值域 0..200 → 冻结为 (-20, 220)，拖到底得到 -20。
        assert!(
            (moved_value + 20.0).abs() < 1e-3,
            "拖到底得到 {moved_value}"
        );
        assert!((moved_time - 2.5).abs() < 1e-4);

        let events = events.borrow();
        assert!(events.iter().any(|event| matches!(
            event,
            TimelineEvent::KeyframeMoved { index: 1, time, value, .. }
                if (*time - 2.5).abs() < 1e-4 && (*value - moved_value).abs() < 1e-4
        )));

        cx.update(|_, cx| state.update(cx, |state, cx| state.on_pointer_up(cx)));
        assert!(state.read_with(cx, |state, _| state.dragging.is_none()));
    }

    /// 命中测试按局部坐标区分标尺 / 关键帧 / 轨道空白 / 标签列。
    #[rgpui::test]
    fn hit_test_maps_local_coordinates(cx: &mut crate::TestAppContext) {
        let (state, _events, mut cx) = setup(cx);

        // 标尺（y < 24）。
        assert_eq!(
            state.read_with(cx, |state, _| state.hit_test(point(px(200.0), px(10.0)))),
            TimelineHit::Ruler
        );
        // 标签列（x < LABEL_WIDTH=96）。
        assert_eq!(
            state.read_with(cx, |state, _| state.hit_test(point(px(10.0), px(40.0)))),
            TimelineHit::None
        );
        // 第一条轨道上的关键帧：t=2s、80px/s、scroll=0 → x=96+160=256。
        assert_eq!(
            state.read_with(cx, |state, _| state.hit_test(point(px(256.0), px(30.0)))),
            TimelineHit::Keyframe { track: 0, index: 1 }
        );
        // 第一条轨道的空白处（t≈0.5s → x=96+40）。
        assert_eq!(
            state.read_with(cx, |state, _| state.hit_test(point(px(136.0), px(30.0)))),
            TimelineHit::Track { track: 0 }
        );
        // 轨道之外。
        assert_eq!(
            state.read_with(cx, |state, _| state.hit_test(point(px(200.0), px(400.0)))),
            TimelineHit::None
        );
    }

    /// 模型层移动轨道：目标下标越界夹取，原地不动返回 `false`。
    #[test]
    fn timeline_move_track_clamps_destination() {
        let mut timeline = demo_timeline();
        assert!(timeline.move_track(0, 99));
        assert_eq!(timeline.track(0).unwrap().name.as_ref(), "透明度");
        assert!(!timeline.move_track(9, 0));
        assert!(!timeline.move_track(0, 0));
    }

    /// 状态层移动轨道：选中项与拖动中的关键帧跟随，并抛出 `TrackMoved`。
    #[rgpui::test]
    fn move_track_reorders_and_keeps_selection(cx: &mut crate::TestAppContext) {
        let (state, events, mut cx) = setup(cx);

        let moved = cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.select_keyframe(1, 0, cx);
                state.move_track(1, 0, cx)
            })
        });
        assert!(moved);
        let names: Vec<String> = state.read_with(cx, |state, _| {
            state
                .timeline()
                .tracks()
                .iter()
                .map(|track| track.name.to_string())
                .collect()
        });
        assert_eq!(names, vec!["透明度".to_string(), "位置".to_string()]);
        assert_eq!(
            state.read_with(cx, |state, _| state.selection()),
            (Some(0), Some(0))
        );
        assert!(
            events
                .borrow()
                .iter()
                .any(|event| matches!(event, TimelineEvent::TrackMoved { from: 1, to: 0 }))
        );

        assert!(!cx.update(|_, cx| state.update(cx, |state, cx| state.move_track(5, 0, cx))));
        assert!(!cx.update(|_, cx| state.update(cx, |state, cx| state.move_track(0, 0, cx))));
    }

    /// 纵向滚动：按内容坐标命中、夹取到可滚动范围、把轨道滚入视野。
    #[rgpui::test]
    fn vertical_scroll_maps_rows_and_clamps(cx: &mut crate::TestAppContext) {
        let (state, _events, mut cx) = setup(cx);

        // 几何断言都在同一次更新内完成：绘制会把 `bounds` 写回真实窗口尺寸。
        let (max, clamped, keyframe, track_row, top, bottom, out_of_range, wide_max, wide) = cx
            .update(|_, cx| {
                state.update(cx, |state, cx| {
                    // 视口高度 60：轨道区 36px（1.5 行），总高 48 → 最多滚 12px。
                    state.bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(400.0), px(60.0)));
                    let max = state.max_vscroll();
                    state.set_vscroll(100.0, cx);
                    let clamped = state.vscroll();
                    // 滚动后第一行上移：y=30 仍在第一行，y=40 落到第二行。
                    let keyframe = state.hit_test(point(px(256.0), px(30.0)));
                    let track_row = state.hit_test(point(px(136.0), px(40.0)));
                    // 滚入视野：第一行回顶、第二行露底。
                    state.scroll_to_track(0, cx);
                    let top = state.vscroll();
                    state.scroll_to_track(1, cx);
                    let bottom = state.vscroll();
                    // 越界下标不动。
                    state.scroll_to_track(9, cx);
                    let out_of_range = state.vscroll();
                    // 视口足够高时不可滚动。
                    state.bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(400.0), px(600.0)));
                    let wide_max = state.max_vscroll();
                    state.set_vscroll(30.0, cx);
                    let wide = state.vscroll();
                    (
                        max,
                        clamped,
                        keyframe,
                        track_row,
                        top,
                        bottom,
                        out_of_range,
                        wide_max,
                        wide,
                    )
                })
            });

        assert_eq!(max, 12.0);
        assert_eq!(clamped, 12.0);
        assert_eq!(keyframe, TimelineHit::Keyframe { track: 0, index: 1 });
        assert_eq!(track_row, TimelineHit::Track { track: 1 });
        assert_eq!(top, 0.0);
        assert_eq!(bottom, 12.0);
        assert_eq!(out_of_range, 12.0);
        assert_eq!(wide_max, 0.0);
        assert_eq!(wide, 0.0);
    }

    /// 轨道折叠状态与行高：默认展开，`set_collapsed` 只在变化时返回 `true`。
    #[test]
    fn track_collapse_changes_row_height() {
        let mut track = Track::new("t");
        assert!(!track.collapsed);
        assert_eq!(track.row_height(), TRACK_HEIGHT);

        // `with_collapsed` 是构造期设置。
        let folded = Track::new("f").with_collapsed(true);
        assert!(folded.collapsed);
        assert_eq!(folded.row_height(), COLLAPSED_TRACK_HEIGHT);

        // 首次切换生效，同样状态不算变化。
        assert!(track.set_collapsed(true));
        assert_eq!(track.row_height(), COLLAPSED_TRACK_HEIGHT);
        assert!(!track.set_collapsed(true));
        assert!(track.set_collapsed(false));
        assert_eq!(track.row_height(), TRACK_HEIGHT);
    }

    /// 折叠轨道：行高缩小、后续轨道上移、折叠行不命中关键帧，并抛出 `TrackCollapsed`。
    #[rgpui::test]
    fn collapse_track_shifts_rows_and_hits(cx: &mut crate::TestAppContext) {
        let (state, events, mut cx) = setup(cx);

        let (before, collapsed, tops, height, folded_hit, next_hit, max, expanded_tops, out) = cx
            .update(|_, cx| {
                state.update(cx, |state, cx| {
                    state
                        .timeline_mut()
                        .add_track(Track::with_keyframes("缩放", vec![Keyframe::new(0.0, 1.0)]));
                    state.bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(400.0), px(60.0)));
                    let before = state.track_top(1);
                    let collapsed = state.toggle_track_collapsed(0, cx);
                    let tops = (state.track_top(0), state.track_top(1), state.track_top(2));
                    let height = state.tracks_height();
                    // 折叠行压在首条轨道的第二个关键帧（x=256）上，仍算轨道空白。
                    let folded_hit = state.hit_test(point(px(256.0), px(30.0)));
                    // 折叠后第二行从内容坐标 12 开始：y=24+13 落在第二行。
                    let next_hit = state.hit_test(point(px(136.0), px(37.0)));
                    let max = state.max_vscroll();
                    state.toggle_track_collapsed(0, cx);
                    let expanded_tops = (state.track_top(1), state.tracks_height());
                    let out = state.toggle_track_collapsed(9, cx);
                    (
                        before,
                        collapsed,
                        tops,
                        height,
                        folded_hit,
                        next_hit,
                        max,
                        expanded_tops,
                        out,
                    )
                })
            });

        assert_eq!(before, 24.0);
        assert!(collapsed);
        assert_eq!(tops, (0.0, 12.0, 36.0));
        assert_eq!(height, 60.0);
        assert_eq!(folded_hit, TimelineHit::Track { track: 0 });
        assert_eq!(next_hit, TimelineHit::Track { track: 1 });
        // 可视 36px，总高 60 → 最多滚 24px。
        assert_eq!(max, 24.0);
        assert_eq!(expanded_tops, (24.0, 72.0));
        assert!(!out);
        assert!(events.borrow().iter().any(|event| matches!(
            event,
            TimelineEvent::TrackCollapsed {
                index: 0,
                collapsed: true
            }
        )));
    }
}
