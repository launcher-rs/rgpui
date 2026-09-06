//! 头像组件。
//!
//! 圆形头像：有图显示图片，无图显示首字母回退，右下角可选在线状态圆点。

use crate::{prelude::*, *};

/// 在线状态。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AvatarStatus {
    /// 无状态圆点。
    #[default]
    None,
    /// 在线（绿）。
    Online,
    /// 忙碌（红）。
    Busy,
    /// 离开（黄）。
    Away,
}

/// 头像。
#[derive(IntoElement)]
pub struct Avatar {
    /// 图片源（无则显示首字母）。
    src: Option<ImageSource>,
    /// 回退文字（一般取名字首字母）。
    fallback: SharedString,
    /// 尺寸（正方形边长）。
    size: Pixels,
    /// 在线状态。
    status: AvatarStatus,
    /// 用户样式。
    style: StyleRefinement,
}

impl Avatar {
    /// 创建头像（`fallback` 无图时显示，如名字首字母）。
    pub fn new(fallback: impl Into<SharedString>) -> Self {
        Self {
            src: None,
            fallback: fallback.into(),
            size: px(32.0),
            status: AvatarStatus::None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置图片源。
    pub fn src(mut self, src: impl Into<ImageSource>) -> Self {
        self.src = Some(src.into());
        self
    }

    /// 设置尺寸。
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into();
        self
    }

    /// 设置在线状态。
    pub fn status(mut self, status: AvatarStatus) -> Self {
        self.status = status;
        self
    }
}

impl Styled for Avatar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Avatar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let muted_foreground = theme.tokens.muted_foreground.color;
        let status_color = match self.status {
            AvatarStatus::None => None,
            AvatarStatus::Online => Some(rgb(0x22c55e)),
            AvatarStatus::Busy => Some(rgb(0xef4444)),
            AvatarStatus::Away => Some(rgb(0xeab308)),
        };
        let user_style = self.style;

        div()
            .relative()
            .size(self.size)
            .child(
                div()
                    .size_full()
                    .rounded_full()
                    .overflow_hidden()
                    .bg(muted_foreground.opacity(0.2))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(muted_foreground)
                    .text_sm()
                    .child(match self.src {
                        Some(src) => img(src).size_full().into_any_element(),
                        None => div().child(self.fallback.clone()).into_any_element(),
                    }),
            )
            .when_some(status_color, |this, color| {
                this.child(
                    div()
                        .absolute()
                        .right(px(0.0))
                        .bottom(px(0.0))
                        .size(px(10.0))
                        .rounded_full()
                        .bg(color)
                        .border_2()
                        .border_color(rgb(0xffffff)),
                )
            })
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
    }
}
