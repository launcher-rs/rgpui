//! DOM 样式 → CSS 内联样式序列化。
//!
//! 布局沿用 Taffy 结果，一律输出 `position: absolute` + `left/top/width/height`，
//! 不依赖浏览器重排；视觉字段按 [`rgpui::DomStyle`] 字段逐一映射。

use rgpui::{
    BorderStyle, CursorStyle, DomDisplay, DomGradientKind, DomOverflow, DomPosition, DomStyle,
    DomTextDecoration, FontStyle, FontWeight, Hsla, TextAlign, WhiteSpace,
};
use std::fmt::Write;

/// 把一个 [`DomStyle`] 序列化为 CSS 内联样式字符串（如 `position:absolute;left:10px;...`）。
pub fn dom_style_to_css(style: &DomStyle) -> String {
    let mut out = String::new();
    match style.position {
        DomPosition::Absolute => {
            out.push_str("position:absolute;");
            push_px(&mut out, "left", style.left);
            push_px(&mut out, "top", style.top);
            push_px(&mut out, "width", style.width);
            push_px(&mut out, "height", style.height);
        }
        DomPosition::Relative => {
            out.push_str("position:relative;");
            push_px(&mut out, "left", style.left);
            push_px(&mut out, "top", style.top);
            push_px(&mut out, "width", style.width);
            push_px(&mut out, "height", style.height);
        }
        DomPosition::Static => {
            // 行内子元素（富文本 run 片段）：不输出定位，随父节点自然流动。
            out.push_str("position:static;");
        }
    }

    match style.display {
        DomDisplay::Block => {}
        DomDisplay::None => out.push_str("display:none;"),
    }
    match style.overflow {
        DomOverflow::Visible => {}
        DomOverflow::Hidden => out.push_str("overflow:hidden;"),
        DomOverflow::Scroll => out.push_str("overflow:auto;"),
    }

    if let Some(color) = style.color {
        let _ = write!(out, "color:{};", hsla_to_css(color));
    }
    if let Some(gradient) = &style.background_gradient {
        let _ = write!(out, "background-image:{};", gradient_to_css(gradient));
    } else if let Some(background) = style.background_color {
        let _ = write!(out, "background-color:{};", hsla_to_css(background));
    }
    if let Some(radius) = style.border_radius {
        push_px(&mut out, "border-radius", radius);
    }
    if let Some(color) = style.border_color {
        let _ = write!(out, "border-color:{};", hsla_to_css(color));
    }
    if let Some(width) = style.border_width {
        push_px(&mut out, "border-width", width);
        // 边框样式默认实线；虚线映射为 CSS dashed（DOM 模式下虚线分隔线依赖此映射）。
        let border_style = match style.border_style {
            Some(BorderStyle::Dashed) => "dashed",
            _ => "solid",
        };
        let _ = write!(out, "border-style:{};", border_style);
    }
    if !style.box_shadows.is_empty() {
        let _ = write!(
            out,
            "box-shadow:{};",
            box_shadows_to_css(&style.box_shadows)
        );
    }
    if let Some(size) = style.font_size {
        push_px(&mut out, "font-size", size);
    }
    if let Some(family) = &style.font_family {
        // 必须加引号：多词字族（如 `Inter Variable`）不加引号会被解析成两个族名，
        // 以 `.` 开头的字族（如 `.SystemUIFont`）不加引号则不是合法 CSS ident，都会静默失效。
        let _ = write!(out, "font-family:\"{}\";", family);
    }
    if let Some(weight) = style.font_weight {
        let _ = write!(out, "font-weight:{};", weight_to_css(weight));
    }
    if let Some(font_style) = style.font_style {
        let _ = write!(out, "font-style:{};", font_style_to_css(font_style));
    }
    if let Some(line_height) = style.line_height {
        push_px(&mut out, "line-height", line_height);
    }
    if let Some(align) = style.text_align {
        let _ = write!(out, "text-align:{};", text_align_to_css(align));
    }
    if let Some(white_space) = style.white_space {
        let _ = write!(out, "white-space:{};", white_space_to_css(white_space));
    }
    match style.text_decoration {
        DomTextDecoration::None => {}
        DomTextDecoration::Underline => out.push_str("text-decoration:underline;"),
        DomTextDecoration::LineThrough => out.push_str("text-decoration:line-through;"),
    }
    if let Some(cursor) = style.cursor {
        let _ = write!(out, "cursor:{};", cursor_to_css(cursor));
    }
    if let Some(opacity) = style.opacity {
        let _ = write!(out, "opacity:{};", opacity);
    }
    let _ = write!(out, "z-index:{};", style.z_index);

    out
}

