//! LSP 诊断支持。

use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, PublishDiagnosticsParams, Uri};

use crate::App;

/// 诊断提供者 trait。
pub trait DiagnosticsProvider {
    /// 请求文档诊断。
    fn diagnostics(
        &self,
        uri: &Uri,
        window: &mut crate::Window,
        _cx: &mut App,
    ) -> crate::Task<anyhow::Result<Vec<DiagnosticEntry>>> {
        let _ = (uri, window);
        crate::Task::ready(Ok(vec![]))
    }

    /// 订阅诊断推送。
    fn on_diagnostics(&self, callback: Box<dyn Fn(PublishDiagnosticsParams)>, cx: &mut App) {
        let _ = (callback, cx);
    }
}

/// 诊断条目（简化版）。
#[derive(Debug, Clone)]
pub struct DiagnosticEntry {
    /// 诊断范围。
    pub range: lsp_types::Range,
    /// 诊断严重程度。
    pub severity: DiagnosticSeverity,
    /// 诊断来源（如 "rustc"、"eslint"）。
    pub source: Option<String>,
    /// 诊断消息。
    pub message: String,
    /// 相关信息链接。
    pub related_information: Vec<RelatedInformation>,
    /// 诊断代码。
    pub code: Option<String>,
    /// 诊断标签（如 "unnecessary"、"deprecated"）。
    pub tags: Vec<DiagnosticTag>,
}

/// 诊断标签。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticTag {
    /// 不必要的代码。
    Unnecessary,
    /// 已弃用的代码。
    Deprecated,
}

/// 相关信息。
#[derive(Debug, Clone)]
pub struct RelatedInformation {
    /// 相关位置。
    pub location: lsp_types::Location,
    /// 相关消息。
    pub message: String,
}

impl DiagnosticEntry {
    /// 从 LSP Diagnostic 构建。
    pub fn from_diagnostic(diagnostic: Diagnostic) -> Self {
        let severity = diagnostic.severity.unwrap_or(DiagnosticSeverity::WARNING);
        let code = diagnostic.code.as_ref().map(|c| match c {
            NumberOrString::Number(n) => n.to_string(),
            NumberOrString::String(s) => s.clone(),
        });

        let tags = diagnostic
            .tags
            .unwrap_or_default()
            .into_iter()
            .filter_map(|t| match t {
                lsp_types::DiagnosticTag::UNNECESSARY => Some(DiagnosticTag::Unnecessary),
                lsp_types::DiagnosticTag::DEPRECATED => Some(DiagnosticTag::Deprecated),
                _ => None,
            })
            .collect();

        let related_information = diagnostic
            .related_information
            .unwrap_or_default()
            .into_iter()
            .map(|info| RelatedInformation {
                location: info.location,
                message: info.message,
            })
            .collect();

        Self {
            range: diagnostic.range,
            severity,
            source: diagnostic.source,
            message: diagnostic.message,
            related_information,
            code,
            tags,
        }
    }

    /// 判断是否为错误。
    pub fn is_error(&self) -> bool {
        self.severity == DiagnosticSeverity::ERROR
    }

    /// 判断是否为警告。
    pub fn is_warning(&self) -> bool {
        self.severity == DiagnosticSeverity::WARNING
    }

    /// 判断是否为信息。
    pub fn is_info(&self) -> bool {
        self.severity == DiagnosticSeverity::INFORMATION
    }

    /// 判断是否为提示。
    pub fn is_hint(&self) -> bool {
        self.severity == DiagnosticSeverity::HINT
    }
}

/// 诊断状态（按文档管理）。
#[derive(Default)]
pub struct DiagnosticsState {
    /// 文档 URI → 诊断列表。
    pub diagnostics: std::collections::HashMap<Uri, Vec<DiagnosticEntry>>,
}

impl DiagnosticsState {
    /// 更新指定文档的诊断。
    pub fn update(&mut self, params: PublishDiagnosticsParams) {
        let diagnostics: Vec<DiagnosticEntry> = params
            .diagnostics
            .into_iter()
            .map(DiagnosticEntry::from_diagnostic)
            .collect();
        self.diagnostics.insert(params.uri, diagnostics);
    }

    /// 获取指定文档的诊断。
    pub fn get(&self, uri: &Uri) -> &[DiagnosticEntry] {
        self.diagnostics.get(uri).map_or(&[], |v| v.as_slice())
    }

    /// 清空指定文档的诊断。
    pub fn clear(&mut self, uri: &Uri) {
        self.diagnostics.remove(uri);
    }

    /// 清空所有诊断。
    pub fn clear_all(&mut self) {
        self.diagnostics.clear();
    }

    /// 获取指定文档的错误数量。
    pub fn error_count(&self, uri: &Uri) -> usize {
        self.get(uri).iter().filter(|d| d.is_error()).count()
    }

    /// 获取指定文档的警告数量。
    pub fn warning_count(&self, uri: &Uri) -> usize {
        self.get(uri).iter().filter(|d| d.is_warning()).count()
    }
}
