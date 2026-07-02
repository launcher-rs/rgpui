//! 选择器组件示例程序
//!
//! 本示例详细展示了选择器组件的用法，包括：
//! - 滑块（Slider）：单值滑块、范围滑块、带步长滑块

use rgpui::*;
use rgpui_component::{
    scroll::ScrollableElement,
    slider::{Slider, SliderState},
    *,
};
use rgpui_component_assets::Assets;

/// 示例应用主结构体
pub struct SelectExample;

impl SelectExample {
    /// 渲染滑块区域
    fn render_sliders(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        section("滑块 Slider")
            .child(description("滑块用于在一定范围内选择数值。"))
            .child(
                v_flex()
                    .gap_4()
                    .child(v_flex().gap_2().child(subsection_title("单值滑块")).child({
                        let state =
                            cx.new(|_| SliderState::new().min(0.0).max(100.0).default_value(50.0));
                        Slider::new(&state).horizontal()
                    }))
                    .child(
                        v_flex()
                            .gap_2()
                            .child(subsection_title("带步长（0.5）"))
                            .child({
                                let state = cx.new(|_| {
                                    SliderState::new()
                                        .min(0.0)
                                        .max(10.0)
                                        .step(0.5)
                                        .default_value(3.0)
                                });
                                Slider::new(&state).horizontal()
                            }),
                    )
                    .child(
                        h_flex()
                            .gap_4()
                            .items_start()
                            .child(subsection_title("垂直滑块"))
                            .child({
                                let state = cx.new(|_| {
                                    SliderState::new().min(0.0).max(100.0).default_value(60.0)
                                });
                                Slider::new(&state).vertical().h(px(150.))
                            }),
                    ),
            )
    }
}

impl Render for SelectExample {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .v_flex()
            .child(TitleBar::new().child("选择器 Select 示例"))
            .child(
                div()
                    .id("select-examples")
                    .p_6()
                    .v_flex()
                    .gap_6()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .child(self.render_sliders(window, cx)),
            )
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}

fn section(title: &str) -> Div {
    div()
        .v_flex()
        .gap_3()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(rgpui::black().opacity(0.1))
        .child(div().text_lg().font_bold().child(title.to_string()))
}

fn subsection_title(title: &str) -> impl IntoElement {
    div()
        .text_sm()
        .font_bold()
        .text_color(rgpui::black().opacity(0.6))
        .child(title.to_string())
}

fn description(text: &str) -> impl IntoElement {
    div()
        .text_sm()
        .text_color(rgpui::black().opacity(0.6))
        .child(text.to_string())
}

fn main() {
    let app = rgpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        rgpui_component::init(cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(900.), px(700.)), cx)),
            ..Default::default()
        };

        cx.spawn(async move |cx| {
            cx.open_window(window_options, |window, cx| {
                let view = cx.new(|_| SelectExample);
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("打开窗口失败");
        })
        .detach();
    });
}
