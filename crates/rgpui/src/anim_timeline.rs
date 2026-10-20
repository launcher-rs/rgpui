//! 动画时间线 —— 多轨关键帧编辑器（标尺 + 轨道 + 关键帧 + 播放头）。
//!
//! 面向动效 / 音视频 / 关键帧动画的**时间线编辑**场景：时间标尺、多条轨道、每条
//! 轨道若干关键帧、可拖拽的播放头。缩略的纯数据模型（[`Timeline`] / [`Track`] /
//! [`Keyframe`]）负责关键帧存储与求值（线性 / 保持 / 平滑），控件
//! （[`TimelineState`] / [`TimelineView`]）负责标尺刻度、命中测试、拖拽与播放头。
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

use crate::{
    ActiveTheme as _, AnyElement, App, Bounds, Context, ElementExt as _, Entity, EventEmitter,
    FocusHandle, Focusable, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point, Render,
    RenderOnce, ScrollDelta, ScrollWheelEvent, SharedString, Styled as _, Window, div, point,
    prelude::FluentBuilder as _, px,
};

use crate::animation::KeyframeAnimation;

/// 标尺高度（像素）。
pub const RULER_HEIGHT: Pixels = px(24.0);
/// 每条轨道行高（像素）。
pub const TRACK_HEIGHT: Pixels = px(24.0);
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
}

impl Interpolation {
    /// 把局部进度 `t`（0~1）映射为插值系数。
    pub fn ease(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Interpolation::Linear => t,
            Interpolation::Hold => 0.0,
            Interpolation::Smooth => t * t * (3.0 - 2.0 * t),
        }
    }
}

/// 一条轨道：名称 + 关键帧（按时间升序、时间唯一）+ 插值方式。
#[derive(Clone, Debug)]
pub struct Track {
    /// 轨道名称。
    pub name: SharedString,
    /// 关键帧列表。
    pub keyframes: Vec<Keyframe>,
    /// 插值方式。
    pub interpolation: Interpolation,
}

impl Track {
    /// 创建空轨道。
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            keyframes: Vec::new(),
            interpolation: Interpolation::Linear,
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
    /// 关键帧被拖到新时间（`index` 为移动后的下标）。
    KeyframeMoved {
        /// 轨道下标。
        track: usize,
        /// 移动后的关键帧下标。
        index: usize,
        /// 新时间（秒，已吸附）。
        time: f32,
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
        let total = TRACK_HEIGHT.as_f32() * self.timeline.len() as f32;
        (total - visible).max(0.0)
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

    /// 把某条轨道滚入可视区（已可见时不动）。
    pub fn scroll_to_track(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.timeline.len() {
            return;
        }
        let visible = (self.bounds.size.height - RULER_HEIGHT).as_f32().max(0.0);
        if visible <= 0.0 {
            return;
        }
        let height = TRACK_HEIGHT.as_f32();
        let top = height * index as f32;
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
        // 轨道区随垂直滚动偏移平移：先换算成内容坐标再取行号。
        let y = (local.y - RULER_HEIGHT).as_f32() + self.vscroll;
        let row = (y / TRACK_HEIGHT.as_f32()).floor();
        if !row.is_finite() || row < 0.0 {
            return TimelineHit::None;
        }
        let track = row as usize;
        let Some(timeline_track) = self.timeline.track(track) else {
            return TimelineHit::None;
        };
        let time = x_to_time(x.as_f32(), self.px_per_second, self.scroll);
        let tolerance = KEYFRAME_HIT_RADIUS.as_f32() / self.px_per_second;
        match timeline_track.keyframe_index_at(time, tolerance) {
            Some(index) => TimelineHit::Keyframe { track, index },
            None => TimelineHit::Track { track },
        }
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
            TimelineHit::None => self.clear_selection(cx),
        }
    }

    /// 指针移动：拖拽播放头 / 拖拽关键帧。
    fn on_pointer_move(&mut self, local: Point<Pixels>, cx: &mut Context<Self>) {
        let x = (local.x - LABEL_WIDTH).as_f32();
        if self.scrubbing {
            let time = x_to_time(x, self.px_per_second, self.scroll);
            self.set_time(time, cx);
        } else if let Some((track, index)) = self.dragging {
            let time = self.snapped(x_to_time(x, self.px_per_second, self.scroll).max(0.0));
            let moved = self
                .timeline
                .track_mut(track)
                .and_then(|t| t.move_keyframe(index, time));
            if let Some(new_index) = moved {
                self.selected_track = Some(track);
                self.selected_keyframe = Some(new_index);
                self.dragging = Some((track, new_index));
                cx.emit(TimelineEvent::KeyframeMoved {
                    track,
                    index: new_index,
                    time,
                });
                cx.notify();
            }
        }
    }

    /// 指针释放：结束拖拽。
    fn on_pointer_up(&mut self, cx: &mut Context<Self>) {
        if self.scrubbing || self.dragging.is_some() {
            self.scrubbing = false;
            self.dragging = None;
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

        let mut markers: Vec<AnyElement> = Vec::with_capacity(track.len());
        for (kf_index, keyframe) in track.keyframes.iter().enumerate() {
            let x = time_to_x(keyframe.time, self.px_per_second, self.scroll);
            let is_selected =
                self.selected_track == Some(index) && self.selected_keyframe == Some(kf_index);
            markers.push(
                div()
                    .absolute()
                    .left(px(x - KEYFRAME_SIZE.as_f32() * 0.5))
                    .top(px((TRACK_HEIGHT.as_f32() - KEYFRAME_SIZE.as_f32()) * 0.5))
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

        div()
            .absolute()
            .left(LABEL_WIDTH)
            .top(TRACK_HEIGHT * index as f32 - px(self.vscroll))
            .right_0()
            .h(TRACK_HEIGHT)
            .overflow_hidden()
            .border_b_1()
            .border_color(border)
            .when(selected, |this| this.bg(selected_bg))
            .when(is_empty, |this| {
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let border = theme.border;
        let accent = theme.primary;
        let muted_foreground = theme.muted_foreground;
        let selected_bg = theme.secondary;
        let background = theme.background;

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
                div()
                    .absolute()
                    .left_0()
                    .top(TRACK_HEIGHT * index as f32 - px(self.vscroll))
                    .w(LABEL_WIDTH)
                    .h(TRACK_HEIGHT)
                    .px_2()
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .border_b_1()
                    .border_color(border)
                    .text_xs()
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
        DEFAULT_PX_PER_SECOND, Interpolation, Keyframe, MAX_PX_PER_SECOND, MIN_PX_PER_SECOND,
        Timeline, TimelineEvent, TimelineHit, TimelineState, TimelineView, Track,
        clamp_px_per_second, format_time, ruler_step, snap_time, time_to_x, x_to_time,
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
}
