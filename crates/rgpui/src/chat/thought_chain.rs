//! 思维链（对标 AntDX ThoughtChain）。
//!
//! 推理步骤折叠列表：状态圆点 + 标题，点击展开看详情。

use crate::{prelude::*, *};
use std::sync::Arc;

/// 步骤状态。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThoughtStatus {
    /// 等待。
    #[default]
    Pending,
    /// 执行中。
    Running,
    /// 完成。
    Done,
    /// 失败。
    Failed,
}

/// 思维链步骤。
#[derive(Clone)]
pub struct ThoughtStep {
    /// 标题。
    pub title: SharedString,
    /// 详情（展开可见）。
    pub detail: SharedString,
    /// 状态。
    pub status: ThoughtStatus,
}

impl ThoughtStep {
    /// 创建步骤。
    pub fn new(
        title: impl Into<SharedString>,
        detail: impl Into<SharedString>,
        status: ThoughtStatus,
    ) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            status,
        }
    }
}

/// 思维链。
#[derive(IntoElement)]
pub struct ThoughtChain {
    /// 是否展开全部详情。
    expanded: bool,
    /// 步骤列表。
    steps: Vec<ThoughtStep>,
    /// 展开切换回调（状态由父持有）。
    on_toggle: Option<Arc<dyn Fn(bool, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl ThoughtChain {
    /// 创建思维链。
    pub fn new(steps: Vec<ThoughtStep>) -> Self {
        Self {
            expanded: false,
            steps,
            on_toggle: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置是否展开。
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// 设置展开切换回调。
    pub fn on_toggle<F>(mut self, f: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_toggle = Some(Arc::new(f));
        self
    }
}

impl Styled for ThoughtChain {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ThoughtChain {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let accent = theme.tokens.accent.color;
        let muted_foreground = theme.tokens.muted_foreground.color;
        let user_style = self.style;
        let expanded = self.expanded;
        let on_toggle = self.on_toggle;

        div()
            .flex()
            .flex_col()
            .w_full()
            .gap(px(4.0))
            .child(
                div()
                    .id("thought-chain-toggle")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .cursor_pointer()
                    .text_sm()
                    .text_color(muted_foreground)
                    .child(if expanded {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .child("思考过程")
                    .on_click(move |_, window, cx| {
                        if let Some(ref cb) = on_toggle {
                            cb(!expanded, window, cx);
                        }
                    }),
            )
            .when(expanded, |this| {
                this.children(self.steps.into_iter().enumerate().map(|(ix, step)| {
                    let dot = match step.status {
                        ThoughtStatus::Pending => muted_foreground,
                        ThoughtStatus::Running => accent,
                        ThoughtStatus::Done => green(),
                        ThoughtStatus::Failed => red(),
                    };
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(8.0))
                        .items_start()
                        .child(
                            div()
                                .w(px(8.0))
                                .h(px(8.0))
                                .mt(px(5.0))
                                .rounded_full()
                                .bg(dot)
                                .flex_shrink_0(),
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
                                        .text_color(muted_foreground)
                                        .child(format!("{}. {}", ix + 1, step.title)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted_foreground)
                                        .opacity(0.8)
                                        .child(step.detail),
                                ),
                        )
                        .into_any_element()
                }))
            })
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
    }
}
