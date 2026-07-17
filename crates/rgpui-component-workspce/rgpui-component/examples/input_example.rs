//! 输入框组件示例程序
//!
//! 本示例详细展示了 Input 组件的所有用法，包括：
//! - 基础文本输入框
//! - 带占位符的输入框
//! - 可清除的输入框
//! - 密码输入框（带显示/隐藏切换）
//! - 禁用状态的输入框
//! - 不同尺寸的输入框
//! - 带前缀/后缀的输入框
//! - 多行输入框

use rgpui::*;
use rgpui_component::{
    input::{Input, InputState},
    label::Label,
    scroll::ScrollableElement,
    *,
};
use rgpui_component_assets::Assets;

/// 示例应用主结构体
pub struct InputExample;

impl InputExample {
    /// 渲染基础输入框区域
    fn render_basic_inputs(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgpui::black().opacity(0.1))
            .child(div().text_lg().font_bold().child("基础输入 Basic"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgpui::black().opacity(0.6))
                    .child("最基本的文本输入框用法。"),
            )
            .child(
                h_flex()
                    .gap_4()
                    .items_start()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(Label::new("用户名"))
                            .child(Input::new(&cx.new(|cx| InputState::new(window, cx)))),
                    )
                    .child(v_flex().gap_1().child(Label::new("邮箱")).child(
                        Input::new(&cx.new(|cx| InputState::new(window, cx))).appearance(true),
                    )),
            )
    }

    /// 渲染可清除输入框区域
    fn render_cleanable_inputs(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgpui::black().opacity(0.1))
            .child(div().text_lg().font_bold().child("可清除 Cleanable"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgpui::black().opacity(0.6))
                    .child("输入框可以显示清除按钮，方便用户一键清空内容。"),
            )
            .child(
                h_flex().gap_4().items_start().child(
                    v_flex().gap_1().child(Label::new("搜索")).child(
                        Input::new(&cx.new(|cx| InputState::new(window, cx))).cleanable(true),
                    ),
                ),
            )
    }

    /// 渲染密码输入框区域
    fn render_password_inputs(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgpui::black().opacity(0.1))
            .child(div().text_lg().font_bold().child("密码输入 Password"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgpui::black().opacity(0.6))
                    .child("密码输入框可以切换显示/隐藏密码。"),
            )
            .child(
                h_flex().gap_4().items_start().child(
                    v_flex()
                        .gap_1()
                        .child(Label::new("密码"))
                        .child(Input::new(&cx.new(|cx| InputState::new(window, cx))).mask_toggle()),
                ),
            )
    }

    /// 渲染禁用状态输入框区域
    fn render_disabled_inputs(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgpui::black().opacity(0.1))
            .child(div().text_lg().font_bold().child("禁用状态 Disabled"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgpui::black().opacity(0.6))
                    .child("禁用状态的输入框不可编辑。"),
            )
            .child(
                h_flex().gap_4().items_start().child(
                    v_flex().gap_1().child(Label::new("禁用输入框")).child(
                        Input::new(&cx.new(|cx| InputState::new(window, cx))).disabled(true),
                    ),
                ),
            )
    }

    /// 渲染不同尺寸输入框区域
    fn render_size_inputs(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgpui::black().opacity(0.1))
            .child(div().text_lg().font_bold().child("尺寸 Size"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgpui::black().opacity(0.6))
                    .child("输入框有多种尺寸可选。"),
            )
            .child(
                h_flex()
                    .gap_4()
                    .items_start()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(Label::new("小号"))
                            .child(Input::new(&cx.new(|cx| InputState::new(window, cx))).small()),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(Label::new("中号（默认）"))
                            .child(Input::new(&cx.new(|cx| InputState::new(window, cx)))),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(Label::new("大号"))
                            .child(Input::new(&cx.new(|cx| InputState::new(window, cx))).large()),
                    ),
            )
    }

    /// 渲染带前缀/后缀输入框区域
    fn render_affix_inputs(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgpui::black().opacity(0.1))
            .child(div().text_lg().font_bold().child("前缀/后缀 Affix"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgpui::black().opacity(0.6))
                    .child("输入框可以添加前缀或后缀元素。"),
            )
            .child(
                h_flex()
                    .gap_4()
                    .items_start()
                    .child(
                        v_flex().gap_1().child(Label::new("带搜索图标")).child(
                            Input::new(&cx.new(|cx| InputState::new(window, cx)))
                                .prefix(Icon::new(IconName::Search)),
                        ),
                    )
                    .child(
                        v_flex().gap_1().child(Label::new("带后缀")).child(
                            Input::new(&cx.new(|cx| InputState::new(window, cx))).suffix(
                                div()
                                    .text_sm()
                                    .text_color(rgpui::black().opacity(0.4))
                                    .child("@example.com"),
                            ),
                        ),
                    ),
            )
    }

    /// 渲染多行输入框区域
    fn render_multiline_inputs(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgpui::black().opacity(0.1))
            .child(div().text_lg().font_bold().child("多行输入 Multiline"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgpui::black().opacity(0.6))
                    .child("设置高度后输入框变为多行文本区域。"),
            )
            .child(
                v_flex()
                    .gap_1()
                    .w_full()
                    .max_w_96()
                    .child(Label::new("多行输入"))
                    .child(Input::new(&cx.new(|cx| InputState::new(window, cx))).h(px(120.))),
            )
    }
}

impl Render for InputExample {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .v_flex()
            .child(TitleBar::new().child("输入框 Input 示例"))
            .child(
                div()
                    .id("input-examples")
                    .p_6()
                    .v_flex()
                    .gap_6()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .child(self.render_basic_inputs(window, cx))
                    .child(self.render_cleanable_inputs(window, cx))
                    .child(self.render_password_inputs(window, cx))
                    .child(self.render_disabled_inputs(window, cx))
                    .child(self.render_size_inputs(window, cx))
                    .child(self.render_affix_inputs(window, cx))
                    .child(self.render_multiline_inputs(window, cx)),
            )
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}

/// 程序入口
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
                let view = cx.new(|_| InputExample);
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("打开窗口失败");
        })
        .detach();
    });
}
