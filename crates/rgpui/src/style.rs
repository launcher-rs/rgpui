use std::{
    hash::{Hash, Hasher},
    iter, mem,
    ops::Range,
};

use crate::{
    black, phi, point, quad, rems, size, AbsoluteLength, App, Background, BackgroundTag,
    BorderStyle, Bounds, ContentMask, Corners, CornersRefinement, CursorStyle, DefiniteLength,
    DevicePixels, Edges, EdgesRefinement, Font, FontFallbacks, FontFeatures, FontStyle, FontWeight,
    GridLocation, Hsla, Length, Pixels, Point, PointRefinement, Rgba, SharedString, Size,
    SizeRefinement, Styled, TextRun, Window,
};
use crate::collections::HashSet;
use crate::refineable::Refineable;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 使用此结构与你自己元素中的 'debug_below' 样式进行交互。
/// 如果父元素设置了此样式，则此结构将被设置为 GPUI 中的全局值。
#[cfg(debug_assertions)]
pub struct DebugBelow;

#[cfg(debug_assertions)]
impl crate::Global for DebugBelow {}

/// 图像如何适应元素边界的方式。
pub enum ObjectFit {
    /// 图像将拉伸以填充元素的边界。
    Fill,
    /// 图像将缩放以适应元素的边界。
    Contain,
    /// 图像将缩放以覆盖元素的边界。
    Cover,
    /// 图像将缩小以适应元素的边界。
    ScaleDown,
    /// 图像将保持其原始大小。
    None,
}

impl ObjectFit {
    /// 获取给定边界内图像的边界。
    pub fn get_bounds(
        &self,
        bounds: Bounds<Pixels>,
        image_size: Size<DevicePixels>,
    ) -> Bounds<Pixels> {
        let image_size = image_size.map(|dimension| Pixels::from(u32::from(dimension)));
        let image_ratio = image_size.width / image_size.height;
        let bounds_ratio = bounds.size.width / bounds.size.height;

        match self {
            ObjectFit::Fill => bounds,
            ObjectFit::Contain => {
                let new_size = if bounds_ratio > image_ratio {
                    size(
                        image_size.width * (bounds.size.height / image_size.height),
                        bounds.size.height,
                    )
                } else {
                    size(
                        bounds.size.width,
                        image_size.height * (bounds.size.width / image_size.width),
                    )
                };

                Bounds {
                    origin: point(
                        bounds.origin.x + (bounds.size.width - new_size.width) / 2.0,
                        bounds.origin.y + (bounds.size.height - new_size.height) / 2.0,
                    ),
                    size: new_size,
                }
            }
            ObjectFit::ScaleDown => {
                // Check if the image is larger than the bounds in either dimension.
                if image_size.width > bounds.size.width || image_size.height > bounds.size.height {
                    // If the image is larger, use the same logic as Contain to scale it down.
                    let new_size = if bounds_ratio > image_ratio {
                        size(
                            image_size.width * (bounds.size.height / image_size.height),
                            bounds.size.height,
                        )
                    } else {
                        size(
                            bounds.size.width,
                            image_size.height * (bounds.size.width / image_size.width),
                        )
                    };

                    Bounds {
                        origin: point(
                            bounds.origin.x + (bounds.size.width - new_size.width) / 2.0,
                            bounds.origin.y + (bounds.size.height - new_size.height) / 2.0,
                        ),
                        size: new_size,
                    }
                } else {
                    // 如果图像小于或等于容器，则以其原始大小显示，
                    // 居中于容器内。
                    let original_size = size(image_size.width, image_size.height);
                    Bounds {
                        origin: point(
                            bounds.origin.x + (bounds.size.width - original_size.width) / 2.0,
                            bounds.origin.y + (bounds.size.height - original_size.height) / 2.0,
                        ),
                        size: original_size,
                    }
                }
            }
            ObjectFit::Cover => {
                let new_size = if bounds_ratio > image_ratio {
                    size(
                        bounds.size.width,
                        image_size.height * (bounds.size.width / image_size.width),
                    )
                } else {
                    size(
                        image_size.width * (bounds.size.height / image_size.height),
                        bounds.size.height,
                    )
                };

                Bounds {
                    origin: point(
                        bounds.origin.x + (bounds.size.width - new_size.width) / 2.0,
                        bounds.origin.y + (bounds.size.height - new_size.height) / 2.0,
                    ),
                    size: new_size,
                }
            }
            ObjectFit::None => Bounds {
                origin: bounds.origin,
                size: image_size,
            },
        }
    }
}

/// 网格布局中列或行的最小尺寸
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default, JsonSchema, Serialize, Deserialize,
)]
pub enum TemplateColumnMinSize {
    /// 列大小可能为 0
    #[default]
    Zero,
    /// 列大小可以由最小内容决定
    MinContent,
    /// 列大小可以由最大内容决定
    MaxContent,
}

/// `grid-template-*` 值的简化表示
#[derive(
    Copy,
    Clone,
    Refineable,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Debug,
    Default,
    JsonSchema,
    Serialize,
    Deserialize,
)]
pub struct GridTemplate {
    /// 此模板指令应如何重复
    pub repeat: u16,
    /// 在 repeat(<>, minmax(_, 1fr)) 方程中的最小大小
    pub min_size: TemplateColumnMinSize,
}

