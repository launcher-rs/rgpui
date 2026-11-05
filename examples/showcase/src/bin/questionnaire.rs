//! 向导式表单（问卷）示例。
//!
//! 演示 [`Questionnaire`] + [`QuestionnaireState`]：三步收集用户信息，
//! 每步用普通表单控件（`Input` / `Switch` / `Checkbox`），答值由宿主视图持有，
//! 向导只管「第几步、能不能走、何时提交」。
//!
//! 运行：
//!
//! ```text
//! cargo run -p showcase --bin questionnaire
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::components::{QuestionStep, Questionnaire, QuestionnaireEvent, QuestionnaireState};
use rgpui::input_ui::{Input, InputState};
use rgpui::{
    App, Bounds, Button, Checkbox, Context, Entity, IntoElement, ParentElement, Render, Switch,
    Window, WindowBounds, WindowOptions, div, field, prelude::*, px, rgb, v_flex, v_form,
};
use rgpui_platform::application;

/// 向导演示根视图，持有各步输入状态与提交结果。
struct QuestionnaireDemo {
    /// 向导状态。
    wizard: Entity<QuestionnaireState>,
    /// 第 0 步：姓名。
    name_input: Entity<InputState>,
    /// 第 0 步：邮箱。
    email_input: Entity<InputState>,
    /// 第 1 步：是否接收通知。
    notify: bool,
    /// 第 2 步：是否同意协议。
    agree: bool,
    /// 提交结果文案。
    result: String,
    /// 提交是否成功。
    result_ok: bool,
}

impl QuestionnaireDemo {
    /// 创建演示视图并初始化向导与输入状态。
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let wizard = cx.new(|_| {
            QuestionnaireState::new().with_steps(vec![
                QuestionStep::new("基本信息").description("请填写姓名与邮箱"),
                QuestionStep::new("偏好设置").description("选择通知与主题偏好"),
                QuestionStep::new("确认提交").description("核对信息后提交"),
            ])
        });
        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入姓名"));
        let email_input = cx.new(|cx| InputState::new(window, cx).placeholder("name@example.com"));

        let this = cx.entity().clone();
        cx.subscribe(&wizard, move |_, _, event, cx| {
            if let QuestionnaireEvent::Submit = event {
                this.update(cx, |this, cx| this.commit(cx));
            }
        })
        .detach();

        Self {
            wizard,
            name_input,
            email_input,
            notify: true,
            agree: false,
            result: String::new(),
            result_ok: false,
        }
    }

    /// 提交时校验并写入结果。
    fn commit(&mut self, cx: &mut Context<Self>) {
        let name = self.name_input.read(cx).value().to_string();
        let email = self.email_input.read(cx).value().to_string();
        if name.trim().is_empty() || !email.contains('@') {
            self.result_ok = false;
            self.result = "校验失败：姓名不能为空，邮箱须包含 @".to_string();
        } else {
            self.result_ok = true;
            self.result = format!(
                "提交成功：{name} / {email} / 通知{} / 同意协议{}",
                if self.notify { "开" } else { "关" },
                if self.agree { "是" } else { "否" }
            );
        }
        cx.notify();
    }

    /// 回到第一步并清空结果。
    fn restart(&mut self, cx: &mut Context<Self>) {
        self.wizard.update(cx, |state, cx| state.reset(cx));
        self.agree = false;
        self.result.clear();
        cx.notify();
    }
}

impl Render for QuestionnaireDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let restart_view = cx.entity().clone();
        let notify_view = cx.entity().clone();
        let agree_view = cx.entity().clone();

        let (result_color, result_bg) = if self.result_ok {
            (rgb(0x107c10), rgb(0xe6f4ea))
        } else if self.result.is_empty() {
            (rgb(0x666666), rgb(0xf5f5f5))
        } else {
            (rgb(0xa4262c), rgb(0xfdecea))
        };

        div()
            .id("questionnaire-demo-root")
            .size_full()
            .overflow_scroll()
            .bg(rgb(0xffffff))
            .child(
                v_flex()
                    .max_w(px(720.0))
                    .mx_auto()
                    .gap(px(16.0))
                    .p(px(24.0))
                    .child(div().text_2xl().child("向导式表单示例"))
                    .child(div().text_sm().text_color(rgb(0x666666)).child(
                        "Questionnaire：三步收集用户信息，答值由宿主持有，向导只管进度与校验门控。",
                    ))
                    .child(
                        Questionnaire::new(&self.wizard)
                            .child(
                                // 第0 步：基本信息
                                v_form()
                                    .child(
                                        field()
                                            .label("姓名")
                                            .required(true)
                                            .description("必填，提交时校验非空。")
                                            .child(Input::new(&self.name_input)),
                                    )
                                    .child(
                                        field()
                                            .label("邮箱")
                                            .required(true)
                                            .description("须包含 @。")
                                            .child(Input::new(&self.email_input)),
                                    ),
                            )
                            .child(
                                // 第 1 步：偏好
                                v_form().child(
                                    field().label("通知").child(
                                        Switch::new("q-notify")
                                            .label("接收新消息通知")
                                            .checked(self.notify)
                                            .on_change(move |checked, _, cx| {
                                                notify_view.update(cx, |this, cx| {
                                                    this.notify = checked;
                                                    cx.notify();
                                                });
                                            }),
                                    ),
                                ),
                            )
                            .child(
                                // 第 2 步：确认
                                v_flex()
                                    .gap(px(12.0))
                                    .child(
                                        div()
                                            .p(px(12.0))
                                            .rounded_md()
                                            .bg(rgb(0xf5f5f5))
                                            .text_sm()
                                            .child(format!(
                                                "姓名：{}\n邮箱：{}\n通知：{}",
                                                self.name_input.read(cx).value(),
                                                self.email_input.read(cx).value(),
                                                if self.notify { "开启" } else { "关闭" }
                                            )),
                                    )
                                    .child(
                                        Checkbox::new("q-agree")
                                            .label("我已阅读并同意用户协议")
                                            .checked(self.agree)
                                            .on_change(move |checked, _, cx| {
                                                agree_view.update(cx, |this, cx| {
                                                    this.agree = checked;
                                                    this.wizard.update(cx, |state, cx| {
                                                        state.set_step_valid(2, checked, cx);
                                                    });
                                                    cx.notify();
                                                });
                                            }),
                                    ),
                            ),
                    )
                    .when(!self.result.is_empty(), |this| {
                        this.child(
                            div()
                                .p(px(12.0))
                                .rounded_md()
                                .bg(result_bg)
                                .text_color(result_color)
                                .text_sm()
                                .child(self.result.clone()),
                        )
                    })
                    .when(self.result_ok, |this| {
                        this.child(
                            Button::new("q-restart")
                                .outline()
                                .label("重新开始")
                                .on_click(move |_, _, cx| {
                                    restart_view.update(cx, |this, cx| this.restart(cx));
                                }),
                        )
                    }),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::theme::init(cx);
        rgpui::input_ui::init(cx);

        let bounds = Bounds::centered(None, rgpui::size(px(720.0), px(640.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| QuestionnaireDemo::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
/// WASM 入口：初始化 Web 环境后启动示例。
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}
