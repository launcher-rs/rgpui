//! Markdown 解析层。
//!
//! 该 crate 只负责把 Markdown 解析为 mdast，不依赖 component 渲染层。

pub use markdown::{mdast, unist};
use rgpui::SharedString;

/// 使用 GFM 规则解析 Markdown，并返回 mdast 根节点。
pub fn parse_gfm(source: &str) -> Result<mdast::Node, SharedString> {
    markdown::to_mdast(source, &markdown::ParseOptions::gfm()).map_err(|err| err.to_string().into())
}
