//! LaTeX 公式渲染：ratex 解析 → 排版 → SVG（字形转路径）→ usvg 光栅化。
//!
//! 由 `latex` feature 门控。整条链路纯 Rust，无系统依赖：
//! - `ratex-parser` 解析 LaTeX 语法；
//! - `ratex-layout` 排版（em 单位坐标，y 向下）；
//! - `ratex-svg` 输出 SVG（`embed_glyphs` 把字形转成路径轮廓，不依赖字体）；
//! - rgpui 核心的 [`SvgRenderer`]（usvg / resvg）把 SVG 光栅化成 [`RenderImage`]。
//!
//! 尺寸口径：ratex 的 SVG 以 `pt` 标注尺寸，而 usvg 会按 1pt = 4/3px 换算，
//! 直接光栅化会与排版字号对不上；因此这里先把根元素单位改写为 `px`，
//! 再按改写后的数值作为逻辑尺寸展示。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use rgpui::{Hsla, Pixels, RenderImage, SvgRenderer, px};

/// 公式图片：光栅化结果 + 逻辑尺寸。
#[derive(Clone)]
pub struct LatexImage {
    /// 光栅化后的图片（可直接交给 `img` 元素）。
    pub image: Arc<RenderImage>,
    /// 逻辑宽（像素）。
    pub width: Pixels,
    /// 逻辑高（像素）。
    pub height: Pixels,
}

/// 公式缓存条目上限（超出后整体清空，避免长文档无限增长）。
const CACHE_LIMIT: usize = 64;

/// 公式图片缓存：键为「源码 + 颜色 + 字号」，值为共享图片。
fn cache() -> &'static Mutex<HashMap<String, Arc<LatexImage>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<LatexImage>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 渲染 LaTeX 为 SVG 文本（字形已转路径，可直接交给 usvg 光栅化）。
///
/// # 参数
/// - `latex`: LaTeX 源码（不含 `$` / `$$` 分隔符）。
/// - `color`: 前景色（替换 ratex 的默认黑色填充）。
/// - `font_size`: 基准字号（像素），决定公式整体大小。
///
/// 解析失败时返回错误文本（调用方一般降级为显示源码）。
pub fn render_latex_svg(latex: &str, color: Hsla, font_size: f32) -> Result<String, String> {
    let nodes = ratex_parser::parse(latex).map_err(|err| err.to_string())?;
    let layout = ratex_layout::layout(&nodes, &ratex_layout::LayoutOptions::default());
    let display_list = ratex_layout::to_display_list(&layout);
    let mut svg = ratex_svg::render_to_svg(
        &display_list,
        &ratex_svg::SvgOptions {
            font_size: f64::from(font_size.max(1.0)),
            padding: f64::from((font_size * 0.35).max(4.0)),
            embed_glyphs: true,
            ..ratex_svg::SvgOptions::default()
        },
    );
    // 单位统一为 px：ratex 用 pt 标注，usvg 会按 1pt = 4/3px 放大。
    svg = rewrite_units_to_px(&svg);
    // 默认黑色填充替换为主题前景色。
    svg = recolor_default_black(&svg, &svg_color(color));
    Ok(svg)
}

/// 公式缓存键（源码 + 颜色 + 字号）。
pub fn latex_cache_key(latex: &str, color: Hsla, font_size: f32) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    use std::hash::{Hash, Hasher};
    latex.hash(&mut hasher);
    svg_color(color).hash(&mut hasher);
    font_size.to_bits().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// 渲染公式并缓存图片（解析 / 光栅化失败时返回 `None`，由调用方降级为源码文本）。
///
/// 缓存按「源码 + 颜色 + 字号」命中，长文档里同一公式只光栅化一次。
pub fn latex_image(
    latex: &str,
    color: Hsla,
    font_size: f32,
    renderer: &SvgRenderer,
) -> Option<LatexImage> {
    let key = latex_cache_key(latex, color, font_size);
    if let Ok(guard) = cache().lock()
        && let Some(hit) = guard.get(&key)
    {
        return Some((**hit).clone());
    }

    let svg = render_latex_svg(latex, color, font_size).ok()?;
    let (width, height) = svg_size_px(&svg)?;
    // 以 1 倍逻辑尺寸光栅化：核心内部按 2 倍超采样，兼顾高分屏的清晰度。
    let image = renderer.render_single_frame(svg.as_bytes(), 1.0).ok()?;
    let result = Arc::new(LatexImage {
        image,
        width: px(width),
        height: px(height),
    });

    if let Ok(mut guard) = cache().lock() {
        if guard.len() >= CACHE_LIMIT {
            guard.clear();
        }
        guard.insert(key, result.clone());
    }
    Some((*result).clone())
}

