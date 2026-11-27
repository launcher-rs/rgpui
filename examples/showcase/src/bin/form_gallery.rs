//! 输入与表单示例：三个相关组件放在同一页面纵向展示。
//!
//! - 输入框：`InputState` + `Input` / `NumberInput`，含前后缀、清空、密码掩码。
//! - 表单：`v_form` / `h_form` / `field`，含必填、描述、校验与提交。
//! - 问卷向导：`QuestionnaireState` 管理步骤，宿主视图持有答值并同步有效性。
//!
//! 运行：`cargo run -p showcase --bin form_gallery`

#![cfg_attr(target_family = "wasm", no_main)]

mod keymap {
    //! keymap 演示：`load_keymap_json` 加载应用 + `HotkeyInput` 录制 + 回显 + 冲突提示。
    //!
    //! 设置页 recipe 落地（M7）：录制 → 绑定 → `bindings_for_action` 回显 →
    //! 同键不同动作即冲突提示。JSON 编辑框预置一条碰撞条目，加载即见提示。

    #![cfg_attr(target_family = "wasm", no_main)]

    use std::collections::HashMap;

    use rgpui::{
        App, Bounds, Context, Render, Window, WindowBounds, WindowOptions,
        components::{HotkeyInput, HotkeyInputState},
        div, h_flex,
        input_ui::{InputState, TextArea},
        prelude::*,
        px, size, v_flex,
    };
    use rgpui_platform::application;

