//! 行操作命令：复制行、删除行、上下移行、行注释切换、合并行。
//!
//! 全部只在多行模式生效（单行模式下传播动作，由外部处理），
//! 走 [`InputState::replace_text_in_range_silent`]，自带历史/装饰/高亮联动。
//!
//! 目标行规则（对标 VS Code）：无选区时取光标所在行；有选区时取覆盖行，
//! 若选区结尾恰在某行行首则不含该行；文档末尾的空幻影行不计入。

use std::ops::Range;

use crate::{Context, Window};

use super::RopeExt as _;
use super::state::InputState;

impl InputState {
    /// 复制行（`CopyLine`）：在目标行下方复刻一份，光标移到复刻行的同列。
    pub(super) fn copy_line(
        &mut self,
        _: &super::CopyLine,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.guard_line_op(cx) {
            return;
        }
        self.collapse_for_line_op(cx);
        let (start_row, end_row) = self.target_rows();
        let chunk = self.rows_text(start_row, end_row);
        let insert_at = self.core.text.line_end_offset(end_row);
        let insertion = format!("\n{chunk}");
        // 快照选区：`replace` 会把选区塌缩到插入末尾，后续位置都按快照算。
        let sel: Range<usize> = self.core.selected_range.into();
        self.replace_text_in_range_silent(
            Some(self.range_to_utf16(&(insert_at..insert_at))),
            &insertion,
            window,
            cx,
        );
        // 复刻块紧跟原文之后，光标/选区整体后移一个插入块。
        let shift = insertion.len();
        self.core.selected_range = (sel.start + shift..sel.end + shift).into();
        self.core.selection_reversed = false;
        cx.notify();
    }

    /// 删除行（`DeleteLine`）：删整行（含换行），光标落到替补行行首。
    pub(super) fn delete_line(
        &mut self,
        _: &super::DeleteLine,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.guard_line_op(cx) {
            return;
        }
        self.collapse_for_line_op(cx);
        let (start_row, end_row) = self.target_rows();
        let range = self.block_range(start_row, end_row);
        self.replace_text_in_range_silent(Some(self.range_to_utf16(&range)), "", window, cx);
        let last = self.core.text.lines_len().saturating_sub(1);
        let row = start_row.min(last);
        let offset = self
            .core
            .text
            .line_start_offset(row)
            .min(self.core.text.len());
        self.core.selected_range = (offset..offset).into();
        self.core.selection_reversed = false;
        cx.notify();
    }

    /// 上移行（`MoveLineUp`）：与上一行交换，选区跟随。
    pub(super) fn move_line_up(
        &mut self,
        _: &super::MoveLineUp,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.guard_line_op(cx) {
            return;
        }
        self.collapse_for_line_op(cx);
        let (start_row, end_row) = self.target_rows();
        if start_row == 0 {
            return;
        }
        let prev = self.rows_text(start_row - 1, start_row - 1);
        let block = self.rows_text(start_row, end_row);
        let range = self.core.text.line_start_offset(start_row - 1)
            ..self.core.text.line_end_offset(end_row);
        let replacement = format!("{block}\n{prev}");
        let sel: Range<usize> = self.core.selected_range.into();
        self.replace_text_in_range_silent(
            Some(self.range_to_utf16(&range)),
            &replacement,
            window,
            cx,
        );
        // 整块上移一个“上一行 + 换行”的长度。
        let shift = prev.len() + 1;
        self.core.selected_range =
            (sel.start.saturating_sub(shift)..sel.end.saturating_sub(shift)).into();
        self.core.selection_reversed = false;
        cx.notify();
    }

