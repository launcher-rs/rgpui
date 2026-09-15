//! 表单组件示例。
//!
//! 演示 `Form` / `Field`（`v_form` / `h_form` / `field`）的常用能力：
//! 垂直表单、水平表单（`label_width`）、网格布局（`columns` + `col_span`）、
//! 必填标记（`required`）、字段描述（`description`）、无标签缩进
//! （`label_indent`），以及与 `Input` / `NumberInput` / `TextArea` /
//! `Select` / `Checkbox` / `Switch` / `RadioGroup` / `Button` 的组合与提交校验。
//!
//! 运行：
//!
//! ```text
//! cargo run -p form
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::input_ui::{Input, InputContentType, InputState, NumberInput, TextArea};
use rgpui::{
    App, Bounds, Button, ButtonVariants, Checkbox, Context, Entity, IntoElement, ParentElement,
    Radio, RadioGroup, Render, SharedString, Switch, Window, WindowBounds, WindowOptions, div,
    field, h_flex, h_form, prelude::*, px, rgb, v_flex, v_form,
};
use rgpui_platform::application;

/// 表单演示根视图，持有全部输入状态与提交结果。
struct FormDemo {
    /// 姓名输入状态（必填）。
    name_input: Entity<InputState>,
    /// 邮箱输入状态（必填，需含 `@`）。
    email_input: Entity<InputState>,
    /// 密码输入状态（必填，至少 6 位）。
    password_input: Entity<InputState>,
    /// 电话输入状态（水平表单）。
    phone_input: Entity<InputState>,
    /// 年龄输入状态（数字输入）。
    age_input: Entity<InputState>,
    /// 简介输入状态（多行文本域）。
    bio_input: Entity<InputState>,
    /// 城市候选项。
    cities: Vec<SharedString>,
    /// 当前选中的城市索引。
    city_selected: Option<usize>,
    /// 当前选中的性别索引（0 男 / 1 女 / 2 保密）。
    gender_selected: Option<usize>,
    /// 是否同意协议（提交前必须勾选）。
    agree: bool,
    /// 是否接收通知。
    notify: bool,
    /// 爱好：阅读。
    hobby_reading: bool,
    /// 爱好：运动。
    hobby_sports: bool,
    /// 提交结果文案。
    submit_result: String,
    /// 提交是否成功（决定结果文案颜色）。
    submit_ok: bool,
}

impl FormDemo {
    /// 创建视图并初始化各输入框状态。
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入姓名"));
        let email_input = cx.new(|cx| InputState::new(window, cx).placeholder("name@example.com"));
        let password_input = cx.new(|cx| InputState::new(window, cx).placeholder("至少 6 位字符"));
        let phone_input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入手机号"));
        let age_input = cx.new(|cx| InputState::new(window, cx).placeholder("0"));
        let bio_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .placeholder("一句话介绍自己")
        });

        Self {
            name_input,
            email_input,
            password_input,
            phone_input,
            age_input,
            bio_input,
            cities: vec!["北京".into(), "上海".into(), "广州".into(), "深圳".into()],
            city_selected: None,
            gender_selected: None,
            agree: false,
            notify: true,
            hobby_reading: false,
            hobby_sports: false,
            submit_result: "填写表单后点击「提交」进行校验。".to_string(),
            submit_ok: false,
        }
    }

    /// 校验并提交表单，把结果写入 `submit_result`。
    fn submit(view: Entity<Self>, _window: &mut Window, cx: &mut App) {
        view.update(cx, |this, cx| {
            let name = this.name_input.read(cx).value().to_string();
            let email = this.email_input.read(cx).value().to_string();
            let password = this.password_input.read(cx).value().to_string();
            let phone = this.phone_input.read(cx).value().to_string();
            let age = this.age_input.read(cx).value().to_string();
            let bio = this.bio_input.read(cx).value().to_string();

            // 逐项校验，收集错误信息。
            let mut errors = Vec::new();
            if name.trim().is_empty() {
                errors.push("姓名不能为空");
            }
            if !email.contains('@') {
                errors.push("邮箱须包含 @");
            }
            if password.len() < 6 {
                errors.push("密码至少 6 位");
            }
            if this.city_selected.is_none() {
                errors.push("请选择城市");
            }
            if this.gender_selected.is_none() {
                errors.push("请选择性别");
            }
            if !this.agree {
                errors.push("请先同意用户协议");
            }

            if errors.is_empty() {
                let city = this
                    .city_selected
                    .and_then(|ix| this.cities.get(ix))
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let gender = match this.gender_selected {
                    Some(0) => "男",
                    Some(1) => "女",
                    _ => "保密",
                };
                this.submit_ok = true;
                this.submit_result = format!(
                    "提交成功：{name} / {email} / {phone} / {age}岁 / {city} / {gender} / 简介{len}字",
                    len = bio.chars().count(),
                );
            } else {
                this.submit_ok = false;
                this.submit_result = format!("校验失败：{}", errors.join("；"));
            }
            cx.notify();
        });
    }

    /// 重置表单：清空输入、恢复默认选项。
    fn reset(view: Entity<Self>, window: &mut Window, cx: &mut App) {
        view.update(cx, |this, cx| {
            this.name_input
                .update(cx, |s, cx| s.set_value("", window, cx));
            this.email_input
                .update(cx, |s, cx| s.set_value("", window, cx));
            this.password_input
                .update(cx, |s, cx| s.set_value("", window, cx));
            this.phone_input
                .update(cx, |s, cx| s.set_value("", window, cx));
            this.age_input
                .update(cx, |s, cx| s.set_value("", window, cx));
            this.bio_input
                .update(cx, |s, cx| s.set_value("", window, cx));
            this.city_selected = None;
            this.gender_selected = None;
            this.agree = false;
            this.notify = true;
            this.hobby_reading = false;
            this.hobby_sports = false;
            this.submit_ok = false;
            this.submit_result = "已重置，请重新填写。".to_string();
            cx.notify();
        });
    }
}