    const SAMPLE_JSON: &str = r#"[
    // 改键示例：Ctrl+S 存盘（input::Undo 只是演示占位，真应用换存盘动作）。
    { "key": "ctrl-s", "action": "input::Undo", "context": "Input" },
    // 碰撞示例：同键不同动作 → 冲突提示。
    { "key": "ctrl-s", "action": "input::Copy", "context": "Input" },
    // 禁用示例：Ctrl+K 发 NoAction。
    { "key": "ctrl-k", "action": null },
]"#;

    struct KeymapDemo {
        json: rgpui::Entity<InputState>,
        hotkey: rgpui::Entity<HotkeyInputState>,
        status: String,
        recorded: String,
    }

    impl KeymapDemo {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let json = cx.new(|cx| {
                let mut state = InputState::new(window, cx).multi_line(true).rows(8);
                state.set_value(SAMPLE_JSON, window, cx);
                state
            });
            let hotkey = cx.new(HotkeyInputState::new);
            Self {
                json,
                hotkey,
                status: "未加载".to_string(),
                recorded: "未录制".to_string(),
            }
        }

        /// 加载并应用（冲突 = 同键不同动作名，提示但不阻止）。
        fn apply(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
            let text = self.json.read_with(cx, |state, _| state.text().to_string());
            match rgpui::load_keymap_json(cx, &text) {
                Ok(bindings) => {
                    let count = bindings.len();
                    let mut by_key: HashMap<String, Vec<&str>> = HashMap::new();
                    for binding in &bindings {
                        let key = binding
                            .keystrokes()
                            .iter()
                            .map(|k| k.unparse())
                            .collect::<Vec<_>>()
                            .join(" ");
                        by_key.entry(key).or_default().push(binding.action().name());
                    }
                    let mut conflicts: Vec<String> = by_key
                        .iter()
                        .filter(|(_, actions)| {
                            let mut unique = (*actions).clone();
                            unique.sort_unstable();
                            unique.dedup();
                            unique.len() > 1
                        })
                        .map(|(key, actions)| format!("{key} ← {}", actions.join(" / ")))
                        .collect();
                    conflicts.sort();
                    cx.bind_keys(bindings);
                    self.status = if conflicts.is_empty() {
                        format!("已应用 {count} 条，无冲突")
                    } else {
                        format!(
                            "已应用 {count} 条，冲突 {} 处：{}",
                            conflicts.len(),
                            conflicts.join("；")
                        )
                    };
                }
                Err(error) => {
                    self.status = format!("加载失败：{error}");
                }
            }
            cx.notify();
        }

        /// 录制值转按键串（`HotkeyValue` 存解析形 key，直拼即可）。
        fn recorded_keystroke(&self, cx: &mut App) -> Option<String> {
            self.hotkey.read_with(cx, |state, _| {
                state.hotkey().map(|hk| {
                    let mut parts = Vec::new();
                    if hk.modifiers.control {
                        parts.push("ctrl".to_string());
                    }
                    if hk.modifiers.alt {
                        parts.push("alt".to_string());
                    }
                    if hk.modifiers.shift {
                        parts.push("shift".to_string());
                    }
                    if hk.modifiers.platform {
                        parts.push("secondary".to_string());
                    }
                    parts.push(hk.key.clone());
                    parts.join("-")
                })
            })
        }

        /// 把录制键绑到 `input::Copy`（演示 `bind_keys` 增量路径）。
        fn bind_recorded(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
            let Some(key) = self.recorded_keystroke(cx) else {
                self.recorded = "先点输入框录制一个按键".to_string();
                cx.notify();
                return;
            };
            match rgpui::KeyBinding::load(
                &key,
                Box::new(rgpui::input_ui::Copy),
                None,
                false,
                None,
                &rgpui::DummyKeyboardMapper,
            ) {
                Ok(binding) => {
                    cx.bind_keys([binding]);
                    self.recorded = format!("已绑 {key} → input::Copy");
                }
                Err(error) => {
                    self.recorded = format!("按键串非法（{key}）：{error}");
                }
            }
            cx.notify();
        }
    }

    impl Render for KeymapDemo {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let demo = cx.entity();
            // 回显：input::Undo 当前绑定（最后一条优先，引擎语义）。
            let echo = window
                .bindings_for_action(&rgpui::input_ui::Undo)
                .iter()
                .map(|b| {
                    b.keystrokes()
                        .iter()
                        .map(|k| k.unparse())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect::<Vec<_>>()
                .join(" ｜ ");
            let echo_text = if echo.is_empty() {
                "input::Undo：无绑定".to_string()
            } else {
                format!("input::Undo 回显：{echo}")
            };
            let recorded_text = self.recorded.clone();
            let status = self.status.clone();
            let demo_for_record = demo.clone();
            v_flex()
            .size_full()
            .gap(px(8.0))
            .p(px(12.0))
            .child(div().text_sm().child("keymap.json（注释/尾逗号可写）"))
            .child(TextArea::new(&self.json).flex_none().h(px(180.0)))
            .child(
                h_flex()
                    .gap(px(8.0))
                    .items_center()
                    .child({
                        let demo = demo.clone();
                        div()
                            .id("keymap-apply")
                            .px(px(10.0))
                            .py(px(4.0))
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .child("加载并应用")
                            .on_click(move |_, window, cx| {
                                demo.update(cx, |this, cx| this.apply(window, cx));
                            })
                    })
                    .child(div().text_xs().child(status)),
            )
            .child(div().text_sm().child("录制（点框后按键）"))
            .child(
                h_flex()
                    .gap(px(8.0))
                    .items_center()
                    .child(
                        HotkeyInput::new(self.hotkey.clone())
                            .placeholder("点这里，然后按键")
                            .w(px(220.0))
                            .on_change(move |hotkey, _, cx| {
                                demo_for_record.update(cx, |this, cx| {
                                    this.recorded = hotkey
                                        .map(|hk| hk.format_display())
                                        .unwrap_or("已清空".to_string());
                                    cx.notify();
                                });
                            }),
                    )
                    .child({
                        div()
                            .id("keymap-bind-recorded")
                            .px(px(10.0))
                            .py(px(4.0))
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .child("绑录制键到 input::Copy")
                            .on_click(move |_, window, cx| {
                                demo.update(cx, |this, cx| this.bind_recorded(window, cx));
                            })
                    })
                    .child(div().text_xs().child(recorded_text)),
            )
            .child(div().text_xs().child(echo_text))
            .child(
                div()
                    .text_xs()
                    .child("约束：JSON 是改键语义（文件列=用户层全集）；未知动作报错；谓词原样透传；null=禁用。"),
            )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(720.0), px(560.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| KeymapDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }
}

mod dialog {
    //! 对话框示例。
    //!
    //! 演示 `Dialog`（经 Root 打开/挂载）、`AlertDialog`（触发器式）和
    //! `FocusTrapElement`（Tab 焦点锁定在容器内循环）。

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::{
        App, Bounds, Context, FocusHandle, FocusTrapElement, Render, Root, Window, WindowBounds,
        WindowOptions, div, h_flex, prelude::*, px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    struct DialogDemo {
        container_focus: FocusHandle,
        last_result: String,
    }

    impl DialogDemo {
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            Self {
                container_focus: cx.focus_handle(),
                last_result: "尚未操作".to_string(),
            }
        }

        /// 打开一个确认对话框。
        fn open_confirm(window: &mut Window, cx: &mut App) {
            let Some(Some(root)) = window.root::<Root>() else {
                return;
            };
            root.update(cx, |root, cx| {
                root.open_dialog(
                    |dialog, _window, _cx| {
                        dialog
                            .title("确认删除？")
                            .content(|content, _, _| {
                                content.child(div().child("删除后无法恢复，请确认是否继续。"))
                            })
                            .width(px(360.0))
                    },
                    window,
                    cx,
                );
            });
        }
    }

    impl Render for DialogDemo {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let result = self.last_result.clone();

            v_flex()
                .size_full()
                .gap(px(16.0))
                .p(px(24.0))
                .bg(rgb(0xffffff))
                // 1. Dialog（经 Root 打开）
                .child(
                    v_flex()
                        .gap(px(8.0))
                        .child(div().text_lg().child("1. Dialog（Root.open_dialog）"))
                        .child(
                            h_flex()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .id("open-dialog")
                                        .px(px(12.0))
                                        .py(px(6.0))
                                        .rounded_md()
                                        .bg(rgb(0x0078d4))
                                        .text_color(rgb(0xffffff))
                                        .cursor_pointer()
                                        .child("打开确认对话框")
                                        .on_click(|_, window, cx| {
                                            Self::open_confirm(window, cx);
                                        }),
                                )
                                .child(div().text_sm().child(format!("上次结果：{result}"))),
                        ),
                )
                // 2. AlertDialog（触发器式）
                .child(
                    v_flex()
                        .gap(px(8.0))
                        .child(div().text_lg().child("2. AlertDialog（触发器）"))
                        .child(
                            rgpui::AlertDialog::new(cx)
                                .trigger(
                                    div()
                                        .id("open-alert")
                                        .px(px(12.0))
                                        .py(px(6.0))
                                        .rounded_md()
                                        .bg(rgb(0x107c10))
                                        .text_color(rgb(0xffffff))
                                        .cursor_pointer()
                                        .child("打开警告框"),
                                )
                                .content(|content, _, _| {
                                    content.child(
                                        div()
                                            .p(px(16.0))
                                            .child("这是一个 AlertDialog，点遮罩或按钮关闭。"),
                                    )
                                }),
                        ),
                )
                // 3. 焦点陷阱：Tab 只在三个按钮间循环
                .child(
                    v_flex()
                        .gap(px(8.0))
                        .child(div().text_lg().child("3. 焦点陷阱（Tab 循环）"))
                        .child(
                            h_flex()
                                .gap(px(8.0))
                                .p(px(12.0))
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(0xe0e0e0))
                                .child(
                                    div()
                                        .id("trap-btn-1")
                                        .px(px(12.0))
                                        .py(px(6.0))
                                        .rounded_sm()
                                        .bg(rgb(0xf0f0f0))
                                        .cursor_pointer()
                                        .child("按钮 1"),
                                )
                                .child(
                                    div()
                                        .id("trap-btn-2")
                                        .px(px(12.0))
                                        .py(px(6.0))
                                        .rounded_sm()
                                        .bg(rgb(0xf0f0f0))
                                        .cursor_pointer()
                                        .child("按钮 2"),
                                )
                                .child(
                                    div()
                                        .id("trap-btn-3")
                                        .px(px(12.0))
                                        .py(px(6.0))
                                        .rounded_sm()
                                        .bg(rgb(0xf0f0f0))
                                        .cursor_pointer()
                                        .child("按钮 3"),
                                )
                                .focus_trap("trap-demo", &self.container_focus),
                        )
                        .child(div().text_sm().child("在三个按钮上按 Tab，焦点不出容器。")),
                )
                // 对话框层挂载
                .when_some(Root::render_dialog_layer(window, cx), |this, layer| {
                    this.child(layer)
                })
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(560.0), px(560.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| DialogDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }
}

use rgpui::components::{QuestionStep, Questionnaire, QuestionnaireEvent, QuestionnaireState};
use rgpui::input_ui::{Input, InputContentType, InputEvent, InputState, NumberInput, TextArea};
use rgpui::{
    App, Bounds, Button, ButtonVariants, Checkbox, Context, Entity, IntoElement, ParentElement,
    Render, SharedString, Switch, Window, WindowBounds, WindowOptions, div, field, h_form,
    prelude::*, px, rgb, size, v_flex, v_form,
};
use rgpui_platform::application;

/// 输入与表单示例根视图，持有三段示例各自的输入状态。
struct FormGallery {
    // 输入框示例
    /// 基础输入框。
    input_name: Entity<InputState>,
    /// 带前后缀的输入框。
    input_search: Entity<InputState>,
    /// 密码输入框。
    input_password: Entity<InputState>,
    /// 数字输入框。
    input_age: Entity<InputState>,

    // 表单示例
    /// 姓名（必填）。
    form_name: Entity<InputState>,
    /// 邮箱（必填）。
    form_email: Entity<InputState>,
    /// 简介。
    form_bio: Entity<InputState>,
    /// 城市选中项。
    form_city: Option<usize>,
    /// 是否同意协议。
    form_agree: bool,
    /// 表单提交结果。
    form_result: String,

    // 问卷向导示例
    /// 向导状态。
    wizard: Entity<QuestionnaireState>,
    /// 第一步姓名。
    quiz_name: Entity<InputState>,
    /// 第一步邮箱。
    quiz_email: Entity<InputState>,
    /// 第二步通知开关。
    quiz_notify: bool,
    /// 第二步协议勾选。
    quiz_agree: bool,
    /// 问卷提交结果。
    quiz_result: String,
}

impl FormGallery {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 向导初始两步都不可用，输入变化时再回写有效性。
        let wizard = cx.new(|cx| {
            let mut state = QuestionnaireState::new().with_steps(vec![
                QuestionStep::new("基本信息").description("请填写姓名与邮箱"),
                QuestionStep::new("确认提交").description("勾选协议后提交"),
            ]);
            state.set_step_valid(0, false, cx);
            state.set_step_valid(1, false, cx);
            state
        });
        let quiz_name = cx.new(|cx| InputState::new(window, cx).placeholder("请输入姓名"));
        let quiz_email = cx.new(|cx| InputState::new(window, cx).placeholder("name@example.com"));

        for input in [&quiz_name, &quiz_email] {
            cx.subscribe(input, |this, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    this.sync_quiz(cx);
                }
            })
            .detach();
        }

        let this = cx.entity();
        cx.subscribe(&wizard, move |_, _, event, cx| {
            if matches!(event, QuestionnaireEvent::Submit) {
                this.update(cx, |gallery, cx| gallery.submit_quiz(cx));
            }
        })
        .detach();

        Self {
            input_name: cx.new(|cx| InputState::new(window, cx).placeholder("请输入姓名")),
            input_search: cx.new(|cx| InputState::new(window, cx).placeholder("搜索内容")),
            input_password: cx.new(|cx| InputState::new(window, cx).placeholder("至少 6 位")),
            input_age: cx.new(|cx| InputState::new(window, cx).placeholder("0")),
            form_name: cx.new(|cx| InputState::new(window, cx).placeholder("请输入姓名")),
            form_email: cx.new(|cx| InputState::new(window, cx).placeholder("name@example.com")),
            form_bio: cx.new(|cx| InputState::new(window, cx).placeholder("一句话介绍自己")),
            form_city: None,
            form_agree: false,
            form_result: String::new(),
            wizard,
            quiz_name,
            quiz_email,
            quiz_notify: true,
            quiz_agree: false,
            quiz_result: String::new(),
        }
    }

    /// 表单提交校验：姓名、邮箱、城市与协议。
    fn submit_form(view: Entity<Self>, window: &mut Window, cx: &mut App) {
        view.update(cx, |this, cx| {
            let name = this.form_name.read(cx).value().to_string();
            let email = this.form_email.read(cx).value().to_string();
            let bio = this.form_bio.read(cx).value().to_string();
            let mut errors = Vec::new();
            if name.trim().is_empty() {
                errors.push("姓名不能为空");
            }
            if !email.contains('@') {
                errors.push("邮箱须包含 @");
            }
            if this.form_city.is_none() {
                errors.push("请选择城市");
            }
            if !this.form_agree {
                errors.push("请先同意用户协议");
            }
            this.form_result = if errors.is_empty() {
                let city = ["北京", "上海", "广州", "深圳"][this.form_city.unwrap_or(0)];
                format!(
                    "提交成功：{name} / {email} / {city} / 简介 {} 字",
                    bio.chars().count()
                )
            } else {
                format!("校验失败：{}", errors.join("、"))
            };
            this.form_name
                .update(cx, |s, cx| s.set_value("", window, cx));
            cx.notify();
        });
    }

    /// 问卷第一步有效性：姓名非空且邮箱含 @。
    fn sync_quiz(&mut self, cx: &mut Context<Self>) {
        let valid = !self.quiz_name.read(cx).value().trim().is_empty()
            && self.quiz_email.read(cx).value().contains('@');
        self.wizard
            .update(cx, |state, cx| state.set_step_valid(0, valid, cx));
    }

    /// 问卷提交：读取第一步数据并写入结果。
    fn submit_quiz(&mut self, cx: &mut Context<Self>) {
        let name = self.quiz_name.read(cx).value().to_string();
        let email = self.quiz_email.read(cx).value().to_string();
        self.quiz_result = format!(
            "提交成功：{name} / {email} / 通知 {}",
            if self.quiz_notify { "开启" } else { "关闭" }
        );
        cx.notify();
    }
}