/// 把主题颜色转为 SVG 颜色串。
fn svg_color(color: Hsla) -> String {
    let color = rgpui::Rgba::from(color);
    format!(
        "rgba({},{},{},{})",
        color_channel(color.r),
        color_channel(color.g),
        color_channel(color.b),
        trim_float(f64::from(color.a.clamp(0.0, 1.0)))
    )
}

/// 取颜色通道字节值。
fn color_channel(channel: f32) -> u8 {
    (channel.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// 精简浮点数字符串。
fn trim_float(value: f64) -> String {
    let formatted = format!("{value:.3}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

/// 把默认黑色填充替换为指定颜色。
fn recolor_default_black(svg: &str, color: &str) -> String {
    svg.replace("rgba(0,0,0,1)", color)
        .replace("rgba(0, 0, 0, 1)", color)
}

/// 把 SVG 根元素上的 `pt` 单位改写为 `px`（usvg 会按 1pt = 4/3px 换算）。
fn rewrite_units_to_px(svg: &str) -> String {
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

/// 解析 SVG 根元素的像素尺寸。
fn svg_size_px(svg: &str) -> Option<(f32, f32)> {
    Some((svg_attr_px(svg, "width")?, svg_attr_px(svg, "height")?))
}

/// 读取 SVG 根元素上的数值属性（忽略单位后缀）。
fn svg_attr_px(svg: &str, name: &str) -> Option<f32> {
    let key = format!("{name}=\"");
    let start = svg.find(&key)? + key.len();
    let end = svg[start..].find('"')? + start;
    let raw = svg[start..end]
        .trim()
        .trim_end_matches("px")
        .trim_end_matches("pt");
    raw.parse::<f32>().ok()
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{latex_cache_key, render_latex_svg, rewrite_units_to_px, svg_size_px};
    use rgpui::{Hsla, rgba};

    /// 公式能渲染出带尺寸的 SVG。
    #[test]
    fn renders_svg_with_size() {
        let svg =
            render_latex_svg("\\frac{1}{2}", Hsla::from(rgba(0xffffffff)), 18.0).expect("公式渲染");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
        // 字形必须转成路径轮廓：usvg 不认识 KaTeX 字体，`standalone` 未生效时会是 <text>。
        assert!(
            svg.contains("<path"),
            "字形未转路径：{}",
            &svg[..svg.len().min(400)]
        );
        let (width, height) = svg_size_px(&svg).expect("尺寸");
        assert!(width > 0.0);
        assert!(height > 0.0);
    }

    /// 非法 LaTeX 返回错误（调用方据此降级为源码文本）。
    #[test]
    fn invalid_latex_returns_error() {
        assert!(render_latex_svg("\\frac{a}", Hsla::from(rgba(0xffffffff)), 18.0).is_err());
    }

    /// 主题色替换掉默认黑色填充。
    #[test]
    fn recolors_default_black() {
        let svg = render_latex_svg("x", Hsla::from(rgba(0xff0000ff)), 18.0).expect("公式渲染");
        assert!(!svg.contains("rgba(0,0,0,1)"));
    }

    /// 缓存键随颜色 / 字号变化。
    #[test]
    fn cache_key_changes_with_visual_inputs() {
        let first = latex_cache_key("\\frac{1}{2}", Hsla::from(rgba(0xffffffff)), 18.0);
        let second = latex_cache_key("\\frac{1}{2}", Hsla::from(rgba(0x000000ff)), 18.0);
        let third = latex_cache_key("\\frac{1}{2}", Hsla::from(rgba(0xffffffff)), 20.0);
        assert_ne!(first, second);
        assert_ne!(first, third);
    }

    /// 单位改写只影响根元素，`pt` 后缀换成 `px`。
    #[test]
    fn rewrites_pt_units_to_px() {
        let rewritten = rewrite_units_to_px("<svg width=\"10pt\" height=\"4pt\"><g/></svg>");
        assert!(rewritten.contains("width=\"10px\""));
        assert!(rewritten.contains("height=\"4px\""));
    }
}
