//! 消息操作行（对标 AntDX Actions）。
//!
//! AI 消息下的小操作按钮组：复制/赞/踩/重试，图标按钮横排。

use crate::*;
use std::sync::Arc;

/// 内置操作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageAction {
    /// 复制。
    Copy,
    /// 赞。
    Like,
    /// 踩。
    Dislike,
    /// 重试。
    Retry,
}

/// 消息操作行。
#[derive(IntoElement)]
pub struct Actions {
    /// 操作列表。
    actions: Vec<MessageAction>,
    /// 点击回调。
    on_action: Option<Arc<dyn Fn(MessageAction, &mut Window, &mut App) + Send + Sync + 'static>>,
}

impl Actions {
    /// 创建操作行（默认复制/赞/踩）。
    pub fn new() -> Self {
        Self {
            actions: vec![
                MessageAction::Copy,
                MessageAction::Like,
                MessageAction::Dislike,
            ],
            on_action: None,
        }
    }

    /// 设置操作列表。
    pub fn actions(mut self, actions: Vec<MessageAction>) -> Self {
        self.actions = actions;
        self
    }

    /// 设置点击回调。
    pub fn on_action<F>(mut self, f: F) -> Self
    where
        F: Fn(MessageAction, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_action = Some(Arc::new(f));
        self
    }
}

impl Default for Actions {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Actions {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let muted_foreground = cx.theme().tokens.muted_foreground.color;
        let on_action = self.on_action;

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .children(self.actions.into_iter().enumerate().map(|(ix, action)| {
                let icon = match action {
                    MessageAction::Copy => IconName::Copy,
                    MessageAction::Like => IconName::ThumbsUp,
                    MessageAction::Dislike => IconName::ThumbsDown,
                    MessageAction::Retry => IconName::Redo,
                };
                let on_action = on_action.clone();
                Button::new(SharedString::from(format!("msg-action-{ix}")))
                    .ghost()
                    .small()
                    .icon(icon)
                    .text_color(muted_foreground)
                    .on_click(move |_, window, cx| {
                        if let Some(ref cb) = on_action {
                            cb(action, window, cx);
                        }
                    })
                    .into_any_element()
            }))
    }
}
