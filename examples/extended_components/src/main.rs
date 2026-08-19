//! 扩展组件示例：展示并入 rgpui 核心的扩展组件库（原 rgpui-ui）。
//!
//! 覆盖：
//! - `SplitPane`（分隔面板）
//! - `TagInput`（标签输入）、`TypeWriter`（打字机动画）
//! - `Waveform`（音频波形）、`Sparkline`（迷你走势图）、`SVGRenderer`（SVG 路径）
//! - 动画原语：`Spring`（值驱动弹簧）、`AnimationPreset`（便捷预设）
//!
//! 运行：
//!
//! ```text
//! cargo run -p extended_components
//! ```

// 在 wasm 目标上禁用 main 函数入口
#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::components::{
    SVGRenderer, Sparkline, SplitPane, SplitPaneState, TagInput, TagInputState, TypeWriter,
    TypeWriterState, Waveform,
};
use rgpui::{
    App, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
    WindowBackgroundAppearance, WindowOptions, div, hsla, prelude::*, px, rgb, v_flex,
};
use rgpui_platform::application;

/// 应用根视图：左侧为交互组件，右侧为数据可视化组件。
struct ShowcaseApp {
    /// 分隔面板状态。
    split_pane_state: Entity<SplitPaneState>,
    /// 标签输入状态。
    tag_input_state: Entity<TagInputState>,
    /// 打字机状态。
    type_writer_state: Entity<TypeWriterState>,
}

impl ShowcaseApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 初始化各组件状态实体。
        let split_pane_state = cx.new(SplitPaneState::new);
        let tag_input_state =
            cx.new(|cx| TagInputState::with_tags(window, cx, vec!["rgpui", "扩展组件"]));
        let type_writer_state = cx.new(|_cx| TypeWriterState::new("rgpui 扩展组件打字机动画演示"));
        // 启动打字动画（逐字显示）。
        type_writer_state.update(cx, |state, cx| state.start(cx));

        Self {
            split_pane_state,
            tag_input_state,
            type_writer_state,
        }
    }
}

impl Render for ShowcaseApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // 分隔面板：左为交互组件，右为数据可视化。
        SplitPane::horizontal(self.split_pane_state.clone())
            .show_collapse_buttons(true)
            .first(interactive_pane(
                self.tag_input_state.clone(),
                self.type_writer_state.clone(),
            ))
            .second(data_pane())
    }
}

/// 交互面板：标签输入 + 打字机动画。
fn interactive_pane(
    tag_input_state: Entity<TagInputState>,
    type_writer_state: Entity<TypeWriterState>,
) -> impl IntoElement {
    v_flex()
        .id("interactive")
        .gap(px(8.0))
        .p(px(12.0))
        .child(
            div()
                .child("标签输入（TagInput）")
                .text_color(rgb(0x9ca3af)),
        )
        .child(TagInput::new(tag_input_state).suggestions(vec!["动画", "手势", "图表", "特效"]))
        .child(div().h(px(16.0)))
        .child(
            div()
                .child("打字机（TypeWriter）")
                .text_color(rgb(0x9ca3af)),
        )
        .child(TypeWriter::new("demo-typewriter", type_writer_state))
}

/// 数据面板：波形 + 走势图 + SVG。
fn data_pane() -> impl IntoElement {
    v_flex()
        .id("data")
        .gap(px(8.0))
        .p(px(12.0))
        .child(
            div()
                .child("音频波形（Waveform）")
                .text_color(rgb(0x9ca3af)),
        )
        .child(waveform())
        .child(
            div()
                .child("迷你走势图（Sparkline）")
                .text_color(rgb(0x9ca3af)),
        )
        .child(sparklines())
        .child(
            div()
                .child("SVG 路径（SVGRenderer）")
                .text_color(rgb(0x9ca3af)),
        )
        .child(svg_path())
}

/// 波形数据示例。
fn waveform() -> impl IntoElement {
    let mut data = Vec::new();
    for i in 0..64 {
        let v = ((i as f32 / 8.0) * std::f32::consts::PI).sin();
        data.push(v * 0.5 + 0.5);
    }
    Waveform::new()
        .data(&data)
        .playback_position(0.35)
        .color(hsla(0.62, 0.1, 0.5, 1.0))
        .active_color(hsla(0.6, 0.8, 0.55, 1.0))
}

/// 三种走势图变体。
fn sparklines() -> impl IntoElement {
    let line_data = vec![3.0, 7.0, 5.0, 9.0, 6.0, 12.0, 8.0, 15.0, 11.0, 18.0];
    let bar_data = vec![4.0, 6.0, 8.0, 5.0, 9.0, 7.0, 10.0];
    let area_data = vec![2.0, 5.0, 4.0, 8.0, 6.0, 9.0, 7.0, 12.0];

    v_flex()
        .id("sparklines")
        .gap(px(8.0))
        .child(Sparkline::line(line_data).width(px(180.0)).height(px(40.0)))
        .child(Sparkline::bar(bar_data).width(px(180.0)).height(px(40.0)))
        .child(Sparkline::area(area_data).width(px(180.0)).height(px(40.0)))
}

/// 简易 SVG 路径（心形）。
fn svg_path() -> impl IntoElement {
    SVGRenderer::new()
        .path_data(
            "M100,40 C100,20 70,10 55,30 C40,10 10,20 10,40 \
             C10,70 55,100 55,100 C55,100 100,70 100,40 Z",
        )
        .view_box(0.0, 0.0, 110.0, 110.0)
        .fill(hsla(0.0, 0.75, 0.55, 1.0))
        .w(px(80.0))
        .h(px(80.0))
}

fn main() {
    application().run(|cx: &mut App| {
        // 初始化主题与输入子系统（TagInput 内嵌的 Input 依赖其按键绑定）。
        rgpui::theme::init(cx);
        rgpui::input_ui::init(cx);

        let window_options = WindowOptions {
            window_background: WindowBackgroundAppearance::Opaque,
            titlebar: Some(rgpui::TitlebarOptions {
                title: Some("扩展组件示例".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let _ = cx.open_window(window_options, |window, cx| {
            cx.new(|cx| ShowcaseApp::new(window, cx))
        });
    });
}
