//! FPS 监控 HUD —— 实时显示帧率、CPU、内存使用情况。

use crate::{
    Context, Entity, IntoElement, ParentElement, Render, Styled, StyledExt, Window, div, h_flex,
};

/// FPS 监控状态。
#[derive(Default)]
pub struct FpsHudState {
    /// 当前帧率。
    pub fps: f64,
    /// 帧时间（毫秒）。
    pub frame_time_ms: f64,
    /// CPU 使用率（百分比）。
    pub cpu_usage: f64,
    /// 内存使用（MB）。
    pub memory_mb: f64,
    /// 是否可见。
    pub visible: bool,
    /// 历史帧率记录。
    pub fps_history: Vec<f64>,
}

impl FpsHudState {
    /// 更新帧率。
    pub fn update_fps(&mut self, fps: f64) {
        self.fps = fps;
        self.frame_time_ms = if fps > 0.0 { 1000.0 / fps } else { 0.0 };
        self.fps_history.push(fps);
        if self.fps_history.len() > 60 {
            self.fps_history.remove(0);
        }
    }

    /// 更新 CPU 使用率。
    pub fn update_cpu(&mut self, usage: f64) {
        self.cpu_usage = usage;
    }

    /// 更新内存使用。
    pub fn update_memory(&mut self, mb: f64) {
        self.memory_mb = mb;
    }

    /// 切换可见性。
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// 获取平均帧率。
    pub fn average_fps(&self) -> f64 {
        if self.fps_history.is_empty() {
            return 0.0;
        }
        self.fps_history.iter().sum::<f64>() / self.fps_history.len() as f64
    }
}

/// FPS 监控 HUD 组件。
pub struct FpsHud {
    state: Entity<FpsHudState>,
}

impl FpsHud {
    /// 创建新的 FPS HUD。
    pub fn new(state: Entity<FpsHudState>) -> Self {
        Self { state }
    }
}

impl Render for FpsHud {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        if !state.visible {
            return div().into_element();
        }

        let fps_color = if state.fps >= 55.0 {
            crate::green_400()
        } else if state.fps >= 30.0 {
            crate::yellow_400()
        } else {
            crate::red_400()
        };

        let cpu_color = if state.cpu_usage < 50.0 {
            crate::green_400()
        } else if state.cpu_usage < 80.0 {
            crate::yellow_400()
        } else {
            crate::red_400()
        };

        div()
            .absolute()
            .top_2()
            .right_2()
            .bg(crate::gray_900())
            .rounded_md()
            .p_3()
            .shadow_lg()
            .child(
                h_flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().text_xs().text_color(crate::gray_400()).child("FPS"))
                            .child(
                                div()
                                    .text_sm()
                                    .font_semibold()
                                    .text_color(fps_color)
                                    .child(format!("{:.1}", state.fps)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(crate::gray_500())
                                    .child(format!("({:.1}ms)", state.frame_time_ms)),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().text_xs().text_color(crate::gray_400()).child("CPU"))
                            .child(
                                div()
                                    .text_sm()
                                    .font_semibold()
                                    .text_color(cpu_color)
                                    .child(format!("{:.1}%", state.cpu_usage)),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().text_xs().text_color(crate::gray_400()).child("MEM"))
                            .child(
                                div()
                                    .text_sm()
                                    .font_semibold()
                                    .text_color(crate::blue_400())
                                    .child(format!("{:.1} MB", state.memory_mb)),
                            ),
                    ),
            )
    }
}
