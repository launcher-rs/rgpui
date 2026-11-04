//! LSP 核心类型与客户端 trait。

use anyhow::Result;
use lsp_types::{
    InitializeParams, InitializeResult, ServerCapabilities, TextDocumentIdentifier, Uri,
};
use ropey::Rope;

use crate::App;

/// LSP 客户端抽象 trait。
///
/// 实现此 trait 即可接入编辑器的 LSP 功能（补全、悬停、定义跳转等）。
/// 应用层通常提供基于 `lsp-server` 或 `tower-lsp` 的具体实现。
///
/// # 示例
///
/// ```rust,ignore
/// struct MyLspClient {
///     server_capabilities: ServerCapabilities,
/// }
///
/// impl LspClient for MyLspClient {
///     fn server_capabilities(&self) -> &ServerCapabilities {
///         &self.server_capabilities
///     }
///
///     fn initialize(&self, _params: InitializeParams, _cx: &mut App) -> Task<Result<InitializeResult>> {
///         // 连接到语言服务器
///     }
///
///     fn completions(&self, _params: lsp_types::CompletionParams, _cx: &mut App) -> Task<Result<lsp_types::CompletionResponse>> {
///         // 请求补全
///     }
/// }
/// ```
pub trait LspClient {
    /// 获取服务器能力描述。
    fn server_capabilities(&self) -> &ServerCapabilities;

    /// 初始化 LSP 连接。
    fn initialize(
        &self,
        params: InitializeParams,
        cx: &mut App,
    ) -> crate::Task<Result<InitializeResult>>;

    /// 关闭 LSP 连接。
    fn shutdown(&self, cx: &mut App) -> crate::Task<Result<()>>;

    /// 发送 `textDocument/completion` 请求。
    fn completions(
        &self,
        params: lsp_types::CompletionParams,
        cx: &mut App,
    ) -> crate::Task<Result<lsp_types::CompletionResponse>>;

    /// 发送 `textDocument/hover` 请求。
    fn hover(
        &self,
        params: lsp_types::HoverParams,
        cx: &mut App,
    ) -> crate::Task<Result<Option<lsp_types::Hover>>>;

    /// 发送 `textDocument/definition` 请求。
    fn definition(
        &self,
        params: lsp_types::GotoDefinitionParams,
        cx: &mut App,
    ) -> crate::Task<Result<lsp_types::GotoDefinitionResponse>>;

    /// 发送 `textDocument/references` 请求。
    fn references(
        &self,
        params: lsp_types::ReferenceParams,
        cx: &mut App,
    ) -> crate::Task<Result<Vec<lsp_types::Location>>>;

    /// 发送 `textDocument/publishDiagnostics` 推送或 `textDocument/diagnostic` 请求。
    fn diagnostics(
        &self,
        params: lsp_types::DocumentDiagnosticParams,
        cx: &mut App,
    ) -> crate::Task<Result<lsp_types::DocumentDiagnosticReport>>;

    /// 发送 `textDocument/semanticTokens/full` 或 `range` 请求。
    fn semantic_tokens_full(
        &self,
        params: lsp_types::SemanticTokensParams,
        cx: &mut App,
    ) -> crate::Task<Result<Option<lsp_types::SemanticTokensResult>>>;

    /// 发送 `textDocument/semanticTokens/range` 请求。
    fn semantic_tokens_range(
        &self,
        params: lsp_types::SemanticTokensRangeParams,
        cx: &mut App,
    ) -> crate::Task<Result<Option<lsp_types::SemanticTokensRangeResult>>>;

    /// 发送 `textDocument/codeAction` 请求。
    fn code_actions(
        &self,
        params: lsp_types::CodeActionParams,
        cx: &mut App,
    ) -> crate::Task<Result<Vec<lsp_types::CodeActionOrCommand>>>;

    /// 发送 `textDocument/formatting` 请求。
    fn formatting(
        &self,
        params: lsp_types::DocumentFormattingParams,
        cx: &mut App,
    ) -> crate::Task<Result<Vec<lsp_types::TextEdit>>>;