/// 可以通过 `Styled` trait 应用于元素的 CSS 样式
#[derive(Clone, Refineable, Debug)]
#[refineable(Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Style {
    /// 应使用什么布局策略？
    pub display: Display,

    /// 元素是否应在屏幕上绘制？
    pub visibility: Visibility,

    // 溢出属性
    /// 子元素溢出容器时如何影响布局
    #[refineable]
    pub overflow: Point<Overflow>,
    /// 应为 `Overflow::Scroll` 和 `Overflow::Auto` 节点的滚动条保留多少空间（以点为单位）。
    pub scrollbar_width: AbsoluteLength,
    /// x 和 y 轴是否应同时可滚动。
    pub allow_concurrent_scroll: bool,
    /// 滚动是否应限制在鼠标滚轮指示的轴上。
    ///
    /// 这意味着：
    /// - 仅使用鼠标滚轮将永远只滚动 Y 轴。
    /// - 按住 `Shift` 并使用鼠标滚轮将滚动 X 轴。
    ///
    /// ## 动机
    ///
    /// 在 Web 上使用鼠标滚轮滚动时，上下滚动将永远只滚动 Y 轴，即使
    /// 鼠标位于水平可滚动元素上方。
    ///
    /// 唯一水平滚动的方法是在滚动时按住 `Shift`，这会将滚动轴
    /// 更改为 X 轴。
    ///
    /// 目前，GPUI 的操作与 Web 不同，它会在仅使用鼠标滚轮滚动时在 X 或 Y 轴上滚动元素。这会导致在包含
    /// 水平可滚动元素的垂直列表中滚动时出现问题，因为当你到达水平可滚动元素时滚动会被
    /// 劫持。
    ///
    /// 理想情况下我们将匹配 Web 的行为并且不需要这个，但目前我们添加了这个可选的
    /// 样式属性以限制潜在的爆炸半径。
    pub restrict_scroll_to_axis: bool,

    // 位置属性
    /// 此结构的 `position` 值应使用什么作为基础偏移量？
    pub position: Position,
    /// 相对于定义的布局，此元素的位置应如何调整？
    #[refineable]
    pub inset: Edges<Length>,

    // 大小属性
    /// 设置项目的初始大小
    #[refineable]
    pub size: Size<Length>,
    /// 控制项目的最小大小
    #[refineable]
    pub min_size: Size<Length>,
    /// 控制项目的最大大小
    #[refineable]
    pub max_size: Size<Length>,
    /// 设置项目的首选宽高比。宽高比计算为宽度除以高度。
    pub aspect_ratio: Option<f32>,

    // 间距属性
    /// 每边的边距应有多大？
    #[refineable]
    pub margin: Edges<Length>,
    /// 每边的内边距应有多大？
    #[refineable]
    pub padding: Edges<DefiniteLength>,
    /// 每边的边框应有多大？
    #[refineable]
    pub border_widths: Edges<AbsoluteLength>,

    // 对齐属性
    /// 此节点的子节点如何在交叉/块轴中对齐？
    pub align_items: Option<AlignItems>,
    /// 此节点如何在交叉/块轴中对齐。如果未设置，则回退到父节点的 [`AlignItems`]
    pub align_self: Option<AlignSelf>,
    /// 此项目中包含的内容如何在交叉/块轴中对齐
    pub align_content: Option<AlignContent>,
    /// 此项目中包含的内容如何在主/内联轴中对齐
    pub justify_content: Option<JustifyContent>,
    /// flex 容器中项目之间的间隙应有多大？
    #[refineable]
    pub gap: Size<DefiniteLength>,

    // Flexbox 属性
    /// 主轴在哪个方向流动？
    pub flex_direction: FlexDirection,
    /// 元素应换行还是保持在单行中？
    pub flex_wrap: FlexWrap,
    /// 设置项目的主轴初始大小
    pub flex_basis: Length,
    /// 此项目在扩展以填充空间时的相对增长率，0.0 是默认值，此值必须为正。
    pub flex_grow: f32,
    /// 此项目在收缩以适应空间时的相对收缩率，1.0 是默认值，此值必须为正。
    pub flex_shrink: f32,

    /// 此元素的填充颜色
    pub background: Option<Fill>,

    /// 此元素的边框颜色
    pub border_color: Option<Hsla>,

    /// 此元素的边框样式
    pub border_style: BorderStyle,

    /// 此元素角的半径
    #[refineable]
    pub corner_radii: Corners<AbsoluteLength>,

    /// 元素的盒阴影
    pub box_shadow: Vec<BoxShadow>,

    /// 此元素的文本样式
    #[refineable]
    pub text: TextStyleRefinement,

    /// 鼠标指针位于元素上方时显示的鼠标光标样式。
    pub mouse_cursor: Option<CursorStyle>,

    /// 此元素的透明度
    pub opacity: Option<f32>,

    /// 此元素的网格列
    /// 大致等效于 Tailwind 的 `grid-cols-<number>`
    pub grid_cols: Option<GridTemplate>,

    /// 此元素的行跨度
    /// 等效于 Tailwind 的 `grid-rows-<number>`
    pub grid_rows: Option<GridTemplate>,

    /// 此元素的网格位置
    pub grid_location: Option<GridLocation>,

    /// 是否在此元素周围绘制红色调试轮廓
    #[cfg(debug_assertions)]
    pub debug: bool,

    /// 是否在此元素及其所有符合条件的子元素周围绘制红色调试轮廓
    #[cfg(debug_assertions)]
    pub debug_below: bool,
}

impl Styled for StyleRefinement {
    fn style(&mut self) -> &mut StyleRefinement {
        self
    }
}

impl StyleRefinement {
    /// 此元素的网格位置
    pub fn grid_location_mut(&mut self) -> &mut GridLocation {
        self.grid_location.get_or_insert_default()
    }
}

