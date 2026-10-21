//! LSP 语义高亮支持。

use std::ops::Range;

use lsp_types::{SemanticToken, SemanticTokenModifier, SemanticTokenType, Uri};

use crate::{App, SharedString};

/// 语义高亮数据（增量 token + 图例）。
#[derive(Clone, Debug, Default)]
pub struct SemanticTokensPayload {
    /// 增量编码的 token 数据（每 5 个 `u32` 一个 token）。
    pub data: Vec<u32>,
    /// token 类型定义（下标 → 类型名）。
    pub token_types: Vec<SemanticTokenType>,
    /// token 修饰符定义。
    pub token_modifiers: Vec<SemanticTokenModifier>,
}

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

    /// 请求完整文档的语义高亮（带图例；编辑器据此解码出样式区间）。
    ///
    /// 默认不支持（返回 `None`）；需要语义高亮的应用实现此方法即可。
    fn semantic_tokens_full_with_legend(
        &self,
        _uri: &Uri,
        _window: &mut crate::Window,
        _cx: &mut App,
    ) -> crate::Task<anyhow::Result<Option<SemanticTokensPayload>>> {
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
#[derive(Clone, Debug)]
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

/// 语义高亮器：把 LSP 语义 token 转成样式区间（实现核心 [`crate::highlight::Highlighter`]）。
///
/// 供编辑器直接消费（`set_highlighter`）：token 区间按**字节列**换算（与诊断下划线同款口径）。
#[derive(Clone, Debug, Default)]
pub struct SemanticTokensHighlighter {
    /// 语言名。
    name: SharedString,
    /// 样式区间（字节区间 + 主题 token 名）。
    runs: Vec<(Range<usize>, String)>,
}

impl SemanticTokensHighlighter {
    /// 创建高亮器（`runs` 为解码后的样式区间）。
    pub fn new(name: impl Into<SharedString>, runs: Vec<(Range<usize>, String)>) -> Self {
        Self {
            name: name.into(),
            runs,
        }
    }

    /// 把语义高亮载荷解码为「字节区间 + 主题 token 名」。
    ///
    /// `data` 为增量编码（每 5 个 `u32`）：行增量、列增量、长度、类型下标、修饰符位集。
    /// 列按**字节列**解释（与仓库内 LSP 换算口径一致）。
    pub fn decode_runs(
        text: &str,
        payload: &SemanticTokensPayload,
        type_map: &SemanticTokenTypeMap,
    ) -> Vec<(Range<usize>, String)> {
        // 每行的字节起点（行首）。
        let mut line_starts: Vec<usize> = vec![0];
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(index + 1);
            }
        }
        let tokens = SemanticTokensState::decode_tokens(
            &payload.token_types,
            &payload.token_modifiers,
            &payload.data,
        );
        let mut runs = Vec::with_capacity(tokens.len());
        let mut line = 0u32;
        let mut char_start = 0u32;
        for token in tokens {
            line += token.delta_line;
            char_start = if token.delta_line == 0 {
                char_start + token.delta_start
            } else {
                token.delta_start
            };
            let Some(base) = line_starts.get(line as usize) else {
                continue;
            };
            let start = base + char_start as usize;
            let end = start + token.length as usize;
            // 类型名 → 主题 token 名（未映射的类型不高亮）。
            let Some(type_name) = payload
                .token_types
                .get(token.token_type as usize)
                .map(|token_type| token_type.as_str())
            else {
                continue;
            };
            let Some(mapped) = type_map.get(type_name) else {
                continue;
            };
            runs.push((start..end, mapped.to_string()));
        }
        runs
    }
}

impl crate::highlight::Highlighter for SemanticTokensHighlighter {
    fn language(&self) -> SharedString {
        self.name.clone()
    }

    fn update(
        &mut self,
        _edit: Option<crate::highlight::TextEdit>,
        _text: &ropey::Rope,
        _folding: bool,
        _window: &mut crate::Window,
        _cx: &mut App,
    ) {
        // 语义 token 由外部请求后整体替换（编辑器重建高亮器），此处无需增量更新。
    }

    fn styles(
        &self,
        range: &Range<usize>,
        resolver: &dyn crate::highlight::HighlightStyleResolver,
    ) -> Vec<(Range<usize>, crate::HighlightStyle)> {
        let mut out = Vec::new();
        let mut cursor = range.start;
        for (run_range, name) in &self.runs {
            if run_range.end <= range.start {
                continue;
            }
            if run_range.start >= range.end {
                break;
            }
            let start = run_range.start.max(range.start);
            let end = run_range.end.min(range.end);
            if cursor < start {
                out.push((cursor..start, crate::HighlightStyle::default()));
            }
            let style = resolver.style(name).unwrap_or_default();
            out.push((start..end, style));
            cursor = end;
        }
        if cursor < range.end {
            out.push((cursor..range.end, crate::HighlightStyle::default()));
        }
        out
    }

    fn fold_ranges(&self, _text: &ropey::Rope) -> Vec<crate::highlight::FoldRange> {
        Vec::new()
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
#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{SemanticTokenTypeMap, SemanticTokensHighlighter, SemanticTokensPayload};
    use crate::highlight::{HighlightStyleResolver, Highlighter};

    /// 只解析命名的解析器（按名字返回固定样式）。
    struct NamedResolver;

    impl HighlightStyleResolver for NamedResolver {
        fn style(&self, name: &str) -> Option<crate::HighlightStyle> {
            if name == "function" {
                Some(crate::HighlightStyle {
                    color: Some(crate::hsla(0.0, 1.0, 0.5, 1.0)),
                    ..Default::default()
                })
            } else {
                None
            }
        }
    }

    /// 语义 token 解码：增量数据 → 字节区间 + 主题 token 名。
    #[test]
    fn decodes_tokens_to_ranges() {
        let payload = SemanticTokensPayload {
            // 第 0 行、第 0 列、长度 2、类型 0（function）、无修饰符。
            data: vec![0, 0, 2, 0, 0],
            token_types: vec![lsp_types::SemanticTokenType::new("function")],
            token_modifiers: Vec::new(),
        };
        let runs = SemanticTokensHighlighter::decode_runs(
            "fn main() {}\n",
            &payload,
            &SemanticTokenTypeMap::default(),
        );
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].0, 0..2);
        assert_eq!(runs[0].1, "function");
    }

    /// 高亮器样式：命中区间返回解析出的样式，区间外为默认样式。
    #[test]
    fn highlighter_resolves_styles() {
        let highlighter = SemanticTokensHighlighter::new(
            "rust",
            vec![
                (0..2, "function".to_string()),
                (6..10, "variable".to_string()),
            ],
        );
        let styles = highlighter.styles(&(0..12), &NamedResolver);
        // 覆盖整段且不重叠。
        assert_eq!(styles.first().map(|s| s.0.start), Some(0));
        assert_eq!(styles.last().map(|s| s.0.end), Some(12));
        for pair in styles.windows(2) {
            assert!(pair[0].0.end <= pair[1].0.start);
        }
        // 命中区间的样式来自解析器。
        let hit = styles.iter().find(|s| s.0 == (0..2)).expect("命中区间");
        assert!(hit.1.color.is_some());
    }
}
