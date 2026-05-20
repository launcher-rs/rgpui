use crate::{
    self as rgpui, AbsoluteLength, AlignContent, AlignItems, AlignSelf, BlendMode, BorderStyle,
    CursorStyle, DefiniteLength, Display, Fill, FlexDirection, FlexWrap, Font, FontFeatures,
    FontStyle, FontWeight, GridPlacement, GridTemplate, Hsla, JustifyContent, Length, SharedString,
    StrikethroughStyle, StyleRefinement, TemplateColumnMinSize, TextAlign, TextOverflow,
    TextStyleRefinement, UnderlineStyle, WhiteSpace, point, px, relative, rems,
};
pub use rgpui_macros::{
    border_style_methods, box_shadow_style_methods, cursor_style_methods, margin_style_methods,
    overflow_style_methods, padding_style_methods, position_style_methods,
    visibility_style_methods,
};
const ELLIPSIS: SharedString = SharedString::new_static("…");

/// 可设置样式的元素的 trait。
/// 使用此功能可选择使用实用程序 CSS 类似样式 API。
// 在 rust-analyzer 上禁用，因为 rust-analyzer 永远不需要展开此宏，由于 rust-analyzer 的 proc-macro srv 中的低效，展开可能需要长达 10 秒
#[cfg_attr(
    all(any(feature = "inspector", debug_assertions), not(rust_analyzer)),
    rgpui_macros::derive_inspector_reflection
)]
pub trait Styled: Sized {
    /// 返回此元素样式内存的引用。
    fn style(&mut self) -> &mut StyleRefinement;

    rgpui_macros::style_helpers!();
    rgpui_macros::visibility_style_methods!();
    rgpui_macros::margin_style_methods!();
    rgpui_macros::padding_style_methods!();
    rgpui_macros::position_style_methods!();
    rgpui_macros::overflow_style_methods!();
    rgpui_macros::cursor_style_methods!();
    rgpui_macros::border_style_methods!();
    rgpui_macros::box_shadow_style_methods!();

    /// 将元素的显示类型设置为 `block`。
    /// [文档](https://tailwindcss.com/docs/display)
    fn block(mut self) -> Self {
        self.style().display = Some(Display::Block);
        self
    }

    /// 将元素的显示类型设置为 `flex`。
    /// [文档](https://tailwindcss.com/docs/display)
    fn flex(mut self) -> Self {
        self.style().display = Some(Display::Flex);
        self
    }

    /// 将元素的显示类型设置为 `grid`。
    /// [文档](https://tailwindcss.com/docs/display)
    fn grid(mut self) -> Self {
        self.style().display = Some(Display::Grid);
        self
    }

    /// 将元素的显示类型设置为 `none`。
    /// [文档](https://tailwindcss.com/docs/display)
    fn hidden(mut self) -> Self {
        self.style().display = Some(Display::None);
        self
    }

    /// 设置用于渲染滚动条的空间。
    ///
    /// 仅当此元素的溢出设置为
    /// `Overflow::Scroll` 时才会影响元素的布局。
    fn scrollbar_width(mut self, width: impl Into<AbsoluteLength>) -> Self {
        self.style().scrollbar_width = Some(width.into());
        self
    }

    /// 将元素的空白设置为 `normal`。
    /// [文档](https://tailwindcss.com/docs/whitespace#normal)
    fn whitespace_normal(mut self) -> Self {
        self.text_style().white_space = Some(WhiteSpace::Normal);
        self
    }

    /// 将元素的空白设置为 `nowrap`。
    /// [文档](https://tailwindcss.com/docs/whitespace#nowrap)
    fn whitespace_nowrap(mut self) -> Self {
        self.text_style().white_space = Some(WhiteSpace::Nowrap);
        self
    }

