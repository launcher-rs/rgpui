use rgpui::*;
use rgpui_component::{
    alert::Alert,
    avatar::Avatar,
    badge::Badge,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::{Input, InputState},
    label::Label,
    progress::{Progress, ProgressCircle},
    scroll::ScrollableElement,
    skeleton::Skeleton,
    spinner::Spinner,
    switch::Switch,
    tag::Tag,
    *,
};
use rgpui_component_assets::Assets;

/// Web 组件示例应用
struct ComponentsWeb {
    counter: i32,
}

impl ComponentsWeb {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self { counter: 0 }
    }
}

impl Render for ComponentsWeb {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .v_flex()
            .child(TitleBar::new().child("rgpui-component Web 示例"))
            .child(
                div()
                    .id("web-components")
                    .p_6()
                    .v_flex()
                    .gap_6()
                    .flex_1()
                    .overflow_y_scrollbar()
                    // 按钮区域
                    .child(self.render_buttons(cx))
                    // 表单控件区域
                    .child(self.render_form_controls(window, cx))
                    // 数据展示区域
                    .child(self.render_data_display())
                    // 反馈组件区域
                    .child(self.render_feedback())
                    // 进度指示区域
                    .child(self.render_progress()),
            )
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}

impl ComponentsWeb {
    /// 渲染按钮展示区域
    fn render_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        section("按钮 Button").child(
            v_flex()
                .gap_3()
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(Button::new("primary").label("主要").primary())
                        .child(Button::new("secondary").label("次要").secondary())
                        .child(Button::new("danger").label("危险").danger())
                        .child(Button::new("ghost").label("幽灵").ghost())
                        .child(Button::new("outline").label("描边").outline()),
                )
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(Button::new("disabled").label("禁用").disabled(true))
                        .child(Button::new("loading").label("加载中").loading(true))
                        .child(
                            Button::new("counter")
                                .label(format!("计数器: {}", self.counter))
                                .primary()
                                .on_click(cx.listener(|this, _event, _window, _cx| {
                                    this.counter += 1;
                                })),
                        ),
                ),
        )
    }

    /// 渲染表单控件展示区域
    fn render_form_controls(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        section("表单控件").child(
            h_flex()
                .gap_8()
                .items_start()
                .child(
                    v_flex()
                        .gap_2()
                        .child(Label::new("输入框 Input"))
                        .child(Input::new(&cx.new(|cx| InputState::new(window, cx))))
                        .child(
                            Input::new(&cx.new(|cx| InputState::new(window, cx)))
                                .cleanable(true),
                        ),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(Label::new("复选框 Checkbox"))
                        .child(Checkbox::new("web-check-1").label("选项一"))
                        .child(Checkbox::new("web-check-2").label("选项二").checked(true)),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(Label::new("开关 Switch"))
                        .child(Switch::new("web-switch-1").label("启用通知"))
                        .child(Switch::new("web-switch-2").label("自动保存").checked(true)),
                ),
        )
    }

    /// 渲染数据展示区域
    fn render_data_display(&self) -> impl IntoElement {
        section("数据展示").child(
            v_flex()
                .gap_4()
                .child(
                    h_flex()
                        .gap_4()
                        .items_center()
                        .child(Label::new("标签:"))
                        .child(Tag::primary().child("主要"))
                        .child(Tag::secondary().child("次要"))
                        .child(Tag::success().child("成功"))
                        .child(Tag::warning().child("警告"))
                        .child(Tag::danger().child("危险")),
                )
                .child(
                    h_flex()
                        .gap_4()
                        .items_center()
                        .child(Label::new("徽章:"))
                        .child(Badge::new().count(5))
                        .child(Badge::new().count(150).max(99))
                        .child(Badge::new().dot()),
                )
                .child(
                    h_flex()
                        .gap_4()
                        .items_center()
                        .child(Label::new("头像:"))
                        .child(Avatar::new().name("张三"))
                        .child(Avatar::new().name("李四").large()),
                ),
        )
    }

    /// 渲染反馈组件区域
    fn render_feedback(&self) -> impl IntoElement {
        section("反馈组件").child(
            v_flex()
                .gap_3()
                .child(Alert::info("web-info", "这是一条信息提示。"))
                .child(Alert::success("web-success", "操作成功完成！"))
                .child(Alert::warning("web-warning", "请注意，此操作不可逆。"))
                .child(Alert::error("web-error", "发生错误，请重试。"))
                .child(
                    h_flex()
                        .gap_4()
                        .items_center()
                        .child(Skeleton::new().w_10().h_10().rounded_full())
                        .child(Spinner::new())
                        .child(Spinner::new().large()),
                ),
        )
    }

    /// 渲染进度指示区域
    fn render_progress(&self) -> impl IntoElement {
        section("进度指示").child(
            h_flex()
                .gap_8()
                .items_start()
                .child(
                    v_flex()
                        .gap_2()
                        .w_full()
                        .child(Progress::new("web-prog-1").value(25.0))
                        .child(Progress::new("web-prog-2").value(60.0).loading(true))
                        .child(Progress::new("web-prog-3").value(85.0)),
                )
                .child(
                    h_flex()
                        .gap_4()
                        .items_center()
                        .child(ProgressCircle::new("web-pc-1").value(30.0))
                        .child(ProgressCircle::new("web-pc-2").value(70.0).loading(true)),
                ),
        )
    }
}

/// 分区容器
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

// ---------------------------------------------------------------------------
// 入口
// ---------------------------------------------------------------------------

fn main() {
    rgpui_platform::web_init();
    rgpui_platform::application().with_assets(Assets::default()).run(move |cx| {
        rgpui_component::init(cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(900.), px(700.)), cx)),
            ..Default::default()
        };

        cx.open_window(window_options, |window, cx| {
            let view = cx.new(ComponentsWeb::new);
            cx.new(|cx| Root::new(view, window, cx))
        })
        .expect("打开窗口失败");

        cx.activate(true);
    });
}
