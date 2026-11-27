//! 输入与表单示例：输入框、数字输入、密码输入、文本域、下拉选择、单选/复选、开关、滑块、表单布局。

use rgpui::input_ui::{Input, InputContentType, InputState, NumberInput, TextArea};
use rgpui::prelude::*;
use rgpui::{
    Checkbox, Context, Entity, IntoElement, ParentElement, Radio, RadioGroup, SharedString, Slider,
    SliderState, Styled, Switch, Window, div, field, h_form, px, v_flex, v_form,
};

use super::StoryItem;

/// 输入与表单故事注册表。
pub fn stories() -> Vec<StoryItem> {
    vec![
        StoryItem {
            title: "输入框",
            build: |window, cx| cx.new(|cx| InputStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "数字输入",
            build: |window, cx| cx.new(|cx| NumberInputStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "密码与掩码",
            build: |window, cx| cx.new(|cx| PasswordStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "文本域",
            build: |window, cx| cx.new(|cx| TextAreaStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "下拉选择",
            build: |window, cx| cx.new(|cx| SelectStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "单选与复选",
            build: |window, cx| cx.new(|cx| ChoiceStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "开关",
            build: |window, cx| cx.new(|cx| SwitchStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "滑块",
            build: |window, cx| cx.new(|cx| SliderStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "表单布局",
            build: |window, cx| cx.new(|cx| FormStory::new(window, cx)).into(),
        },
    ]
}

/// 输入框示例视图。
struct InputStory {
    input: Entity<InputState>,
}

impl InputStory {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入内容"));
        Self { input }
    }
}

impl rgpui::Render for InputStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("input-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("基础输入框"))
            .child(v_flex().w(px(320.0)).child(Input::new(&self.input)))
            .child(div().h(px(16.0)))
            .child(section_title("带前缀后缀"))
            .child(
                v_flex()
                    .w(px(320.0))
                    .child(Input::new(&self.input).prefix("https://").suffix(".com")),
            )
            .child(div().h(px(16.0)))
            .child(section_title("可清空"))
            .child(
                v_flex()
                    .w(px(320.0))
                    .child(Input::new(&self.input).cleanable(true)),
            )
    }
}

/// 数字输入示例视图。
struct NumberInputStory {
    input: Entity<InputState>,
}

impl NumberInputStory {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("0"));
        Self { input }
    }
}

impl rgpui::Render for NumberInputStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("number-input-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("数字输入（NumberInput）"))
            .child(v_flex().w(px(320.0)).child(NumberInput::new(&self.input)))
    }
}

/// 密码输入示例视图。
struct PasswordStory {
    input: Entity<InputState>,
}

impl PasswordStory {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入密码"));
        Self { input }
    }
}

impl rgpui::Render for PasswordStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("password-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("密码输入（掩码切换）"))
            .child(
                v_flex().w(px(320.0)).child(
                    Input::new(&self.input)
                        .content_type(InputContentType::Password)
                        .mask_toggle(),
                ),
            )
    }
}

/// 文本域示例视图。
struct TextAreaStory {
    input: Entity<InputState>,
}

impl TextAreaStory {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .placeholder("请输入多行内容")
        });
        Self { input }
    }
}

impl rgpui::Render for TextAreaStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("textarea-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("文本域（TextArea）"))
            .child(
                v_flex()
                    .w(px(420.0))
                    .child(TextArea::new(&self.input).rows(5)),
            )
    }
}

/// 下拉选择示例视图。
struct SelectStory {
    selected: Option<usize>,
}

impl SelectStory {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { selected: None }
    }
}

