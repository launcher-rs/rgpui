//! FPS 监控 HUD —— 实时显示帧率（可选 CPU / 内存）。
//!
//! [`FpsHud`] 是一个 `Render` 视图：把它作为窗口根视图的子节点插入即可。它默认
//! **持续采样**（每帧记录一次帧间隔并经指数移动平均平滑，同时请求下一帧以保证
//! HUD 实时跳动）；如果只想在应用自然重绘时采样，用 [`FpsHud::continuous(false)`]。
//!
//! CPU / 内存两项 rgpui 不内置平台采样：默认不显示，需要时由调用方经
//! [`FpsHudState::update_cpu`] / [`FpsHudState::update_memory`] 喂入，再用
//! [`FpsHud::show_cpu`] / [`FpsHud::show_memory`] 打开对应行。

use std::time::Instant;

use crate::{
    ActiveTheme as _, Context, Entity, IntoElement, ParentElement, Pixels, Point, Render, Styled,
    StyledExt as _, Window, div, h_flex, prelude::FluentBuilder as _, px,
};

/// 帧率历史最多保留的条数（约 1 秒 @60fps）。
const FPS_HISTORY_LIMIT: usize = 60;

/// 指数移动平均系数：越小越平滑、越迟钝。
const FPS_SMOOTHING: f64 = 0.1;

/// 瞬时帧率的防御性上限（避免极小间隔产生天文数字）。
const FPS_CEILING: f64 = 1000.0;

/// FPS 监控状态。
pub struct FpsHudState {
    /// 当前帧率（已平滑）。
    pub fps: f64,
    /// 帧时间（毫秒）。
    pub frame_time_ms: f64,
    /// CPU 使用率（百分比）。需调用方经 [`update_cpu`](Self::update_cpu) 喂入。
    pub cpu_usage: f64,
    /// 内存占用（MB）。需调用方经 [`update_memory`](Self::update_memory) 喂入。
    pub memory_mb: f64,
    /// 是否可见。
    pub visible: bool,
    /// 是否显示 CPU 行。
    pub show_cpu: bool,
    /// 是否显示内存行。
    pub show_memory: bool,
    /// 最近的瞬时帧率（最多 [`FPS_HISTORY_LIMIT`] 条）。
    pub fps_history: Vec<f64>,
    /// 上一帧时间戳（内部用于计算帧间隔）。
    last_frame: Option<Instant>,
}

impl Default for FpsHudState {
    fn default() -> Self {
        Self::new()
    }
}

impl FpsHudState {
    /// 创建可见、仅显示帧率的状态。
    pub fn new() -> Self {
        Self {
            fps: 0.0,
            frame_time_ms: 0.0,
            cpu_usage: 0.0,
            memory_mb: 0.0,
            visible: true,
            show_cpu: false,
            show_memory: false,
            fps_history: Vec::new(),
            last_frame: None,
        }
    }

    /// 直接写入帧率（跳过采样），同时更新帧时间与历史。
    pub fn update_fps(&mut self, fps: f64) {
        self.fps = fps;
        self.frame_time_ms = if fps > 0.0 { 1000.0 / fps } else { 0.0 };
        self.push_history(fps);
    }

    /// 记录一帧：按与上一帧的间隔算出瞬时帧率，平滑后写入。
    ///
    /// 第一帧只记录时间戳、不产生帧率。
    pub fn record_frame(&mut self, now: Instant) {
        let Some(previous) = self.last_frame.replace(now) else {
            return;
        };
        let seconds = now.saturating_duration_since(previous).as_secs_f64();
        if seconds <= 0.0 {
            return;
        }
        let instant = (1.0 / seconds).min(FPS_CEILING);
        // 第一帧之后用指数移动平均；否则直接采用，避免从 0 慢慢爬。
        self.fps = if self.fps > 0.0 {
            self.fps * (1.0 - FPS_SMOOTHING) + instant * FPS_SMOOTHING
        } else {
            instant
        };
        self.frame_time_ms = 1000.0 / self.fps;
        self.push_history(instant);
    }

