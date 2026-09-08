//! 多光标多选区：主选区之外可再加若干选区，同写同删。
//!
//! 模型：`selected_range` 仍是主光标（既有代码零改动），`extra_selections`
//! 存其余光标（有序、互不重叠、不与主光标重叠，由 [`InputState::normalize_extras`]
//! 维护）。渲染、括号匹配、当前行等跟随主光标；编辑（键入/退格/删除/剪切/
//! 复制/粘贴/回车）扇出到所有光标，自后向前应用，偏移不失效。
//!
//! 范围（v1，文档注明即契约）：
//! - 新增光标：`Alt+点击`、上下加光标动作；纯键盘移动/鼠标左键/Esc/全选坍缩为单光标，
//!   `Shift+方向` 只扩展主选区、其余保留；行操作先清额外光标再执行；
//! - 扇出编辑成组进撤销栈（一次撤销整体回退）；
//! - 键入时自动闭合不参与（按原文插入）；粘贴板多行不按光标分发（每处贴全文）；
//! - 掩码/单行输入建不了额外光标。

use std::ops::Range;

use crate::{ClipboardItem, Context, Window};

use super::RopeExt as _;
use super::state::InputState;

impl InputState {
    /// 是否存在额外光标。
    pub fn has_multiple_cursors(&self) -> bool {
        !self.extra_selections.is_empty()
    }

    /// 额外光标（主光标 `selected_range` 之外）。
    pub fn extra_selections(&self) -> &[super::Selection] {
        &self.extra_selections
    }

    /// 清除额外光标，只留主光标。
    pub fn clear_extra_cursors(&mut self, cx: &mut Context<Self>) {
        if self.extra_selections.is_empty() {
            return;
        }
        self.extra_selections.clear();
        cx.notify();
    }

    /// 在主光标上方同列加一个光标（`AddCursorAbove`）。
    pub(super) fn add_cursor_above(
        &mut self,
        _: &super::AddCursorAbove,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.multicursor_allowed(cx) {
            return;
        }
        self.add_cursors(-1);
        cx.notify();
    }

    /// 在主光标下方同列加一个光标（`AddCursorBelow`）。
    pub(super) fn add_cursor_below(
        &mut self,
        _: &super::AddCursorBelow,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.multicursor_allowed(cx) {
            return;
        }
        self.add_cursors(1);
        cx.notify();
    }

    /// 多光标守卫：仅多行可编辑非掩码输入。
    fn multicursor_allowed(&self, cx: &mut Context<Self>) -> bool {
        if !self.mode.is_multi_line() || self.disabled || self.read_only || self.masked {
            cx.propagate();
            return false;
        }
        true
    }

    /// 给每个已有光标在上下 `delta_rows` 行的同列加一个光标。
    fn add_cursors(&mut self, delta_rows: isize) {
        let last_row = self.core.text.lines_len().saturating_sub(1);
        let mut offsets: Vec<usize> = std::iter::once(self.core.selected_range.end)
            .chain(self.extra_selections.iter().map(|sel| sel.end))
            .collect();
        for offset in offsets.drain(..) {
            let point = self.core.text.offset_to_point(offset);
            let column = offset.saturating_sub(self.core.text.line_start_offset(point.row));
            let target = point.row.saturating_add_signed(delta_rows).min(last_row);
            if target == point.row {
                continue;
            }
            let line_start = self.core.text.line_start_offset(target);
            let line_len = self.core.text.line_end_offset(target) - line_start;
            let at = self
                .core
                .text
                .floor_char_boundary(line_start + column.min(line_len));
            self.extra_selections.push(super::Selection::new(at, at));
        }
        self.normalize_extras();
    }

    /// 整理额外光标：排序、合并重叠、去掉与主光标重叠的（主光标保留）。
    pub(super) fn normalize_extras(&mut self) {
        let primary: Range<usize> = self.core.selected_range.into();
        self.extra_selections.sort_by_key(|sel| sel.start);
        let mut merged: Vec<super::Selection> = Vec::new();
        for sel in self.extra_selections.drain(..) {
            // 与主光标重叠/相接：主光标保留，丢弃。
            if sel.start <= primary.end && primary.start <= sel.end {
                continue;
            }
            if let Some(last) = merged.last_mut() {
                if sel.start <= last.end {
                    last.end = last.end.max(sel.end);
                    continue;
                }
            }
            merged.push(sel);
        }
        self.extra_selections = merged;
    }

    /// 所有光标范围（含主光标），按起始偏移降序（自后向前编辑用）。
    fn cursors_back_to_front(&self) -> Vec<Range<usize>> {
        let mut all: Vec<Range<usize>> = std::iter::once(self.core.selected_range.into())
            .chain(self.extra_selections.iter().map(|sel| (*sel).into()))
            .collect();
        all.sort_by_key(|range| std::cmp::Reverse(range.start));
        all
    }

