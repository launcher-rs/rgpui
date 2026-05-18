use crate::{ColorName, Sizable, Size, StyledExt, theme::ActiveTheme as _};
use rgpui::{
    AbsoluteLength, AnyElement, App, Hsla, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, relative, rems,
    transparent_white,
};

/// 标签的变体类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagVariant {
    /// 主要标签
    Primary,
    /// 次要标签（默认）
    #[default]
    Secondary,
    /// 危险/错误标签
    Danger,
    /// 成功标签
    Success,
    /// 警告标签
    Warning,
    /// 信息标签
    Info,
    /// 自定义颜色名称
    Color(ColorName),
    /// 完全自定义颜色
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
    /// 获取背景色
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

    /// 获取边框色
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

    /// 获取前景色
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

/// 标签组件，用于显示小型状态指示器
///
/// 仅支持：Medium, Small 尺寸
#[derive(IntoElement)]
pub struct Tag {
    /// 样式引用
    style: StyleRefinement,
    /// 标签变体
    variant: TagVariant,
    /// 是否为轮廓样式
    outline: bool,
    /// 组件尺寸
    size: Size,
    /// 圆角半径
    rounded: Option<AbsoluteLength>,
    /// 子元素
    children: Vec<AnyElement>,
}

impl Tag {
    /// 创建新的标签
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            variant: TagVariant::default(),
            outline: false,
            size: Size::default(),
            rounded: None,
            children: Vec::new(),
        }
    }

    /// 创建主要标签（[`TagVariant::Primary`]）
    pub fn primary() -> Self {
        Self::new().with_variant(TagVariant::Primary)
    }

    /// 创建次要标签（[`TagVariant::Secondary`]）
    pub fn secondary() -> Self {
        Self::new().with_variant(TagVariant::Secondary)
    }

    /// 创建危险标签（[`TagVariant::Danger`]）
    pub fn danger() -> Self {
        Self::new().with_variant(TagVariant::Danger)
    }

    /// 创建成功标签（[`TagVariant::Success`]）
    pub fn success() -> Self {
        Self::new().with_variant(TagVariant::Success)
    }

    /// 创建警告标签（[`TagVariant::Warning`]）
    pub fn warning() -> Self {
        Self::new().with_variant(TagVariant::Warning)
    }

    /// 创建信息标签（[`TagVariant::Info`]）
    pub fn info() -> Self {
        Self::new().with_variant(TagVariant::Info)
    }

    /// 创建自定义颜色标签（[`TagVariant::Custom`]）
    pub fn custom(color: Hsla, foreground: Hsla, border: Hsla) -> Self {
        Self::new().with_variant(TagVariant::Custom {
            color,
            foreground,
            border,
        })
    }

    /// 创建指定颜色名称的标签（[`TagVariant::Color`]）
    pub fn color(color: impl Into<ColorName>) -> Self {
        Self::new().with_variant(TagVariant::Color(color.into()))
    }

    /// 设置标签的变体类型
    pub fn with_variant(mut self, variant: TagVariant) -> Self {
        self.variant = variant;
        self
    }

    /// 使用轮廓样式
    pub fn outline(mut self) -> Self {
        self.outline = true;
        self
    }

    /// 设置圆角半径
    pub fn rounded(mut self, radius: impl Into<AbsoluteLength>) -> Self {
        self.rounded = Some(radius.into());
        self
    }

    /// 设置为全圆角
    pub fn rounded_full(mut self) -> Self {
        self.rounded = Some(rems(1.).into());
        self
    }
}

impl Sizable for Tag {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
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
                Size::XSmall | Size::Small => cx.theme().radius / 2.,
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
                Size::XSmall | Size::Small => this.px_1p5().py_0p5(),
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
