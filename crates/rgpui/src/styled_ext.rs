use crate::{
    App, BoxShadow, Corners, DefiniteLength, Edges, ElementSize, FocusHandle, Hsla, ParentElement,
    Pixels, Refineable, StyleRefinement, Styled, Window, div, point, px,
};
use crate::theme::{green_500, pink_500, red_500, blue_500, yellow_500};
use crate::ActiveTheme;

/// 侧边方向枚举，用于组件中指定左/右位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    /// 左侧
    #[default]
    Left,
    /// 右侧
    Right,
}

impl Side {
    /// 是否为左侧。
    #[inline]
    pub fn is_left(&self) -> bool {
        matches!(self, Self::Left)
    }

    /// 是否为右侧。
    #[inline]
    pub fn is_right(&self) -> bool {
        matches!(self, Self::Right)
    }
}

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

/// 定义可以被选中的元素 trait。
#[allow(patterns_in_fns_without_body)]
pub trait Selectable: Sized {
    /// 设置元素的选中状态。
    fn selected(mut self, selected: bool) -> Self;

    /// 返回该元素是否处于选中状态。
    fn is_selected(&self) -> bool;

    /// 设置该元素是否为鼠标右键选中，默认不做任何操作。
    fn secondary_selected(self, _: bool) -> Self {
        self
    }
}

/// 定义可以被禁用的元素 trait。
#[allow(patterns_in_fns_without_body)]
pub trait Disableable {
    /// 设置元素的禁用状态。
    fn disabled(mut self, disabled: bool) -> Self;
}

/// 定义元素尺寸的 trait，默认使用 `Size::Medium`。
#[allow(patterns_in_fns_without_body)]
pub trait Sizable: Sized {
    /// 设置该元素的 [`ElementSize`]。
    ///
    /// 也可接收一个 `ButtonSize` 以转换为 `IconSize`，
    /// 或一个 `Pixels` 来设置自定义尺寸：`px(30.)`
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self;

    /// 设置为 `Size::XSmall`。
    #[inline(always)]
    fn xsmall(self) -> Self {
        self.with_size(ElementSize::XSmall)
    }

    /// 设置为 `Size::Small`。
    #[inline(always)]
    fn small(self) -> Self {
        self.with_size(ElementSize::Small)
    }

    /// 设置为 `Size::Large`。
    #[inline(always)]
    fn large(self) -> Self {
        self.with_size(ElementSize::Large)
    }
}

/// 应用元素尺寸相关样式的 trait。
#[allow(unused)]
pub trait StyleSized<T: Styled> {
    /// 根据给定尺寸设置输入框文字大小。
    fn input_text_size(self, size: ElementSize) -> Self;
    /// 根据给定尺寸设置输入框整体尺寸。
    fn input_size(self, size: ElementSize) -> Self;
    /// 根据给定尺寸设置输入框左内边距。
    fn input_pl(self, size: ElementSize) -> Self;
    /// 根据给定尺寸设置输入框右内边距。
    fn input_pr(self, size: ElementSize) -> Self;
    /// 根据给定尺寸设置输入框水平内边距。
    fn input_px(self, size: ElementSize) -> Self;
    /// 根据给定尺寸设置输入框垂直内边距。
    fn input_py(self, size: ElementSize) -> Self;
    /// 根据给定尺寸设置输入框高度。
    fn input_h(self, size: ElementSize) -> Self;
    /// 根据给定尺寸设置列表整体尺寸。
    fn list_size(self, size: ElementSize) -> Self;
    /// 根据给定尺寸设置列表水平内边距。
    fn list_px(self, size: ElementSize) -> Self;
    /// 根据给定尺寸设置列表垂直内边距。
    fn list_py(self, size: ElementSize) -> Self;
    /// 应用给定尺寸。
    fn size_with(self, size: ElementSize) -> Self;
    /// 应用表格单元格尺寸（字体大小、内边距）。
    fn table_cell_size(self, size: ElementSize) -> Self;
    /// 根据给定尺寸设置按钮文字大小。
    fn button_text_size(self, size: ElementSize) -> Self;
}

impl<T: Styled> StyleSized<T> for T {
    #[inline]
    fn input_text_size(self, size: ElementSize) -> Self {
        match size {
            ElementSize::XSmall => self.text_xs(),
            ElementSize::Small => self.text_sm(),
            ElementSize::Medium => self.text_sm(),
            ElementSize::Large => self.text_base(),
            ElementSize::ElementSize(size) => self.text_size(size * 0.875),
        }
    }

    #[inline]
    fn input_size(self, size: ElementSize) -> Self {
        self.input_px(size).input_py(size).input_h(size)
    }

