//! 追问建议行（对标 AntDX Suggestion）。
//!
//! AI 回复下的横向建议 chips，点击快速追问。

use crate::{prelude::*, *};
use std::sync::Arc;

/// 追问建议行。
#[derive(IntoElement)]
pub struct Suggestion {
    /// 建议文本列表。
    items: Vec<SharedString>,
    /// 选中回调（下标, 文本）。
    on_pick: Option<Arc<dyn Fn(usize, &SharedString, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl Suggestion {
    /// 创建建议行。
    pub fn new(items: Vec<SharedString>) -> Self {
        Self {
            items,
            on_pick: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置选中回调。
    pub fn on_pick<F>(mut self, f: F) -> Self
    where
        F: Fn(usize, &SharedString, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_pick = Some(Arc::new(f));
        self
    }
}

impl Styled for Suggestion {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Suggestion {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let border = theme.tokens.border.color;
        let accent = theme.tokens.accent.color;
        let user_style = self.style;
        let on_pick = self.on_pick;

        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(6.0))
            .children(self.items.into_iter().enumerate().map(|(ix, text)| {
                let on_pick = on_pick.clone();
                let label = text.clone();
                div()
                    .id(SharedString::from(format!("suggestion-{ix}")))
                    .px(px(10.0))
                    .py(px(6.0))
                    .rounded_full()
                    .border_1()
                    .border_color(border)
                    .text_xs()
                    .text_color(accent)
                    .cursor_pointer()
                    .hover(|this| this.bg(accent.opacity(0.1)))
                    .child(text)
                    .on_click(move |_, window, cx| {
                        if let Some(ref cb) = on_pick {
                            cb(ix, &label, window, cx);
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
