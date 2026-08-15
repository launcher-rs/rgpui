use crate::{
    App, BoxShadow, Corners, DefiniteLength, Edges, FocusHandle, Hsla, Pixels, Refineable,
    StyleRefinement, Styled, Window, point,
};
use crate::theme::{green_500, pink_500, red_500, blue_500, yellow_500};
use crate::ActiveTheme;

macro_rules! font_weight {
    ($fn:ident, $const:ident) => {
        /// [文档](https://tailwindcss.com/docs/font-weight)
        #[inline]
        fn $fn(self) -> Self {
            self.font_weight(crate::FontWeight::$const)
        }
    };
}

/// 扩展 [`crate::Styled`] 的特定样式方法。
pub trait StyledExt: Styled + Sized {
    /// 应用给定的样式精炼到该元素。
    fn refine_style(mut self, style: &StyleRefinement) -> Self {
        self.style().refine(style);
        self
    }

    /// 应用到水平 flex 布局。
    #[inline(always)]
    fn h_flex(self) -> Self {
        self.flex().flex_row().items_center()
    }

    /// 应用到垂直 flex 布局。
    #[inline(always)]
    fn v_flex(self) -> Self {
        self.flex().flex_col()
    }

    /// 应用内边距到该元素。
    fn paddings<L>(self, paddings: impl Into<Edges<L>>) -> Self
    where
        L: Into<DefiniteLength> + Clone + Default + std::fmt::Debug + PartialEq,
    {
        let paddings = paddings.into();
        self.pt(paddings.top.into())
            .pb(paddings.bottom.into())
            .pl(paddings.left.into())
            .pr(paddings.right.into())
    }

    /// 应用外边距到该元素。
    fn margins<L>(self, margins: impl Into<Edges<L>>) -> Self
    where
        L: Into<DefiniteLength> + Clone + Default + std::fmt::Debug + PartialEq,
    {
        let margins = margins.into();
        self.mt(margins.top.into())
            .mb(margins.bottom.into())
            .ml(margins.left.into())
            .mr(margins.right.into())
    }

    /// 绘制一个宽度为 1px 的红色边框。
    fn debug_red(self) -> Self {
        if cfg!(debug_assertions) {
            self.border_1().border_color(red_500())
        } else {
            self
        }
    }

    /// 绘制一个宽度为 1px 的蓝色边框。
    fn debug_blue(self) -> Self {
        if cfg!(debug_assertions) {
            self.border_1().border_color(blue_500())
        } else {
            self
        }
    }

    /// 绘制一个宽度为 1px 的黄色边框。
    fn debug_yellow(self) -> Self {
        if cfg!(debug_assertions) {
            self.border_1().border_color(yellow_500())
        } else {
            self
        }
    }

    /// 绘制一个宽度为 1px 的绿色边框。
    fn debug_green(self) -> Self {
        if cfg!(debug_assertions) {
            self.border_1().border_color(green_500())
        } else {
            self
        }
    }

    /// 绘制一个宽度为 1px 的粉色边框。
    fn debug_pink(self) -> Self {
        if cfg!(debug_assertions) {
            self.border_1().border_color(pink_500())
        } else {
            self
        }
    }

    /// 当元素聚焦时绘制 1px 蓝色边框。
    fn debug_focused(self, focus_handle: &FocusHandle, window: &Window, cx: &App) -> Self {
        if cfg!(debug_assertions) {
            if focus_handle.contains_focused(window, cx) {
                self.debug_blue()
            } else {
                self
            }
        } else {
            self
        }
    }

    /// 绘制一个宽度为 1px、颜色为环色的边框。
    #[inline]
    fn focused_border(self, cx: &App) -> Self {
        self.border_1().border_color(cx.theme().ring)
    }

    font_weight!(font_thin, THIN);
    font_weight!(font_extralight, EXTRA_LIGHT);
    font_weight!(font_light, LIGHT);
    font_weight!(font_normal, NORMAL);
    font_weight!(font_medium, MEDIUM);
    font_weight!(font_semibold, SEMIBOLD);
    font_weight!(font_bold, BOLD);
    font_weight!(font_extrabold, EXTRA_BOLD);
    font_weight!(font_black, BLACK);

    /// 设置为 Popover 样式。
    #[inline]
    fn popover_style(self, cx: &App) -> Self {
        self.bg(cx.theme().tokens.popover)
            .text_color(cx.theme().popover_foreground)
            .border_1()
            .border_color(cx.theme().border)
            .shadow_lg()
            .rounded(cx.theme().radius)
    }

    /// 设置元素的四角圆角。
    fn corner_radii(self, radius: Corners<Pixels>) -> Self {
        self.rounded_tl(radius.top_left)
            .rounded_tr(radius.top_right)
            .rounded_bl(radius.bottom_left)
            .rounded_br(radius.bottom_right)
    }
}

impl<E: Styled> StyledExt for E {}

/// 创建一个 CSS 风格的对象阴影 [`BoxShadow`]。
///
/// 例如：
///
/// 如果 CSS 是 `box-shadow: 0 0 10px 0 rgba(0, 0, 0, 0.1);`
///
/// 则 Rust 中对应的是 `box_shadow(0., 0., 10., 0., hsla(0., 0., 0., 0.1))`
#[inline(always)]
pub fn box_shadow(
    x: impl Into<Pixels>,
    y: impl Into<Pixels>,
    blur: impl Into<Pixels>,
    spread: impl Into<Pixels>,
    color: Hsla,
) -> BoxShadow {
    BoxShadow {
        offset: point(x.into(), y.into()),
        blur_radius: blur.into(),
        spread_radius: spread.into(),
        inset: false,
        color,
    }
}

/// 返回一个水平 flex 布局的 `Div`。
#[inline(always)]
pub fn h_flex() -> crate::Div {
    crate::div().h_flex()
}

/// 返回一个垂直 flex 布局的 `Div`。
#[inline(always)]
pub fn v_flex() -> crate::Div {
    crate::div().v_flex()
}