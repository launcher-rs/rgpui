//! LSP 补全弹窗组件 —— 显示代码补全列表。
//!
//! # 示例
//!
//! ```rust,ignore
//! use rgpui::prelude::*;
//! use rgpui::lsp::completions::{CompletionPopup, CompletionPopupState};
//!
//! let popup_state = cx.new(|_| CompletionPopupState::default());
//! CompletionPopup::new(popup_state)
//! ```

use crate::{
    Context, Entity, InteractiveElement, IntoElement, ParentElement, Pixels, Render, Styled,
    StyledExt, Window, div, h_flex, px,
};

use super::completions::{Completion, CompletionState, CompletionMenuOptions};

/// 补全弹窗状态。
#[derive(Default)]
pub struct CompletionPopupState {
    /// 当前补全列表。
    pub completions: Vec<Completion>,
    /// 当前选中索引。
    pub selected_index: usize,
    /// 是否可见。
    pub visible: bool,
    /// 弹窗位置（像素坐标）。
    pub position: crate::Point<Pixels>,
    /// 菜单选项。
    pub options: CompletionMenuOptions,
}

impl CompletionPopupState {
    /// 从 CompletionState 更新弹窗状态。
    pub fn update_from_state(&mut self, state: &CompletionState) {
        self.completions = state.completions.clone();
        self.selected_index = state.selected_index;
        self.visible = state.visible;
        self.options = state.options;
    }

    /// 清空状态。
    pub fn clear(&mut self) {
        self.completions.clear();
        self.selected_index = 0;
        self.visible = false;
    }

    /// 选中下一个。
    pub fn select_next(&mut self) {
        if self.completions.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.completions.len();
    }

    /// 选中上一个。
    pub fn select_previous(&mut self) {
        if self.completions.is_empty() {
            return;
        }
        if self.selected_index == 0 {
            self.selected_index = self.completions.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    /// 获取当前选中的补全项。
    pub fn selected(&self) -> Option<&Completion> {
        self.completions.get(self.selected_index)
    }
}

/// 补全弹窗组件。
pub struct CompletionPopup {
    state: Entity<CompletionPopupState>,
}

impl CompletionPopup {
    /// 创建新的补全弹窗。
    pub fn new(state: Entity<CompletionPopupState>) -> Self {
        Self { state }
    }
}

impl Render for CompletionPopup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        if !state.visible || state.completions.is_empty() {
            return div().into_element();
        }

        let max_width = state.options.max_width;
        let max_items = state.options.max_visible_items.min(state.completions.len());

        let items: Vec<_> = state.completions
            .iter()
            .take(max_items)
            .enumerate()
            .map(|(i, completion)| {
                let is_selected = i == state.selected_index;

                let kind_label = completion.kind.map(|k| {
                    let name = match k {
                        lsp_types::CompletionItemKind::FUNCTION => "fn",
                        lsp_types::CompletionItemKind::METHOD => "fn",
                        lsp_types::CompletionItemKind::STRUCT => "struct",
                        lsp_types::CompletionItemKind::ENUM => "enum",
                        lsp_types::CompletionItemKind::MODULE => "mod",
                        lsp_types::CompletionItemKind::VARIABLE => "let",
                        lsp_types::CompletionItemKind::FIELD => "field",
                        lsp_types::CompletionItemKind::KEYWORD => "kw",
                        lsp_types::CompletionItemKind::VALUE => "val",
                        lsp_types::CompletionItemKind::CLASS => "class",
                        lsp_types::CompletionItemKind::INTERFACE => "trait",
                        lsp_types::CompletionItemKind::TYPE_PARAMETER => "type",
                        _ => "?",
                    };
                    div()
                        .text_xs()
                        .text_color(crate::gray_500())
                        .child(name)
                });

                let label = div()
                    .text_sm()
                    .child(completion.label.clone());

                let detail = if state.options.show_detail {
                    completion.detail.as_ref().map(|d| {
                        div()
                            .text_xs()
                            .text_color(crate::gray_400())
                            .ml_auto()
                            .child(d.clone())
                    })
                } else {
                    None
                };

                let bg = if is_selected {
                    crate::gray_800()
                } else {
                    crate::gray_900()
                };

                h_flex()
                    .w_full()
                    .px_2()
                    .py_1()
                    .bg(bg)
                    .hover(|s| s.bg(crate::gray_700()))
                    .gap_2()
                    .items_center()
                    .children(kind_label)
                    .child(label)
                    .children(detail)
            })
            .collect();

        div()
            .absolute()
            .w(max_width)
            .max_h(px(300.))
            .bg(crate::gray_900())
            .border_1()
            .border_color(crate::gray_700())
            .rounded_md()
            .shadow_lg()
            .overflow_hidden()
            .flex()
            .flex_col()
            .children(items)
    }
}

/// 补全项图标组件。
pub struct CompletionIcon {
    /// 补全类型。
    kind: lsp_types::CompletionItemKind,
}

impl CompletionIcon {
    /// 创建新的补全项图标。
    pub fn new(kind: lsp_types::CompletionItemKind) -> Self {
        Self { kind }
    }
}

impl Render for CompletionIcon {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let (symbol, color) = match self.kind {
            lsp_types::CompletionItemKind::FUNCTION | lsp_types::CompletionItemKind::METHOD => {
                ("f", crate::blue_400())
            }
            lsp_types::CompletionItemKind::STRUCT => ("S", crate::green_400()),
            lsp_types::CompletionItemKind::ENUM => ("E", crate::yellow_400()),
            lsp_types::CompletionItemKind::MODULE => ("M", crate::purple_400()),
            lsp_types::CompletionItemKind::VARIABLE | lsp_types::CompletionItemKind::FIELD => {
                ("v", crate::cyan_400())
            }
            lsp_types::CompletionItemKind::KEYWORD => ("k", crate::red_400()),
            lsp_types::CompletionItemKind::VALUE => ("V", crate::orange_400()),
            lsp_types::CompletionItemKind::CLASS | lsp_types::CompletionItemKind::INTERFACE => {
                ("T", crate::teal_400())
            }
            lsp_types::CompletionItemKind::TYPE_PARAMETER => ("T", crate::pink_400()),
            _ => ("?", crate::gray_400()),
        };

        div()
            .w_4()
            .h_4()
            .flex()
            .items_center()
            .justify_center()
            .text_xs()
            .font_bold()
            .text_color(color)
            .child(symbol)
    }
}