    /// 清空上一帧时间戳，用于暂停后重新计时。
    pub fn reset_sampling(&mut self) {
        self.last_frame = None;
    }

    /// 更新 CPU 使用率。
    pub fn update_cpu(&mut self, usage: f64) {
        self.cpu_usage = usage;
    }

    /// 更新内存占用（MB）。
    pub fn update_memory(&mut self, mb: f64) {
        self.memory_mb = mb;
    }

    /// 切换可见性。
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            // 隐藏后不再计时，避免重新显示时算出一个巨大的帧间隔。
            self.reset_sampling();
        }
    }

    /// 返回历史平均帧率（无历史时为 0）。
    pub fn average_fps(&self) -> f64 {
        if self.fps_history.is_empty() {
            return 0.0;
        }
        self.fps_history.iter().sum::<f64>() / self.fps_history.len() as f64
    }

    /// 追加一条历史并裁剪到上限。
    fn push_history(&mut self, fps: f64) {
        self.fps_history.push(fps);
        if self.fps_history.len() > FPS_HISTORY_LIMIT {
            self.fps_history.remove(0);
        }
    }
}

/// FPS 监控 HUD 视图。
pub struct FpsHud {
    state: Entity<FpsHudState>,
    /// 自定义位置（相对窗口左上角）；`None` 表示默认贴右上角。
    position: Option<Point<Pixels>>,
    /// 是否显示 CPU 行（与状态里的开关取「或」）。
    show_cpu: bool,
    /// 是否显示内存行（与状态里的开关取「或」）。
    show_memory: bool,
    /// 是否持续采样并请求下一帧（默认 `true`）。
    continuous: bool,
}

impl FpsHud {
    /// 创建 FPS HUD。
    pub fn new(state: Entity<FpsHudState>) -> Self {
        Self {
            state,
            position: None,
            show_cpu: false,
            show_memory: false,
            continuous: true,
        }
    }

    /// 设置位置（相对窗口左上角的偏移）。不设置时贴右上角。
    pub fn position(mut self, position: Point<Pixels>) -> Self {
        self.position = Some(position);
        self
    }

    /// 显示/隐藏 CPU 行。
    pub fn show_cpu(mut self, show: bool) -> Self {
        self.show_cpu = show;
        self
    }

    /// 显示/隐藏内存行。
    pub fn show_memory(mut self, show: bool) -> Self {
        self.show_memory = show;
        self
    }

    /// 是否持续采样并请求下一帧，默认 `true`。
    ///
    /// 开启时 HUD 会一直以窗口刷新率重绘（这正是它能实时跳动的原因），空闲时也
    /// 不会真正停下来；不想要这份开销时设为 `false`，改为只在应用自然重绘时采样。
    pub fn continuous(mut self, continuous: bool) -> Self {
        self.continuous = continuous;
        self
    }
}

impl Render for FpsHud {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.state.read(cx).visible {
            return div();
        }

        if self.continuous {
            // 采样本帧间隔，并安排下一帧继续采样（等价于持续的动画帧请求）。
            self.state
                .update(cx, |state, _| state.record_frame(Instant::now()));
            window.request_animation_frame();
        }

        let (fps, frame_time_ms, cpu_usage, memory_mb, show_cpu, show_memory) = {
            let state = self.state.read(cx);
            (
                state.fps,
                state.frame_time_ms,
                state.cpu_usage,
                state.memory_mb,
                state.show_cpu || self.show_cpu,
                state.show_memory || self.show_memory,
            )
        };

        let theme = cx.theme();
        let background = theme.popover;
        let border = theme.border;
        let radius = theme.radius;
        let label_color = theme.muted_foreground;
        let value_color = theme.popover_foreground;

        let fps_color = if fps >= 55.0 {
            crate::green_400()
        } else if fps >= 30.0 {
            crate::yellow_400()
        } else {
            crate::red_400()
        };
        let cpu_color = if cpu_usage < 50.0 {
            crate::green_400()
        } else if cpu_usage < 80.0 {
            crate::yellow_400()
        } else {
            crate::red_400()
        };

