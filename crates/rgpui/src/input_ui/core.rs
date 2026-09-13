//! 文本核心：与组件无关的纯文本编辑状态。
//!
//! `TextCore` 是表单输入（`InputState`）与代码编辑器（未来 `EditorState`）
//! 共享的文本地基：Rope 文本、选区/光标、撤销历史、装饰存储、IME 标记。
//! 视图相关（滚动/布局/焦点/主题/掩码/校验）一律留在外层 state，不进核。

use ropey::Rope;

use super::change::Change;
use super::cursor::Selection;
use super::decorations::DecorationCollections;
use super::history::History;
use super::rope_ext::RopeExt as _;
use crate::sum_tree::Bias;
use std::ops::Range;

/// 共享文本核：纯数据 + 无界面依赖的操作。
///
/// 字段可见性为 `pub(super)`（仅 `input_ui` 子树内使用），外部一律走方法。
pub struct TextCore {
    /// 全文（UTF-8 Rope）。
    pub(super) text: Rope,
    /// 以 UTF-8 字节数计的选择范围。
    pub(super) selected_range: Selection,
    /// 选择方向是否反转（Shift+左扩展时）。
    pub(super) selection_reversed: bool,
    /// 撤销/重做历史。
    pub(super) history: History<Change>,
    /// 装饰集合存储（搜索标黄/括号匹配/当前行等共用）。
    pub(super) decorations: DecorationCollections,
    /// IME 输入中的临时标记范围。
    pub(super) ime_marked_range: Option<Selection>,
}

impl TextCore {
    /// 创建空文本核（历史按 1 秒间隔分组，与原 `InputState::new` 一致）。
    pub(super) fn new() -> Self {
        Self {
            text: "".into(),
            selected_range: Selection::default(),
            selection_reversed: false,
            history: History::new().group_interval(std::time::Duration::from_secs(1)),
            decorations: DecorationCollections::default(),
            ime_marked_range: None,
        }
    }

    /// 当前光标的字节偏移（UTF-8）。
    ///
    /// IME 组字中返回标记末尾；反选时返回选区起点，否则返回选区末尾。
    pub(super) fn cursor(&self) -> usize {
        if let Some(ime_marked_range) = &self.ime_marked_range {
            return ime_marked_range.end;
        }

        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    /// 记录一次撤销历史条目（`ignore` 时跳过）。
    pub(super) fn push_history(&mut self, old_text: &Rope, range: &Range<usize>, new_text: &str) {
        if self.history.ignore {
            return;
        }

        let range = old_text.clip_offset(range.start, Bias::Left)
            ..old_text.clip_offset(range.end, Bias::Right);
        let old_snippet = old_text.slice(range.clone()).to_string();
        let new_range = range.start..range.start + new_text.len();

        self.history
            .push(Change::new(range, &old_snippet, new_range, new_text));
    }

    /// 应用一次文本替换（正常路径；调用方已完成规范化/校验/掩码判定）。
    ///
    /// 做：文本替换、装饰跟随、历史记录（除非 `ignore`）、分组结束、
    /// 选区坍缩到落点、清除 IME 标记。返回落点偏移供调用方做视图更新；
    /// 替换前文本调用方自持有（display_map/折叠调整用）。
    pub(super) fn apply_replace(&mut self, range: Range<usize>, new_text: &str) -> usize {
        let old_text = self.text.clone();
        self.text.replace(range.clone(), new_text);
        let new_offset = (range.start + new_text.len()).min(self.text.len());
        self.decorations.adjust_for_edit(&range, new_text.len());
        self.push_history(&old_text, &range, new_text);
        self.history.end_grouping();
        self.selected_range = (new_offset..new_offset).into();
        self.ime_marked_range.take();
        new_offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 空核默认值：空文本、空选区、无历史、无标记。
    #[test]
    fn empty_core_defaults() {
        let core = TextCore::new();
        assert_eq!(core.text.len(), 0);
        assert!(core.selected_range.is_empty());
        assert_eq!(core.cursor(), 0);
        assert!(core.ime_marked_range.is_none());
    }

    /// 光标语义：反选取起点，IME 组字中取标记末尾。
    #[test]
    fn cursor_semantics() {
        let mut core = TextCore::new();
        core.selected_range = Selection::new(2, 5);
        assert_eq!(core.cursor(), 5);
        core.selection_reversed = true;
        assert_eq!(core.cursor(), 2);
        core.ime_marked_range = Some(Selection::new(1, 4));
        assert_eq!(core.cursor(), 4);
    }

    /// apply_replace：替换文本、坍缩选区、清 IME 标记、记历史。
    #[test]
    fn apply_replace_basic() {
        let mut core = TextCore::new();
        core.text = Rope::from("hello world");
        let new_offset = core.apply_replace(6..11, "rgpui");
        assert_eq!(core.text.to_string(), "hello rgpui");
        assert_eq!(new_offset, 11);
        assert_eq!(core.cursor(), 11);
        assert!(core.ime_marked_range.is_none());
        // 撤销回到原文（历史条目正确）。
        let changes = core.history.undo().expect("has undo");
        assert_eq!(changes.len(), 1);
        let change = &changes[0];
        let mut text = core.text.clone();
        let range = change.new_range.start..change.new_range.end;
        text.replace(range, &change.old_text);
        assert_eq!(text.to_string(), "hello world");
    }

    /// push_history 的 ignore 开关：撤销路径不重复记录。
    #[test]
    fn push_history_respects_ignore() {
        let mut core = TextCore::new();
        core.text = Rope::from("abc");
        core.history.ignore = true;
        core.push_history(&core.text.clone(), &(1..2), "X");
        assert!(core.history.undo().is_none());
        core.history.ignore = false;
        core.push_history(&core.text.clone(), &(1..2), "X");
        assert!(core.history.undo().is_some());
    }
}
