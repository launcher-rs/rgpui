//! 反馈组件示例程序
//!
//! 本示例详细展示了反馈组件的用法，包括：
//! - 警告条（Alert）：各种类型、可关闭
//! - 工具提示（Tooltip）：文本提示
//! - 弹出框（Popover）：基础弹出
//! - 骨架屏（Skeleton）：加载占位符
//! - 加载器（Spinner）：加载动画
//! - 折叠面板（Accordion）：可折叠内容区域
//! - 折叠（Collapsible）：简单折叠

use rgpui::*;
use rgpui_component::{
    accordion::Accordion,
    alert::Alert,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    label::Label,
    popover::Popover,
    scroll::ScrollableElement,
    skeleton::Skeleton,
    spinner::Spinner,
    switch::Switch,
    *,
};
use rgpui_component_assets::Assets;

/// 示例应用主结构体
pub struct FeedbackExample;

impl FeedbackExample {
    /// 渲染警告条区域
    fn render_alerts(&self) -> impl IntoElement {
        section("警告条 Alert")
            .child(description("警告条用于显示重要的提示信息。"))
            .child(
                v_flex()
                    .gap_3()
                    .child(Alert::info("info-1", "这是一条信息提示。"))
                    .child(Alert::success("success-1", "操作成功完成！"))
                    .child(Alert::warning("warning-1", "请注意，此操作不可逆。"))
                    .child(Alert::error("error-1", "发生错误，请重试。"))
                    .child(Alert::info("info-2", "详细信息内容。").title("提示"))
                    .child(Alert::warning("warning-2", "这条警告可以关闭。").on_close(
                        |_, _, _| {
                            println!("警告已关闭");
                        },
                    )),
            )
    }

    /// 渲染工具提示区域
    fn render_tooltips(&self) -> impl IntoElement {
        section("工具提示 Tooltip")
            .child(description("工具提示在悬浮时显示额外信息。"))
            .child(
                h_flex()
                    .gap_4()
                    .items_center()
                    .child(
                        Button::new("tooltip-1")
                            .label("悬浮查看")
                            .tooltip("这是一条提示信息"),
                    )
                    .child(Button::new("tooltip-2").label("保存").tooltip("保存文件")),
            )
    }

    /// 渲染弹出框区域
    fn render_popovers(&self) -> impl IntoElement {
        section("弹出框 Popover")
            .child(description("弹出框在点击时显示浮动内容。"))
            .child(
                h_flex()
                    .gap_4()
                    .items_start()
                    .child(
                        Popover::new("popover-1")
                            .trigger(Button::new("popover-btn-1").label("点击弹出").secondary())
                            .content(|_, _, _| {
                                div().p_4().w_64().child(Label::new("这是弹出框的内容。"))
                            }),
                    )
                    .child(
                        Popover::new("popover-2")
                            .trigger(Button::new("popover-btn-2").label("设置").secondary())
                            .content(|_, _, _| {
                                div()
                                    .p_4()
                                    .w_64()
                                    .v_flex()
                                    .gap_3()
                                    .child(Label::new("设置选项"))
                                    .child(Switch::new("setting-1").label("启用通知"))
                                    .child(Switch::new("setting-2").label("自动保存").checked(true))
                            }),
                    ),
            )
    }

    /// 渲染骨架屏和加载器区域
    fn render_loading(&self) -> impl IntoElement {
        section("加载状态 Loading")
            .child(description("骨架屏和加载器用于显示加载状态。"))
            .child(
                h_flex()
                    .gap_8()
                    .items_start()
                    .child(
                        v_flex()
                            .gap_3()
                            .child(subsection_title("骨架屏 Skeleton"))
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .child(Skeleton::new().w_10().h_10().rounded_full())
                                    .child(
                                        v_flex()
                                            .gap_2()
                                            .child(Skeleton::new().w_48().h_4())
                                            .child(Skeleton::new().w_32().h_3()),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_2()
                                    .child(Skeleton::new().w_full().h_4())
                                    .child(Skeleton::new().w_3_4().h_4())
                                    .child(Skeleton::new().w_1_2().h_4()),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_3()
                            .child(subsection_title("加载器 Spinner"))
                            .child(
                                h_flex()
                                    .gap_4()
                                    .items_center()
                                    .child(Spinner::new())
                                    .child(Spinner::new().large()),
                            ),
                    ),
            )
    }

    /// 渲染折叠组件区域
    fn render_collapsibles(&self) -> impl IntoElement {
        section("折叠组件")
            .child(description("折叠组件用于隐藏/显示内容区域。"))
            .child(
                h_flex()
                    .gap_8()
                    .items_start()
                    .child(
                        v_flex()
                            .gap_3()
                            .w_full()
                            .child(subsection_title("折叠面板 Accordion"))
                            .child(
                                Accordion::new("accordion-1")
                                    .item(|item| {
                                        item.title("什么是 rgpui？").child(Label::new(
                                            "rgpui 是一个基于 gpui 的 Rust UI 组件库。",
                                        ))
                                    })
                                    .item(|item| {
                                        item.title("如何开始使用？")
                                            .child(Label::new("在 Cargo.toml 中添加依赖即可使用。"))
                                    })
                                    .item(|item| {
                                        item.title("支持哪些平台？")
                                            .child(Label::new("支持 macOS、Linux 和 Windows。"))
                                    }),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_3()
                            .w_full()
                            .child(subsection_title("折叠 Collapsible"))
                            .child(
                                Collapsible::new()
                                    .open(true)
                                    .content(
                                        div()
                                            .p_4()
                                            .bg(rgpui::white().opacity(0.5))
                                            .rounded_md()
                                            .child(Label::new(
                                                "这是可折叠的内容区域。点击标题可以展开或收起。",
                                            )),
                                    )
                                    .child(div().text_sm().font_bold().child("点击展开/收起")),
                            ),
                    ),
            )
    }
}

impl Render for FeedbackExample {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .v_flex()
            .child(TitleBar::new().child("反馈组件 Feedback 示例"))
            .child(
                div()
                    .id("feedback-examples")
                    .p_6()
                    .v_flex()
                    .gap_6()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .child(self.render_alerts())
                    .child(self.render_tooltips())
                    .child(self.render_popovers())
                    .child(self.render_loading())
                    .child(self.render_collapsibles()),
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
            window_bounds: Some(WindowBounds::centered(size(px(900.), px(800.)), cx)),
            ..Default::default()
        };

        cx.spawn(async move |cx| {
            cx.open_window(window_options, |window, cx| {
                let view = cx.new(|_| FeedbackExample);
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("打开窗口失败");
        })
        .detach();
    });
}