        let position = self.position;
        let (offset_x, offset_y) = match position {
            Some(position) => (position.x, position.y),
            None => (px(8.0), px(8.0)),
        };

        div()
            .absolute()
            .top(offset_y)
            .when(position.is_some(), |this| this.left(offset_x))
            .when(position.is_none(), |this| this.right(offset_x))
            .bg(background)
            .border_1()
            .border_color(border)
            .rounded(radius)
            .p(px(8.0))
            .shadow_lg()
            .child(
                h_flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .items_center()
                            .child(div().text_xs().text_color(label_color).child("FPS"))
                            .child(
                                div()
                                    .text_sm()
                                    .font_semibold()
                                    .text_color(fps_color)
                                    .child(format!("{fps:.1}")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(label_color)
                                    .child(format!("({frame_time_ms:.1}ms)")),
                            ),
                    )
                    .when(show_cpu, |this| {
                        this.child(
                            h_flex()
                                .gap(px(8.0))
                                .items_center()
                                .child(div().text_xs().text_color(label_color).child("CPU"))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_semibold()
                                        .text_color(cpu_color)
                                        .child(format!("{cpu_usage:.1}%")),
                                ),
                        )
                    })
                    .when(show_memory, |this| {
                        this.child(
                            h_flex()
                                .gap(px(8.0))
                                .items_center()
                                .child(div().text_xs().text_color(label_color).child("MEM"))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_semibold()
                                        .text_color(value_color)
                                        .child(format!("{memory_mb:.1} MB")),
                                ),
                        )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{FPS_HISTORY_LIMIT, FpsHud, FpsHudState};
    use crate::{AppContext as _, Point, px};

    /// 采样：第一帧只记时间戳；之后按间隔算出帧率并平滑。
    #[test]
    fn test_record_frame_computes_fps() {
        let mut state = FpsHudState::new();
        let start = Instant::now();

        state.record_frame(start);
        assert_eq!(state.fps, 0.0, "首帧不产生帧率");

        // 60fps：16.67ms 后。
        state.record_frame(start + Duration::from_micros(16_667));
        assert!(
            (state.fps - 60.0).abs() < 0.5,
            "首帧后直接采用瞬时帧率，得到 {}",
            state.fps
        );
        assert!((state.frame_time_ms - 16.667).abs() < 0.5);

        // 仍是 16.67ms 间隔：EMA 保持接近 60。
        state.record_frame(start + Duration::from_micros(33_334));
        assert!((state.fps - 60.0).abs() < 1.0, "得到 {}", state.fps);
        assert_eq!(state.fps_history.len(), 2);
    }

    /// 历史裁剪到上限。
    #[test]
    fn test_history_is_capped() {
        let mut state = FpsHudState::new();
        for i in 0..(FPS_HISTORY_LIMIT + 10) {
            state.update_fps(60.0 + i as f64);
        }
        assert_eq!(state.fps_history.len(), FPS_HISTORY_LIMIT);
        assert!(state.average_fps() > 60.0);
    }

    /// 隐藏后重置采样，避免重新显示时算出巨大帧间隔。
    #[test]
    fn test_toggle_resets_sampling() {
        let mut state = FpsHudState::new();
        assert!(state.visible);
        state.record_frame(Instant::now());

        state.toggle();
        assert!(!state.visible);

        // 隐藏后即使隔很久再记录，也不会产生帧率（时间戳已清空）。
        state.record_frame(Instant::now() + Duration::from_secs(10));
        assert_eq!(state.fps, 0.0);
    }

    /// 构建器写入位置与显示开关。
    #[rgpui::test]
    fn test_fps_hud_builder(cx: &mut rgpui::TestAppContext) {
        cx.update(|cx| {
            let state = cx.new(|_| FpsHudState::new());
            let hud = FpsHud::new(state)
                .position(Point::new(px(10.0), px(20.0)))
                .show_cpu(true)
                .show_memory(true)
                .continuous(false);

            assert!(hud.show_cpu);
            assert!(hud.show_memory);
            assert!(!hud.continuous);
            assert_eq!(hud.position, Some(Point::new(px(10.0), px(20.0))));
        });
    }
}