    /// 设置如果需要则用省略号 (...) 截断溢出的文本。
    /// [文档](https://tailwindcss.com/docs/text-overflow#ellipsis)
    fn text_ellipsis(mut self) -> Self {
        self.text_style().text_overflow = Some(TextOverflow::Truncate(ELLIPSIS));
        self
    }

    /// 设置如果需要则在开头用省略号 (...) 截断溢出的文本。
    /// 通常更适合文件路径，因为结尾比开头更重要。
    /// 注意：这在 Tailwind CSS 中不存在。
    fn text_ellipsis_start(mut self) -> Self {
        self.text_style().text_overflow = Some(TextOverflow::TruncateStart(ELLIPSIS));
        self
    }

    /// 设置元素的文本溢出行为。
    fn text_overflow(mut self, overflow: TextOverflow) -> Self {
        self.text_style().text_overflow = Some(overflow);
        self
    }

    /// 设置元素的对齐方式。
    fn text_align(mut self, align: TextAlign) -> Self {
        self.text_style().text_align = Some(align);
        self
    }

    /// 将文本对齐设置为左
    fn text_left(mut self) -> Self {
        self.text_align(TextAlign::Left)
    }

    /// 将文本对齐设置为居中
    fn text_center(mut self) -> Self {
        self.text_align(TextAlign::Center)
    }

    /// 将文本对齐设置为右
    fn text_right(mut self) -> Self {
        self.text_align(TextAlign::Right)
    }

    /// 设置截断以防止文本换行，并在需要时用省略号 (...) 截断溢出的文本。
    /// [文档](https://tailwindcss.com/docs/text-overflow#truncate)
    fn truncate(mut self) -> Self {
        self.overflow_hidden().whitespace_nowrap().text_ellipsis()
    }

    /// 设置截断文本前显示的行数。
    /// [文档](https://tailwindcss.com/docs/line-clamp)
    fn line_clamp(mut self, lines: usize) -> Self {
        let mut text_style = self.text_style();
        text_style.line_clamp = Some(lines);
        self.overflow_hidden()
    }

    /// 将元素的 flex 方向设置为 `column`。
    /// [文档](https://tailwindcss.com/docs/flex-direction#column)
    fn flex_col(mut self) -> Self {
        self.style().flex_direction = Some(FlexDirection::Column);
        self
    }

    /// 将元素的 flex 方向设置为 `column-reverse`。
    /// [文档](https://tailwindcss.com/docs/flex-direction#column-reverse)
    fn flex_col_reverse(mut self) -> Self {
        self.style().flex_direction = Some(FlexDirection::ColumnReverse);
        self
    }

    /// 将元素的 flex 方向设置为 `row`。
    /// [文档](https://tailwindcss.com/docs/flex-direction#row)
    fn flex_row(mut self) -> Self {
        self.style().flex_direction = Some(FlexDirection::Row);
        self
    }

    /// 将元素的 flex 方向设置为 `row-reverse`。
    /// [文档](https://tailwindcss.com/docs/flex-direction#row-reverse)
    fn flex_row_reverse(mut self) -> Self {
        self.style().flex_direction = Some(FlexDirection::RowReverse);
        self
    }

    /// 设置元素允许 flex 项目根据需要增长和收缩，忽略其初始大小。
    /// [文档](https://tailwindcss.com/docs/flex#flex-1)
    fn flex_1(mut self) -> Self {
        self.style().flex_grow = Some(1.);
        self.style().flex_shrink = Some(1.);
        self.style().flex_basis = Some(relative(0.).into());
        self
    }

    /// 设置元素允许 flex 项目增长和收缩，考虑其初始大小。
    /// [文档](https://tailwindcss.com/docs/flex#auto)
    fn flex_auto(mut self) -> Self {
        self.style().flex_grow = Some(1.);
        self.style().flex_shrink = Some(1.);
        self.style().flex_basis = Some(Length::Auto);
        self
    }

