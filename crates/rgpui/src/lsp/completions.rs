//! LSP 补全支持。

use std::{cell::RefCell, rc::Rc};

use lsp_types::{CompletionContext, CompletionItem, CompletionResponse};
use ropey::Rope;

use crate::App;

/// 补全菜单显示选项。
#[derive(Debug, Clone, Copy)]
pub struct CompletionMenuOptions {
    /// 弹窗最大宽度。
    pub max_width: crate::Pixels,
    /// 最大显示条目数。
    pub max_visible_items: usize,
    /// 是否显示文档预览。
    pub show_documentation: bool,
    /// 是否显示详细信息。
    pub show_detail: bool,
}

impl Default for CompletionMenuOptions {
    fn default() -> Self {
        Self {
            max_width: crate::px(360.),
            max_visible_items: 15,
            show_documentation: true,
            show_detail: true,
        }
    }
}

/// 补全提供者 trait。
///
/// 实现此 trait 即可为编辑器提供代码补全能力。
/// 应用层通常通过 LSP 客户端实现此 trait。
pub trait CompletionProvider {
    /// 请求补全列表。
    ///
    /// # 参数
    /// * `text` - 当前文档内容
    /// * `offset` - 光标字节偏移
    /// * `trigger` - 补全触发上下文
    /// * `window` - 窗口引用
    /// * `cx` - 应用上下文
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        trigger: CompletionContext,
        window: &mut crate::Window,
        cx: &mut App,
    ) -> crate::Task<anyhow::Result<CompletionResponse>>;

    /// 解析补全项的详细信息（懒加载）。
    fn resolve_completions(
        &self,
        indices: Vec<usize>,
        completions: Rc<RefCell<Box<[Completion]>>>,
        _cx: &mut App,
    ) -> crate::Task<anyhow::Result<bool>> {
        let _ = (indices, completions);
        crate::Task::ready(Ok(false))
    }

    /// 补全触发字符列表。
    fn trigger_characters(&self) -> &[String] {
        &[]
    }

    /// 补全菜单选项。
    fn menu_options(&self) -> CompletionMenuOptions {
        CompletionMenuOptions::default()
    }
}

/// 补全条目（简化版）。
#[derive(Debug, Clone)]
pub struct Completion {
    /// 显示标签。
    pub label: String,
    /// 插入文本。
    pub insert_text: String,
    /// 补全项详情。
    pub detail: Option<String>,
    /// 文档内容。
    pub documentation: Option<String>,
    /// 补全类型（函数、变量等）。
    pub kind: Option<lsp_types::CompletionItemKind>,
    /// 排序分数（越小越靠前）。
    pub score: f32,
    /// 原始 LSP 补全项。
    pub lsp_item: CompletionItem,
}

/// 补全状态。
#[derive(Default)]
pub struct CompletionState {
    /// 当前补全列表。
    pub completions: Vec<Completion>,
    /// 当前选中索引。
    pub selected_index: usize,
    /// 补全文档。
    pub prefix: String,
    /// 是否正在显示。
    pub visible: bool,
    /// 补全菜单选项。
    pub options: CompletionMenuOptions,
}

impl CompletionState {
    /// 清空补全状态。
    pub fn clear(&mut self) {
        self.completions.clear();
        self.selected_index = 0;
        self.prefix.clear();
        self.visible = false;
    }

    /// 选中下一个补全项。
    pub fn select_next(&mut self) {
        if self.completions.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.completions.len();
    }

    /// 选中上一个补全项。
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

    /// 从 LSP 补全响应构建补全列表。
    pub fn from_response(response: CompletionResponse, max_items: usize) -> Vec<Completion> {
        let items = match response {
            CompletionResponse::Array(items) => items,
            CompletionResponse::List(list) => list.items,
        };

        items
            .into_iter()
            .take(max_items)
            .map(|item| {
                let insert_text = item
                    .text_edit
                    .as_ref()
                    .and_then(|edit| match edit {
                        lsp_types::CompletionTextEdit::Edit(edit) => Some(edit.new_text.clone()),
                        lsp_types::CompletionTextEdit::InsertAndReplace(edit) => {
                            Some(edit.new_text.clone())
                        }
                    })
                    .unwrap_or_else(|| item.label.clone());

                Completion {
                    label: item.label.clone(),
                    insert_text,
                    detail: item.detail.clone(),
                    documentation: item.documentation.as_ref().map(|doc| match doc {
                        lsp_types::Documentation::String(s) => s.clone(),
                        lsp_types::Documentation::MarkupContent(content) => content.value.clone(),
                    }),
                    kind: item.kind,
                    score: 0.0,
                    lsp_item: item,
                }
            })
            .collect()
    }
}

/// 内联补全（Ghost Text）支持。
pub struct InlineCompletion {
    /// 补全文本（可能包含多行）。
    pub text: String,
    /// 补全范围。
    pub range: std::ops::Range<usize>,
    /// 是否为必选项（如 copilot suggestions）。
    pub is_required: bool,
}