    /// 发送 `textDocument/rename` 请求。
    fn rename(
        &self,
        params: lsp_types::RenameParams,
        cx: &mut App,
    ) -> crate::Task<Result<Option<lsp_types::WorkspaceEdit>>>;

    /// 发送 `textDocument/references` 请求（带高亮上下文）。
    fn document_highlights(
        &self,
        params: lsp_types::DocumentHighlightParams,
        cx: &mut App,
    ) -> crate::Task<Result<Vec<lsp_types::DocumentHighlight>>>;

    /// 发送 `textDocument/signatureHelp` 请求。
    fn signature_help(
        &self,
        params: lsp_types::SignatureHelpParams,
        cx: &mut App,
    ) -> crate::Task<Result<Option<lsp_types::SignatureHelp>>>;

    /// 通知服务器文档内容已变更。
    fn did_change(
        &self,
        identifier: TextDocumentIdentifier,
        changes: Vec<lsp_types::TextDocumentContentChangeEvent>,
        cx: &mut App,
    );

    /// 通知服务器文档已打开。
    fn did_open(
        &self,
        identifier: TextDocumentIdentifier,
        language_id: &str,
        text: &str,
        cx: &mut App,
    );

    /// 通知服务器文档已关闭。
    fn did_close(&self, identifier: TextDocumentIdentifier, cx: &mut App);

    /// 通知服务器文档已保存。
    fn did_save(&self, identifier: TextDocumentIdentifier, text: Option<String>, cx: &mut App);
}

/// LSP 会话状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspState {
    /// 未连接。
    Disconnected,
    /// 正在初始化。
    Initializing,
    /// 已连接并就绪。
    Connected,
    /// 正在关闭。
    ShuttingDown,
    /// 连接出错。
    Error,
}

/// 文档 URI 与版本号的组合。
#[derive(Debug, Clone)]
pub struct DocumentVersion {
    /// 文档 URI。
    pub uri: Uri,
    /// 文档版本号（每次变更递增）。
    pub version: i32,
}

impl DocumentVersion {
    /// 创建新的文档版本。
    pub fn new(uri: Uri) -> Self {
        Self { uri, version: 0 }
    }

    /// 递增版本号并返回新版本。
    pub fn increment(&mut self) -> i32 {
        self.version += 1;
        self.version
    }
}

/// 位置转换工具：字节偏移 ↔ LSP Position（按协商的 `positionEncoding` 换算）。
///
/// LSP 的 `Position.character` **不是字节偏移**：单位由服务器在 `initialize`
/// 结果里用 [`lsp_types::PositionEncodingKind`] 声明，缺省是 UTF-16 码元。
/// 用字节列直接当 `character` 发出去，任何一行含中文（3 字节）或 emoji
/// （UTF-16 代理对）都会错位；反过来把服务器给的单位列当字节用，诊断区间
/// 和下划线就会落在错误位置。
///
/// 仓库里另有一套**码点**口径（[`crate::RopeExt::position_to_offset`] /
/// [`crate::RopeExt::offset_to_position`]），服务于 `InputState::cursor_position`
/// 这类人读坐标，与线上报文无关；两套不要混用。
pub struct PositionMapping;

impl PositionMapping {
    /// 将字节偏移转换为 LSP Position（UTF-16 口径，协议缺省）。
    pub fn offset_to_position(text: &Rope, offset: usize) -> lsp_types::Position {
        Self::offset_to_position_with(text, offset, lsp_types::PositionEncodingKind::UTF16)
    }

    /// 将 LSP Position 转换为字节偏移（UTF-16 口径，协议缺省）。
    pub fn position_to_offset(text: &Rope, position: lsp_types::Position) -> usize {
        Self::position_to_offset_with(text, position, lsp_types::PositionEncodingKind::UTF16)
    }

