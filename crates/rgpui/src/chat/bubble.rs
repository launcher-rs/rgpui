//! 聊天气泡：左右布局 + 圆角 + 可选头像。
//!
//! 内容由调用方经 `ParentElement` 传入（文本/代码块/附件均可），
//! 本组件只负责“容器层”（对齐、气泡底色、头像槽位）。

use crate::{prelude::FluentBuilder as _, *};

/// 气泡朝向（左：对方/助手；右：自己/用户）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BubbleSide {
    /// 左侧（对方消息）。
    #[default]
    Left,
    /// 右侧（自己消息）。
    Right,
}

/// 聊天气泡容器。
#[derive(IntoElement)]
pub struct Bubble {
    side: BubbleSide,
    avatar: Option<AnyElement>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl Bubble {
    /// 创建左侧气泡。
    pub fn new() -> Self {
        Self {
            side: BubbleSide::Left,
            avatar: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// 设置朝向。
    pub fn side(mut self, side: BubbleSide) -> Self {
        self.side = side;
        self
    }

    /// 设置头像（任意元素，如 `Avatar`）。
    pub fn avatar(mut self, avatar: impl IntoElement) -> Self {
        self.avatar = Some(avatar.into_any_element());
        self
    }
}

impl Default for Bubble {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for Bubble {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Bubble {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Bubble {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let accent = theme.tokens.accent.color;
        let muted = theme.tokens.muted;
        let user_style = self.style;

        let bubble_bg = match self.side {
            BubbleSide::Left => muted.opacity(0.12),
            BubbleSide::Right => accent.opacity(0.18),
        };

        let mut row = div().flex().w_full().gap(px(8.0)).items_start().py(px(4.0));
        row = match self.side {
            BubbleSide::Left => row.justify_start(),
            BubbleSide::Right => row.justify_end().flex_row_reverse(),
        };

        if let Some(avatar) = self.avatar {
            row = row.child(div().flex_shrink_0().child(avatar));
        }

        row.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .px(px(12.0))
                .py(px(8.0))
                .rounded_lg()
                .bg(bubble_bg)
                .max_w(relative(0.75))
                .children(self.children),
        )
        .map(|mut this| {
            this.style().refine(&user_style);
            this
        })
    }
}