    /// 下移行（`MoveLineDown`）：与下一行交换，选区跟随。
    pub(super) fn move_line_down(
        &mut self,
        _: &super::MoveLineDown,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.guard_line_op(cx) {
            return;
        }
        self.collapse_for_line_op(cx);
        let (start_row, end_row) = self.target_rows();
        if end_row + 1 >= self.effective_line_count() {
            return;
        }
        let next = self.rows_text(end_row + 1, end_row + 1);
        let block = self.rows_text(start_row, end_row);
        let range = self.core.text.line_start_offset(start_row)
            ..self.core.text.line_end_offset(end_row + 1);
        let replacement = format!("{next}\n{block}");
        let sel: Range<usize> = self.core.selected_range.into();
        self.replace_text_in_range_silent(
            Some(self.range_to_utf16(&range)),
            &replacement,
            window,
            cx,
        );
        // 整块下移一个“下一行 + 换行”的长度。
        let shift = next.len() + 1;
        self.core.selected_range = (sel.start + shift..sel.end + shift).into();
        self.core.selection_reversed = false;
        cx.notify();
    }

    /// 行注释切换（`ToggleLineComment`）：全已注释则解注释，否则加注释。
    ///
    /// 注释符由 [`InputState::line_comment_prefix`] 决定（默认 `//`），
    /// 空行跳过，注释加在缩进之后（`// ` 带一个空格）。
    pub(super) fn toggle_line_comment(
        &mut self,
        _: &super::ToggleLineComment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.guard_line_op(cx) {
            return;
        }
        self.collapse_for_line_op(cx);
        let (start_row, end_row) = self.target_rows();
        let prefix: String = self.line_comment_prefix.to_string();
        // 先收集各非空行的缩进末尾与是否已注释（只读阶段，不动文本）。
        let mut rows = Vec::new();
        for row in start_row..=end_row {
            let line_start = self.core.text.line_start_offset(row);
            let line_end = self.core.text.line_end_offset(row);
            let line = self.core.text.slice(line_start..line_end).to_string();
            let indent_len = line.len() - line.trim_start().len();
            if line.trim().is_empty() {
                continue;
            }
            let commented = line[indent_len..].starts_with(&prefix);
            rows.push((row, line_start + indent_len, commented));
        }
        if rows.is_empty() {
            return;
        }
        let uncomment = rows.iter().all(|(_, _, commented)| *commented);
        let sel: Range<usize> = self.core.selected_range.into();
        let mut cursor = sel.start;
        let mut sel_end = sel.end;
        // 自下而上编辑，偏移不失效；光标/选区按编辑增量跟随。
        for (row, pos, _) in rows.iter().rev() {
            if uncomment {
                let line_start = self.core.text.line_start_offset(*row);
                let line_end = self.core.text.line_end_offset(*row);
                let line = self.core.text.slice(line_start..line_end).to_string();
                let indent_len = line.len() - line.trim_start().len();
                let mut remove_end = *pos + prefix.len();
                if line[indent_len + prefix.len()..].starts_with(' ') {
                    remove_end += 1;
                }
                let removed = remove_end - *pos;
                self.replace_text_in_range_silent(
                    Some(self.range_to_utf16(&(*pos..remove_end))),
                    "",
                    window,
                    cx,
                );
                cursor = shift_for_edit(cursor, *pos, removed, removed);
                sel_end = shift_for_edit(sel_end, *pos, removed, removed);
            } else {
                let insertion = format!("{prefix} ");
                self.replace_text_in_range_silent(
                    Some(self.range_to_utf16(&(*pos..*pos))),
                    &insertion,
                    window,
                    cx,
                );
                cursor = shift_for_edit(cursor, *pos, 0, insertion.len());
                sel_end = shift_for_edit(sel_end, *pos, 0, insertion.len());
            }
        }
        self.core.selected_range = (cursor..sel_end).into();
        self.core.selection_reversed = false;
        cx.notify();
    }