    /// 设置元素允许 flex 项目收缩但不增长，考虑其初始大小。
    /// [文档](https://tailwindcss.com/docs/flex#initial)
    fn flex_initial(mut self) -> Self {
        self.style().flex_grow = Some(0.);
        self.style().flex_shrink = Some(1.);
        self.style().flex_basis = Some(Length::Auto);
        self
    }

    /// 设置元素防止 flex 项目增长或收缩。
    /// [文档](https://tailwindcss.com/docs/flex#none)
    fn flex_none(mut self) -> Self {
        self.style().flex_grow = Some(0.);
        self.style().flex_shrink = Some(0.);
        self
    }

    /// 设置此元素的 flex 项目的初始大小。
    /// [文档](https://tailwindcss.com/docs/flex-basis)
    fn flex_basis(mut self, basis: impl Into<Length>) -> Self {
        self.style().flex_basis = Some(basis.into());
        self
    }

    /// 设置元素允许 flex 项目增长以填充任何可用空间。
    /// [文档](https://tailwindcss.com/docs/flex-grow)
    fn flex_grow(mut self) -> Self {
        self.style().flex_grow = Some(1.);
        self
    }

    /// 设置元素防止 flex 项目增长。
    /// [文档](https://tailwindcss.com/docs/flex-grow#dont-grow)
    fn flex_grow_0(mut self) -> Self {
        self.style().flex_grow = Some(0.);
        self
    }

    /// 设置元素允许 flex 项目在需要时收缩。
    /// [文档](https://tailwindcss.com/docs/flex-shrink)
    fn flex_shrink(mut self) -> Self {
        self.style().flex_shrink = Some(1.);
        self
    }

    /// 设置元素防止 flex 项目收缩。
    /// [文档](https://tailwindcss.com/docs/flex-shrink#dont-shrink)
    fn flex_shrink_0(mut self) -> Self {
        self.style().flex_shrink = Some(0.);
        self
    }

    /// 设置元素允许 flex 项目换行。
    /// [文档](https://tailwindcss.com/docs/flex-wrap#wrap-normally)
    fn flex_wrap(mut self) -> Self {
        self.style().flex_wrap = Some(FlexWrap::Wrap);
        self
    }

    /// 设置元素以相反方向换行 flex 项目。
    /// [文档](https://tailwindcss.com/docs/flex-wrap#wrap-reversed)
    fn flex_wrap_reverse(mut self) -> Self {
        self.style().flex_wrap = Some(FlexWrap::WrapReverse);
        self
    }

    /// 设置元素防止 flex 项目换行，导致不可变项目在必要时溢出容器。
    /// [文档](https://tailwindcss.com/docs/flex-wrap#dont-wrap)
    fn flex_nowrap(mut self) -> Self {
        self.style().flex_wrap = Some(FlexWrap::NoWrap);
        self
    }

    /// 设置元素将 flex 项目对齐到容器交叉轴的起始位置。
    /// [文档](https://tailwindcss.com/docs/align-items#start)
    fn items_start(mut self) -> Self {
        self.style().align_items = Some(AlignItems::FlexStart);
        self
    }

    /// 设置元素将 flex 项目对齐到容器交叉轴的末尾。
    /// [文档](https://tailwindcss.com/docs/align-items#end)
    fn items_end(mut self) -> Self {
        self.style().align_items = Some(AlignItems::FlexEnd);
        self
    }

    /// 设置元素将 flex 项目沿容器交叉轴的中心对齐。
    /// [文档](https://tailwindcss.com/docs/align-items#center)
    fn items_center(mut self) -> Self {
        self.style().align_items = Some(AlignItems::Center);
        self
    }

    /// 设置元素将 flex 项目沿容器交叉轴的基线对齐。
    /// [文档](https://tailwindcss.com/docs/align-items#baseline)
    fn items_baseline(mut self) -> Self {
        self.style().align_items = Some(AlignItems::Baseline);
        self
    }