/// 把渐变序列化为 CSS `background-image` 函数字符串。
fn gradient_to_css(gradient: &rgpui::DomGradient) -> String {
    let mut stops = String::new();
    for (i, (color, position)) in gradient.stops.iter().enumerate() {
        if i > 0 {
            stops.push_str(", ");
        }
        let _ = write!(stops, "{} {}%", hsla_to_css(*color), position * 100.0);
    }
    match gradient.kind {
        DomGradientKind::Linear => {
            format!("linear-gradient({}deg, {})", gradient.angle, stops)
        }
        DomGradientKind::Radial => {
            // v1 径向渐变以圆形近似（中心默认 50% 50%）。
            format!("radial-gradient(circle, {})", stops)
        }
        DomGradientKind::Conic => {
            format!("conic-gradient(from {}deg, {})", gradient.angle, stops)
        }
    }
}

/// 把盒阴影列表序列化为 CSS `box-shadow` 值。
fn box_shadows_to_css(shadows: &[rgpui::DomBoxShadow]) -> String {
    let mut parts = Vec::with_capacity(shadows.len());
    for shadow in shadows {
        let inset = if shadow.inset { "inset " } else { "" };
        parts.push(format!(
            "{}{}px {}px {}px {}px {}",
            inset,
            shadow.offset_x.as_f32(),
            shadow.offset_y.as_f32(),
            shadow.blur_radius.as_f32(),
            shadow.spread_radius.as_f32(),
            hsla_to_css(shadow.color)
        ));
    }
    parts.join(", ")
}

/// 拼接一个像素值（px）。
fn push_px(out: &mut String, name: &str, value: rgpui::Pixels) {
    let _ = write!(out, "{name}:{}px;", value.as_f32());
}

/// 把 [`Hsla`] 序列化为 CSS 颜色（`hsla(deg, sat%, light%, alpha)`）。
fn hsla_to_css(color: Hsla) -> String {
    format!(
        "hsla({}deg, {}%, {}%, {})",
        color.h * 360.0,
        color.s * 100.0,
        color.l * 100.0,
        color.a
    )
}

/// 把 [`FontWeight`] 序列化为 CSS `font-weight` 数值。
fn weight_to_css(weight: FontWeight) -> String {
    format!("{}", weight.0)
}

/// 把 [`FontStyle`] 序列化为 CSS `font-style`。
fn font_style_to_css(style: FontStyle) -> &'static str {
    match style {
        FontStyle::Normal => "normal",
        FontStyle::Italic => "italic",
        FontStyle::Oblique => "oblique",
    }
}

/// 把 [`TextAlign`] 序列化为 CSS `text-align`。
fn text_align_to_css(align: TextAlign) -> &'static str {
    match align {
        TextAlign::Left => "left",
        TextAlign::Center => "center",
        TextAlign::Right => "right",
    }
}

/// 把 [`WhiteSpace`] 序列化为 CSS `white-space`。
fn white_space_to_css(white_space: WhiteSpace) -> &'static str {
    match white_space {
        WhiteSpace::Normal => "normal",
        WhiteSpace::Nowrap => "nowrap",
    }
}

