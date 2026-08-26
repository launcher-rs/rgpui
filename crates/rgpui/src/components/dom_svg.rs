//! 纯 DOM 模式下图表/矢量组件的 SVG 显示辅助。
//!
//! 这些组件（Sparkline / Waveform / SVGRenderer）通过 [`crate::elements::canvas`]
//! 用 paint API 自绘，纯 DOM 模式下 canvas 被隐藏、自绘内容不可见。这里提供把
//! 组件数据转成 SVG 字符串、再编码为 data URI 的 `<img>` 节点的方法，使组件在
//! DOM 覆盖层中原样显示（浏览器原生渲染 SVG，无需 SVG 命名空间与嵌套子节点）。

use crate::{Bounds, DomNode, DomNodeKind, DomStyle, Hsla, Pixels, px};

/// 把 [`Hsla`] 颜色转为 CSS `rgba(r,g,b,a)` 字符串。
pub(crate) fn css_color(color: Hsla) -> String {
    let rgba = color.to_rgb();
    let r = (rgba.r * 255.0).round() as u8;
    let g = (rgba.g * 255.0).round() as u8;
    let b = (rgba.b * 255.0).round() as u8;
    format!("rgba({},{},{},{})", r, g, b, rgba.a)
}

/// 生成指向 SVG 字符串的 base64 data URI（用于 `<img src>`）。
pub(crate) fn svg_data_uri(svg: &str) -> String {
    format!(
        "data:image/svg+xml;base64,{}",
        base64_encode(svg.as_bytes())
    )
}

/// RFC 4648 base64 编码（无外部依赖，SVG 内容为纯 ASCII，无需 UTF-8 处理）。
fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// 构造一个指向 SVG data URI 的 `<img>` 节点，尺寸与布局 bounds 对齐。
///
/// 返回 `None` 表示尺寸无效（宽高非正），组件不应登记 DOM 节点。
pub(crate) fn svg_img_node(bounds: Bounds<Pixels>, svg: String) -> Option<DomNode> {
    if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
        return None;
    }
    Some(DomNode {
        kind: DomNodeKind::Element {
            tag: "img",
            attrs: vec![
                ("src".into(), svg_data_uri(&svg)),
                ("draggable".into(), "false".into()),
            ],
            children: Vec::new(),
        },
        style: DomStyle::from_bounds(bounds),
    })
}
