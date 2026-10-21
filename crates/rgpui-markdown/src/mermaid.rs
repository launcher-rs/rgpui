//! Mermaid 图表渲染：mermaid-rs-renderer → SVG → usvg 光栅化。
//!
//! 由 `mermaid` feature 门控。`mermaid-rs-renderer` 是纯 Rust 实现（无浏览器、
//! 无系统依赖），能把 Mermaid 源码渲染成自包含 SVG；再交给 rgpui 核心的
//! [`SvgRenderer`]（usvg / resvg）光栅化成图片。
//!
//! 与核心 `components/mermaid.rs` 的 `MermaidDiagram` 的分工：后者是 **flowchart
//! 子集 + div 渲染**（不依赖 SVG 管道、可直接用 div 排版）；本模块走 **完整
//! Mermaid 语法 + SVG 渲染**（围栏代码块自动识别）。
//!
//! 尺寸口径：SVG 以 `px` 标注，按其固有尺寸光栅化；超出容器宽度时按比例缩放
//! （图片本身是 2 倍超采样，缩放后仍清晰）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use rgpui::{Pixels, RenderImage, SvgRenderer, px};

use crate::svg_image::{rasterize, svg_size_px};

/// 图表图片缓存条目上限（超出后整体清空，避免长文档无限增长）。
const CACHE_LIMIT: usize = 32;

/// Mermaid 图片：光栅化结果 + 逻辑尺寸。
#[derive(Clone)]
pub struct MermaidImage {
    /// 光栅化后的图片（可直接交给 `img` 元素）。
    pub image: Arc<RenderImage>,
    /// 逻辑宽（像素）。
    pub width: Pixels,
    /// 逻辑高（像素）。
    pub height: Pixels,
}

/// 图表图片缓存：键为源码哈希。
fn cache() -> &'static Mutex<HashMap<u64, Arc<MermaidImage>>> {
    static CACHE: OnceLock<Mutex<HashMap<u64, Arc<MermaidImage>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 判断代码块语言是否声明 Mermaid 内容（`mermaid` / `mmd`，大小写不敏感）。
pub fn is_mermaid_language(language: Option<&str>) -> bool {
    language
        .and_then(|lang| lang.split_whitespace().next())
        .is_some_and(|first| {
            first.eq_ignore_ascii_case("mermaid") || first.eq_ignore_ascii_case("mmd")
        })
}

/// 渲染 Mermaid 源码为 SVG 文本（`dark` 决定图表配色，解析失败时返回错误文本）。
pub fn render_mermaid_svg(source: &str, dark: bool) -> Result<String, String> {
    let options = mermaid_rs_renderer::RenderOptions {
        theme: if dark {
            mermaid_rs_renderer::Theme::dark()
        } else {
            mermaid_rs_renderer::Theme::modern()
        },
        ..Default::default()
    };
    mermaid_rs_renderer::render_with_options(source, options).map_err(|err| err.to_string())
}

/// 图表缓存键（源码内容哈希 + 深浅配色标记）。
pub fn mermaid_cache_key(source: &str, dark: bool) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    use std::hash::{Hash, Hasher};
    source.hash(&mut hasher);
    dark.hash(&mut hasher);
    hasher.finish()
}

/// 渲染并缓存 Mermaid 图片（解析 / 光栅化失败时返回 `None`，由调用方降级为代码块）。
///
/// 缓存按源码内容命中，长文档里同一张图只渲染一次。
pub fn mermaid_image(source: &str, renderer: &SvgRenderer, dark: bool) -> Option<MermaidImage> {
    let key = mermaid_cache_key(source, dark);
    if let Ok(guard) = cache().lock()
        && let Some(hit) = guard.get(&key)
    {
        return Some((**hit).clone());
    }

    let svg = render_mermaid_svg(source, dark).ok()?;
    let (width, height) = svg_size_px(&svg)?;
    let image = rasterize(&svg, renderer)?;
    let result = Arc::new(MermaidImage {
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

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{is_mermaid_language, mermaid_cache_key, render_mermaid_svg};

    /// 代码块语言识别：`mermaid` / `mmd`（大小写不敏感），其余不算。
    #[test]
    fn detects_mermaid_language() {
        assert!(is_mermaid_language(Some("mermaid")));
        assert!(is_mermaid_language(Some("Mermaid")));
        assert!(is_mermaid_language(Some("mmd")));
        assert!(is_mermaid_language(Some("mermaid title=x")));
        assert!(!is_mermaid_language(Some("rust")));
        assert!(!is_mermaid_language(None));
    }

    /// 能渲染出 SVG（flowchart 子集）。
    #[test]
    fn renders_flowchart_svg() {
        let svg = render_mermaid_svg("flowchart TD\nA[开始] --> B[结束]", false).expect("图表渲染");
        assert!(svg.contains("<svg"));
    }

    /// 非法源码返回错误（调用方据此降级为代码块）。
    #[test]
    fn invalid_source_returns_error() {
        assert!(render_mermaid_svg("这不是 mermaid", false).is_err());
    }

    /// 缓存键随源码 / 配色变化。
    #[test]
    fn cache_key_changes_with_source() {
        let first = mermaid_cache_key("flowchart TD\nA --> B", false);
        let second = mermaid_cache_key("flowchart TD\nA --> C", false);
        let third = mermaid_cache_key("flowchart TD\nA --> B", true);
        assert_ne!(first, second);
        assert_ne!(first, third);
    }
}
