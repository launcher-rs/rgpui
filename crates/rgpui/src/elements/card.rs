//! 卡片容器。
//!
//! 带可选标题头的内容卡片：边框 + 圆角 + 内边距，内容区为任意子元素。

use crate::{prelude::*, *};

/// 卡片容器。
#[derive(IntoElement)]
pub struct Card {
    /// 标题（可选）。
    title: Option<SharedString>,
    /// 内容子元素。
    children: Vec<AnyElement>,
    /// 用户样式。
    style: StyleRefinement,
}

impl Card {
    /// 创建卡片。
    pub fn new() -> Self {
        Self {
            title: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// 设置标题头。
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for Card {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Card {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let border = theme.tokens.border;
        let popover = theme.tokens.popover;
        let muted_foreground = theme.tokens.muted_foreground.color;
        let user_style = self.style;
        let title = self.title;
        let children = self.children;

        div()
            .flex()
            .flex_col()
            .w_full()
            .rounded_md()
            .border_1()
            .border_color(border)
            .bg(popover)
            .overflow_hidden()
            .when_some(title, |this, title| {
                this.child(
                    div()
                        .w_full()
                        .px(px(12.0))
                        .py(px(8.0))
                        .border_b(px(1.0))
                        .border_color(border)
                        .text_sm()
                        .text_color(muted_foreground)
                        .child(title),
                )
            })
            .child(div().flex().flex_col().p(px(12.0)).children(children))
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
    }
}