    /// 自后向前应用一批互不重叠的替换，并跟随一组光标位置。
    ///
    /// 返回与 `points` 同顺序的新位置。规则：编辑区之后的光标按增量平移，
    /// 落在编辑区内的收敛到新区末尾，恰在插入点的跟到插入末尾。
    fn apply_edits_and_track(
        &mut self,
        edits: &mut Vec<(Range<usize>, String)>,
        points: &mut Vec<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        let before = self.core.history.undos().len();
        for (range, text) in edits.iter() {
            let old_len = range.len();
            let new_len = text.len();
            self.replace_text_in_range_silent(Some(self.range_to_utf16(range)), text, window, cx);
            for point in points.iter_mut() {
                if *point >= range.end {
                    *point = point.saturating_add(new_len).saturating_sub(old_len);
                } else if *point > range.start {
                    *point = range.start + new_len;
                } else if *point == range.start && new_len > 0 {
                    *point = range.start + new_len;
                }
            }
        }
        self.regroup_history(before);
    }

    /// 收敛多光标编辑结果：主光标身份保持，extras 保持原顺序。
    fn collapse_cursors(&mut self, mut points: Vec<usize>, cx: &mut Context<Self>) {
        let mut points_iter = points.drain(..);
        let primary = points_iter.next().unwrap_or(0);
        self.core.selected_range = super::Selection::new(primary, primary);
        self.extra_selections = points_iter
            .map(|at| super::Selection::new(at, at))
            .collect();
        self.core.selection_reversed = false;
        cx.notify();
    }

    /// 在所有光标处插入同一文本（键入/粘贴扇出），每处光标落插入末尾。
    pub(super) fn multi_insert(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if text.is_empty() {
            return;
        }
        let mut edits: Vec<(Range<usize>, String)> = self
            .cursors_back_to_front()
            .into_iter()
            .map(|range| (range, text.to_string()))
            .collect();
        let mut points: Vec<usize> = std::iter::once(self.core.selected_range.start)
            .chain(self.extra_selections.iter().map(|sel| sel.start))
            .collect();
        self.apply_edits_and_track(&mut edits, &mut points, window, cx);
        self.collapse_cursors(points, cx);
    }

    /// 在所有光标处删除（退格/删除扇出）：有选区删选区，否则删一个边界单位。
    pub(super) fn multi_delete(
        &mut self,
        backward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cursors = self.cursors_back_to_front();
        // 先算出所有待删范围（边界计算依赖原文本，一次算完再动手）。
        let mut ranges = Vec::new();
        for range in &cursors {
            let delete = if !range.is_empty() {
                Some(range.clone())
            } else if backward {
                let start = self.previous_boundary(range.start);
                (start < range.start).then(|| start..range.start)
            } else {
                let end = self.next_boundary(range.end);
                (end > range.end).then(|| range.end..end)
            };
            if let Some(delete) = delete {
                ranges.push(delete);
            }
        }
        if ranges.is_empty() {
            return;
        }
        let mut edits: Vec<(Range<usize>, String)> = ranges
            .into_iter()
            .map(|range| (range, String::new()))
            .collect();
        let mut points: Vec<usize> = std::iter::once(self.core.selected_range.start)
            .chain(self.extra_selections.iter().map(|sel| sel.start))
            .collect();
        self.apply_edits_and_track(&mut edits, &mut points, window, cx);
        self.collapse_cursors(points, cx);
    }

    /// 多光标复制：各选区文本换行拼接；全塌缩时复制各光标所在整行。
    pub(super) fn copy_cursors(&mut self, cx: &mut Context<Self>) {
        let mut selections = self.cursors_back_to_front();
        selections.sort_by_key(|range| range.start);
        let all_collapsed = selections.iter().all(|range| range.is_empty());
        let parts: Vec<String> = if all_collapsed {
            selections
                .iter()
                .map(|range| {
                    let row = self.core.text.offset_to_point(range.start).row;
                    self.core
                        .text
                        .slice(
                            self.core.text.line_start_offset(row)
                                ..self.core.text.line_end_offset(row),
                        )
                        .to_string()
                })
                .collect()
        } else {
            selections
                .iter()
                .map(|range| self.core.text.slice(range.clone()).to_string())
                .collect()
        };
        cx.write_to_clipboard(ClipboardItem::new_string(parts.join("\n")));
    }

