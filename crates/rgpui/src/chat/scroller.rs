//! 消息滚动容器：吸底 + 新消息浮标。
//!
//! 新消息到达时：用户在底部则自动吸底；用户翻上去看历史则累积未读数，
//! 显示“↓ N 条新消息”浮标，点击回到最新并清零。

use crate::{prelude::FluentBuilder as _, *};

/// 消息滚动状态实体。
pub struct MessageScrollerState {
    scroll: ScrollHandle,
    last_count: usize,
    unread: usize,
}

impl MessageScrollerState {
    /// 创建滚动状态。
    pub fn new() -> Self {
        Self {
            scroll: ScrollHandle::new(),
            last_count: 0,
            unread: 0,
        }
    }

    /// 底层滚动句柄（供父组件手动控制）。
    pub fn scroll_handle(&self) -> &ScrollHandle {
        &self.scroll
    }

    /// 未读新消息数。
    pub fn unread(&self) -> usize {
        self.unread
    }

    /// 回到底部并清零未读。
    pub fn scroll_to_bottom(&mut self, cx: &mut Context<Self>) {
        self.scroll.scroll_to_bottom();
        self.unread = 0;
        cx.notify();
    }
}

impl Default for MessageScrollerState {
    fn default() -> Self {
        Self::new()
    }
}

/// 消息滚动容器元素（状态实体 + 消息 children）。
#[derive(IntoElement)]
pub struct MessageScroller {
    state: Entity<MessageScrollerState>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl MessageScroller {
    /// 由状态实体创建。
    pub fn new(state: Entity<MessageScrollerState>) -> Self {
        Self {
            state,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }
}

impl ParentElement for MessageScroller {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for MessageScroller {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for MessageScroller {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let count = self.children.len();
        let (last_count, unread) = self
            .state
            .read_with(cx, |state, _| (state.last_count, state.unread));

        // 新消息到达：底部吸底，否则累积未读（只在变化时 notify，避免渲染循环）。
        if count > last_count {
            let at_bottom = self.state.read_with(cx, |state, _| {
                let scrolled = (-state.scroll.offset().y).max(px(0.0));
                let max = state.scroll.max_offset().y;
                max <= px(0.0) || scrolled >= max - px(2.0)
            });
            self.state.update(cx, |state, cx| {
                state.last_count = count;
                if at_bottom {
                    state.scroll.scroll_to_bottom();
                } else {
                    state.unread += count - last_count;
                }
                cx.notify();
            });
        } else if count < last_count {
            // 消息被清空/截断：同步计数并清零未读。
            self.state.update(cx, |state, cx| {
                state.last_count = count;
                state.unread = 0;
                cx.notify();
            });
        }

        let theme = cx.theme();
        let accent = theme.tokens.accent.color;
        let user_style = self.style;
        let scroll = self.state.read_with(cx, |state, _| state.scroll.clone());
        let show_pill = unread > 0;
        let state = self.state.clone();

        div()
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .overflow_hidden()
            .child(
                div()
                    .id("message-scroller")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .track_scroll(&scroll)
                    .children(self.children),
            )
            .when(show_pill, |this| {
                this.child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .pb(px(12.0))
                        .child(
                            div()
                                .id("message-scroller-new")
                                .px(px(12.0))
                                .py(px(6.0))
                                .rounded_full()
                                .bg(accent)
                                .text_sm()
                                .text_color(crate::rgb(0xffffff))
                                .cursor_pointer()
                                .child(format!("↓ {unread} 条新消息"))
                                .on_click(move |_, _, cx| {
                                    state.update(cx, |state, cx| {
                                        state.scroll.scroll_to_bottom();
                                        state.unread = 0;
                                        cx.notify();
                                    });
                                }),
                        ),
                )
            })
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
    }
}
