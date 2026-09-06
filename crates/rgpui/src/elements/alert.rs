//! 行内提示条。
//!
//! 四种变体（info/success/warning/danger）的行内消息展示，带图标与可选关闭按钮。
//! 与 `dialog/alert_dialog.rs` 的模态弹窗区分：Alert 不阻塞交互。

use crate::{prelude::*, *};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// Alert 实例计数器（关闭按钮 ID 唯一，避免同页多实例冲突）。
static ALERT_ID: AtomicU64 = AtomicU64::new(0);

/// 提示条变体。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlertVariant {
    /// 信息（默认，蓝）。
    #[default]
    Info,
    /// 成功（绿）。
    Success,
    /// 警告（黄）。
    Warning,
    /// 危险（红）。
    Danger,
}

/// 行内提示条。
#[derive(IntoElement)]
pub struct Alert {
    /// 元素 ID（默认唯一生成）。
    id: SharedString,
    /// 变体。
    variant: AlertVariant,
    /// 标题。
    title: SharedString,
    /// 正文（可选）。
    body: Option<SharedString>,
    /// 是否可关闭。
    closable: bool,
    /// 关闭回调。
    on_close: Option<Arc<dyn Fn(&mut Window, &mut App) + Send + Sync + 'static>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl Alert {
    /// 创建提示条。
    pub fn new(title: impl Into<SharedString>) -> Self {
        let id = ALERT_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            id: SharedString::from(format!("alert-{id}")),
            variant: AlertVariant::Info,
            title: title.into(),
            body: None,
            closable: false,
            on_close: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置变体。
    pub fn variant(mut self, variant: AlertVariant) -> Self {
        self.variant = variant;
        self
    }

    /// 设置正文。
    pub fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// 设置可关闭（右上角关闭按钮，点击触发 `on_close`）。
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    /// 设置关闭回调。
    pub fn on_close<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_close = Some(Arc::new(f));
        self
    }
}

impl Styled for Alert {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Alert {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let border = theme.tokens.border;
        let muted_foreground = theme.tokens.muted_foreground.color;

        let (accent, icon) = match self.variant {
            AlertVariant::Info => (rgb(0x3b82f6), IconName::Info),
            AlertVariant::Success => (rgb(0x22c55e), IconName::CircleCheck),
            AlertVariant::Warning => (rgb(0xeab308), IconName::TriangleAlert),
            AlertVariant::Danger => (rgb(0xef4444), IconName::CircleX),
        };
        let user_style = self.style;

        div()
            .flex()
            .flex_row()
            .w_full()
            .rounded_md()
            .border_1()
            .border_color(border)
            .bg(accent.opacity(0.08))
            .overflow_hidden()
            .child(div().w(px(3.0)).flex_shrink_0().bg(accent))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(8.0))
                    .flex_1()
                    .px(px(12.0))
                    .py(px(10.0))
                    .child(div().text_color(accent).child(icon))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap(px(2.0))
                            .child(div().text_sm().text_color(accent).child(self.title))
                            .when_some(self.body, |this, body| {
                                this.child(div().text_xs().text_color(muted_foreground).child(body))
                            }),
                    ),
            )
            .when(self.closable, |this| {
                let on_close = self.on_close.clone();
                let close_id = SharedString::from(format!("{}-close", self.id));
                this.child(
                    Button::new(close_id)
                        .ghost()
                        .small()
                        .icon(IconName::Close)
                        .on_click(move |_, window, cx| {
                            if let Some(ref cb) = on_close {
                                cb(window, cx);
                            }
                        }),
                )
            })
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
    }
}
