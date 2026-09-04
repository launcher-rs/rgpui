//! LSP 诊断标记组件 —— 在编辑器中显示错误/警告/信息下划线。
//!
//! # 示例
//!
//! ```rust,ignore
//! use rgpui::prelude::*;
//! use rgpui::lsp::diagnostics_ui::{DiagnosticMarkers, DiagnosticMarkersState};
//!
//! let markers_state = cx.new(|_| DiagnosticMarkersState::default());
//! DiagnosticMarkers::new(markers_state)
//! ```

use crate::{
    Context, Entity, IntoElement, ParentElement, Pixels, Render, Styled, StyledExt, Window, div, px,
};

use super::diagnostics::{DiagnosticEntry, DiagnosticTag};

/// 诊断标记状态。
#[derive(Default)]
pub struct DiagnosticMarkersState {
    /// 当前文档的诊断列表。
    pub diagnostics: Vec<DiagnosticEntry>,
    /// 是否启用显示。
    pub enabled: bool,
}

impl DiagnosticMarkersState {
    /// 更新诊断列表。
    pub fn update(&mut self, diagnostics: Vec<DiagnosticEntry>) {
        self.diagnostics = diagnostics;
    }

    /// 清空诊断。
    pub fn clear(&mut self) {
        self.diagnostics.clear();
    }

    /// 获取指定行的诊断。
    pub fn diagnostics_for_line(&self, line: u32) -> Vec<&DiagnosticEntry> {
        self.diagnostics
            .iter()
            .filter(|d| {
                d.range.start.line <= line && d.range.end.line >= line
            })
            .collect()
    }

    /// 获取错误数量。
    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.is_error()).count()
    }

    /// 获取警告数量。
    pub fn warning_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.is_warning()).count()
    }
}

/// 诊断标记类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticMarkerStyle {
    /// 波浪下划线（错误）。
    Squiggly,
    /// 直线下划线（警告）。
    Straight,
    /// 点状下划线（信息）。
    Dotted,
    /// 删除线（不必要）。
    Strikethrough,
}

impl DiagnosticMarkerStyle {
    /// 从诊断严重程度创建。
    pub fn from_severity(severity: lsp_types::DiagnosticSeverity) -> Self {
        match severity {
            lsp_types::DiagnosticSeverity::ERROR => Self::Squiggly,
            lsp_types::DiagnosticSeverity::WARNING => Self::Straight,
            lsp_types::DiagnosticSeverity::INFORMATION => Self::Dotted,
            lsp_types::DiagnosticSeverity::HINT => Self::Dotted,
            _ => Self::Straight,
        }
    }

    /// 从诊断标签创建。
    pub fn from_tags(tags: &[DiagnosticTag]) -> Option<Self> {
        if tags.contains(&DiagnosticTag::Unnecessary) {
            Some(Self::Strikethrough)
        } else if tags.contains(&DiagnosticTag::Deprecated) {
            Some(Self::Strikethrough)
        } else {
            None
        }
    }
}

/// 诊断标记颜色。
pub struct DiagnosticColors {
    /// 错误颜色。
    pub error: crate::Hsla,
    /// 警告颜色。
    pub warning: crate::Hsla,
    /// 信息颜色。
    pub info: crate::Hsla,
    /// 提示颜色。
    pub hint: crate::Hsla,
    /// 不必要代码颜色。
    pub unnecessary: crate::Hsla,
    /// 弃用代码颜色。
    pub deprecated: crate::Hsla,
}

impl Default for DiagnosticColors {
    fn default() -> Self {
        Self {
            error: crate::red_500(),
            warning: crate::yellow_500(),
            info: crate::blue_500(),
            hint: crate::gray_500(),
            unnecessary: crate::gray_500(),
            deprecated: crate::gray_500(),
        }
    }
}

impl DiagnosticColors {
    /// 根据诊断条目获取颜色。
    pub fn color_for_diagnostic(&self, diagnostic: &DiagnosticEntry) -> crate::Hsla {
        if diagnostic.tags.contains(&DiagnosticTag::Unnecessary) {
            self.unnecessary
        } else if diagnostic.tags.contains(&DiagnosticTag::Deprecated) {
            self.deprecated
        } else {
            match diagnostic.severity {
                lsp_types::DiagnosticSeverity::ERROR => self.error,
                lsp_types::DiagnosticSeverity::WARNING => self.warning,
                lsp_types::DiagnosticSeverity::INFORMATION => self.info,
                lsp_types::DiagnosticSeverity::HINT => self.hint,
                _ => self.info,
            }
        }
    }
}

