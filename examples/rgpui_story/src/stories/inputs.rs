//! 输入与表单示例：输入框、数字输入、密码输入、文本域、表单布局。

use rgpui::input_ui::{Input, InputContentType, InputState, NumberInput};
use rgpui::prelude::*;
use rgpui::{
    Context, Entity, IntoElement, ParentElement, Styled, Window, div, field, h_form, px, v_flex,
    v_form,
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

/// 表单布局示例视图。
struct FormStory {
    name_input: Entity<InputState>,
    email_input: Entity<InputState>,
}

impl FormStory {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入姓名"));
        let email_input = cx.new(|cx| InputState::new(window, cx).placeholder("name@example.com"));
        Self {
            name_input,
            email_input,
        }
    }
}

impl rgpui::Render for FormStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("form-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("垂直表单（v_form）"))
            .child(
                v_form()
                    .child(field().label("姓名").child(Input::new(&self.name_input)))
                    .child(field().label("邮箱").child(Input::new(&self.email_input))),
            )
            .child(div().h(px(16.0)))
            .child(section_title("水平表单（h_form）"))
            .child(
                h_form()
                    .child(field().label("姓名").child(Input::new(&self.name_input)))
                    .child(field().label("邮箱").child(Input::new(&self.email_input))),
            )
    }
}

/// 章节标题辅助函数。
fn section_title(text: impl Into<rgpui::SharedString> + IntoElement) -> impl IntoElement {
    div()
        .text_size(px(14.0))
        .text_color(rgpui::hsla(0.0, 0.0, 0.55, 1.0))
        .child(text)
}