/// 类似于 CSS `visibility` 属性的可见性值
#[derive(Default, Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Visibility {
    /// 元素应正常绘制。
    #[default]
    Visible,
    /// 元素不应绘制，但仍应在布局中占据空间。
    Hidden,
}

/// box-shadow 属性的可能值
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BoxShadow {
    /// 阴影应有什么颜色？
    pub color: Hsla,
    /// 阴影应与其元素偏移多少？
    pub offset: Point<Pixels>,
    /// 阴影应模糊多少？
    pub blur_radius: Pixels,
    /// 阴影应扩散多少？
    pub spread_radius: Pixels,
}

/// 如何处理文本中的空白字符
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum WhiteSpace {
    /// 文本溢出元素宽度时正常换行
    #[default]
    Normal,
    /// 不换行，文本将溢出元素宽度
    Nowrap,
}

/// 如何截断超出元素宽度的文本
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum TextOverflow {
    /// 文本不适合时在末尾截断，并通过显示提供的字符串表示此截断（例如，"很长的 te..."）。
    Truncate(SharedString),
    /// 文本不适合时在开头截断，并在开头显示提供的字符串（例如，"...ong text here"）。
    /// 通常更适合文件路径，因为结尾比开头更重要。
    TruncateStart(SharedString),
}

/// 如何对齐元素内的文本
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum TextAlign {
    /// 将文本对齐到元素的左侧
    #[default]
    Left,

    /// 在元素内居中
    Center,

    /// 将文本对齐到元素的右侧
    Right,
}

/// 可用于在 GPUI 中设置文本样式的属性
#[derive(Refineable, Clone, Debug, PartialEq)]
#[refineable(Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TextStyle {
    /// 文本的颜色
    pub color: Hsla,

    /// 要使用的字体系列
    pub font_family: SharedString,

    /// 要使用的字体特性
    pub font_features: FontFeatures,

    /// 要使用的后备字体
    pub font_fallbacks: Option<FontFallbacks>,

    /// 要使用的字体大小，以像素或 rem 为单位。
    pub font_size: AbsoluteLength,

    /// 要使用的行高，以像素或分数为单位
    pub line_height: DefiniteLength,

    /// 字体粗细，例如 bold
    pub font_weight: FontWeight,

    /// 字体样式，例如 italic
    pub font_style: FontStyle,

    /// 文本的背景颜色
    pub background_color: Option<Hsla>,

    /// 文本的下划线样式
    pub underline: Option<UnderlineStyle>,

    /// 文本的删除线样式
    pub strikethrough: Option<StrikethroughStyle>,

    /// 如何处理文本中的空白字符
    pub white_space: WhiteSpace,

    /// 如果文本溢出元素的宽度，则应截断文本
    pub text_overflow: Option<TextOverflow>,

    /// 文本应如何在元素内对齐
    pub text_align: TextAlign,

    /// 截断文本前显示的行数
    pub line_clamp: Option<usize>,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            color: black(),
            // todo(linux) 使其可配置或选择更好的默认值
            font_family: ".SystemUIFont".into(),
            font_features: FontFeatures::default(),
            font_fallbacks: None,
            font_size: rems(1.).into(),
            line_height: phi(),
            font_weight: FontWeight::default(),
            font_style: FontStyle::default(),
            background_color: None,
            underline: None,
            strikethrough: None,
            white_space: WhiteSpace::Normal,
            text_overflow: None,
            text_align: TextAlign::default(),
            line_clamp: None,
        }
    }
}

impl TextStyle {
    /// 创建应用给定高亮的新文本样式。
    pub fn highlight(mut self, style: impl Into<HighlightStyle>) -> Self {
        let style = style.into();
        if let Some(weight) = style.font_weight {
            self.font_weight = weight;
        }
        if let Some(style) = style.font_style {
            self.font_style = style;
        }

        if let Some(color) = style.color {
            self.color = self.color.blend(color);
        }

        if let Some(factor) = style.fade_out {
            self.color.fade_out(factor);
        }

        if let Some(background_color) = style.background_color {
            self.background_color = Some(background_color);
        }

        if let Some(underline) = style.underline {
            self.underline = Some(underline);
        }

        if let Some(strikethrough) = style.strikethrough {
            self.strikethrough = Some(strikethrough);
        }

        self
    }

    /// 获取为此文本样式配置的字体。
    pub fn font(&self) -> Font {
        Font {
            family: self.font_family.clone(),
            features: self.font_features.clone(),
            fallbacks: self.font_fallbacks.clone(),
            weight: self.font_weight,
            style: self.font_style,
        }
    }

    /// 返回四舍五入的行高（以像素为单位）。
    pub fn line_height_in_pixels(&self, rem_size: Pixels) -> Pixels {
        self.line_height.to_pixels(self.font_size, rem_size).round()
    }

    /// 将此文本样式转换为给定文本长度的 [`TextRun`]。
    pub fn to_run(&self, len: usize) -> TextRun {
        TextRun {
            len,
            font: Font {
                family: self.font_family.clone(),
                features: self.font_features.clone(),
                fallbacks: self.font_fallbacks.clone(),
                weight: self.font_weight,
                style: self.font_style,
            },
            color: self.color,
            background_color: self.background_color,
            underline: self.underline,
            strikethrough: self.strikethrough,
        }
    }
}

/// 高亮样式，类似于 `TextStyle`，但适用于单一字体、
/// 统一大小和间距的文本。
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct HighlightStyle {
    /// 文本的颜色
    pub color: Option<Hsla>,

    /// 字体粗细，例如 bold
    pub font_weight: Option<FontWeight>,

    /// 字体样式，例如 italic
    pub font_style: Option<FontStyle>,

    /// 文本的背景颜色
    pub background_color: Option<Hsla>,

    /// 文本的下划线样式
    pub underline: Option<UnderlineStyle>,

    /// 文本的删除线样式
    pub strikethrough: Option<StrikethroughStyle>,

    /// 类似于 CSS 的 `opacity` 属性，这将使文本变得不那么鲜艳。
    pub fade_out: Option<f32>,
}