    #[inline]
    fn input_pl(self, size: ElementSize) -> Self {
        self.pl(size.input_px())
    }

    #[inline]
    fn input_pr(self, size: ElementSize) -> Self {
        self.pr(size.input_px())
    }

    #[inline]
    fn input_px(self, size: ElementSize) -> Self {
        self.px(size.input_px())
    }

    #[inline]
    fn input_py(self, size: ElementSize) -> Self {
        self.py(size.input_py())
    }

    #[inline]
    fn input_h(self, size: ElementSize) -> Self {
        match size {
            ElementSize::Large => self.h_11(),
            ElementSize::Medium => self.h_8(),
            ElementSize::Small => self.h_6(),
            ElementSize::XSmall => self.h_5(),
            _ => self.h_6(),
        }
    }

    #[inline]
    fn list_size(self, size: ElementSize) -> Self {
        self.list_px(size).list_py(size).input_text_size(size)
    }

    #[inline]
    fn list_px(self, size: ElementSize) -> Self {
        match size {
            ElementSize::Small => self.px_2(),
            _ => self.px_3(),
        }
    }

    #[inline]
    fn list_py(self, size: ElementSize) -> Self {
        match size {
            ElementSize::Large => self.py_2(),
            ElementSize::Medium => self.py_1(),
            ElementSize::Small => self.py_0p5(),
            _ => self.py_1(),
        }
    }

    #[inline]
    fn size_with(self, size: ElementSize) -> Self {
        match size {
            ElementSize::Large => self.size_11(),
            ElementSize::Medium => self.size_8(),
            ElementSize::Small => self.size_5(),
            ElementSize::XSmall => self.size_4(),
            ElementSize::ElementSize(size) => self.size(size),
        }
    }

    #[inline]
    fn table_cell_size(self, size: ElementSize) -> Self {
        let padding = size.table_cell_padding();
        match size {
            ElementSize::XSmall => self.text_sm(),
            ElementSize::Small => self.text_sm(),
            _ => self,
        }
        .pl(padding.left)
        .pr(padding.right)
        .pt(padding.top)
        .pb(padding.bottom)
    }

    fn button_text_size(self, size: ElementSize) -> Self {
        match size {
            ElementSize::XSmall => self.text_xs(),
            ElementSize::Small => self.text_sm(),
            _ => self.text_base(),
        }
    }
}

/// 为可聚焦元素添加焦点光环（focus ring）的 trait。
pub trait FocusableExt<T: ParentElement + Styled + Sized> {
    /// 为元素添加焦点光环。
    fn focus_ring(self, is_focused: bool, margins: Pixels, window: &Window, cx: &App) -> Self;
}

impl<T: ParentElement + Styled + Sized> FocusableExt<T> for T {
    fn focus_ring(mut self, is_focused: bool, margins: Pixels, window: &Window, cx: &App) -> Self {
        if !is_focused {
            return self;
        }

        const RING_BORDER_WIDTH: Pixels = px(1.5);
        let rem_size = window.rem_size();
        let style = self.style();

        let border_widths = Edges::<Pixels> {
            top: style
                .border_widths
                .top
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or_default(),
            bottom: style
                .border_widths
                .bottom
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or_default(),
            left: style
                .border_widths
                .left
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or_default(),
            right: style
                .border_widths
                .right
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or_default(),
        };

        // 基于元素的四角圆角与光环边框宽度更新圆角
        let radius = Corners::<Pixels> {
            top_left: style
                .corner_radii
                .top_left
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or_default(),
            top_right: style
                .corner_radii
                .top_right
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or_default(),
            bottom_left: style
                .corner_radii
                .bottom_left
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or_default(),
            bottom_right: style
                .corner_radii
                .bottom_right
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or_default(),
        }
        .map(|v| *v + RING_BORDER_WIDTH);

        let mut inner_style = StyleRefinement::default();
        inner_style.corner_radii.top_left = Some(radius.top_left.into());
        inner_style.corner_radii.top_right = Some(radius.top_right.into());
        inner_style.corner_radii.bottom_left = Some(radius.bottom_left.into());
        inner_style.corner_radii.bottom_right = Some(radius.bottom_right.into());

        let inset = RING_BORDER_WIDTH + margins;

        self.child(
            div()
                .flex_none()
                .absolute()
                .top(-(inset + border_widths.top))
                .left(-(inset + border_widths.left))
                .right(-(inset + border_widths.right))
                .bottom(-(inset + border_widths.bottom))
                .border(RING_BORDER_WIDTH)
                .border_color(cx.theme().ring.alpha(0.2))
                .refine_style(&inner_style),
        )
    }
}







