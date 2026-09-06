//! Chat UI 组件 —— 聊天消息展示组件。

use crate::{
    Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, StyledExt, Window,
    div, v_flex,
};

/// 消息类型。
#[derive(Debug, Clone)]
pub enum MessageType {
    /// 文本消息。
    Text(String),
    /// 代码块。
    CodeBlock {
        /// 语言标识符。
        language: String,
        /// 代码内容。
        code: String,
    },
}

/// 消息。
#[derive(Debug, Clone)]
pub struct Message {
    /// 消息内容。
    pub content: MessageType,
    /// 消息 ID。
    pub id: String,
}

impl Message {
    /// 创建文本消息。
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: MessageType::Text(content.into()),
            id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// 创建代码块消息。
    pub fn code_block(language: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            content: MessageType::CodeBlock {
                language: language.into(),
                code: code.into(),
            },
            id: uuid::Uuid::new_v4().to_string(),
        }
    }
}

/// 消息组（同一发送者的连续消息）。
#[derive(Debug, Clone)]
pub struct MessageGroup {
    /// 发送者名称。
    pub sender: SharedString,
    /// 消息列表。
    pub messages: Vec<Message>,
}

/// 聊天状态。
#[derive(Default)]
pub struct ChatState {
    /// 消息组列表。
    pub groups: Vec<MessageGroup>,
    /// 当前输入文本。
    pub input_text: String,
}

impl ChatState {
    /// 添加消息组。
    pub fn add_group(&mut self, group: MessageGroup) {
        self.groups.push(group);
    }

    /// 清空消息。
    pub fn clear(&mut self) {
        self.groups.clear();
    }
}

/// 聊天视图组件。
pub struct ChatView {
    state: Entity<ChatState>,
}

impl ChatView {
    /// 创建新的聊天视图。
    pub fn new(state: Entity<ChatState>) -> Self {
        Self { state }
    }
}

impl Render for ChatView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        v_flex()
            .w_full()
            .h_full()
            .children(state.groups.iter().map(|group| {
                v_flex()
                    .w_full()
                    .gap_1()
                    .py_2()
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(crate::gray_300())
                            .child(group.sender.clone()),
                    )
                    .children(group.messages.iter().map(|msg| {
                        match &msg.content {
                            MessageType::Text(text) => div()
                                .text_sm()
                                .text_color(crate::gray_100())
                                .child(text.clone()),
                            MessageType::CodeBlock { language, code } => v_flex()
                                .w_full()
                                .bg(crate::gray_800())
                                .rounded_md()
                                .overflow_hidden()
                                .child(
                                    div()
                                        .px_3()
                                        .py_1()
                                        .bg(crate::gray_700())
                                        .text_xs()
                                        .text_color(crate::gray_400())
                                        .child(language.clone()),
                                )
                                .child(div().px_3().py_2().text_sm().child(code.clone())),
                        }
                    }))
            }))
    }
}
