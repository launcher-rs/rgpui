//! 按钮组件示例程序
//!
//! 本示例详细展示了按钮组件的用法，包括：
//! - 按钮变体：Primary（主要）、Secondary（次要）、Danger（危险）、Ghost（幽灵）、Text（文字）
//! - 按钮状态：禁用、加载中、选中
//! - 图标按钮：纯图标、带文字图标
//! - 提示信息：悬浮提示
//! - 紧凑模式
//! - 按钮组

use rgpui::*;
use rgpui_component::{
    button::{Button, ButtonGroup, ButtonVariants},
    scroll::ScrollableElement,
    *,
};
use rgpui_component_assets::Assets;

/// 示例应用主结构体
pub struct ButtonExample;

impl ButtonExample {
    /// 渲染变体区域
    fn render_variants(&self) -> impl IntoElement {
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgpui::black().opacity(0.1))
            .child(div().text_lg().font_bold().child("按钮变体 Variant"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgpui::black().opacity(0.6))
                    .child("按钮的不同视觉变体，适用于不同的交互场景。"),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(Button::new("primary").label("主要").primary())
                    .child(Button::new("secondary").label("次要").secondary())
                    .child(Button::new("danger").label("危险").danger())
                    .child(Button::new("ghost").label("幽灵").ghost())
                    .child(Button::new("text").label("文字").text())
                    .child(Button::new("outline-btn").label("描边").outline()),
            )
    }

    /// 渲染状态区域
    fn render_states(&self) -> impl IntoElement {
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgpui::black().opacity(0.1))
            .child(div().text_lg().font_bold().child("按钮状态 State"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgpui::black().opacity(0.6))
                    .child("按钮的不同交互状态。"),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(Button::new("disabled").label("禁用").disabled(true))
                    .child(Button::new("loading").label("加载中").loading(true))
                    .child(Button::new("selected").label("已选中").selected(true)),
            )
    }

    /// 渲染图标按钮区域
    fn render_icons(&self) -> impl IntoElement {
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgpui::black().opacity(0.1))
            .child(div().text_lg().font_bold().child("图标按钮 Icon"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgpui::black().opacity(0.6))
                    .child("按钮可以包含图标，增强视觉表达。"),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(Button::new("icon-only").icon(IconName::Plus))
                    .child(Button::new("icon-left").icon(IconName::Plus).label("添加"))
                    .child(
                        Button::new("icon-right")
                            .label("下载")
                            .icon(IconName::ArrowDown),
                    )
                    .child(
                        Button::new("icon-search")
                            .label("搜索")
                            .icon(IconName::Search),
                    ),
            )
    }

    /// 渲染带提示的按钮
    fn render_tooltips(&self) -> impl IntoElement {
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgpui::black().opacity(0.1))
            .child(div().text_lg().font_bold().child("提示信息 Tooltip"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgpui::black().opacity(0.6))
                    .child("按钮可以附带提示信息，鼠标悬浮时显示。"),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        Button::new("tooltip-btn")
                            .label("悬浮查看")
                            .tooltip("这是一条提示信息"),
                    )
                    .child(
                        Button::new("icon-tooltip")
                            .icon(IconName::Settings)
                            .tooltip("设置"),
                    ),
            )
    }

    /// 渲染紧凑模式按钮
    fn render_compact(&self) -> impl IntoElement {
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgpui::black().opacity(0.1))
            .child(div().text_lg().font_bold().child("紧凑模式 Compact"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgpui::black().opacity(0.6))
                    .child("紧凑模式下按钮间距更小。"),
            )
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(Button::new("compact-1").label("复制").compact())
                    .child(Button::new("compact-2").label("粘贴").compact())
                    .child(Button::new("compact-3").label("剪切").compact()),
            )
    }

    /// 渲染按钮组
    fn render_groups(&self) -> impl IntoElement {
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgpui::black().opacity(0.1))
            .child(div().text_lg().font_bold().child("按钮组 Group"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgpui::black().opacity(0.6))
                    .child("按钮组将多个按钮组合在一起，支持单选和多选模式。"),
            )
            .child(
                v_flex()
                    .gap_4()
                    .child(
                        div()
                            .text_sm()
                            .font_bold()
                            .text_color(rgpui::black().opacity(0.6))
                            .child("基础按钮组"),
                    )
                    .child(
                        ButtonGroup::new("basic-group")
                            .child(Button::new("g1").label("左").secondary())
                            .child(Button::new("g2").label("中").secondary())
                            .child(Button::new("g3").label("右").secondary()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_bold()
                            .text_color(rgpui::black().opacity(0.6))
                            .child("紧凑按钮组"),
                    )
                    .child(
                        ButtonGroup::new("compact-group")
                            .compact()
                            .child(Button::new("c1").label("复制").secondary())
                            .child(Button::new("c2").label("粘贴").secondary())
                            .child(Button::new("c3").label("剪切").secondary()),
                    ),
            )
    }
}

impl Render for ButtonExample {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .v_flex()
            .child(TitleBar::new().child("按钮组件 Button 示例"))
            .child(
                div()
                    .id("button-examples")
                    .p_6()
                    .v_flex()
                    .gap_6()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .child(self.render_variants())
                    .child(self.render_states())
                    .child(self.render_icons())
                    .child(self.render_tooltips())
                    .child(self.render_compact())
                    .child(self.render_groups()),
            )
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
                let view = cx.new(|_| ButtonExample);
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("打开窗口失败");
        })
        .detach();
    });
}