impl Eq for HighlightStyle {}

impl Hash for HighlightStyle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.color.hash(state);
        self.font_weight.hash(state);
        self.font_style.hash(state);
        self.background_color.hash(state);
        self.underline.hash(state);
        self.strikethrough.hash(state);
        state.write_u32(u32::from_be_bytes(
            self.fade_out.map(|f| f.to_be_bytes()).unwrap_or_default(),
        ));
    }
}

impl Style {
    /// 如果样式可见且背景不透明则返回 true。
    pub fn has_opaque_background(&self) -> bool {
        self.background
            .as_ref()
            .is_some_and(|fill| fill.color().is_some_and(|color| !color.is_transparent()))
    }

    /// 获取此元素样式中的文本样式。
    pub fn text_style(&self) -> Option<&TextStyleRefinement> {
        if self.text.is_some() {
            Some(&self.text)
        } else {
            None
        }
    }

    /// 获取此元素样式的内容掩码，基于给定的边界。
    /// 如果元素不隐藏其溢出，则将返回 `None`。
    pub fn overflow_mask(
        &self,
        bounds: Bounds<Pixels>,
        rem_size: Pixels,
    ) -> Option<ContentMask<Pixels>> {
        match self.overflow {
            Point {
                x: Overflow::Visible,
                y: Overflow::Visible,
            } => None,
            _ => {
                let mut min = bounds.origin;
                let mut max = bounds.bottom_right();

                if self
                    .border_color
                    .is_some_and(|color| !color.is_transparent())
                {
                    min.x += self.border_widths.left.to_pixels(rem_size);
                    max.x -= self.border_widths.right.to_pixels(rem_size);
                    min.y += self.border_widths.top.to_pixels(rem_size);
                    max.y -= self.border_widths.bottom.to_pixels(rem_size);
                }

                let bounds = match (
                    self.overflow.x == Overflow::Visible,
                    self.overflow.y == Overflow::Visible,
                ) {
                    // x and y both visible
                    (true, true) => return None,
                    // x visible, y hidden
                    (true, false) => Bounds::from_corners(
                        point(min.x, bounds.origin.y),
                        point(max.x, bounds.bottom_right().y),
                    ),
                    // x hidden, y visible
                    (false, true) => Bounds::from_corners(
                        point(bounds.origin.x, min.y),
                        point(bounds.bottom_right().x, max.y),
                    ),
                    // both hidden
                    (false, false) => Bounds::from_corners(min, max),
                };

                Some(ContentMask { bounds })
            }
        }
    }

    /// 使用此样式绘制元素的背景。
    pub fn paint(
        &self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        continuation: impl FnOnce(&mut Window, &mut App),
    ) {
        #[cfg(debug_assertions)]
        if self.debug_below {
            cx.set_global(DebugBelow)
        }

        #[cfg(debug_assertions)]
        if self.debug || cx.has_global::<DebugBelow>() {
            window.paint_quad(crate::outline(bounds, crate::red(), BorderStyle::default()));
        }

        let rem_size = window.rem_size();
        let corner_radii = self
            .corner_radii
            .to_pixels(rem_size)
            .clamp_radii_for_quad_size(bounds.size);

        window.paint_shadows(bounds, corner_radii, &self.box_shadow);

        let background_color = self.background.as_ref().and_then(Fill::color);
        if background_color.is_some_and(|color| !color.is_transparent()) {
            let mut border_color = match background_color {
                Some(color) => match color.tag {
                    BackgroundTag::Solid
                    | BackgroundTag::PatternSlash
                    | BackgroundTag::Checkerboard => color.solid,

                    BackgroundTag::LinearGradient => color
                        .colors
                        .first()
                        .map(|stop| stop.color)
                        .unwrap_or_default(),
                },
                None => Hsla::default(),
            };
            border_color.a = 0.;
            window.paint_quad(quad(
                bounds,
                corner_radii,
                background_color.unwrap_or_default(),
                Edges::default(),
                border_color,
                self.border_style,
            ));
        }

        continuation(window, cx);

        if self.is_border_visible() {
            let border_widths = self.border_widths.to_pixels(rem_size);
            let mut background = self.border_color.unwrap_or_default();
            background.a = 0.;
            window.paint_quad(quad(
                bounds,
                corner_radii,
                background,
                border_widths,
                self.border_color.unwrap_or_default(),
                self.border_style,
            ));
        }

        #[cfg(debug_assertions)]
        if self.debug_below {
            cx.remove_global::<DebugBelow>();
        }
    }

    fn is_border_visible(&self) -> bool {
        self.border_color
            .is_some_and(|color| !color.is_transparent())
            && self.border_widths.any(|length| !length.is_zero())
    }
}