    /// 按 `encoding` 把字节偏移转换为 LSP Position。
    pub fn offset_to_position_with(
        text: &Rope,
        offset: usize,
        encoding: lsp_types::PositionEncodingKind,
    ) -> lsp_types::Position {
        use ropey::LineType;
        let offset = offset.min(text.len());
        let line_ix = text.byte_to_line_idx(offset, LineType::LF);
        let line_start = text.line_to_byte_idx(line_ix, LineType::LF);
        let line = text.line(line_ix, LineType::LF);
        let byte_col = line.floor_char_boundary((offset - line_start).min(line.len()));
        let character = byte_col_to_unit_col(line, byte_col, &encoding);
        lsp_types::Position::new(line_ix as u32, character as u32)
    }

    /// 按 `encoding` 把 LSP Position 转换为字节偏移。
    ///
    /// 越界的行号收敛到最后一行；超出行长的列收敛到行尾（含换行符之前），
    /// 与 LSP 规范「character 大于行长时回退到行长」一致。
    pub fn position_to_offset_with(
        text: &Rope,
        position: lsp_types::Position,
        encoding: lsp_types::PositionEncodingKind,
    ) -> usize {
        use ropey::LineType;
        let last_line = text.len_lines(LineType::LF).saturating_sub(1);
        let line_ix = (position.line as usize).min(last_line);
        let line_start = text.line_to_byte_idx(line_ix, LineType::LF);
        let line = text.line(line_ix, LineType::LF);
        let content_len = line_content_len(line);
        let byte_col =
            unit_col_to_byte_col(line, position.character as usize, &encoding, content_len);
        (line_start + byte_col).min(text.len())
    }
}

/// 行内容的字节长度（不含行尾的 `\n` / `\r\n`）。
fn line_content_len(line: ropey::RopeSlice<'_>) -> usize {
    let len = line.len();
    if len > 0 && line.byte(len - 1) == b'\n' {
        let len = len - 1;
        if len > 0 && line.byte(len - 1) == b'\r' {
            return len - 1;
        }
        return len;
    }
    len
}

/// 行内字节列 → `encoding` 单位列（`byte_col` 必须落在字符边界上）。
fn byte_col_to_unit_col(
    line: ropey::RopeSlice<'_>,
    byte_col: usize,
    encoding: &lsp_types::PositionEncodingKind,
) -> usize {
    if *encoding == lsp_types::PositionEncodingKind::UTF8 {
        byte_col
    } else if *encoding == lsp_types::PositionEncodingKind::UTF32 {
        line.slice(..byte_col).chars().count()
    } else {
        line.byte_to_utf16_idx(byte_col)
    }
}

/// 行内 `encoding` 单位列 → 字节列（超出 `content_len` 截断，代理对中间向下取整）。
fn unit_col_to_byte_col(
    line: ropey::RopeSlice<'_>,
    unit_col: usize,
    encoding: &lsp_types::PositionEncodingKind,
    content_len: usize,
) -> usize {
    let content = line.slice(..content_len);
    if *encoding == lsp_types::PositionEncodingKind::UTF8 {
        // 服务端声称 UTF-8 却给了非字符边界时向下取整，避免切出半个字符。
        content.floor_char_boundary(unit_col.min(content_len))
    } else if *encoding == lsp_types::PositionEncodingKind::UTF32 {
        let mut byte_col = 0;
        for (taken, ch) in content.chars().enumerate() {
            if taken == unit_col {
                break;
            }
            byte_col += ch.len_utf8();
        }
        byte_col
    } else {
        content.utf16_to_byte_idx(unit_col.min(content.len_utf16()))
    }
}

#[cfg(test)]
mod tests {
    use super::PositionMapping;
    use lsp_types::{Position, PositionEncodingKind};
    use ropey::Rope;

    /// 含中文（3 字节 / 1 个 UTF-16 码元）与 emoji（4 字节 / 2 个码元）的一行。
    fn sample() -> Rope {
        Rope::from("let s = \"中文🎉\";\nB\n")
    }