impl rgpui::Render for SelectStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let cities: Vec<SharedString> =
            vec!["北京".into(), "上海".into(), "广州".into(), "深圳".into()];
        v_flex()
            .id("select-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("下拉选择（Select）"))
            .child(
                v_flex().w(px(260.0)).child(
                    rgpui::components::Select::new(cities)
                        .placeholder("请选择城市")
                        .selected(self.selected)
                        .on_change(move |ix, _, _, cx| {
                            this.update(cx, |this, cx| {
                                this.selected = Some(ix);
                                cx.notify();
                            });
                        }),
                ),
            )
            .child(div().text_sm().child(format!(
                "当前选择：{}",
                self.selected.map(|i| i + 1).unwrap_or(0)
            )))
    }
}

/// 单选与复选示例视图。
struct ChoiceStory {
    check: bool,
    radio: Option<usize>,
}

impl ChoiceStory {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            check: false,
            radio: None,
        }
    }
}

impl rgpui::Render for ChoiceStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let check_view = this.clone();
        v_flex()
            .id("choice-story")
            .gap(px(10.0))
            .p(px(16.0))
            .child(section_title("复选框（Checkbox）"))
            .child(
                Checkbox::new("story-check")
                    .label("我已阅读并同意")
                    .checked(self.check)
                    .on_change(move |checked, _, cx| {
                        check_view.update(cx, |this, cx| {
                            this.check = checked;
                            cx.notify();
                        });
                    }),
            )
            .child(div().h(px(8.0)))
            .child(section_title("单选组（RadioGroup）"))
            .child(
                RadioGroup::horizontal("story-radio")
                    .child(Radio::new("r1").label("选项一"))
                    .child(Radio::new("r2").label("选项二"))
                    .child(Radio::new("r3").label("选项三"))
                    .selected_index(self.radio)
                    .on_change(move |ix, _, cx| {
                        this.update(cx, |this, cx| {
                            this.radio = Some(ix);
                            cx.notify();
                        });
                    }),
            )
    }
}

/// 开关示例视图。
struct SwitchStory {
    on: bool,
}

impl SwitchStory {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { on: true }
    }
}

impl rgpui::Render for SwitchStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        v_flex()
            .id("switch-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("开关（Switch）"))
            .child(
                Switch::new("story-switch")
                    .label("接收消息通知")
                    .checked(self.on)
                    .on_change(move |checked, _, cx| {
                        this.update(cx, |this, cx| {
                            this.on = checked;
                            cx.notify();
                        });
                    }),
            )
            .child(
                div()
                    .text_sm()
                    .child(format!("状态：{}", if self.on { "开启" } else { "关闭" })),
            )
    }
}

/// 滑块示例视图。
struct SliderStory {
    state: Entity<SliderState>,
}

impl SliderStory {
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            state: cx.new(|_| SliderState::new()),
        }
    }
}

impl rgpui::Render for SliderStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("slider-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("滑块（Slider）"))
            .child(
                v_flex()
                    .w(px(360.0))
                    .child(Slider::new(&self.state).horizontal()),
            )
    }
}

/// 表单布局示例视图。
struct FormStory {
    name: Entity<InputState>,
    email: Entity<InputState>,
}

impl FormStory {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            name: cx.new(|cx| InputState::new(window, cx).placeholder("请输入姓名")),
            email: cx.new(|cx| InputState::new(window, cx).placeholder("name@example.com")),
        }
    }
}

impl rgpui::Render for FormStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("form-story")
            .gap(px(12.0))
            .p(px(16.0))
            .child(section_title("垂直表单（v_form）"))
            .child(
                v_form()
                    .child(
                        field()
                            .label("姓名")
                            .required(true)
                            .child(Input::new(&self.name)),
                    )
                    .child(
                        field()
                            .label("邮箱")
                            .required(true)
                            .child(Input::new(&self.email)),
                    ),
            )
            .child(section_title("水平表单（h_form + label_width）"))
            .child(
                h_form()
                    .label_width(px(72.0))
                    .child(field().label("姓名").child(Input::new(&self.name)))
                    .child(field().label("邮箱").child(Input::new(&self.email))),
            )
    }
}

/// 小节标题。
fn section_title(text: impl Into<SharedString> + IntoElement) -> impl IntoElement {
    div().text_sm().font_semibold().child(text)
}
