//! LSP 定义跳转支持。

use lsp_types::{GotoDefinitionResponse, Location, LocationLink, Uri};

use crate::App;

/// 定义提供者 trait。
pub trait DefinitionProvider {
    /// 跳转到定义。
    fn definition(
        &self,
        text: &ropey::Rope,
        offset: usize,
        window: &mut crate::Window,
        cx: &mut App,
    ) -> crate::Task<anyhow::Result<Vec<DefinitionLocation>>>;

    /// 查找所有引用。
    fn references(
        &self,
        text: &ropey::Rope,
        offset: usize,
        include_declarations: bool,
        window: &mut crate::Window,
        cx: &mut App,
    ) -> crate::Task<anyhow::Result<Vec<DefinitionLocation>>>;

    /// 实现符号高亮（相同符号高亮）。
    fn document_highlights(
        &self,
        text: &ropey::Rope,
        offset: usize,
        window: &mut crate::Window,
        _cx: &mut App,
    ) -> crate::Task<anyhow::Result<Vec<DocumentHighlight>>> {
        let _ = (text, offset, window);
        crate::Task::ready(Ok(vec![]))
    }
}

/// 定义位置。
#[derive(Debug, Clone)]
pub struct DefinitionLocation {
    /// 目标 URI。
    pub uri: Uri,
    /// 目标范围。
    pub range: lsp_types::Range,
    /// 显示名称（如文件名）。
    pub display_name: Option<String>,
}

impl DefinitionLocation {
    /// 从 LSP Location 构建。
    pub fn from_location(location: Location) -> Self {
        Self {
            uri: location.uri,
            range: location.range,
            display_name: None,
        }
    }

    /// 从 LSP LocationLink 构建。
    pub fn from_location_link(link: LocationLink) -> Self {
        Self {
            uri: link.target_uri,
            range: link.target_selection_range,
            display_name: None,
        }
    }

    /// 从 GotoDefinitionResponse 构建。
    pub fn from_response(response: GotoDefinitionResponse) -> Vec<Self> {
        match response {
            GotoDefinitionResponse::Scalar(location) => vec![Self::from_location(location)],
            GotoDefinitionResponse::Array(locations) => {
                locations.into_iter().map(Self::from_location).collect()
            }
            GotoDefinitionResponse::Link(links) => {
                links.into_iter().map(Self::from_location_link).collect()
            }
        }
    }
}

/// 文档高亮类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentHighlightKind {
    /// 文本高亮。
    Text,
    /// 读取高亮。
    Read,
    /// 写入高亮。
    Write,
}

/// 文档高亮项。
#[derive(Debug, Clone)]
pub struct DocumentHighlight {
    /// 高亮范围。
    pub range: lsp_types::Range,
    /// 高亮类型。
    pub kind: Option<DocumentHighlightKind>,
}

impl DocumentHighlight {
    /// 从 LSP DocumentHighlight 构建。
    pub fn from_lsp(highlight: lsp_types::DocumentHighlight) -> Self {
        Self {
            range: highlight.range,
            kind: highlight.kind.map(|k| match k {
                lsp_types::DocumentHighlightKind::TEXT => DocumentHighlightKind::Text,
                lsp_types::DocumentHighlightKind::READ => DocumentHighlightKind::Read,
                lsp_types::DocumentHighlightKind::WRITE => DocumentHighlightKind::Write,
                _ => DocumentHighlightKind::Text,
            }),
        }
    }
}