    /// 设置元素拉伸 flex 项目以填充容器交叉轴的可用空间。
    /// [文档](https://tailwindcss.com/docs/align-items#stretch)
    fn items_stretch(mut self) -> Self {
        self.style().align_items = Some(AlignItems::Stretch);
        self
    }

    /// 设置此特定元素如何沿容器交叉轴对齐。
    /// [文档](https://tailwindcss.com/docs/align-self#start)
    fn self_start(mut self) -> Self {
        self.style().align_self = Some(AlignSelf::Start);
        self
    }

    /// 设置此元素对齐到容器交叉轴的末尾。
    /// [文档](https://tailwindcss.com/docs/align-self#end)
    fn self_end(mut self) -> Self {
        self.style().align_self = Some(AlignSelf::End);
        self
    }

    /// 设置此元素对齐到容器交叉轴的起始位置。
    /// [文档](https://tailwindcss.com/docs/align-self#start)
    fn self_flex_start(mut self) -> Self {
        self.style().align_self = Some(AlignSelf::FlexStart);
        self
    }

    /// 设置此元素对齐到容器交叉轴的末尾。
    /// [文档](https://tailwindcss.com/docs/align-self#end)
    fn self_flex_end(mut self) -> Self {
        self.style().align_self = Some(AlignSelf::FlexEnd);
        self
    }

    /// 设置此元素沿容器交叉轴的中心对齐。
    /// [文档](https://tailwindcss.com/docs/align-self#center)
    fn self_center(mut self) -> Self {
        self.style().align_self = Some(AlignSelf::Center);
        self
    }

    /// 设置此元素沿容器交叉轴的基线对齐。
    /// [文档](https://tailwindcss.com/docs/align-self#baseline)
    fn self_baseline(mut self) -> Self {
        self.style().align_self = Some(AlignSelf::Baseline);
        self
    }

    /// 设置此元素拉伸以填充容器交叉轴的可用空间。
    /// [文档](https://tailwindcss.com/docs/align-self#stretch)
    fn self_stretch(mut self) -> Self {
        self.style().align_self = Some(AlignSelf::Stretch);
        self
    }

    /// 设置元素将 flex 项目对齐到容器主轴的起始位置。
    /// [文档](https://tailwindcss.com/docs/justify-content#start)
    fn justify_start(mut self) -> Self {
        self.style().justify_content = Some(JustifyContent::Start);
        self
    }

    /// 设置元素将 flex 项目对齐到容器主轴的末尾。
    /// [文档](https://tailwindcss.com/docs/justify-content#end)
    fn justify_end(mut self) -> Self {
        self.style().justify_content = Some(JustifyContent::End);
        self
    }

    /// 设置元素将 flex 项目沿容器主轴的中心对齐。
    /// [文档](https://tailwindcss.com/docs/justify-content#center)
    fn justify_center(mut self) -> Self {
        self.style().justify_content = Some(JustifyContent::Center);
        self
    }

    /// 设置元素将 flex 项目沿容器主轴对齐，使得每个项目之间有相等的空间。
    /// [文档](https://tailwindcss.com/docs/justify-content#space-between)
    fn justify_between(mut self) -> Self {
        self.style().justify_content = Some(JustifyContent::SpaceBetween);
        self
    }

    /// 设置元素将项目沿容器主轴对齐，使得每个项目两侧有相等的空间。
    /// [文档](https://tailwindcss.com/docs/justify-content#space-around)
    fn justify_around(mut self) -> Self {
        self.style().justify_content = Some(JustifyContent::SpaceAround);
        self
    }

    /// 设置元素将项目沿容器主轴对齐，使得每个项目周围有相等的空间，同时
    /// 考虑到使用 justify-around 时通常会在每个项目之间看到的空间加倍。
    /// [文档](https://tailwindcss.com/docs/justify-content#space-evenly)
    fn justify_evenly(mut self) -> Self {
        self.style().justify_content = Some(JustifyContent::SpaceEvenly);
        self
    }

