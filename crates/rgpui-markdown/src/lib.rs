//! # rgpui-markdown
//!
//! rgpui 的 Markdown 渲染组件库。
//!
//! 独立成库（见 `docs/ui-crate-plan.md` §5 方案 A）：重依赖（pulldown-cmark）不污染
//! rgpui 核心默认构建；单用途自包含，符合 rgpui-term / rgpui-3d 的独立库模式。
//!
//! 提供能力：
//! - [`Markdown`]：将 Markdown 源码解析并渲染为富文本元素。
//! - [`rich_text`]：富文本块模型（段落/标题/列表/表格/引用/代码块）与渲染函数。
//! - [`code_block`]：带行号、复制按钮与简易高亮的代码块组件。
//! - [`toc`]：文档大纲提取（标题级别 / 文本 / 源码区间 / 锚点）。
//! - [`latex`]（`latex` feature）：LaTeX 公式渲染成图片。
//! - [`mermaid`]（`mermaid` feature）：Mermaid 围栏代码块渲染成图表。

// 生产代码禁用 `unwrap` / `expect` / `panic!`（与 rgpui 核心同一口径）；
// 测试代码由根目录 `clippy.toml` 的 `allow-*-in-tests` 放行。
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod code_block;
pub mod markdown;
pub mod rich_text;
pub mod toc;

/// SVG 光栅化辅助（公式 / 图表管线共用）。
#[cfg(any(feature = "latex", feature = "mermaid"))]
mod svg_image;

/// LaTeX 公式渲染（解析 → 排版 → SVG → 光栅化），由 `latex` feature 门控。
#[cfg(feature = "latex")]
pub mod latex;

/// Mermaid 图表渲染（SVG → 光栅化），由 `mermaid` feature 门控。
#[cfg(feature = "mermaid")]
pub mod mermaid;

pub use code_block::{CodeBlock, CodeBlockCopyState};
#[cfg(feature = "latex")]
pub use latex::{LatexImage, latex_image, render_latex_svg};
pub use markdown::Markdown;
#[cfg(feature = "mermaid")]
pub use mermaid::{MermaidImage, is_mermaid_language, mermaid_image, render_mermaid_svg};
pub use rich_text::{
    CalloutKind, CodeBlockRenderer, LinkClickHandler, ListItem, RichBlock, RichInline,
    TableAlignment, render_blocks_with,
};
pub use toc::{Heading, extract_headings};
