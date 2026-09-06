//! 来源引用（对标 AntDX Sources）。
//!
//! AI 回复的引用来源 chips：序号 + 标题，点击跳转。

use crate::{prelude::*, *};
use std::sync::Arc;

/// 单条引用来源。
#[derive(Clone)]
pub struct SourceItem {
    /// 标题。
    pub title: SharedString,
    /// 链接（可选，仅展示用）。
    pub url: Option<SharedString>,
}

impl SourceItem {
    /// 创建引用来源。
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            url: None,
        }
    }

    /// 设置链接。
    pub fn url(mut self, url: impl Into<SharedString>) -> Self {
        self.url = Some(url.into());
        self
    }
}

/// 来源引用列表。
#[derive(IntoElement)]
pub struct Sources {
    /// 来源列表。
    items: Vec<SourceItem>,
    /// 点击回调（下标）。
    on_open: Option<Arc<dyn Fn(usize, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl Sources {
    /// 创建来源引用列表。
    pub fn new(items: Vec<SourceItem>) -> Self {
        Self {
            items,
            on_open: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置点击回调。
    pub fn on_open<F>(mut self, f: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_open = Some(Arc::new(f));
        self
    }
}

impl Styled for Sources {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Sources {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let border = theme.tokens.border.color;
        let accent = theme.tokens.accent.color;
        let user_style = self.style;
        let on_open = self.on_open;

        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(6.0))
            .children(self.items.into_iter().enumerate().map(|(ix, item)| {
                let on_open = on_open.clone();
                div()
                    .id(SharedString::from(format!("source-{ix}")))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded_full()
                    .border_1()
                    .border_color(border)
                    .text_xs()
                    .text_color(accent)
                    .cursor_pointer()
                    .child(format!("[{}]", ix + 1))
                    .child(item.title)
                    .on_click(move |_, window, cx| {
                        if let Some(ref cb) = on_open {
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