    /// UTF-16 口径（协议缺省）：列按码元计，emoji 占 2 列。
    #[test]
    fn utf16_columns_count_code_units() {
        let text = sample();
        assert_eq!(
            PositionMapping::offset_to_position(&text, 0),
            Position::new(0, 0)
        );
        assert_eq!(
            PositionMapping::offset_to_position(&text, 9),
            Position::new(0, 9)
        );
        assert_eq!(
            PositionMapping::offset_to_position(&text, 12),
            Position::new(0, 10)
        );
        assert_eq!(
            PositionMapping::offset_to_position(&text, 15),
            Position::new(0, 11)
        );
        // 🎉 = 4 字节 / 2 码元：跳过后列号 +2 而不是 +4。
        assert_eq!(
            PositionMapping::offset_to_position(&text, 19),
            Position::new(0, 13)
        );
        assert_eq!(
            PositionMapping::offset_to_position(&text, 20),
            Position::new(0, 14)
        );
        assert_eq!(
            PositionMapping::offset_to_position(&text, 21),
            Position::new(0, 15)
        );
        assert_eq!(
            PositionMapping::offset_to_position(&text, 22),
            Position::new(1, 0)
        );
    }

    /// 原地往返：前端给的位置换算回字节偏移后必须落回原字符边界。
    #[test]
    fn utf16_round_trips_at_char_boundaries() {
        let text = sample();
        for offset in [0, 1, 9, 12, 15, 19, 20, 21, 22, 23, 24] {
            let position = PositionMapping::offset_to_position(&text, offset);
            assert_eq!(
                PositionMapping::position_to_offset(&text, position),
                offset,
                "offset {offset} 往返后不一致（position {position:?}）"
            );
        }
    }

    /// UTF-8 口径：列就是字节数。
    #[test]
    fn utf8_columns_are_bytes() {
        let text = sample();
        let utf8 = PositionEncodingKind::UTF8;
        assert_eq!(
            PositionMapping::offset_to_position_with(&text, 19, utf8.clone()),
            Position::new(0, 19)
        );
        assert_eq!(
            PositionMapping::position_to_offset_with(&text, Position::new(0, 19), utf8),
            19
        );
    }

    /// UTF-32 口径：列按码点计，emoji 只占 1 列。
    #[test]
    fn utf32_columns_count_code_points() {
        let text = sample();
        let utf32 = PositionEncodingKind::UTF32;
        assert_eq!(
            PositionMapping::offset_to_position_with(&text, 9, utf32.clone()),
            Position::new(0, 9)
        );
        // `"中文` 之后是 11 个码点。
        assert_eq!(
            PositionMapping::offset_to_position_with(&text, 15, utf32.clone()),
            Position::new(0, 11)
        );
        assert_eq!(
            PositionMapping::offset_to_position_with(&text, 19, utf32.clone()),
            Position::new(0, 12)
        );
        for offset in [0, 9, 12, 15, 19, 20, 21, 22, 23, 24] {
            let position = PositionMapping::offset_to_position_with(&text, offset, utf32.clone());
            assert_eq!(
                PositionMapping::position_to_offset_with(&text, position, utf32.clone()),
                offset
            );
        }
    }

    /// 越界收敛：行号超界取最后一行，列超界取行尾（换行符之前）。
    #[test]
    fn out_of_range_positions_clamp_to_line_end() {
        let text = sample();
        assert_eq!(
            PositionMapping::position_to_offset(&text, Position::new(99, 0)),
            24,
            "超界行号应落到最后一行（末尾换行之后的空行）行首"
        );
        assert_eq!(
            PositionMapping::position_to_offset(&text, Position::new(0, 999)),
            21,
            "超界列应停在换行符之前"
        );
        // 代理对中间的单位列向下取整到字符起点。
        assert_eq!(
            PositionMapping::position_to_offset(&text, Position::new(0, 12)),
            15
        );
    }
}