impl Default for Style {
    fn default() -> Self {
        Style {
            display: Display::Block,
            visibility: Visibility::Visible,
            overflow: Point {
                x: Overflow::Visible,
                y: Overflow::Visible,
            },
            allow_concurrent_scroll: false,
            restrict_scroll_to_axis: false,
            scrollbar_width: AbsoluteLength::default(),
            position: Position::Relative,
            inset: Edges::auto(),
            margin: Edges::<Length>::zero(),
            padding: Edges::<DefiniteLength>::zero(),
            border_widths: Edges::<AbsoluteLength>::zero(),
            size: Size::auto(),
            min_size: Size::auto(),
            max_size: Size::auto(),
            aspect_ratio: None,
            gap: Size::default(),
            // Alignment
            align_items: None,
            align_self: None,
            align_content: None,
            justify_content: None,
            // Flexbox
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Length::Auto,
            background: None,
            border_color: None,
            border_style: BorderStyle::default(),
            corner_radii: Corners::default(),
            box_shadow: Default::default(),
            text: TextStyleRefinement::default(),
            mouse_cursor: None,
            opacity: None,
            grid_rows: None,
            grid_cols: None,
            grid_location: None,

            #[cfg(debug_assertions)]
            debug: false,
            #[cfg(debug_assertions)]
            debug_below: false,
        }
    }
}

/// 可以应用于下划线的属性。
#[derive(
    Refineable, Copy, Clone, Default, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct UnderlineStyle {
    /// The thickness of the underline.
    pub thickness: Pixels,

    /// The color of the underline.
    pub color: Option<Hsla>,

    /// Whether the underline should be wavy, like in a spell checker.
    pub wavy: bool,
}

/// 可以应用于删除线的属性。
#[derive(
    Refineable, Copy, Clone, Default, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct StrikethroughStyle {
    /// The thickness of the strikethrough.
    pub thickness: Pixels,

    /// The color of the strikethrough.
    pub color: Option<Hsla>,
}

/// 可以应用于形状的填充类型。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Fill {
    /// 纯色填充。
    Color(Background),
}

impl Fill {
    /// 如果这是纯色填充，则将其解包为纯色。
    ///
    /// 如果填充不是纯色，则此方法返回 `None`。
    pub fn color(&self) -> Option<Background> {
        match self {
            Fill::Color(color) => Some(*color),
        }
    }
}

impl Default for Fill {
    fn default() -> Self {
        Self::Color(Background::default())
    }
}

impl From<Hsla> for Fill {
    fn from(color: Hsla) -> Self {
        Self::Color(color.into())
    }
}

impl From<Rgba> for Fill {
    fn from(color: Rgba) -> Self {
        Self::Color(color.into())
    }
}

impl From<Background> for Fill {
    fn from(background: Background) -> Self {
        Self::Color(background)
    }
}

impl From<TextStyle> for HighlightStyle {
    fn from(other: TextStyle) -> Self {
        Self::from(&other)
    }
}

impl From<&TextStyle> for HighlightStyle {
    fn from(other: &TextStyle) -> Self {
        Self {
            color: Some(other.color),
            font_weight: Some(other.font_weight),
            font_style: Some(other.font_style),
            background_color: other.background_color,
            underline: other.underline,
            strikethrough: other.strikethrough,
            fade_out: None,
        }
    }
}

impl HighlightStyle {
    /// 创建仅包含颜色的高亮样式
    pub fn color(color: Hsla) -> Self {
        Self {
            color: Some(color),
            ..Default::default()
        }
    }
    /// 将此高亮样式与另一个混合。
    /// 非连续属性（如 font_weight 和 font_style）将被覆盖。
    #[must_use]
    pub fn highlight(self, other: HighlightStyle) -> Self {
        Self {
            color: other
                .color
                .map(|other_color| {
                    if let Some(color) = self.color {
                        color.blend(other_color)
                    } else {
                        other_color
                    }
                })
                .or(self.color),
            font_weight: other.font_weight.or(self.font_weight),
            font_style: other.font_style.or(self.font_style),
            background_color: other.background_color.or(self.background_color),
            underline: other.underline.or(self.underline),
            strikethrough: other.strikethrough.or(self.strikethrough),
            fade_out: other
                .fade_out
                .map(|source_fade| {
                    self.fade_out
                        .map(|dest_fade| (dest_fade * (1. + source_fade)).clamp(0., 1.))
                        .unwrap_or(source_fade)
                })
                .or(self.fade_out),
        }
    }
}

impl From<Hsla> for HighlightStyle {
    fn from(color: Hsla) -> Self {
        Self {
            color: Some(color),
            ..Default::default()
        }
    }
}

impl From<FontWeight> for HighlightStyle {
    fn from(font_weight: FontWeight) -> Self {
        Self {
            font_weight: Some(font_weight),
            ..Default::default()
        }
    }
}

impl From<FontStyle> for HighlightStyle {
    fn from(font_style: FontStyle) -> Self {
        Self {
            font_style: Some(font_style),
            ..Default::default()
        }
    }
}

impl From<Rgba> for HighlightStyle {
    fn from(color: Rgba) -> Self {
        Self {
            color: Some(color.into()),
            ..Default::default()
        }
    }
}

/// 组合并合并两个迭代器中的高亮和范围。
pub fn combine_highlights(
    a: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
    b: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
) -> impl Iterator<Item = (Range<usize>, HighlightStyle)> {
    let mut endpoints = Vec::new();
    let mut highlights = Vec::new();
    for (range, highlight) in a.into_iter().chain(b) {
        if !range.is_empty() {
            let highlight_id = highlights.len();
            endpoints.push((range.start, highlight_id, true));
            endpoints.push((range.end, highlight_id, false));
            highlights.push(highlight);
        }
    }
    endpoints.sort_unstable_by_key(|(position, _, _)| *position);
    let mut endpoints = endpoints.into_iter().peekable();

    let mut active_styles = HashSet::default();
    let mut ix = 0;
    iter::from_fn(move || {
        while let Some((endpoint_ix, highlight_id, is_start)) = endpoints.peek() {
            let prev_index = mem::replace(&mut ix, *endpoint_ix);
            if ix > prev_index && !active_styles.is_empty() {
                let current_style = active_styles
                    .iter()
                    .fold(HighlightStyle::default(), |acc, highlight_id| {
                        acc.highlight(highlights[*highlight_id])
                    });
                return Some((prev_index..ix, current_style));
            }

            if *is_start {
                active_styles.insert(*highlight_id);
            } else {
                active_styles.remove(highlight_id);
            }
            endpoints.next();
        }
        None
    })
}

