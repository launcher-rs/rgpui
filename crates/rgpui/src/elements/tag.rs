use crate::{
    AbsoluteLength, ActiveTheme, AnyElement, App, ColorName, ElementSize, Hsla,
    InteractiveElement as _, IntoElement, ParentElement, RenderOnce, Sizable,
    StatefulInteractiveElement as _, StyleRefinement, Styled, StyledExt as _, Window, div,
    relative, rems, transparent_white,
};
use crate::prelude::FluentBuilder as _;

/// Tag 的变体。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagVariant {
    /// 主色
    Primary,
    /// 次要色
    #[default]
    Secondary,
    /// 危险
    Danger,
    /// 成功
    Success,
    /// 警告
    Warning,
    /// 信息
    Info,
    /// 指定颜色名
    Color(ColorName),
    /// 自定义颜色
    Custom {
        /// 背景色
        color: Hsla,
        /// 前景色
        foreground: Hsla,
        /// 边框色
        border: Hsla,
    },
}

impl TagVariant {
    fn bg(&self, cx: &App) -> Hsla {
        match self {
            Self::Primary => cx.theme().primary,
            Self::Secondary => cx.theme().secondary,
            Self::Danger => cx.theme().danger,
            Self::Success => cx.theme().success,
            Self::Warning => cx.theme().warning,
            Self::Info => cx.theme().info,
            Self::Color(color) => {
                if cx.theme().is_dark() {
                    color.scale(950).opacity(0.5)
                } else {
                    color.scale(50)
                }
            }
            Self::Custom { color, .. } => *color,
        }
    }

    fn border(&self, cx: &App) -> Hsla {
        match self {
            Self::Primary => cx.theme().primary,
            Self::Secondary => cx.theme().border,
            Self::Danger => cx.theme().danger,
            Self::Success => cx.theme().success,
            Self::Warning => cx.theme().warning,
            Self::Info => cx.theme().info,
            Self::Color(color) => {
                if cx.theme().is_dark() {
                    color.scale(800).opacity(0.5)
                } else {
                    color.scale(200)
                }
            }
            Self::Custom { border, .. } => *border,
        }
    }

    fn fg(&self, outline: bool, cx: &App) -> Hsla {
        match self {
            Self::Primary => {
                if outline {
                    cx.theme().primary
                } else {
                    cx.theme().primary_foreground
                }
            }
            Self::Secondary => {
                if outline {
                    cx.theme().muted_foreground
                } else {
                    cx.theme().secondary_foreground
                }
            }
            Self::Danger => {
                if outline {
                    cx.theme().danger
                } else {
                    cx.theme().danger_foreground
                }
            }
            Self::Success => {
                if outline {
                    cx.theme().success
                } else {
                    cx.theme().success_foreground
                }
            }
            Self::Warning => {
                if outline {
                    cx.theme().warning
                } else {
                    cx.theme().warning_foreground
                }
            }
            Self::Info => {
                if outline {
                    cx.theme().info
                } else {
                    cx.theme().info_foreground
                }
            }
            Self::Color(color) => {
                if cx.theme().is_dark() {
                    color.scale(300)
                } else {
                    color.scale(600)
                }
            }
            Self::Custom { foreground, .. } => *foreground,
        }
    }
}

/// Tag 是一个小型状态指示器。
///
/// 仅支持：Medium、Small
#[derive(IntoElement)]
pub struct Tag {
    /// 样式精炼
    style: StyleRefinement,
    /// 变体
    variant: TagVariant,
    /// 是否描边
    outline: bool,
    /// 尺寸
    size: ElementSize,
    /// 圆角
    rounded: Option<AbsoluteLength>,
    /// 子元素
    children: Vec<AnyElement>,
}