    /// 设置元素将内容项目打包到其默认位置，就像未设置 align-content 值一样。
    /// [文档](https://tailwindcss.com/docs/align-content#normal)
    fn content_normal(mut self) -> Self {
        self.style().align_content = None;
        self
    }

    /// 设置元素将内容项目打包到容器交叉轴的中心。
    /// [文档](https://tailwindcss.com/docs/align-content#center)
    fn content_center(mut self) -> Self {
        self.style().align_content = Some(AlignContent::Center);
        self
    }

    /// 设置元素将内容项目打包到容器交叉轴的起始位置。
    /// [文档](https://tailwindcss.com/docs/align-content#start)
    fn content_start(mut self) -> Self {
        self.style().align_content = Some(AlignContent::FlexStart);
        self
    }

    /// 设置元素将内容项目打包到容器交叉轴的末尾。
    /// [文档](https://tailwindcss.com/docs/align-content#end)
    fn content_end(mut self) -> Self {
        self.style().align_content = Some(AlignContent::FlexEnd);
        self
    }

    /// 设置元素将内容项目沿容器交叉轴打包，使得每个项目之间有相等的空间。
    /// [文档](https://tailwindcss.com/docs/align-content#space-between)
    fn content_between(mut self) -> Self {
        self.style().align_content = Some(AlignContent::SpaceBetween);
        self
    }

    /// 设置元素将内容项目沿容器交叉轴打包，使得每个项目两侧有相等的空间。
    /// [文档](https://tailwindcss.com/docs/align-content#space-around)
    fn content_around(mut self) -> Self {
        self.style().align_content = Some(AlignContent::SpaceAround);
        self
    }

    /// 设置元素将内容项目沿容器交叉轴打包，使得每个项目之间有相等的空间。
    /// [文档](https://tailwindcss.com/docs/align-content#space-evenly)
    fn content_evenly(mut self) -> Self {
        self.style().align_content = Some(AlignContent::SpaceEvenly);
        self
    }

    /// 设置元素允许内容项目填充容器交叉轴的可用空间。
    /// [文档](https://tailwindcss.com/docs/align-content#stretch)
    fn content_stretch(mut self) -> Self {
        self.style().align_content = Some(AlignContent::Stretch);
        self
    }

    /// 设置元素的宽高比。
    /// [文档](https://tailwindcss.com/docs/aspect-ratio)
    fn aspect_ratio(mut self, ratio: f32) -> Self {
        self.style().aspect_ratio = Some(ratio);
        self
    }

    /// 将元素的宽高比设置为 1/1 —— 等宽等高。
    /// [文档](https://tailwindcss.com/docs/aspect-ratio)
    fn aspect_square(mut self) -> Self {
        self.style().aspect_ratio = Some(1.0);
        self
    }

    /// 设置元素的背景颜色。
    fn bg<F>(mut self, fill: F) -> Self
    where
        F: Into<Fill>,
        Self: Sized,
    {
        self.style().background = Some(fill.into());
        self
    }

    /// 设置元素的边框样式。
    fn border_dashed(mut self) -> Self {
        self.style().border_style = Some(BorderStyle::Dashed);
        self
    }

    /// 启用连续（圆角）角舍入而不是圆形。
    /// 这会产生平滑的 Apple 风格圆角，匹配 SwiftUI 的连续角样式。
    fn continuous_corners(mut self) -> Self {
        self.style().continuous_corners = Some(true);
        self
    }

    /// 返回已在此元素上配置的文本样式的可变引用。
    fn text_style(&mut self) -> &mut TextStyleRefinement {
        let style: &mut StyleRefinement = self.style();
        &mut style.text
    }

    /// 设置此元素的文本颜色。
    ///
    /// 此值将级联到其子元素。
    fn text_color(mut self, color: impl Into<Hsla>) -> Self {
        self.text_style().color = Some(color.into());
        self
    }

