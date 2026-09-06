//! 步骤条。
//!
//! 向导式流程指示：序号圆点 + 标题 + 连接线，当前步骤高亮，已完成打勾。

use crate::{prelude::*, *};

/// 单个步骤。
#[derive(Clone)]
pub struct StepItem {
    /// 标题。
    pub title: SharedString,
    /// 描述（可选）。
    pub description: Option<SharedString>,
}

impl StepItem {
    /// 创建步骤。
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
        }
    }

    /// 设置描述。
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// 步骤条。
#[derive(IntoElement)]
pub struct Steps {
    /// 步骤列表。
    items: Vec<StepItem>,
    /// 当前步骤下标（0 起始）。
    current: usize,
    /// 用户样式。
    style: StyleRefinement,
}

impl Steps {
    /// 创建步骤条。
    pub fn new(items: Vec<StepItem>, current: usize) -> Self {
        Self {
            items,
            current,
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for Steps {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Steps {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let accent = theme.tokens.accent.color;
        let muted_foreground = theme.tokens.muted_foreground.color;
        let border = theme.tokens.border.color;
        let user_style = self.style;
        let current = self.current;
        let total = self.items.len();

        div()
            .flex()
            .flex_row()
            .items_start()
            .w_full()
            .children(self.items.into_iter().enumerate().map(|(ix, item)| {
                let done = ix < current;
                let active = ix == current;
                let dot = if done { "✓" } else { &ix.to_string() };
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .items_start()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(24.0))
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .text_xs()
                            .border_1()
                            .border_color(if done || active { accent } else { border })
                            .when(done || active, |this| this.bg(accent.opacity(0.15)))
                            .text_color(if done || active {
                                accent
                            } else {
                                muted_foreground
                            })
                            .child(dot.to_string()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(if active { accent } else { muted_foreground })
                                    .child(item.title.clone()),
                            )
                            .when_some(item.description, |this, desc| {
                                this.child(div().text_xs().text_color(muted_foreground).child(desc))
                            }),
                    )
                    .when(ix + 1 < total, |this| {
                        this.child(div().flex_1().h(px(1.0)).mt(px(12.0)).bg(if done {
                            accent
                        } else {
                            border
                        }))
                    })
                    .into_any_element()
            }))
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
    }
}