    /// 合并行（`JoinLines`）：无选区时合并光标行与下一行，否则合并覆盖行。
    ///
    /// 行尾/行首空白被清理，行间只留一个空格。
    pub(super) fn join_lines(
        &mut self,
        _: &super::JoinLines,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.guard_line_op(cx) {
            return;
        }
        self.collapse_for_line_op(cx);
        let (mut start_row, mut end_row) = self.target_rows();
        let last = self.effective_line_count().saturating_sub(1);
        if self.core.selected_range.is_empty() {
            end_row = (start_row + 1).min(last);
        } else {
            end_row = end_row.min(last);
        }
        start_row = start_row.min(last);
        if start_row >= end_row {
            return;
        }
        let mut parts = Vec::new();
        for row in start_row..=end_row {
            let line = self
                .core
                .text
                .slice(self.core.text.line_start_offset(row)..self.core.text.line_end_offset(row))
                .to_string();
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() || parts.is_empty() {
                parts.push(trimmed);
            }
        }
        let joined = parts.join(" ");
        let range =
            self.core.text.line_start_offset(start_row)..self.core.text.line_end_offset(end_row);
        let sel: Range<usize> = self.core.selected_range.into();
        self.replace_text_in_range_silent(Some(self.range_to_utf16(&range)), &joined, window, cx);
        // 光标保持原行内位置，钳制到合并后的行内。
        let cursor = sel.start.max(range.start).min(range.start + joined.len());
        self.core.selected_range = (cursor..cursor).into();
        self.core.selection_reversed = false;
        cx.notify();
    }

    /// 行操作公共守卫：只读/禁用/单行模式直接返回 false。
    fn guard_line_op(&self, cx: &mut Context<Self>) -> bool {
        if self.read_only || self.disabled || !self.mode.is_multi_line() {
            cx.propagate();
            return false;
        }
        true
    }

    /// 行操作入口统一先坍缩多光标（各操作只认主光标，偏移会整体漂移）。
    fn collapse_for_line_op(&mut self, cx: &mut Context<Self>) {
        if !self.extra_selections.is_empty() {
            self.extra_selections.clear();
            cx.notify();
        }
    }

    /// 选区覆盖的目标行（闭区间），含选区结尾行首排除与末尾幻影行钳制。
    fn target_rows(&self) -> (usize, usize) {
        let sel: Range<usize> = self.core.selected_range.into();
        let last = self.effective_line_count().saturating_sub(1);
        let start_row = self
            .core
            .text
            .offset_to_point(sel.start.min(self.core.text.len()))
            .row
            .min(last);
        let mut end_row = self
            .core
            .text
            .offset_to_point(sel.end.min(self.core.text.len()))
            .row
            .min(last);
        if !sel.is_empty()
            && sel.end == self.core.text.line_start_offset(end_row)
            && end_row > start_row
        {
            end_row -= 1;
        }
        (start_row, end_row)
    }

    /// 有效行数（文档末尾的空幻影行不计入，避免移行/合并行撞上空行）。
    fn effective_line_count(&self) -> usize {
        let count = self.core.text.lines_len();
        if count > 1 {
            let last_start = self.core.text.line_start_offset(count - 1);
            if last_start == self.core.text.len() {
                return count - 1;
            }
        }
        count
    }

    /// 若干整行的文本（不含换行符，`\n` 连接）。
    pub(super) fn rows_text(&self, start_row: usize, end_row: usize) -> String {
        let mut out = String::new();
        for row in start_row..=end_row {
            if row > start_row {
                out.push('\n');
            }
            out.push_str(
                &self
                    .core
                    .text
                    .slice(
                        self.core.text.line_start_offset(row)..self.core.text.line_end_offset(row),
                    )
                    .to_string(),
            );
        }
        out
    }

    /// 目标行块的字节范围（含换行：优先吞后换行，末行吞前换行）。
    pub(super) fn block_range(&self, start_row: usize, end_row: usize) -> Range<usize> {
        let line_count = self.core.text.lines_len();
        if end_row + 1 < line_count {
            self.core.text.line_start_offset(start_row)
                ..self.core.text.line_start_offset(end_row + 1)
        } else if start_row > 0 {
            self.core.text.line_end_offset(start_row - 1)..self.core.text.len()
        } else {
            0..self.core.text.len()
        }
    }
}