    /// 设置此元素的字体粗细
    ///
    /// 此值将级联到其子元素。
    fn font_weight(mut self, weight: FontWeight) -> Self {
        self.text_style().font_weight = Some(weight);
        self
    }

    /// 设置此元素的背景颜色。
    ///
    /// 此值将级联到其子元素。
    fn text_bg(mut self, bg: impl Into<Hsla>) -> Self {
        self.text_style().background_color = Some(bg.into());
        self
    }

    /// 设置此元素的文本大小。
    ///
    /// 此值将级联到其子元素。
    fn text_size(mut self, size: impl Into<AbsoluteLength>) -> Self {
        self.text_style().font_size = Some(size.into());
        self
    }

    /// 将文本大小设置为 'extra small'。
    /// [文档](https://tailwindcss.com/docs/font-size#setting-the-font-size)
    fn text_xs(mut self) -> Self {
        self.text_style().font_size = Some(rems(0.75).into());
        self
    }

    /// 将文本大小设置为 'small'。
    /// [文档](https://tailwindcss.com/docs/font-size#setting-the-font-size)
    fn text_sm(mut self) -> Self {
        self.text_style().font_size = Some(rems(0.875).into());
        self
    }

    /// 将文本大小设置为 'base'。
    /// [文档](https://tailwindcss.com/docs/font-size#setting-the-font-size)
    fn text_base(mut self) -> Self {
        self.text_style().font_size = Some(rems(1.0).into());
        self
    }

    /// 将文本大小设置为 'large'。
    /// [文档](https://tailwindcss.com/docs/font-size#setting-the-font-size)
    fn text_lg(mut self) -> Self {
        self.text_style().font_size = Some(rems(1.125).into());
        self
    }

    /// 将文本大小设置为 'extra large'。
    /// [文档](https://tailwindcss.com/docs/font-size#setting-the-font-size)
    fn text_xl(mut self) -> Self {
        self.text_style().font_size = Some(rems(1.25).into());
        self
    }

    /// 将文本大小设置为 'extra extra large'。
    /// [文档](https://tailwindcss.com/docs/font-size#setting-the-font-size)
    fn text_2xl(mut self) -> Self {
        self.text_style().font_size = Some(rems(1.5).into());
        self
    }

    /// 将文本大小设置为 'extra extra extra large'。
    /// [文档](https://tailwindcss.com/docs/font-size#setting-the-font-size)
    fn text_3xl(mut self) -> Self {
        self.text_style().font_size = Some(rems(1.875).into());
        self
    }

    /// 将元素的字体样式设置为 italic。
    /// [文档](https://tailwindcss.com/docs/font-style#italicizing-text)
    fn italic(mut self) -> Self {
        self.text_style().font_style = Some(FontStyle::Italic);
        self
    }

    /// 将元素的字体样式设置为 normal（非 italic）。
    /// [文档](https://tailwindcss.com/docs/font-style#displaying-text-normally)
    fn not_italic(mut self) -> Self {
        self.text_style().font_style = Some(FontStyle::Normal);
        self
    }

    /// 将文本装饰设置为下划线。
    /// [文档](https://tailwindcss.com/docs/text-decoration-line#underling-text)
    fn underline(mut self) -> Self {
        let style = self.text_style();
        style.underline = Some(UnderlineStyle {
            thickness: px(1.),
            ..Default::default()
        });
        self
    }

    /// 将文本装饰设置为删除线。
    /// [文档](https://tailwindcss.com/docs/text-decoration-line#adding-a-line-through-text)
    fn line_through(mut self) -> Self {
        let style = self.text_style();
        style.strikethrough = Some(StrikethroughStyle {
            thickness: px(1.),
            ..Default::default()
        });
        self
    }

