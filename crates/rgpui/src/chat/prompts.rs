//! 提示词集（对标 AntDX Prompts）。
//!
//! 一组预设提示词卡片（标题 + 描述），点击填入输入框或直接发送。

use crate::{prelude::*, *};
use std::sync::Arc;

/// 单条提示词。
#[derive(Clone)]
pub struct PromptItem {
    /// 标题。
    pub title: SharedString,
    /// 描述。
    pub description: SharedString,
}

impl PromptItem {
    /// 创建提示词。
    pub fn new(title: impl Into<SharedString>, description: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
        }
    }
}

/// 提示词集。
#[derive(IntoElement)]
pub struct Prompts {
    /// 提示词列表。
    items: Vec<PromptItem>,
    /// 选中回调（下标）。
    on_pick: Option<Arc<dyn Fn(usize, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl Prompts {
    /// 创建提示词集。
    pub fn new(items: Vec<PromptItem>) -> Self {
        Self {
            items,
            on_pick: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置选中回调。
    pub fn on_pick<F>(mut self, f: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_pick = Some(Arc::new(f));
        self
    }
}

impl Styled for Prompts {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Prompts {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let border = theme.tokens.border.color;
        let popover = theme.tokens.popover;
        let accent = theme.tokens.accent.color;
        let muted_foreground = theme.tokens.muted_foreground.color;
        let user_style = self.style;
        let on_pick = self.on_pick;

        div()
            .flex()
            .flex_col()
            .w_full()
            .gap(px(8.0))
            .children(self.items.into_iter().enumerate().map(|(ix, item)| {
                let on_pick = on_pick.clone();
                div()
                    .id(SharedString::from(format!("prompt-{ix}")))
                    .flex()
                    .flex_col()
                    .w_full()
                    .gap(px(2.0))
                    .p(px(10.0))
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .bg(popover)
                    .cursor_pointer()
                    .hover(|this| this.border_color(accent))
                    .child(div().text_sm().text_color(accent).child(item.title))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted_foreground)
                            .child(item.description),
                    )
                    .on_click(move |_, window, cx| {
                        if let Some(ref cb) = on_pick {
                            cb(ix, window, cx);
                        }
                    })
                    .into_any_element()
            }))
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
    }
}