impl Render for FormDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 视图实体克隆，供各控件回调中回写状态。
        let view = cx.entity().clone();
        let submit_view = view.clone();
        let reset_view = view.clone();
        let city_view = view.clone();
        let gender_view = view.clone();
        let agree_view = view.clone();
        let notify_view = view.clone();
        let reading_view = view.clone();
        let sports_view = view.clone();

        let city_options = self.cities.clone();
        let city_selected = self.city_selected;
        let gender_selected = self.gender_selected;
        let (result_color, result_bg) = if self.submit_ok {
            (rgb(0x107c10), rgb(0xe6f4ea))
        } else {
            (rgb(0xa4262c), rgb(0xfdecea))
        };

        div()
            .id("form-root")
            .size_full()
            .overflow_scroll()
            .bg(rgb(0xffffff))
            .child(
                v_flex()
                    .max_w(px(860.0))
                    .mx_auto()
                    .gap(px(16.0))
                    .p(px(24.0))
                    .child(div().text_2xl().child("表单组件示例"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x666666))
                            .child("覆盖 v_form / h_form、columns 网格、required 与 description，以及常用输入控件组合。"),
                    )
                    // 1. 垂直表单：基础输入 + 必填 + 描述
                    .child(section_title("1. 垂直表单（v_form）"))
                    .child(
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
                                    .description("须包含 @，如 name@example.com。")
                                    .child(Input::new(&self.email_input)),
                            )
                            .child(
                                field()
                                    .label("密码")
                                    .required(true)
                                    .description("至少 6 位，点击眼睛图标可显隐。")
                                    .child(
                                        Input::new(&self.password_input)
                                            .content_type(InputContentType::Password)
                                            .mask_toggle(),
                                    ),
                            )
                            .child(
                                field()
                                    .label("简介")
                                    .description("多行文本域（TextArea），固定 3 行高度。")
                                    .child(TextArea::new(&self.bio_input).rows(3)),
                            ),
                    )
                    // 2. 水平表单：label_width + 选择类控件
                    .child(section_title("2. 水平表单（h_form + label_width）"))
                    .child(
                        h_form()
                            .label_width(px(72.0))
                            .child(
                                field()
                                    .label("电话")
                                    .description("水平布局下标签固定宽度。")
                                    .child(Input::new(&self.phone_input)),
                            )
                            .child(
                                field()
                                    .label("城市")
                                    .required(true)
                                    .child(
                                        Select::new(city_options)
                                            .placeholder("请选择城市")
                                            .selected(city_selected)
                                            .on_change(move |ix, _, _, cx| {
                                                city_view.update(cx, |this, cx| {
                                                    this.city_selected = Some(ix);
                                                    cx.notify();
                                                });
                                            }),
                                    ),
                            )
                            .child(
                                field().label("性别").required(true).child(
                                    RadioGroup::horizontal("form-gender")
                                        .child(Radio::new("gender-male").label("男"))
                                        .child(Radio::new("gender-female").label("女"))
                                        .child(Radio::new("gender-unknown").label("保密"))
                                        .selected_index(gender_selected)
                                        .on_click(move |ix, _, cx| {
                                            gender_view.update(cx, |this, cx| {
                                                this.gender_selected = Some(*ix);
                                                cx.notify();
                                            });
                                        }),
                                ),
                            ),
                    )
                    // 3. 网格表单：columns(2) + col_span + 开关/复选
                    .child(section_title("3. 网格表单（columns = 2）"))
                    .child(
                        v_form()
                            .columns(2)
                            .child(
                                field()
                                    .label("年龄")
                                    .description("数字输入，支持上下键步进。")
                                    .child(NumberInput::new(&self.age_input).placeholder("请输入年龄")),
                            )
                            .child(
                                field().label("通知").child(
                                    Switch::new("form-notify")
                                        .label("接收新消息通知")
                                        .checked(self.notify)
                                        .on_click(move |checked, _, cx| {
                                            notify_view.update(cx, |this, cx| {
                                                this.notify = *checked;
                                                cx.notify();
                                            });
                                        }),
                                ),
                            )
                            .child(
                                field().label("爱好").child(
                                    h_flex()
                                        .gap(px(12.0))
                                        .child(
                                            Checkbox::new("hobby-reading")
                                                .label("阅读")
                                                .checked(self.hobby_reading)
                                                .on_click(move |checked, _, cx| {
                                                    reading_view.update(cx, |this, cx| {
                                                        this.hobby_reading = *checked;
                                                        cx.notify();
                                                    });
                                                }),
                                        )
                                        .child(
                                            Checkbox::new("hobby-sports")
                                                .label("运动")
                                                .checked(self.hobby_sports)
                                                .on_click(move |checked, _, cx| {
                                                    sports_view.update(cx, |this, cx| {
                                                        this.hobby_sports = *checked;
                                                        cx.notify();
                                                    });
                                                }),
                                        ),
                                ),
                            )
                            .child(
                                field()
                                    .label("协议")
                                    .child(
                                        Checkbox::new("form-agree")
                                            .label("我已阅读并同意用户协议")
                                            .checked(self.agree)
                                            .on_click(move |checked, _, cx| {
                                                agree_view.update(cx, |this, cx| {
                                                    this.agree = *checked;
                                                    cx.notify();
                                                });
                                            }),
                                    ),
                            ),
                    )
                    // 4. 提交栏：label_indent(false) 左对齐按钮 + 结果展示
                    .child(section_title("4. 提交与校验"))
                    .child(
                        v_form().child(
                            field()
                                .label_indent(false)
                                .child(
                                    h_flex()
                                        .gap(px(8.0))
                                        .child(
                                            Button::new("form-submit")
                                                .label("提交")
                                                .primary()
                                                .on_click(move |_, window, cx| {
                                                    Self::submit(submit_view.clone(), window, cx);
                                                }),
                                        )
                                        .child(
                                            Button::new("form-reset")
                                                .label("重置")
                                                .ghost()
                                                .on_click(move |_, window, cx| {
                                                    Self::reset(reset_view.clone(), window, cx);
                                                }),
                                        ),
                                )
                                .description("提交时校验姓名 / 邮箱 / 密码 / 城市 / 性别 / 协议。"),
                        ),
                    )
                    .child(
                        div()
                            .p(px(12.0))
                            .rounded_md()
                            .bg(result_bg)
                            .text_color(result_color)
                            .text_sm()
                            .child(self.submit_result.clone()),
                    ),
            )
    }
}

/// 章节标题辅助函数。
fn section_title(text: impl Into<SharedString> + IntoElement) -> impl IntoElement {
    div()
        .text_size(px(15.0))
        .font_semibold()
        .mt(px(8.0))
        .child(text)
}

/// 启动示例窗口并初始化主题 / 输入 / 菜单子系统。
fn run_example() {
    application().run(|cx: &mut App| {
        // 表单依赖主题、输入框与菜单（Select 下拉）子系统。
        rgpui::theme::init(cx);
        rgpui::input_ui::init(cx);
        rgpui::menu::init(cx);

        let bounds = Bounds::centered(None, rgpui::size(px(960.0), px(860.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| FormDemo::new(window, cx)),
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