    /// 移除此元素上的文本装饰。
    ///
    /// 此值将级联到其子元素。
    fn text_decoration_none(mut self) -> Self {
        self.text_style().underline = None;
        self
    }

    /// 设置此元素下划线的颜色
    fn text_decoration_color(mut self, color: impl Into<Hsla>) -> Self {
        let style = self.text_style();
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.color = Some(color.into());
        self
    }

    /// 将文本装饰样式设置为实线。
    /// [文档](https://tailwindcss.com/docs/text-decoration-style)
    fn text_decoration_solid(mut self) -> Self {
        let style = self.text_style();
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.wavy = false;
        self
    }

    /// 将文本装饰样式设置为波浪线。
    /// [文档](https://tailwindcss.com/docs/text-decoration-style)
    fn text_decoration_wavy(mut self) -> Self {
        let style = self.text_style();
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.wavy = true;
        self
    }

    /// 将文本装饰设置为 0px 粗。
    /// [文档](https://tailwindcss.com/docs/text-decoration-thickness)
    fn text_decoration_0(mut self) -> Self {
        let style = self.text_style();
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.thickness = px(0.);
        self
    }

    /// 将文本装饰设置为 1px 粗。
    /// [文档](https://tailwindcss.com/docs/text-decoration-thickness)
    fn text_decoration_1(mut self) -> Self {
        let style = self.text_style();
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.thickness = px(1.);
        self
    }

    /// 将文本装饰设置为 2px 粗。
    /// [文档](https://tailwindcss.com/docs/text-decoration-thickness)
    fn text_decoration_2(mut self) -> Self {
        let style = self.text_style();
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.thickness = px(2.);
        self
    }

    /// 将文本装饰设置为 4px 粗。
    /// [文档](https://tailwindcss.com/docs/text-decoration-thickness)
    fn text_decoration_4(mut self) -> Self {
        let style = self.text_style();
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.thickness = px(4.);
        self
    }

    /// 将文本装饰设置为 8px 粗。
    /// [文档](https://tailwindcss.com/docs/text-decoration-thickness)
    fn text_decoration_8(mut self) -> Self {
        let style = self.text_style();
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.thickness = px(8.);
        self
    }

    /// 设置此元素及其子元素的字体系列。
    fn font_family(mut self, family_name: impl Into<SharedString>) -> Self {
        self.text_style().font_family = Some(family_name.into());
        self
    }

    /// 设置此元素及其子元素的字体特性。
    fn font_features(mut self, features: FontFeatures) -> Self {
        self.text_style().font_features = Some(features);
        self
    }

    /// 设置此元素及其子元素的字体。
    fn font(mut self, font: Font) -> Self {
        let Font {
            family,
            features,
            fallbacks,
            weight,
            style,
        } = font;

        let text_style = self.text_style();
        text_style.font_family = Some(family);
        text_style.font_features = Some(features);
        text_style.font_weight = Some(weight);
        text_style.font_style = Some(style);
        text_style.font_fallbacks = fallbacks;

        self
    }

    /// 设置此元素及其子元素的行高。
    fn line_height(mut self, line_height: impl Into<DefiniteLength>) -> Self {
        self.text_style().line_height = Some(line_height.into());
        self
    }

    /// 设置此元素及其子元素的透明度。
    fn opacity(mut self, opacity: f32) -> Self {
        self.style().opacity = Some(opacity);
        self
    }

    /// 设置渲染此元素背景时应用的混合模式。
    fn blend_mode(mut self, mode: BlendMode) -> Self {
        self.style().blend_mode = Some(mode);
        self
    }

    /// 设置顺时针旋转角度（度数）。
    fn rotate(mut self, angle_degrees: f32) -> Self {
        self.style().rotate = Some(angle_degrees.to_radians());
        self
    }

    /// 设置统一缩放因子。
    fn scale(mut self, factor: f32) -> Self {
        self.style().scale = Some(point(factor, factor));
        self
    }