/// 把 [`CursorStyle`] 映射为 CSS `cursor` 值（枚举文档中即标注了对应 CSS 值）。
fn cursor_to_css(cursor: CursorStyle) -> &'static str {
    match cursor {
        CursorStyle::Arrow => "default",
        CursorStyle::IBeam => "text",
        CursorStyle::Crosshair => "crosshair",
        CursorStyle::ClosedHand => "grabbing",
        CursorStyle::OpenHand => "grab",
        CursorStyle::PointingHand => "pointer",
        CursorStyle::ResizeLeft => "w-resize",
        CursorStyle::ResizeRight => "e-resize",
        CursorStyle::ResizeLeftRight => "ew-resize",
        CursorStyle::ResizeUp => "n-resize",
        CursorStyle::ResizeDown => "s-resize",
        CursorStyle::ResizeUpDown => "ns-resize",
        CursorStyle::ResizeUpLeftDownRight => "nesw-resize",
        CursorStyle::ResizeUpRightDownLeft => "nwse-resize",
        CursorStyle::ResizeColumn => "col-resize",
        CursorStyle::ResizeRow => "row-resize",
        CursorStyle::IBeamCursorForVerticalLayout => "vertical-text",
        CursorStyle::OperationNotAllowed => "not-allowed",
        CursorStyle::DragLink => "alias",
        CursorStyle::DragCopy => "copy",
        CursorStyle::ContextualMenu => "context-menu",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgpui::px;

    #[test]
    fn test_dom_style_to_css() {
        let style = DomStyle {
            left: px(10.0),
            top: px(20.0),
            width: px(100.0),
            height: px(50.0),
            background_color: Some(Hsla {
                h: 0.5,
                s: 1.0,
                l: 0.5,
                a: 1.0,
            }),
            border_radius: Some(px(8.0)),
            cursor: Some(CursorStyle::PointingHand),
            z_index: 3,
            ..Default::default()
        };
        let css = dom_style_to_css(&style);
        assert!(css.contains("position:absolute"));
        assert!(css.contains("left:10px"));
        assert!(css.contains("top:20px"));
        assert!(css.contains("width:100px"));
        assert!(css.contains("height:50px"));
        assert!(css.contains("background-color:hsla(180deg, 100%, 50%, 1)"));
        assert!(css.contains("border-radius:8px"));
        assert!(css.contains("cursor:pointer"));
        assert!(css.contains("z-index:3"));
    }

    #[test]
    fn test_dom_style_to_css_hidden() {
        let style = DomStyle {
            display: DomDisplay::None,
            overflow: DomOverflow::Hidden,
            ..Default::default()
        };
        let css = dom_style_to_css(&style);
        assert!(css.contains("display:none"));
        assert!(css.contains("overflow:hidden"));
    }

    #[test]
    fn test_dom_style_to_css_font_family_quoted() {
        // 多词/点开头的字族必须加引号，否则浏览器会静默忽略或当成多个族名。
        for family in ["Inter Variable", ".SystemUIFont"] {
            let style = DomStyle {
                font_family: Some(family.into()),
                ..Default::default()
            };
            let css = dom_style_to_css(&style);
            assert!(css.contains(&format!("font-family:\"{family}\";")), "{css}");
        }
    }

    #[test]
    fn test_dom_style_to_css_gradient_border_shadow() {
        // 渐变/边框/盒阴影序列化（纯 DOM 渲染模式下的完整视觉样式）。
        use rgpui::{DomBoxShadow, DomGradient, DomGradientKind, hsla};

        let style = DomStyle {
            left: px(0.0),
            top: px(0.0),
            width: px(40.0),
            height: px(40.0),
            background_gradient: Some(DomGradient {
                kind: DomGradientKind::Linear,
                angle: 135.0,
                stops: vec![
                    (hsla(0.6, 1.0, 0.5, 1.0), 0.0),
                    (hsla(0.3, 1.0, 0.5, 1.0), 1.0),
                ],
            }),
            border_color: Some(hsla(0.0, 0.0, 0.0, 1.0)),
            border_width: Some(px(2.0)),
            box_shadows: vec![DomBoxShadow {
                color: hsla(0.0, 0.0, 0.0, 0.5),
                offset_x: px(1.0),
                offset_y: px(2.0),
                blur_radius: px(3.0),
                spread_radius: px(4.0),
                inset: false,
            }],
            z_index: 0,
            ..Default::default()
        };
        let css = dom_style_to_css(&style);
        assert!(
            css.contains("background-image:linear-gradient(135deg,"),
            "{css}"
        );
        assert!(css.contains("0%"), "{css}");
        assert!(css.contains("100%"), "{css}");
        assert!(css.contains("border-width:2px"), "{css}");
        assert!(css.contains("border-style:solid"), "{css}");
        assert!(css.contains("border-color:"), "{css}");
        assert!(css.contains("box-shadow:1px 2px 3px 4px "), "{css}");
        // 渐变与纯色背景互斥：有渐变时不输出 background-color。
        assert!(!css.contains("background-color"), "{css}");
    }
}