/// Used to control how child nodes are aligned.
/// For Flexbox it controls alignment in the cross axis
/// For Grid it controls alignment in the block axis
///
/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/align-items)
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
// Copy of taffy::style type of the same name, to derive JsonSchema.
pub enum AlignItems {
    /// 项目打包到轴的起始位置
    Start,
    /// 项目打包到轴的末尾
    End,
    /// 项目打包到相对于 flex 的轴的起始位置。
    ///
    /// 对于 flex_direction 为 RowReverse 或 ColumnReverse 的 flex 容器，这等效于 End。在所有其他情况下，它等效于 Start。
    FlexStart,
    /// 项目打包到相对于 flex 的轴的末尾。
    ///
    /// 对于 flex_direction 为 RowReverse 或 ColumnReverse 的 flex 容器，这等效于 Start。在所有其他情况下，它等效于 End。
    FlexEnd,
    /// 项目沿交叉轴中心打包
    Center,
    /// 项目对齐使其基线对齐
    Baseline,
    /// 拉伸以填充容器
    Stretch,
}
/// 用于控制如何对齐子节点。
/// 不适用于 Flexbox，如果在 flex 容器上指定将被忽略
/// 对于 Grid，它控制内联轴中的对齐
///
/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/justify-items)
pub type JustifyItems = AlignItems;
/// 用于控制如何对齐指定的节点。
/// 覆盖父节点的 `AlignItems` 属性。
/// 对于 Flexbox，它控制交叉轴中的对齐
/// 对于 Grid，它控制块轴中的对齐
///
/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/align-self)
pub type AlignSelf = AlignItems;
/// 用于控制如何对齐指定的节点。
/// 覆盖父节点的 `JustifyItems` 属性。
/// 不适用于 Flexbox，如果在 flex 子项上指定将被忽略
/// 对于 Grid，它控制内联轴中的对齐
///
/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/justify-self)
pub type JustifySelf = AlignItems;

/// 设置内容之间和周围的空间分布
/// 对于 Flexbox，它控制交叉轴中的对齐
/// 对于 Grid，它控制块轴中的对齐
///
/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/align-content)
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
// 复制同名的 taffy::style 类型，以派生 JsonSchema。
pub enum AlignContent {
    /// 项目打包到轴的起始位置
    Start,
    /// 项目打包到轴的末尾
    End,
    /// 项目打包到相对于 flex 的轴的起始位置。
    ///
    /// 对于 flex_direction 为 RowReverse 或 ColumnReverse 的 flex 容器，这等效于 End。在所有其他情况下，它等效于 Start。
    FlexStart,
    /// 项目打包到相对于 flex 的轴的末尾。
    ///
    /// 对于 flex_direction 为 RowReverse 或 ColumnReverse 的 flex 容器，这等效于 Start。在所有其他情况下，它等效于 End。
    FlexEnd,
    /// 项目沿轴中心居中
    Center,
    /// 项目拉伸以填充容器
    Stretch,
    /// 第一个和最后一个项目与容器边缘对齐（无间隙）
    /// 项目之间的间隙均匀分布。
    SpaceBetween,
    /// 第一个和最后一个项目之间的间隙与项目之间的间隙完全相同。
    /// 间隙均匀分布
    SpaceEvenly,
    /// 第一个和最后一个项目之间的间隙恰好是项目之间间隙的一半。
    /// 间隙按这些比例均匀分布。
    SpaceAround,
}

/// 设置内容之间和周围的空间分布
/// 对于 Flexbox，它控制主轴中的对齐
/// 对于 Grid，它控制内联轴中的对齐
///
/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/justify-content)
pub type JustifyContent = AlignContent;

/// 设置此节点子项使用的布局
///
/// 默认值取决于启用的功能标志。优先级顺序为：Flex、Grid、Block、None。
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
// 复制同名的 taffy::style 类型，以派生 JsonSchema。
pub enum Display {
    /// 子项将遵循块布局算法
    Block,
    /// 子项将遵循 flexbox 布局算法
    #[default]
    Flex,
    /// 子项将遵循 CSS Grid 布局算法
    Grid,
    /// 子项将不进行布局，并将遵循绝对定位
    None,
}

/// 控制 flex 项目是否强制放在一行或可以换行到多行。
///
/// 默认值为 [`FlexWrap::NoWrap`]
///
/// [规范](https://www.w3.org/TR/css-flexbox-1/#flex-wrap-property)
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
// 复制同名的 taffy::style 类型，以派生 JsonSchema。
pub enum FlexWrap {
    /// 项目不会换行并保持在单行上
    #[default]
    NoWrap,
    /// 项目将根据此项目的 [`FlexDirection`] 换行
    Wrap,
    /// 项目将以与此项目的 [`FlexDirection`] 相反的方向换行
    WrapReverse,
}