/// 区块标题（仅作结构标识，不承载介绍文案）。
fn section_title(text: impl Into<SharedString> + IntoElement) -> impl IntoElement {
    div()
        .text_size(px(15.0))
        .font_semibold()
        .mt(px(10.0))
        .child(text)
}

impl Render for FormGallery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let submit_view = this.clone();
        let city_view = this.clone();
        let agree_view = this.clone();
        let notify_view = this.clone();
        let quiz_agree_view = this;

        let cities: Vec<SharedString> =
            vec!["北京".into(), "上海".into(), "广州".into(), "深圳".into()];
        let city_selected = self.form_city;
        let form_ok = self.form_result.starts_with("提交成功");

        div()
            .id("form-gallery")
            .size_full()
            .overflow_y_scroll()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                v_flex()
                    .max_w(px(860.0))
                    .mx_auto()
                    .gap(px(14.0))
                    .p(px(24.0))
                    // 1. 输入框
                    .child(section_title("输入框"))
                    .child(
                        v_form()
                            .child(
                                field().label("基础").child(
                                    v_flex().w(px(360.0)).child(Input::new(&self.input_name)),
                                ),
                            )
                            .child(
                                field().label("前后缀 / 可清空").child(
                                    v_flex().w(px(360.0)).child(
                                        Input::new(&self.input_search)
                                            .prefix("https://")
                                            .suffix(".com")
                                            .cleanable(true),
                                    ),
                                ),
                            )
                            .child(
                                field().label("密码").child(
                                    v_flex().w(px(360.0)).child(
                                        Input::new(&self.input_password)
                                            .content_type(InputContentType::Password)
                                            .mask_toggle(),
                                    ),
                                ),
                            )
                            .child(
                                field().label("数字").child(
                                    v_flex()
                                        .w(px(180.0))
                                        .child(NumberInput::new(&self.input_age)),
                                ),
                            ),
                    )
                    // 2. 表单
                    .child(section_title("表单"))
                    .child(
                        v_form()
                            .child(
                                field()
                                    .label("姓名")
                                    .required(true)
                                    .child(Input::new(&self.form_name)),
                            )
                            .child(
                                field()
                                    .label("邮箱")
                                    .required(true)
                                    .description("须包含 @")
                                    .child(Input::new(&self.form_email)),
                            )
                            .child(
                                field()
                                    .label("简介")
                                    .child(TextArea::new(&self.form_bio).rows(3)),
                            )
                            .child(
                                field().label("协议").child(
                                    Checkbox::new("form-agree")
                                        .label("我已阅读并同意用户协议")
                                        .checked(self.form_agree)
                                        .on_change(move |checked, _, cx| {
                                            agree_view.update(cx, |gallery, cx| {
                                                gallery.form_agree = checked;
                                                cx.notify();
                                            });
                                        }),
                                ),
                            )
                            .child(field().label_indent(false).child(
                                Button::new("form-submit").label("提交").primary().on_click(
                                    move |_, window, cx| {
                                        FormGallery::submit_form(submit_view.clone(), window, cx);
                                    },
                                ),
                            )),
                    )
                    .child(
                        h_form().label_width(px(72.0)).child(
                            field().label("城市").required(true).child(
                                rgpui::components::Select::new(cities)
                                    .placeholder("请选择城市")
                                    .selected(city_selected)
                                    .on_change(move |ix, _, _, cx| {
                                        city_view.update(cx, |gallery, cx| {
                                            gallery.form_city = Some(ix);
                                            cx.notify();
                                        });
                                    }),
                            ),
                        ),
                    )
                    .when(!self.form_result.is_empty(), |this| {
                        let (color, bg) = if form_ok {
                            (rgb(0x107c10), rgb(0xe6f4ea))
                        } else {
                            (rgb(0xa4262c), rgb(0xfdecea))
                        };
                        this.child(
                            div()
                                .p(px(10.0))
                                .rounded_md()
                                .bg(bg)
                                .text_color(color)
                                .text_sm()
                                .child(self.form_result.clone()),
                        )
                    })
                    // 3. 问卷向导
                    .child(section_title("问卷向导"))
                    .child(
                        Questionnaire::new(&self.wizard)
                            .child(
                                v_form()
                                    .child(
                                        field()
                                            .label("姓名")
                                            .required(true)
                                            .child(Input::new(&self.quiz_name)),
                                    )
                                    .child(
                                        field()
                                            .label("邮箱")
                                            .required(true)
                                            .child(Input::new(&self.quiz_email)),
                                    ),
                            )
                            .child(
                                v_form().child(
                                    field().label("通知").child(
                                        Switch::new("quiz-notify")
                                            .label("接收消息通知")
                                            .checked(self.quiz_notify)
                                            .on_change(move |checked, _, cx| {
                                                notify_view.update(cx, |gallery, cx| {
                                                    gallery.quiz_notify = checked;
                                                    cx.notify();
                                                });
                                            }),
                                    ),
                                ),
                            )
                            .child(
                                v_flex().gap(px(12.0)).child(
                                    Checkbox::new("quiz-agree")
                                        .label("我已阅读并同意用户协议")
                                        .checked(self.quiz_agree)
                                        .on_change(move |checked, _, cx| {
                                            quiz_agree_view.update(cx, |gallery, cx| {
                                                gallery.quiz_agree = checked;
                                                gallery.wizard.update(cx, |state, cx| {
                                                    state.set_step_valid(1, checked, cx);
                                                });
                                                cx.notify();
                                            });
                                        }),
                                ),
                            ),
                    )
                    .when(!self.quiz_result.is_empty(), |this| {
                        this.child(
                            div()
                                .p(px(10.0))
                                .rounded_md()
                                .bg(rgb(0xe6f4ea))
                                .text_color(rgb(0x107c10))
                                .text_sm()
                                .child(self.quiz_result.clone()),
                        )
                    }),
            )
    }
}

fn run() {
    match std::env::args().nth(1).as_deref() {
        Some("keymap") => {
            keymap::run_example();
            return;
        }
        Some("dialog") => {
            dialog::run_example();
            return;
        }
        _ => {}
    }
    application().run(|cx: &mut App| {
        // 表单与向导依赖主题、输入子系统的按键绑定。
        rgpui::theme::init(cx);
        rgpui::input_ui::init(cx);

        let bounds = Bounds::centered(None, size(px(960.0), px(760.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| FormGallery::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run();
}
