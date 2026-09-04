//! LSP 语义高亮支持。

use lsp_types::{SemanticToken, SemanticTokenModifier, SemanticTokenType, Uri};

use crate::App;

/// 语义高亮提供者 trait。
pub trait SemanticTokensProvider {
    /// 请求完整文档的语义高亮。
    fn semantic_tokens_full(
        &self,
        uri: &Uri,
        window: &mut crate::Window,
        _cx: &mut App,
    ) -> crate::Task<anyhow::Result<Option<Vec<SemanticToken>>>> {
        let _ = (uri, window);
        crate::Task::ready(Ok(None))
    }

    /// 请求指定范围的语义高亮。
    fn semantic_tokens_range(
        &self,
        uri: &Uri,
        start: lsp_types::Position,
        end: lsp_types::Position,
        window: &mut crate::Window,
        _cx: &mut App,
    ) -> crate::Task<anyhow::Result<Option<Vec<SemanticToken>>>> {
        let _ = (uri, start, end, window);
        crate::Task::ready(Ok(None))
    }
}

/// 语义高亮类型映射表。
///
/// 将 LSP 语义高亮类型名称映射到主题 token 名称。
pub struct SemanticTokenTypeMap {
    /// 类型名称 → 主题 token 名称的映射。
    mappings: std::collections::HashMap<String, String>,
}

impl Default for SemanticTokenTypeMap {
    fn default() -> Self {
        let mut mappings = std::collections::HashMap::new();

        // 标准 LSP 语义高亮类型
        mappings.insert("namespace".into(), "namespace".into());
        mappings.insert("type".into(), "type".into());
        mappings.insert("class".into(), "class".into());
        mappings.insert("enum".into(), "enum".into());
        mappings.insert("interface".into(), "interface".into());
        mappings.insert("struct".into(), "struct".into());
        mappings.insert("typeParameter".into(), "typeParameter".into());
        mappings.insert("parameter".into(), "parameter".into());
        mappings.insert("variable".into(), "variable".into());
        mappings.insert("property".into(), "property".into());
        mappings.insert("enumMember".into(), "enumMember".into());
        mappings.insert("event".into(), "event".into());
        mappings.insert("function".into(), "function".into());
        mappings.insert("method".into(), "method".into());
        mappings.insert("macro".into(), "macro".into());
        mappings.insert("keyword".into(), "keyword".into());
        mappings.insert("modifier".into(), "modifier".into());
        mappings.insert("comment".into(), "comment".into());
        mappings.insert("string".into(), "string".into());
        mappings.insert("number".into(), "number".into());
        mappings.insert("regexp".into(), "regexp".into());
        mappings.insert("operator".into(), "operator".into());
        mappings.insert("decorator".into(), "decorator".into());
        mappings.insert("label".into(), "label".into());

        Self { mappings }
    }
}

impl SemanticTokenTypeMap {
    /// 自定义映射。
    pub fn with_mapping(mut self, from: &str, to: &str) -> Self {
        self.mappings.insert(from.into(), to.into());
        self
    }

    /// 获取映射的主题 token 名称。
    pub fn get(&self, lsp_type: &str) -> Option<&str> {
        self.mappings.get(lsp_type).map(|s| s.as_str())
    }
}

/// 语义高亮状态。
#[derive(Default)]
pub struct SemanticTokensState {
    /// 语义高亮 token 数据。
    pub tokens: Vec<SemanticToken>,
    /// token 类型定义（从服务器获取）。
    pub token_types: Vec<SemanticTokenType>,
    /// token 修饰符定义。
    pub token_modifiers: Vec<SemanticTokenModifier>,
    /// 类型映射表。
    pub type_map: SemanticTokenTypeMap,
}

impl SemanticTokensState {
    /// 解码增量语义高亮数据。
    pub fn decode_tokens(
        _token_types: &[SemanticTokenType],
        _token_modifiers: &[SemanticTokenModifier],
        data: &[u32],
    ) -> Vec<SemanticToken> {
        let mut tokens = Vec::new();
        let mut delta_line = 0u32;
        let mut delta_start = 0u32;
        let mut modifier_bitset = 0u32;

        let mut i = 0;
        while i + 4 < data.len() {
            delta_line += data[i];
            delta_start = if delta_line == 0 {
                delta_start + data[i + 1]
            } else {
                data[i + 1]
            };
            let length = data[i + 2];
            let token_type = data[i + 3];

            if i + 5 < data.len() {
                modifier_bitset = data[i + 4];
                i += 5;
            } else {
                i += 4;
            }

            tokens.push(SemanticToken {
                delta_line,
                delta_start,
                length,
                token_type,
                token_modifiers_bitset: modifier_bitset,
            });
        }

        tokens
    }
}
