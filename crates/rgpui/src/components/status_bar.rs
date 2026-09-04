//! 状态栏组件 —— 显示编辑器状态信息（行列号、语言、编码、LSP 状态等）。
//!
//! # 示例
//!
//! ```rust,ignore
//! use rgpui::prelude::*;
//! use rgpui::components::status_bar::{StatusBar, StatusBarState};
//!
//! let status = cx.new(|_| StatusBarState {
//!     line: 1,
//!     column: 1,
//!     language: "Rust".into(),
//!     encoding: "UTF-8".into(),
//!     ..Default::default()
//! });
//!
//! StatusBar::new(status)
//! ```

use std::rc::Rc;

use crate::{
    App, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Window,
    div, h_flex, px,
};

/// 状态栏状态。
#[derive(Clone)]
pub struct StatusBarState {
    /// 当前行号（1-based）。
    pub line: usize,
    /// 当前列号（1-based）。
    pub column: usize,
    /// 选区中的字符数。
    pub selection_chars: Option<usize>,
    /// 当前文件语言。
    pub language: SharedString,
    /// 文件编码。
    pub encoding: SharedString,
    /// 行尾序列（LF/CRLF）。
    pub line_ending: SharedString,
    /// LSP 连接状态。
    pub lsp_status: LspStatus,
    /// LSP 服务器名称。
    pub lsp_server_name: Option<SharedString>,
    /// 错误数量。
    pub error_count: usize,
    /// 警告数量。
    pub warning_count: usize,
    /// 信息数量。
    pub info_count: usize,
    /// 缩进信息（如 "Spaces: 4"）。
    pub indent_info: Option<SharedString>,
    /// Git 分支名称。
    pub git_branch: Option<SharedString>,
    /// 自定义状态项。
    pub custom_items: Vec<StatusBarItem>,
}

impl Default for StatusBarState {
    fn default() -> Self {
        Self {
            line: 1,
            column: 1,
            selection_chars: None,
            language: "Plain Text".into(),
            encoding: "UTF-8".into(),
            line_ending: "LF".into(),
            lsp_status: LspStatus::Disconnected,
            lsp_server_name: None,
            error_count: 0,
            warning_count: 0,
            info_count: 0,
            indent_info: None,
            git_branch: None,
            custom_items: Vec::new(),
        }
    }
}

/// LSP 连接状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspStatus {
    /// 已连接。
    Connected,
    /// 正在初始化。
    Initializing,
    /// 已断开。
    Disconnected,
    /// 出错。
    Error,
}

/// 状态栏自定义项。
#[derive(Clone)]
pub struct StatusBarItem {
    /// 显示文本。
    pub label: SharedString,
    /// 工具提示。
    pub tooltip: Option<SharedString>,
    /// 点击回调。
    pub on_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
}

/// 状态栏组件。
pub struct StatusBar {
    state: Entity<StatusBarState>,
}

impl StatusBar {
    /// 创建新的状态栏。
    pub fn new(state: Entity<StatusBarState>) -> Self {
        Self { state }
    }
}

impl Render for StatusBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        let lsp_indicator = match state.lsp_status {
            LspStatus::Connected => {
                let name = state
                    .lsp_server_name
                    .clone()
                    .unwrap_or_else(|| "LSP".into());
                h_flex()
                    .gap_1()
                    .child(
                        div()
                            .w_2()
                            .h_2()
                            .rounded_full()
                            .bg(crate::green_500()),
                    )
                    .child(
                        div().text_xs().child(name),
                    )
            }
            LspStatus::Initializing => h_flex()
                .gap_1()
                .child(
                    div()
                        .w_2()
                        .h_2()
                        .rounded_full()
                        .bg(crate::yellow_500()),
                )
                .child("LSP..."),
            LspStatus::Disconnected => h_flex()
                .gap_1()
                .child(
                    div()
                        .w_2()
                        .h_2()
                        .rounded_full()
                        .bg(crate::gray_500()),
                )
                .child("No LSP"),
            LspStatus::Error => h_flex()
                .gap_1()
                .child(
                    div()
                        .w_2()
                        .h_2()
                        .rounded_full()
                        .bg(crate::red_500()),
                )
                .child("LSP Error"),
        };

        let diagnostics = if state.error_count > 0 || state.warning_count > 0 {
            let mut items = Vec::new();
            if state.error_count > 0 {
                items.push(
                    div()
                        .text_xs()
                        .child(format!("{} errors", state.error_count)),
                );
            }
            if state.warning_count > 0 {
                items.push(
                    div()
                        .text_xs()
                        .child(format!("{} warnings", state.warning_count)),
                );
            }
            h_flex().gap_2().children(items)
        } else {
            div()
        };

        let position_info = h_flex()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .child(format!("Ln {}, Col {}", state.line, state.column)),
            )
            .children(state.selection_chars.map(|chars| {
                div()
                    .text_xs()
                    .child(format!("({} selected)", chars))
            }));

        let language_info = h_flex()
            .gap_2()
            .child(div().text_xs().child(state.language.clone()))
            .child(div().text_xs().child(state.encoding.clone()))
            .child(div().text_xs().child(state.line_ending.clone()))
            .children(state.indent_info.as_ref().map(|info| {
                div().text_xs().child(info.clone())
            }));

        let git_info = state.git_branch.as_ref().map(|branch| {
            h_flex()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .child(format!(" {}", branch)),
                )
        });

        h_flex()
            .w_full()
            .h(px(28.))
            .items_center()
            .justify_between()
            .px_2()
            .bg(crate::gray_900())
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(lsp_indicator)
                    .child(diagnostics),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .children(git_info)
                    .child(position_info)
                    .child(language_info),
            )
    }
}
