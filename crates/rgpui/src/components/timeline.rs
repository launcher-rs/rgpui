//! 时间线。
//!
//! 纵向事件流：时间点 + 圆点标记 + 内容卡， commits/动态/日志类场景。

use crate::{prelude::*, *};

/// 时间线条目。
#[derive(Clone)]
pub struct TimelineItem {
    /// 时间标签（如 "10:24"）。
    pub time: SharedString,
    /// 标题。
    pub title: SharedString,
    /// 内容（可选）。
    pub content: Option<SharedString>,
    /// 圆点颜色（默认 accent）。
    pub dot: Option<Hsla>,
}

impl TimelineItem {
    /// 创建条目。
    pub fn new(time: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            time: time.into(),
            title: title.into(),
            content: None,
            dot: None,
        }
    }

    /// 设置内容。
    pub fn content(mut self, content: impl Into<SharedString>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// 设置圆点颜色。
    pub fn dot(mut self, color: Hsla) -> Self {
        self.dot = Some(color);
        self
    }
}

/// 时间线。
#[derive(IntoElement)]
pub struct Timeline {
    /// 条目列表。
    items: Vec<TimelineItem>,
    /// 用户样式。
    style: StyleRefinement,
}

impl Timeline {
    /// 创建时间线。
    pub fn new(items: Vec<TimelineItem>) -> Self {
        Self {
            items,
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for Timeline {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Timeline {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let border = theme.tokens.border;
        let accent = theme.tokens.accent.color;
        let muted_foreground = theme.tokens.muted_foreground.color;
        let user_style = self.style;
        let total = self.items.len();

        div()
            .flex()
            .flex_col()
            .w_full()
            .children(self.items.into_iter().enumerate().map(|(ix, item)| {
                let dot = item.dot.unwrap_or(accent);
                div()
                    .flex()
                    .flex_row()
                    .gap(px(10.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .child(
                                div()
                                    .w(px(10.0))
                                    .h(px(10.0))
                                    .mt(px(4.0))
                                    .rounded_full()
                                    .bg(dot),
                            )
                            .when(ix + 1 < total, |this| {
                                this.child(div().w(px(1.0)).flex_1().bg(border))
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap(px(2.0))
                            .pb(px(12.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(accent)
                                            .child(item.title.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(muted_foreground)
                                            .child(item.time.clone()),
                                    ),
                            )
                            .when_some(item.content, |this, content| {
                                this.child(
                                    div().text_sm().text_color(muted_foreground).child(content),
                                )
                            }),
                    )
                    .into_any_element()
            }))
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
    }
}
