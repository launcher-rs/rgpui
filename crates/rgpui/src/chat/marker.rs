//! 分隔线标记：时间分隔（“今天 14:00”）/ 未读分隔（“以下是新消息”）。
//!
//! 纯展示 RenderOnce：一行小字居中，两侧横线。

use crate::{prelude::FluentBuilder as _, *};

/// 分隔线标记。
#[derive(IntoElement)]
pub struct Marker {
    label: SharedString,
    highlight: bool,
    style: StyleRefinement,
}

impl Marker {
    /// 创建标记（`highlight` 为 true 时用强调色，如未读线）。
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            highlight: false,
            style: StyleRefinement::default(),
        }
    }

    /// 设置是否强调显示。
    pub fn highlight(mut self, highlight: bool) -> Self {
        self.highlight = highlight;
        self
    }
}

impl Styled for Marker {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Marker {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let line = theme.tokens.border;
        let color = if self.highlight {
            theme.tokens.accent.color
        } else {
            theme.tokens.muted_foreground.color
        };
        let user_style = self.style;

        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .w_full()
            .py(px(6.0))
            .child(div().h(px(1.0)).flex_1().bg(line))
            .child(div().text_xs().text_color(color).child(self.label))
            .child(div().h(px(1.0)).flex_1().bg(line))
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
    }
}