impl Tag {
    /// 创建新的 Tag。
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            variant: TagVariant::default(),
            outline: false,
            size: ElementSize::default(),
            rounded: None,
            children: Vec::new(),
        }
    }

    /// 创建默认变体为 [`TagVariant::Primary`] 的新 Tag。
    pub fn primary() -> Self {
        Self::new().with_variant(TagVariant::Primary)
    }

    /// 创建默认变体为 [`TagVariant::Secondary`] 的新 Tag。
    pub fn secondary() -> Self {
        Self::new().with_variant(TagVariant::Secondary)
    }

    /// 创建默认变体为 [`TagVariant::Danger`] 的新 Tag。
    pub fn danger() -> Self {
        Self::new().with_variant(TagVariant::Danger)
    }

    /// 创建默认变体为 [`TagVariant::Success`] 的新 Tag。
    pub fn success() -> Self {
        Self::new().with_variant(TagVariant::Success)
    }

    /// 创建默认变体为 [`TagVariant::Warning`] 的新 Tag。
    pub fn warning() -> Self {
        Self::new().with_variant(TagVariant::Warning)
    }

    /// 创建默认变体为 [`TagVariant::Info`] 的新 Tag。
    pub fn info() -> Self {
        Self::new().with_variant(TagVariant::Info)
    }

    /// 创建默认变体为 [`TagVariant::Custom`] 的新 Tag。
    pub fn custom(color: Hsla, foreground: Hsla, border: Hsla) -> Self {
        Self::new().with_variant(TagVariant::Custom {
            color,
            foreground,
            border,
        })
    }

    /// 创建默认变体为 [`TagVariant::Color`] 的新 Tag。
    pub fn color(color: impl Into<ColorName>) -> Self {
        Self::new().with_variant(TagVariant::Color(color.into()))
    }

    /// 设置 Tag 的变体。
    pub fn with_variant(mut self, variant: TagVariant) -> Self {
        self.variant = variant;
        self
    }

    /// 使用描边样式。
    pub fn outline(mut self) -> Self {
        self.outline = true;
        self
    }

    /// 设置圆角。
    pub fn rounded(mut self, radius: impl Into<AbsoluteLength>) -> Self {
        self.rounded = Some(radius.into());
        self
    }

    /// 设置全圆角。
    pub fn rounded_full(mut self) -> Self {
        self.rounded = Some(rems(1.).into());
        self
    }
}

impl Default for Tag {
    fn default() -> Self {
        Self::new()
    }
}

impl Sizable for Tag {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl ParentElement for Tag {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Tag {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Tag {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let bg = if self.outline {
            transparent_white()
        } else {
            self.variant.bg(cx)
        };
        let fg = self.variant.fg(self.outline, cx);
        let border = self.variant.border(cx);
        let rounded = self.rounded.unwrap_or(
            match self.size {
                ElementSize::XSmall | ElementSize::Small => cx.theme().radius / 2.,
                _ => cx.theme().radius,
            }
            .into(),
        );

        div()
            .flex()
            .items_center()
            .border_1()
            .line_height(relative(1.))
            .text_xs()
            .map(|this| match self.size {
                ElementSize::XSmall | ElementSize::Small => this.px_1p5().py_0p5(),
                _ => this.px_2p5().py_1(),
            })
            .bg(bg)
            .text_color(fg)
            .border_color(border)
            .rounded(rounded)
            .hover(|this| this.opacity(0.9))
            .refine_style(&self.style)
            .children(self.children)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 Tag 变体构造
    #[test]
    fn test_tag_variants() {
        assert_eq!(Tag::primary().variant, TagVariant::Primary);
        assert_eq!(Tag::danger().variant, TagVariant::Danger);
        assert_eq!(Tag::success().variant, TagVariant::Success);
        assert_eq!(Tag::info().variant, TagVariant::Info);
    }

    /// 测试 Tag outline
    #[test]
    fn test_tag_outline() {
        let t = Tag::primary().outline();
        assert!(t.outline);
    }

    /// 测试 Tag 圆角
    #[test]
    fn test_tag_rounded() {
        let t = Tag::new().rounded_full();
        assert!(t.rounded.is_some());
    }
}