    /// 多光标剪切：复制后删除所有光标范围（空光标删整行）。
    pub(super) fn cut_cursors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.copy_cursors(cx);
        let mut selections = self.cursors_back_to_front();
        selections.sort_by_key(|range| range.start);
        let all_collapsed = selections.iter().all(|range| range.is_empty());
        let mut ranges: Vec<Range<usize>> = if all_collapsed {
            selections
                .iter()
                .map(|range| {
                    let row = self.core.text.offset_to_point(range.start).row;
                    self.block_range(row, row)
                })
                .collect()
        } else {
            selections
        };
        // 自后向前删。
        ranges.sort_by_key(|range| std::cmp::Reverse(range.start));
        let mut edits: Vec<(Range<usize>, String)> = ranges
            .into_iter()
            .map(|range| (range, String::new()))
            .collect();
        let mut points: Vec<usize> = std::iter::once(self.core.selected_range.start)
            .chain(self.extra_selections.iter().map(|sel| sel.start))
            .collect();
        self.apply_edits_and_track(&mut edits, &mut points, window, cx);
        // 光标落删除起点（钳制到新文本内）。
        let len = self.core.text.len();
        let points: Vec<usize> = points.into_iter().map(|at| at.min(len)).collect();
        self.collapse_cursors(points, cx);
    }

    /// 多光标回车：每处换行并延续该行缩进（电缩进拆行不参与，落插入末尾）。
    pub(super) fn enter_cursors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 先算好每处的插入文本（依赖原文本，一次算完再动手）。
        let mut edits = Vec::new();
        for range in self.cursors_back_to_front() {
            let start = range.start.min(self.core.text.len());
            let line_start = self
                .core
                .text
                .line_start_offset(self.core.text.offset_to_point(start).row);
            let indent: String = self
                .core
                .text
                .slice(line_start..start)
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect();
            edits.push((range, format!("\n{indent}")));
        }
        let mut points: Vec<usize> = std::iter::once(self.core.selected_range.start)
            .chain(self.extra_selections.iter().map(|sel| sel.start))
            .collect();
        self.apply_edits_and_track(&mut edits, &mut points, window, cx);
        self.pause_blink_cursor(cx);
        self.collapse_cursors(points, cx);
    }

    /// 把 `before` 之后新增的历史条目并为同一版本（一次撤销整体回退）。
    fn regroup_history(&mut self, before: usize) {
        let pushed = self.core.history.undos().len().saturating_sub(before);
        self.core.history.regroup_last(pushed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppContext as _;
    use crate::input_ui::{AddCursorBelow, Undo};
    use crate::{Entity, Window};

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

    /// 渲染输入框的宿主视图（渲染冒烟用）。
    struct InputProbe {
        state: Entity<InputState>,
    }

    impl crate::Render for InputProbe {
        fn render(
            &mut self,
            window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl crate::IntoElement {
            use crate::{IntoElement as _, RenderOnce as _};
            super::super::Input::new(&self.state)
                .render(window, cx)
                .into_element()
        }
    }

    #[rgpui::test]
    fn add_cursors_below_and_type_everywhere(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            // `set_value` 清空历史，避免 1 秒分组窗口把建仓文本也并入撤销组。
            state.update(cx, |state, cx| state.set_value("a\nb\nc", window, cx));
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_selected_range(0..0, cx));
        });
        // 加两次：第 1、2 行各添一个，主光标不动。
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                state.add_cursor_below(&AddCursorBelow, window, cx)
            });
            state.update(cx, |state, cx| {
                state.add_cursor_below(&AddCursorBelow, window, cx)
            });
        });
        assert!(state.read_with(cx, |state, _| state.has_multiple_cursors()));
        let extras: Vec<(usize, usize)> = state.read_with(cx, |state, _| {
            state
                .extra_selections()
                .iter()
                .map(|sel| (sel.start, sel.end))
                .collect()
        });
        assert_eq!(extras, vec![(2, 2), (4, 4)]);
        // 键入扇出到三处。
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                crate::EntityInputHandler::replace_text_in_range(state, None, "x", window, cx);
            });
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.value().to_string()),
            "xa\nxb\nxc".to_string()
        );
        // 一次撤销整体回退（扇出编辑已并组）。
        cx.update(|window, cx| {
            state.update(cx, |state, cx| state.undo(&Undo, window, cx));
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.value().to_string()),
            "a\nb\nc".to_string()
        );
    }

    #[rgpui::test]
    fn escape_and_navigation_collapse_to_single(cx: &mut crate::TestAppContext) {
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
                state.add_cursor_below(&AddCursorBelow, window, cx)
            });
        });
        assert!(state.read_with(cx, |state, _| state.has_multiple_cursors()));
        // Esc 先坍缩多光标。
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                state.escape(&crate::input_ui::Escape, window, cx)
            });
        });
        assert!(!state.read_with(cx, |state, _| state.has_multiple_cursors()));
        // 再加一个，方向键移动坍缩。
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                state.add_cursor_below(&AddCursorBelow, window, cx)
            });
        });
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                state.right(&crate::input_ui::MoveRight, window, cx)
            });
        });
        assert!(!state.read_with(cx, |state, _| state.has_multiple_cursors()));
    }

    #[rgpui::test]
    fn extra_cursors_render_without_panic(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| InputProbe {
            state: cx.new(|cx| {
                let mut state = InputState::new(window, cx).multi_line(true);
                state.replace("a\nb\nc", window, cx);
                state
            }),
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_selected_range(0..3, cx));
        });
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                state.add_cursor_below(&AddCursorBelow, window, cx)
            });
        });
        // 主选区 + 额外选区 + 额外光标一次 paint，不 panic 即过。
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
        assert!(state.read_with(cx, |state, _| state.has_multiple_cursors()));
    }
}