/// flexbox 布局主轴的方向。
///
/// 总是有两个垂直的布局轴：主（或主要）和交叉（或次要）。
/// 添加项目将导致它们沿主轴彼此相邻定位。
/// 通过在整个树中变化此值，你可以创建复杂的轴对齐布局。
///
/// 项目始终相对于交叉轴对齐，并相对于主轴对齐。
///
/// 默认行为是 [`FlexDirection::Row`]。
///
/// [规范](https://www.w3.org/TR/css-flexbox-1/#flex-direction-property)
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
// 复制同名的 taffy::style 类型，以派生 JsonSchema。
pub enum FlexDirection {
    /// 定义 +x 为主轴
    ///
    /// 项目将从左到右添加到行中。
    #[default]
    Row,
    /// 定义 +y 为主轴
    ///
    /// 项目将从上到下添加到列中。
    Column,
    /// 定义 -x 为主轴
    ///
    /// 项目将从右到左添加到行中。
    RowReverse,
    /// 定义 -y 为主轴
    ///
    /// 项目将从下到上添加到列中。
    ColumnReverse,
}

/// 子元素溢出容器时如何影响布局
///
/// 在 CSS 中，此属性的主要效果是控制溢出父容器的内容是否应
/// 仍然显示、被裁剪或触发容器成为滚动容器。但它也对布局有次要影响，
/// 主要的是：
///
///   - 具有非 `Visible` 溢出的 Flexbox/CSS Grid 项目的自动最小大小为 `0` 而不是基于内容
///   - `Overflow::Scroll` 节点在布局中为滚动条保留空间（宽度由 `scrollbar_width` 属性控制）
///
/// 在 Taffy 中，我们仅实现与布局相关的次要效果，因为我们不关心绘制/绘画。为
/// 滚动条保留的空间量由 `scrollbar_width` 属性控制。如果这是 `0`，则 `Scroll` 的行为与 `Hidden` 相同。
///
/// <https://developer.mozilla.org/en-US/docs/Web/CSS/overflow>
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
// 复制同名的 taffy::style 类型，以派生 JsonSchema。
pub enum Overflow {
    /// 此节点作为 flexbox/grid 项目的自动最小大小应基于其内容的大小。
    /// 溢出此节点的内容*应*贡献到其父节点的滚动区域。
    #[default]
    Visible,
    /// 此节点作为 flexbox/grid 项目的自动最小大小应基于其内容的大小。
    /// 溢出此节点的内容*不应*贡献到其父节点的滚动区域。
    Clip,
    /// 此节点作为 flexbox/grid 项目的自动最小大小应为 `0`。
    /// 溢出此节点的内容*不应*贡献到其父节点的滚动区域。
    Hidden,
    /// 此节点作为 flexbox/grid 项目的自动最小大小应为 `0`。此外，应保留空间
    /// 用于滚动条。保留的空间量由 `scrollbar_width` 属性控制。
    /// 溢出此节点的内容*不应*贡献到其父节点的滚动区域。
    Scroll,
}

/// 此项目的定位策略。
///
/// 这控制 [`Style::position`] 字段的原点如何确定，
/// 以及项目是否将由 flexbox 的布局算法控制。
///
/// 警告：此枚举遵循 [CSS 的 `position` 属性](https://developer.mozilla.org/en-US/docs/Web/CSS/position) 的行为，
/// 这可能不直观。
///
/// [`Position::Relative`] 是默认值，与 CSS 中的默认行为相反。
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
// 复制同名的 taffy::style 类型，以派生 JsonSchema。
pub enum Position {
    /// 偏移量相对于布局算法给出的最终位置计算。
    /// 偏移量不影响任何其他项目的位置；它们实际上是在最后应用的校正因子。
    #[default]
    Relative,
    /// 偏移量相对于此项目最近定位的祖先计算（如果有）。
    /// 否则，它相对于原点放置。
    /// 页面布局中不为项目创建空间，其大小也不会改变。
    ///
    /// 警告：要完全退出布局，你必须在 [`Style`] 对象上使用 [`Display::None`]。
    Absolute,
}

impl From<AlignItems> for taffy::style::AlignItems {
    fn from(value: AlignItems) -> Self {
        match value {
            AlignItems::Start => Self::Start,
            AlignItems::End => Self::End,
            AlignItems::FlexStart => Self::FlexStart,
            AlignItems::FlexEnd => Self::FlexEnd,
            AlignItems::Center => Self::Center,
            AlignItems::Baseline => Self::Baseline,
            AlignItems::Stretch => Self::Stretch,
        }
    }
}

impl From<AlignContent> for taffy::style::AlignContent {
    fn from(value: AlignContent) -> Self {
        match value {
            AlignContent::Start => Self::Start,
            AlignContent::End => Self::End,
            AlignContent::FlexStart => Self::FlexStart,
            AlignContent::FlexEnd => Self::FlexEnd,
            AlignContent::Center => Self::Center,
            AlignContent::Stretch => Self::Stretch,
            AlignContent::SpaceBetween => Self::SpaceBetween,
            AlignContent::SpaceEvenly => Self::SpaceEvenly,
            AlignContent::SpaceAround => Self::SpaceAround,
        }
    }
}

impl From<Display> for taffy::style::Display {
    fn from(value: Display) -> Self {
        match value {
            Display::Block => Self::Block,
            Display::Flex => Self::Flex,
            Display::Grid => Self::Grid,
            Display::None => Self::None,
        }
    }
}

impl From<FlexWrap> for taffy::style::FlexWrap {
    fn from(value: FlexWrap) -> Self {
        match value {
            FlexWrap::NoWrap => Self::NoWrap,
            FlexWrap::Wrap => Self::Wrap,
            FlexWrap::WrapReverse => Self::WrapReverse,
        }
    }
}

