//! 输入延迟直方图示例（`input-latency-histogram` feature 门控）。
//!
//! 展示如何读取窗口的输入延迟统计（`window.input_latency_snapshot()`）：
//! - 输入事件到帧呈现的延迟直方图（纳秒，界面以毫秒展示）
//! - 每帧合并的输入事件数直方图
//! - 帧绘制期间到达而被丢弃的事件计数
//!
//! 窗口会持续请求动画帧，让统计直方图持续累积数据。
//!
//! 运行：
//!
//! ```text
//! cargo run -p rgpui --example input_latency --features input-latency-histogram
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    ActiveTheme, App, AppContext as _, Bounds, Context, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement as _, Styled, StyledExt, Window,
    WindowBounds, WindowOptions, div, h_flex, px, size, v_flex,
};
use rgpui_platform::application;

/// 输入延迟直方图示例根视图。
///
/// 注意：延迟统计只记录“触发了界面更新”的输入事件（未引起重绘的
/// 事件不会产生输入→帧呈现的延迟样本）。因此下面的演示区通过
/// 点击计数与悬停变色保证鼠标交互每次都触发更新，从而能被统计到。
struct InputLatencyExample {
    /// 点击计数：点击窗口任意位置累加。
    clicks: usize,
    /// 演示区悬停状态：鼠标移入/移出改变外观。
    demo_hovered: bool,
}

impl Render for InputLatencyExample {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 持续请求动画帧，让输入延迟统计持续累积样本。
        window.request_animation_frame();

        // 读取当前窗口的输入延迟统计快照。
        let snapshot = window.input_latency_snapshot();
        let clicks = self.clicks;
        // 悬停高亮背景：提前算好，避免在 `Stateful<Div>` 上调用其不支持的 `.when()`。
        let demo_bg = if self.demo_hovered {
            rgpui::hsla(0.6, 0.5, 0.5, 0.25)
        } else {
            rgpui::hsla(0.0, 0.0, 0.0, 0.0)
        };

        v_flex()
            .id("input-latency-example")
            .size_full()
            .p(px(32.0))
            .gap(px(24.0))
            .on_click(cx.listener(|this, _, _, cx| {
                this.clicks += 1;
                cx.notify();
            }))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(div().text_2xl().font_semibold().child("输入延迟直方图"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                "提示：在下方演示区移动鼠标或点击窗口任意位置（已点击响应的事件才会被统计），观察延迟统计实时变化。",
                            ),
                    ),
            )
            .child(
                h_flex()
                    .gap(px(24.0))
                    .child(latency_panel(&snapshot.latency_histogram))
                    .child(events_panel(&snapshot.events_per_frame_histogram)),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "帧绘制期间到达而被丢弃的事件：{}",
                        snapshot.mid_draw_events_dropped
                    )),
            )
            .child(
                div()
                    .id("latency-interaction-demo")
                    .p(px(16.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(rgpui::hsla(0.0, 0.0, 0.5, 0.3))
                    .bg(demo_bg)
                    .on_hover(cx.listener(|this, hovered, _, cx| {
                        this.demo_hovered = *hovered;
                        cx.notify();
                    }))
                    .child(format!(
                        "交互演示区：悬停会高亮，累计点击 {clicks} 次（每次点击/悬停变化都会触发重绘并被统计）",
                    )),
            )
    }
}

/// 延迟直方图统计面板：输入→帧呈现延迟，纳秒换算为毫秒显示。
fn latency_panel(h: &hdrhistogram::Histogram<u64>) -> impl IntoElement {
    let to_ms = |v: u64| format!("{:.3} ms", v as f64 / 1_000_000.0);
    stat_panel(
        "输入→帧呈现延迟",
        vec![
            ("样本数", h.len().to_string()),
            ("均值", to_ms(h.mean() as u64)),
            ("最大值", to_ms(h.max())),
            ("P50", to_ms(h.value_at_quantile(0.5))),
            ("P90", to_ms(h.value_at_quantile(0.9))),
            ("P99", to_ms(h.value_at_quantile(0.99))),
        ],
    )
}

/// 每帧输入事件数直方图统计面板。
fn events_panel(h: &hdrhistogram::Histogram<u64>) -> impl IntoElement {
    stat_panel(
        "每帧合并输入事件数",
        vec![
            ("样本数", h.len().to_string()),
            ("均值", format!("{:.2} 个", h.mean())),
            ("最大值", format!("{} 个", h.max())),
            ("P50", format!("{} 个", h.value_at_quantile(0.5))),
            ("P90", format!("{} 个", h.value_at_quantile(0.9))),
            ("P99", format!("{} 个", h.value_at_quantile(0.99))),
        ],
    )
}

/// 渲染一个统计面板：标题 + 若干行名称/数值。
fn stat_panel(title: &str, rows: Vec<(&str, String)>) -> impl IntoElement {
    let title = title.to_string();
    v_flex()
        .gap(px(8.0))
        .p(px(16.0))
        .rounded(px(8.0))
        .border_1()
        .border_color(rgpui::hsla(0.0, 0.0, 0.5, 0.3))
        .child(div().text_base().font_medium().child(title))
        .children(
            rows.into_iter()
                .map(|(name, value)| stat_row(name.to_string(), value)),
        )
}

/// 渲染单行统计信息：名称 + 数值。
fn stat_row(name: String, value: String) -> impl IntoElement {
    div()
        .flex()
        .justify_between()
        .w_full()
        .child(
            div()
                .text_sm()
                .text_color(rgpui::hsla(0.0, 0.0, 0.5, 0.8))
                .child(name),
        )
        .child(div().text_sm().child(value))
}

fn run_example() {
    application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(720.0), px(480.0)),
                    cx,
                ))),
                ..Default::default()
            },
            |_window, cx| {
                cx.new(|_| InputLatencyExample {
                    clicks: 0,
                    demo_hovered: false,
                })
            },
        )
        .ok();

        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}
