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

use std::rc::Rc;

use crate::{
    ActiveTheme as _, Anchor, App, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Pixels, Render, RenderOnce, StatefulInteractiveElement, Styled, StyledExt, Window, anchored,
    deferred, div, h_flex, prelude::FluentBuilder as _, px,
};

use super::completions::{Completion, CompletionMenuOptions, CompletionState};

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
#[derive(IntoElement)]
pub struct CompletionPopup {
    state: Entity<CompletionPopupState>,
    /// 行点击回调（参数为条目索引；应用层回写 `accept_completion`）。
    on_select: Option<Rc<dyn Fn(usize, &mut Window, &mut App)>>,
}

impl CompletionPopup {
    /// 创建新的补全弹窗。
    pub fn new(state: Entity<CompletionPopupState>) -> Self {
        Self {
            state,
            on_select: None,
        }
    }

    /// 设置行点击回调（不设置则行不可点，仅展示）。
    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for CompletionPopup {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let on_select = self.on_select.clone();

        if !state.visible || state.completions.is_empty() {
            return div().into_any_element();
        }

        let max_width = state.options.max_width;
        let max_items = state.options.max_visible_items.min(state.completions.len());
        let anchor_position = state.position;

        let items: Vec<_> = state
            .completions
            .iter()
            .take(max_items)
            .enumerate()
            .map(|(i, completion)| {
                let is_selected = i == state.selected_index;
                let on_select = on_select.clone();

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
                        .text_color(cx.theme().muted_foreground)
                        .child(name)
                });

                let label = div()
                    .text_sm()
                    .text_color(cx.theme().popover_foreground)
                    .child(completion.label.clone());

                let detail = if state.options.show_detail {
                    completion.detail.as_ref().map(|d| {
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .ml_auto()
                            .child(d.clone())
                    })
                } else {
                    None
                };

                let row = h_flex()
                    .id(("completion-item", i))
                    .w_full()
                    .px_2()
                    .py_1()
                    .rounded(cx.theme().radius.min(px(6.)))
                    .gap_2()
                    .items_center()
                    .text_color(cx.theme().popover_foreground)
                    .when(is_selected, |this| {
                        this.bg(cx.theme().tokens.accent)
                            .text_color(cx.theme().accent_foreground)
                    })
                    .hover(|s| s.bg(cx.theme().tokens.accent.opacity(0.5)))
                    .children(kind_label)
                    .child(label)
                    .children(detail);
                // 有回调才挂点击（纯展示时行不可点）。
                if let Some(on_select) = on_select {
                    row.cursor_pointer().on_click(move |_, window, cx| {
                        on_select(i, window, cx);
                    })
                } else {
                    row
                }
            })
            .collect();

        // 窗口坐标锚定到光标处（`EditorState` 在同步弹窗时写入 `position`），
        // `deferred` 保证浮于编辑器之上，`snap_to_window` 防溢出。
        deferred(
            anchored()
                .position(anchor_position)
                .anchor(Anchor::TopLeft)
                .snap_to_window_with_margin(px(8.))
                .child(
                    div()
                        .w(max_width)
                        .max_h(px(300.))
                        .popover_style(cx)
                        .p_1()
                        .flex()
                        .flex_col()
                        .gap_y_0p5()
                        .occlude()
                        .children(items),
                ),
        )
        .with_priority(1)
        .into_any_element()
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
