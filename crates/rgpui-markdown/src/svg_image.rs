//! SVG 光栅化辅助：公式（`latex`）与图表（`mermaid`）两条管线共用。
//!
//! 仅在 `latex` / `mermaid` 任一 feature 开启时编译。

use std::sync::Arc;

use rgpui::{RenderImage, SvgRenderer};

/// 解析 SVG 根元素的像素尺寸。
pub(crate) fn svg_size_px(svg: &str) -> Option<(f32, f32)> {
    Some((svg_attr_px(svg, "width")?, svg_attr_px(svg, "height")?))
}

/// 读取 SVG 根元素上的数值属性（忽略单位后缀）。
pub(crate) fn svg_attr_px(svg: &str, name: &str) -> Option<f32> {
    let key = format!("{name}=\"");
    let start = svg.find(&key)? + key.len();
    let end = svg[start..].find('"')? + start;
    let raw = svg[start..end]
        .trim()
        .trim_end_matches("px")
        .trim_end_matches("pt");
    raw.parse::<f32>().ok()
}

/// 把 SVG 根元素上的 `pt` 单位改写为 `px`（usvg 会按 1pt = 4/3px 换算）。
///
/// ratex 的公式 SVG 以 `pt` 标注尺寸而实际数值是像素口径，不改写会与排版字号脱节。
#[cfg(feature = "latex")]
pub(crate) fn rewrite_units_to_px(svg: &str) -> String {
    let Some(start) = svg.find("<svg") else {
        return svg.to_string();
    };
    let Some(rel_end) = svg[start..].find('>') else {
        return svg.to_string();
    };
    let end = start + rel_end;
    let (head, tail) = svg.split_at(end);
    let head = head.replace("pt\"", "px\"");
    format!("{head}{tail}")
}

/// 光栅化 SVG 为图片（按 1 倍逻辑尺寸，核心内部按 2 倍超采样，兼顾高分屏清晰度）。
pub(crate) fn rasterize(svg: &str, renderer: &SvgRenderer) -> Option<Arc<RenderImage>> {
    renderer.render_single_frame(svg.as_bytes(), 1.0).ok()
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::svg_size_px;

    /// 单位改写只影响根元素，`pt` 后缀换成 `px`。
    #[cfg(feature = "latex")]
    #[test]
    fn rewrites_pt_units_to_px() {
        let rewritten = super::rewrite_units_to_px("<svg width=\"10pt\" height=\"4pt\"><g/></svg>");
        assert!(rewritten.contains("width=\"10px\""));
        assert!(rewritten.contains("height=\"4px\""));
    }

    /// 尺寸解析忽略单位后缀。
    #[test]
    fn parses_svg_size() {
        let svg = "<svg width=\"120.5px\" height=\"64px\"></svg>";
        let (width, height) = svg_size_px(svg).expect("尺寸");
        assert!((width - 120.5).abs() < 0.01);
        assert!((height - 64.0).abs() < 0.01);
    }
}