/// 诊断标记配置。
pub struct DiagnosticMarkerConfig {
    /// 标记样式。
    pub style: DiagnosticMarkerStyle,
    /// 标记颜色。
    pub colors: DiagnosticColors,
    /// 下划线偏移（像素）。
    pub underline_offset: Pixels,
    /// 下划线粗细（像素）。
    pub underline_height: Pixels,
    /// 是否显示源信息。
    pub show_source: bool,
    /// 是否显示代码。
    pub show_code: bool,
}

impl Default for DiagnosticMarkerConfig {
    fn default() -> Self {
        Self {
            style: DiagnosticMarkerStyle::Squiggly,
            colors: DiagnosticColors::default(),
            underline_offset: crate::px(2.),
            underline_height: crate::px(2.),
            show_source: true,
            show_code: true,
        }
    }
}

/// 诊断标记组件。
pub struct DiagnosticMarkers {
    state: Entity<DiagnosticMarkersState>,
    config: DiagnosticMarkerConfig,
}

impl DiagnosticMarkers {
    /// 创建新的诊断标记组件。
    pub fn new(state: Entity<DiagnosticMarkersState>) -> Self {
        Self {
            state,
            config: DiagnosticMarkerConfig::default(),
        }
    }

    /// 设置配置。
    pub fn with_config(mut self, config: DiagnosticMarkerConfig) -> Self {
        self.config = config;
        self
    }
}

impl Render for DiagnosticMarkers {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        if !state.enabled || state.diagnostics.is_empty() {
            return div().into_element();
        }

        // 诊断标记通常由编辑器元素直接绘制，这里提供状态查询接口
        div()
    }
}

/// 诊断工具提示。
pub struct DiagnosticTooltip {
    /// 诊断条目。
    pub diagnostic: DiagnosticEntry,
    /// 配置。
    pub config: DiagnosticMarkerConfig,
}

impl DiagnosticTooltip {
    /// 创建新的诊断工具提示。
    pub fn new(diagnostic: DiagnosticEntry) -> Self {
        Self {
            diagnostic,
            config: DiagnosticMarkerConfig::default(),
        }
    }

    /// 获取严重程度标签。
    pub fn severity_label(&self) -> &'static str {
        match self.diagnostic.severity {
            lsp_types::DiagnosticSeverity::ERROR => "Error",
            lsp_types::DiagnosticSeverity::WARNING => "Warning",
            lsp_types::DiagnosticSeverity::INFORMATION => "Info",
            lsp_types::DiagnosticSeverity::HINT => "Hint",
            _ => "Diagnostic",
        }
    }

    /// 获取严重程度颜色。
    pub fn severity_color(&self) -> crate::Hsla {
        self.config.colors.color_for_diagnostic(&self.diagnostic)
    }
}

impl Render for DiagnosticTooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let source_info = if self.config.show_source {
            self.diagnostic.source.as_ref().map(|s| {
                div()
                    .text_xs()
                    .text_color(crate::gray_400())
                    .child(format!("[{}]", s))
            })
        } else {
            None
        };

        let code_info = if self.config.show_code {
            self.diagnostic.code.as_ref().map(|c| {
                div()
                    .text_xs()
                    .text_color(crate::gray_400())
                    .child(format!("({})", c))
            })
        } else {
            None
        };

        div()
            .max_w(px(400.))
            .p_2()
            .bg(crate::gray_900())
            .border_1()
            .border_color(crate::gray_700())
            .rounded_md()
            .shadow_lg()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .mb_1()
                    .child(
                        div()
                            .text_xs()
                            .font_bold()
                            .text_color(self.severity_color())
                            .child(self.severity_label()),
                    )
                    .children(source_info)
                    .children(code_info),
            )
            .child(
                div()
                    .text_sm()
                    .child(self.diagnostic.message.clone()),
            )
    }
}