    /// 设置 x 和 y 轴的非统一缩放因子。
    fn scale_xy(mut self, x: f32, y: f32) -> Self {
        self.style().scale = Some(point(x, y));
        self
    }

    /// 设置变换原点为元素大小的比例 (0.0-1.0)。
    /// 默认为中心 (0.5, 0.5)。
    fn transform_origin(mut self, x: f32, y: f32) -> Self {
        self.style().transform_origin = Some(point(x, y));
        self
    }

    /// 设置此元素的网格列。
    fn grid_cols(mut self, cols: u16) -> Self {
        self.style().grid_cols = Some(GridTemplate {
            repeat: cols,
            min_size: TemplateColumnMinSize::Zero,
        });
        self
    }

    /// 设置具有最小内容最小大小的网格列。
    /// 与 grid_cols 不同，它不会在 AvailableSpace::MinContent 约束下收缩到宽度 0。
    fn grid_cols_min_content(mut self, cols: u16) -> Self {
        self.style().grid_cols = Some(GridTemplate {
            repeat: cols,
            min_size: TemplateColumnMinSize::MinContent,
        });
        self
    }

    /// 设置具有最大内容最大大小的网格列，用于基于内容宽度的列。
    fn grid_cols_max_content(mut self, cols: u16) -> Self {
        self.style().grid_cols = Some(GridTemplate {
            repeat: cols,
            min_size: TemplateColumnMinSize::MaxContent,
        });
        self
    }

    /// 设置此元素的网格行。
    fn grid_rows(mut self, rows: u16) -> Self {
        self.style().grid_rows = Some(GridTemplate {
            repeat: rows,
            min_size: TemplateColumnMinSize::Zero,
        });
        self
    }

    /// 设置此元素的列起始位置。
    fn col_start(mut self, start: i16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column.start = GridPlacement::Line(start);
        self
    }

    /// 将此元素的列起始位置设置为 auto。
    fn col_start_auto(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column.start = GridPlacement::Auto;
        self
    }

    /// 设置此元素的列结束位置。
    fn col_end(mut self, end: i16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column.end = GridPlacement::Line(end);
        self
    }

    /// 将此元素的列结束位置设置为 auto。
    fn col_end_auto(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column.end = GridPlacement::Auto;
        self
    }

    /// 设置此元素的列跨度。
    fn col_span(mut self, span: u16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column = GridPlacement::Span(span)..GridPlacement::Span(span);
        self
    }

    /// 设置此元素的行跨度。
    fn col_span_full(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column = GridPlacement::Line(1)..GridPlacement::Line(-1);
        self
    }

    /// 设置此元素的行起始位置。
    fn row_start(mut self, start: i16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row.start = GridPlacement::Line(start);
        self
    }

    /// 将此元素的行起始位置设置为 "auto"
    fn row_start_auto(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row.start = GridPlacement::Auto;
        self
    }

    /// 设置此元素的行结束位置。
    fn row_end(mut self, end: i16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row.end = GridPlacement::Line(end);
        self
    }

    /// 将此元素的行结束位置设置为 "auto"
    fn row_end_auto(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row.end = GridPlacement::Auto;
        self
    }

    /// 设置此元素的行跨度。
    fn row_span(mut self, span: u16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row = GridPlacement::Span(span)..GridPlacement::Span(span);
        self
    }

    /// 设置此元素的行跨度。
    fn row_span_full(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row = GridPlacement::Line(1)..GridPlacement::Line(-1);
        self
    }

    /// 在此元素周围绘制调试边框。
    #[cfg(debug_assertions)]
    fn debug(mut self) -> Self {
        self.style().debug = Some(true);
        self
    }

    /// 在此元素下方所有符合条件的元素上绘制调试边框。
    #[cfg(debug_assertions)]
    fn debug_below(mut self) -> Self {
        self.style().debug_below = Some(true);
        self
    }
}
