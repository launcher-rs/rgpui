//! LSP 核心类型与客户端 trait。

use anyhow::Result;
use lsp_types::{
    InitializeParams, InitializeResult, ServerCapabilities, TextDocumentIdentifier,
    Uri,
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

/// 位置转换工具：字节偏移 ↔ LSP Position。
pub struct PositionMapping;

impl PositionMapping {
    /// 将字节偏移转换为 LSP Position（行号 + 列号）。
    pub fn offset_to_position(text: &Rope, offset: usize) -> lsp_types::Position {
        use ropey::LineType;
        let line = text.byte_to_line_idx(offset, LineType::LF);
        let line_start = text.line_to_byte_idx(line, LineType::LF);
        let column = offset - line_start;
        lsp_types::Position::new(line as u32, column as u32)
    }

    /// 将 LSP Position 转换为字节偏移。
    pub fn position_to_offset(text: &Rope, position: lsp_types::Position) -> usize {
        use ropey::LineType;
        let line = position.line as usize;
        let last_line = text.len_lines(LineType::LF).saturating_sub(1);
        let line = line.min(last_line);
        let line_start = text.line_to_byte_idx(line, LineType::LF);
        let column = position.character as usize;
        (line_start + column).min(text.len())
    }
}
