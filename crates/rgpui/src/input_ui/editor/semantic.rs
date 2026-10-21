//! LSP 语义高亮接入状态（`editor` feature 门控）。
//!
//! 语义 token 的请求与高亮器装配在 `lsp_attach.rs`（与其它 LSP 接线同处），
//! 本模块只放状态结构。

use std::rc::Rc;

use crate::lsp::{SemanticTokenTypeMap, SemanticTokensProvider};

/// 语义高亮接入状态（`EditorState` 内嵌小字段）。
pub(super) struct SemanticState {
    /// provider（应用注入，传输自理；`None` 即关闭）。
    pub(super) provider: Option<Rc<dyn SemanticTokensProvider>>,
    /// 类型映射表（LSP 类型名 → 主题 token 名）。
    pub(super) type_map: SemanticTokenTypeMap,
    /// 语言名（高亮器上报用）。
    pub(super) language: String,
}

impl SemanticState {
    /// 创建状态（未接 provider）。
    pub(super) fn new() -> Self {
        Self {
            provider: None,
            type_map: SemanticTokenTypeMap::default(),
            language: String::new(),
        }
    }
}