/// 自下而上编辑时跟随光标：`edit` 处删 `removed` 字节、增 `added` 字节。
fn shift_for_edit(offset: usize, edit: usize, removed: usize, added: usize) -> usize {
    if offset <= edit {
        offset
    } else if offset >= edit + removed {
        offset + added - removed
    } else {
        edit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppContext as _;
    use crate::Entity;
    use crate::input_ui::{
        CopyLine, DeleteLine, JoinLines, MoveLineDown, MoveLineUp, ToggleLineComment,
    };

    /// 持有多行输入框状态的测试宿主视图。
    struct Probe {
        state: Entity<InputState>,
    }

    impl crate::Render for Probe {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> impl crate::IntoElement {
            crate::div()
        }
    }

    /// 读取文本与光标，便于断言。
    macro_rules! assert_text_cursor {
        ($state:expr, $cx:expr, $text:expr, $cursor:expr) => {
            assert_eq!(
                $state.read_with($cx, |state, _| (state.value().to_string(), state.cursor())),
                ($text.to_string(), $cursor)
            );
        };
    }

    #[rgpui::test]
    fn copy_line_down_duplicates_current_line(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            state.update(cx, |state, cx| state.replace("a\nb\nc", window, cx));
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_selected_range(2..2, cx));
        });
        cx.update(|window, cx| {
            state.update(cx, |state, cx| state.copy_line(&CopyLine, window, cx));
        });
        assert_text_cursor!(state, cx, "a\nb\nb\nc", 4);
    }

    #[rgpui::test]
    fn delete_first_line_moves_cursor_to_substitute(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            state.update(cx, |state, cx| state.replace("a\nb\nc", window, cx));
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_selected_range(0..0, cx));
        });
        cx.update(|window, cx| {
            state.update(cx, |state, cx| state.delete_line(&DeleteLine, window, cx));
        });
        assert_text_cursor!(state, cx, "b\nc", 0);
    }

    #[rgpui::test]
    fn move_line_down_and_up_roundtrip(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            state.update(cx, |state, cx| state.replace("a\nb\nc", window, cx));
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_selected_range(0..0, cx));
        });
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                state.move_line_down(&MoveLineDown, window, cx)
            });
        });
        assert_text_cursor!(state, cx, "b\na\nc", 2);
        cx.update(|window, cx| {
            state.update(cx, |state, cx| state.move_line_up(&MoveLineUp, window, cx));
        });
        assert_text_cursor!(state, cx, "a\nb\nc", 0);
    }

    #[rgpui::test]
    fn move_line_at_boundary_is_noop(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            state.update(cx, |state, cx| state.replace("a\nb\nc", window, cx));
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        // 首行上移无操作。
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_selected_range(0..0, cx));
        });
        cx.update(|window, cx| {
            state.update(cx, |state, cx| state.move_line_up(&MoveLineUp, window, cx));
        });
        assert_text_cursor!(state, cx, "a\nb\nc", 0);
        // 末行下移无操作。
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_selected_range(4..4, cx));
        });
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                state.move_line_down(&MoveLineDown, window, cx)
            });
        });
        assert_text_cursor!(state, cx, "a\nb\nc", 4);
    }

    #[rgpui::test]
    fn toggle_comment_roundtrip_skips_blank_lines(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            state.update(cx, |state, cx| {
                state.replace("fn main() {\n    let x = 1;\n\n}", window, cx)
            });
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        // 选区 0..27 覆盖前三行（结尾恰在 `}` 行行首，不含该行）。
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_selected_range(0..27, cx));
        });
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                state.toggle_line_comment(&ToggleLineComment, window, cx)
            });
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.value().to_string()),
            "// fn main() {\n    // let x = 1;\n\n}".to_string()
        );
        // 再切一次恢复原文（圈定前两行：结尾落在空行行尾，不含 `}` 行）。
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_selected_range(0..33, cx));
        });
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                state.toggle_line_comment(&ToggleLineComment, window, cx)
            });
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.value().to_string()),
            "fn main() {\n    let x = 1;\n\n}".to_string()
        );
    }

    #[rgpui::test]
    fn join_lines_collapses_whitespace(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            state.update(cx, |state, cx| {
                state.replace("hello\n   world\nrust", window, cx)
            });
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_selected_range(0..0, cx));
        });
        cx.update(|window, cx| {
            state.update(cx, |state, cx| state.join_lines(&JoinLines, window, cx));
        });
        assert_text_cursor!(state, cx, "hello world\nrust", 0);
    }
}
