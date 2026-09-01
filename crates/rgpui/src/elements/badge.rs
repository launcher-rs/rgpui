//! 徽章组件，用于显示计数、状态或简短标签的角标元素。

use crate::prelude::FluentBuilder as _;
use crate::{
    ActiveTheme, AnyElement, App, ElementSize, Hsla, Icon, IntoElement, ParentElement, RenderOnce,
    Sizable, StyleRefinement, Styled, StyledExt as _, Window, div, h_flex, px, relative, white,
};

/// 徽标变体。
#[derive(Default, Clone)]
enum BadgeVariant {
    /// 显示数字计数
    #[default]
    Number,
    /// 显示圆点
    Dot,
    /// 显示图标
    Icon(Box<Icon>),
}

#[allow(unused)]
impl BadgeVariant {
    #[inline]
    fn is_icon(&self) -> bool {
        matches!(self, BadgeVariant::Icon(_))
    }

    #[inline]
    fn is_number(&self) -> bool {
        matches!(self, BadgeVariant::Number)
    }
}

/// 用于在元素上显示计数、圆点或图标的徽标（Badge）。
#[derive(IntoElement)]
pub struct Badge {
    /// 样式精炼
    style: StyleRefinement,
    /// 计数
    count: usize,
    /// 最大显示计数
    max: usize,
    /// 变体
    variant: BadgeVariant,
    /// 子元素
    children: Vec<AnyElement>,
    /// 背景颜色
    color: Option<Hsla>,
    /// 尺寸
    size: ElementSize,
}

impl Badge {
    /// 创建新的徽标。
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            count: 0,
            max: 99,
            variant: Default::default(),
            color: None,
            children: Vec::new(),
            size: ElementSize::default(),
        }
    }

    /// 设置为 [`BadgeVariant::Dot`] 显示圆点。
    pub fn dot(mut self) -> Self {
        self.variant = BadgeVariant::Dot;
        self
    }

    /// 设置为 [`BadgeVariant::Number`] 显示计数。
    ///
    /// 如果计数为 0，徽标将被隐藏。
    pub fn count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    /// 设置为 [`BadgeVariant::Icon`] 显示图标。
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.variant = BadgeVariant::Icon(Box::new(icon.into()));
        self
    }

    /// 设置显示的最大计数（仅当使用 [`BadgeVariant::Number`] 时生效）。
    pub fn max(mut self, max: usize) -> Self {
        self.max = max;
        self
    }

    /// 设置徽标的（背景）颜色。
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl Default for Badge {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for Badge {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Sizable for Badge {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let visible = match self.variant {
            BadgeVariant::Number => self.count > 0,
            BadgeVariant::Dot | BadgeVariant::Icon(_) => true,
        };

        let (size, text_size) = match self.size {
            ElementSize::Large => (px(24.), px(14.)),
            ElementSize::Medium | ElementSize::ElementSize(_) => (px(16.), px(10.)),
            ElementSize::Small | ElementSize::XSmall => (px(10.), px(8.)),
        };

        div()
            .relative()
            .refine_style(&self.style)
            .children(self.children)
            .when(visible, |this| {
                this.child(
                    h_flex()
                        .absolute()
                        .justify_center()
                        .items_center()
                        .rounded_full()
                        .bg(self.color.unwrap_or(cx.theme().red))
                        .text_color(white())
                        .text_size(text_size)
                        .map(|this| match self.variant {
                            BadgeVariant::Dot => this.top_0().right_0().size(px(6.)),
                            BadgeVariant::Number => {
                                let count = if self.count > self.max {
                                    format!("{}+", self.max)
                                } else {
                                    self.count.to_string()
                                };

                                let (top, left) = match self.size {
                                    ElementSize::Large => (px(2.), -px(count.len() as f32)),
                                    ElementSize::Medium | ElementSize::ElementSize(_) => {
                                        (-px(3.), -px(3.) * count.len())
                                    }
                                    ElementSize::Small | ElementSize::XSmall => {
                                        (-px(4.), -px(4.) * count.len())
                                    }
                                };

                                this.top(top)
                                    .right(left)
                                    .py_0p5()
                                    .px_0p5()
                                    .min_w_3p5()
                                    .text_size(px(10.))
                                    .line_height(relative(1.))
                                    .child(count)
                            }
                            BadgeVariant::Icon(icon) => this
                                .right_0()
                                .bottom_0()
                                .size(size)
                                .border_1()
                                .border_color(cx.theme().background)
                                .child(*icon),
                        }),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 Badge 基本构造
    #[test]
    fn test_badge_build() {
        let b = Badge::new().count(5).with_size(ElementSize::Small);
        assert_eq!(b.count, 5);
        assert_eq!(b.size, ElementSize::Small);
    }

    /// 测试 Badge 变体设置
    #[test]
    fn test_badge_variant() {
        let b = Badge::new().dot();
        assert!(b.variant.is_icon() == false);
        let b2 = Badge::new().icon(crate::IconName::Bell);
        assert!(b2.variant.is_icon());
    }

    /// 测试 Badge max 截断
    #[test]
    fn test_badge_count_max() {
        let b = Badge::new().count(150).max(99);
        assert_eq!(b.max, 99);
        assert_eq!(b.count, 150);
    }
}
