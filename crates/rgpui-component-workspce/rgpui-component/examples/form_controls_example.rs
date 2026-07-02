//! 表单控件示例程序
//!
//! 本示例详细展示了表单控件的用法，包括：
//! - 复选框（Checkbox）：单个复选框、复选框组、禁用状态
//! - 开关（Switch）：基础开关、不同尺寸、自定义颜色
//! - 单选框（Radio）：单个单选框、单选组（水平/垂直布局）

use rgpui::*;
use rgpui_component::{
    checkbox::Checkbox,
    radio::{Radio, RadioGroup},
    scroll::ScrollableElement,
    switch::Switch,
    *,
};
use rgpui_component_assets::Assets;

/// 示例应用主结构体
pub struct FormControlsExample;

impl FormControlsExample {
    /// 渲染复选框区域
    fn render_checkboxes(&self) -> impl IntoElement {
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgpui::black().opacity(0.1))
            .child(div().text_lg().font_bold().child("复选框 Checkbox"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgpui::black().opacity(0.6))
                    .child("复选框用于选择一个或多个选项。"),
            )
            .child(
                h_flex()
                    .gap_8()
                    .items_start()
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_bold()
                                    .text_color(rgpui::black().opacity(0.6))
                                    .child("基础用法"),
                            )
                            .child(Checkbox::new("check-1").label("同意用户协议"))
                            .child(Checkbox::new("check-2").label("订阅邮件通知").checked(true))
                            .child(Checkbox::new("check-3").label("禁用选项").disabled(true)),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_bold()
                                    .text_color(rgpui::black().opacity(0.6))
                                    .child("不同尺寸"),
                            )
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .child(Checkbox::new("size-xs").label("超小").xsmall())
                                    .child(Checkbox::new("size-sm").label("小号").small())
                                    .child(Checkbox::new("size-md").label("中号"))
                                    .child(Checkbox::new("size-lg").label("大号").large()),
                            ),
                    ),
            )
    }

    /// 渲染开关区域
    fn render_switches(&self) -> impl IntoElement {
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgpui::black().opacity(0.1))
            .child(div().text_lg().font_bold().child("开关 Switch"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgpui::black().opacity(0.6))
                    .child("开关用于切换开/关状态。"),
            )
            .child(
                h_flex()
                    .gap_8()
                    .items_start()
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_bold()
                                    .text_color(rgpui::black().opacity(0.6))
                                    .child("基础用法"),
                            )
                            .child(Switch::new("switch-1").label("启用通知"))
                            .child(Switch::new("switch-2").label("自动保存").checked(true))
                            .child(Switch::new("switch-3").label("禁用").disabled(true)),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_bold()
                                    .text_color(rgpui::black().opacity(0.6))
                                    .child("不同尺寸"),
                            )
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .child(Switch::new("sw-xs").label("超小").xsmall())
                                    .child(Switch::new("sw-sm").label("小号").small())
                                    .child(Switch::new("sw-md").label("中号"))
                                    .child(Switch::new("sw-lg").label("大号").large()),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_bold()
                                    .text_color(rgpui::black().opacity(0.6))
                                    .child("自定义颜色"),
                            )
                            .child(Switch::new("sw-color1").label("绿色").color(rgpui::green()))
                            .child(Switch::new("sw-color2").label("红色").color(rgpui::red()))
                            .child(Switch::new("sw-color3").label("蓝色").color(rgpui::blue())),
                    ),
            )
    }

    /// 渲染单选框区域
    fn render_radios(&self) -> impl IntoElement {
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgpui::black().opacity(0.1))
            .child(div().text_lg().font_bold().child("单选框 Radio"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgpui::black().opacity(0.6))
                    .child("单选框用于从多个选项中选择一个。"),
            )
            .child(
                h_flex()
                    .gap_8()
                    .items_start()
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_bold()
                                    .text_color(rgpui::black().opacity(0.6))
                                    .child("水平布局"),
                            )
                            .child(
                                RadioGroup::horizontal("radio-h")
                                    .child(Radio::new("h-opt-a").label("选项 A"))
                                    .child(Radio::new("h-opt-b").label("选项 B").checked(true))
                                    .child(Radio::new("h-opt-c").label("选项 C")),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_bold()
                                    .text_color(rgpui::black().opacity(0.6))
                                    .child("垂直布局"),
                            )
                            .child(
                                RadioGroup::vertical("radio-v")
                                    .child(Radio::new("v-opt-a").label("选项 A"))
                                    .child(Radio::new("v-opt-b").label("选项 B"))
                                    .child(Radio::new("v-opt-c").label("选项 C").checked(true)),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_bold()
                                    .text_color(rgpui::black().opacity(0.6))
                                    .child("不同尺寸"),
                            )
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .child(Radio::new("r-xs").label("超小").xsmall())
                                    .child(Radio::new("r-sm").label("小号").small())
                                    .child(Radio::new("r-md").label("中号"))
                                    .child(Radio::new("r-lg").label("大号").large()),
                            ),
                    ),
            )
    }
}

impl Render for FormControlsExample {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .v_flex()
            .child(TitleBar::new().child("表单控件 Form Controls 示例"))
            .child(
                div()
                    .id("form-controls-examples")
                    .p_6()
                    .v_flex()
                    .gap_6()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .child(self.render_checkboxes())
                    .child(self.render_switches())
                    .child(self.render_radios()),
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
            window_bounds: Some(WindowBounds::centered(size(px(900.), px(600.)), cx)),
            ..Default::default()
        };

        cx.spawn(async move |cx| {
            cx.open_window(window_options, |window, cx| {
                let view = cx.new(|_| FormControlsExample);
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("打开窗口失败");
        })
        .detach();
    });
}