impl From<FlexDirection> for taffy::style::FlexDirection {
    fn from(value: FlexDirection) -> Self {
        match value {
            FlexDirection::Row => Self::Row,
            FlexDirection::Column => Self::Column,
            FlexDirection::RowReverse => Self::RowReverse,
            FlexDirection::ColumnReverse => Self::ColumnReverse,
        }
    }
}

impl From<Overflow> for taffy::style::Overflow {
    fn from(value: Overflow) -> Self {
        match value {
            Overflow::Visible => Self::Visible,
            Overflow::Clip => Self::Clip,
            Overflow::Hidden => Self::Hidden,
            Overflow::Scroll => Self::Scroll,
        }
    }
}

impl From<Position> for taffy::style::Position {
    fn from(value: Position) -> Self {
        match value {
            Position::Relative => Self::Relative,
            Position::Absolute => Self::Absolute,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{blue, green, px, red, yellow};

    use super::*;

    use rgpui_macros::perf;

    #[perf]
    fn test_basic_highlight_style_combination() {
        let style_a = HighlightStyle::default();
        let style_b = HighlightStyle::default();
        let style_a = style_a.highlight(style_b);
        assert_eq!(
            style_a,
            HighlightStyle::default(),
            "Combining empty styles should not produce a non-empty style."
        );

        let mut style_b = HighlightStyle {
            color: Some(red()),
            strikethrough: Some(StrikethroughStyle {
                thickness: px(2.),
                color: Some(blue()),
            }),
            fade_out: Some(0.),
            font_style: Some(FontStyle::Italic),
            font_weight: Some(FontWeight(300.)),
            background_color: Some(yellow()),
            underline: Some(UnderlineStyle {
                thickness: px(2.),
                color: Some(red()),
                wavy: true,
            }),
        };
        let expected_style = style_b;

        let style_a = style_a.highlight(style_b);
        assert_eq!(
            style_a, expected_style,
            "Blending an empty style with another style should return the other style"
        );

        let style_b = style_b.highlight(Default::default());
        assert_eq!(
            style_b, expected_style,
            "Blending a style with an empty style should not change the style."
        );

        let mut style_c = expected_style;

        let style_d = HighlightStyle {
            color: Some(blue().alpha(0.7)),
            strikethrough: Some(StrikethroughStyle {
                thickness: px(4.),
                color: Some(crate::red()),
            }),
            fade_out: Some(0.),
            font_style: Some(FontStyle::Oblique),
            font_weight: Some(FontWeight(800.)),
            background_color: Some(green()),
            underline: Some(UnderlineStyle {
                thickness: px(4.),
                color: None,
                wavy: false,
            }),
        };

        let expected_style = HighlightStyle {
            color: Some(red().blend(blue().alpha(0.7))),
            strikethrough: Some(StrikethroughStyle {
                thickness: px(4.),
                color: Some(red()),
            }),
            // TODO this does not seem right
            fade_out: Some(0.),
            font_style: Some(FontStyle::Oblique),
            font_weight: Some(FontWeight(800.)),
            background_color: Some(green()),
            underline: Some(UnderlineStyle {
                thickness: px(4.),
                color: None,
                wavy: false,
            }),
        };

        let style_c = style_c.highlight(style_d);
        assert_eq!(
            style_c, expected_style,
            "Blending styles should blend properties where possible and override all others"
        );
    }

    #[perf]
    fn test_combine_highlights() {
        assert_eq!(
            combine_highlights(
                [
                    (0..5, green().into()),
                    (4..10, FontWeight::BOLD.into()),
                    (15..20, yellow().into()),
                ],
                [
                    (2..6, FontStyle::Italic.into()),
                    (1..3, blue().into()),
                    (21..23, red().into()),
                ]
            )
            .collect::<Vec<_>>(),
            [
                (
                    0..1,
                    HighlightStyle {
                        color: Some(green()),
                        ..Default::default()
                    }
                ),
                (
                    1..2,
                    HighlightStyle {
                        color: Some(blue()),
                        ..Default::default()
                    }
                ),
                (
                    2..3,
                    HighlightStyle {
                        color: Some(blue()),
                        font_style: Some(FontStyle::Italic),
                        ..Default::default()
                    }
                ),
                (
                    3..4,
                    HighlightStyle {
                        color: Some(green()),
                        font_style: Some(FontStyle::Italic),
                        ..Default::default()
                    }
                ),
                (
                    4..5,
                    HighlightStyle {
                        color: Some(green()),
                        font_weight: Some(FontWeight::BOLD),
                        font_style: Some(FontStyle::Italic),
                        ..Default::default()
                    }
                ),
                (
                    5..6,
                    HighlightStyle {
                        font_weight: Some(FontWeight::BOLD),
                        font_style: Some(FontStyle::Italic),
                        ..Default::default()
                    }
                ),
                (
                    6..10,
                    HighlightStyle {
                        font_weight: Some(FontWeight::BOLD),
                        ..Default::default()
                    }
                ),
                (
                    15..20,
                    HighlightStyle {
                        color: Some(yellow()),
                        ..Default::default()
                    }
                ),
                (
                    21..23,
                    HighlightStyle {
                        color: Some(red()),
                        ..Default::default()
                    }
                )
            ]
        );
    }

    #[perf]
    fn test_text_style_refinement() {
        let mut style = Style::default();
        style.refine(&StyleRefinement::default().text_size(px(20.0)));
        style.refine(&StyleRefinement::default().font_weight(FontWeight::SEMIBOLD));

        assert_eq!(
            Some(AbsoluteLength::from(px(20.0))),
            style.text_style().unwrap().font_size
        );

        assert_eq!(
            Some(FontWeight::SEMIBOLD),
            style.text_style().unwrap().font_weight
        );
    }
}
