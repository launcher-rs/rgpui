//! 括号匹配高亮：光标旁的括号与其配对项标底色。
//!
//! 纯扫描（`find_bracket_match`，无语法树）：忽略字符串/注释内的括号，
//! 大文档只看光标前后各 [`SCAN_BUDGET`] 字节窗口。渲染走独立装饰集合，
//! 编辑时范围自动跟随（[`DecorationCollections::adjust_for_edit`]）。
//!
//! 刷新点：`move_to` / `select_to` / 替换收尾——全部处于 entity 借用内，
//! 因此一律原地写存储（`set_in_place`），不用走 `entity.update` 的
//! `set`/`clear`（会重入 panic，见 [`TextDecorationCollection::set`] 文档）。

use std::ops::Range;

use ropey::Rope;

use crate::theme::ActiveTheme as _;
use crate::{Context, HighlightStyle, Hsla};

use super::decorations::{TextDecoration, normalize};
use super::state::InputState;

/// 光标前后各扫描这么多字节（窗口切片，超出部分直接视为无匹配）。
const SCAN_BUDGET: usize = 100_000;

/// 参与匹配的括号对（与自动闭合一致，尖括号 excluded）。
const PAIRS: [(char, char); 3] = [('(', ')'), ('[', ']'), ('{', '}')];

/// 给定左括号找右括号。
fn matching_closer(open: char) -> Option<char> {
    PAIRS.iter().find(|(o, _)| *o == open).map(|(_, c)| *c)
}

/// 给定右括号找左括号。
fn matching_opener(close: char) -> Option<char> {
    PAIRS.iter().find(|(_, c)| *c == close).map(|(o, _)| *o)
}

/// 查找光标旁括号的配对项，返回 `(锚括号范围, 配对括号范围)`（UTF-8 字节偏移）。
///
/// 优先光标前的括号，否则看光标处的左括号；都不是括号返回 `None`。
pub(super) fn find_bracket_match(
    text: &Rope,
    cursor: usize,
) -> Option<(Range<usize>, Range<usize>)> {
    let len = text.len();
    let cursor = cursor.min(len);
    // 窗口切片（吸附字符边界），大文档只扫窗口，偏移最后 rebased 回全文。
    let win_start = text.floor_char_boundary(cursor.saturating_sub(SCAN_BUDGET));
    let win_end = text
        .ceil_char_boundary((cursor + SCAN_BUDGET).min(len))
        .min(len);
    let window = text.slice(win_start..win_end).to_string();
    let relative = cursor - win_start;
    find_in_str(&window, relative).map(|(a, b)| {
        (
            a.start + win_start..a.end + win_start,
            b.start + win_start..b.end + win_start,
        )
    })
}

/// 字符串内的匹配（`cursor` 为字节偏移且落在字符边界上）。
fn find_in_str(text: &str, cursor: usize) -> Option<(Range<usize>, Range<usize>)> {
    let before = text[..cursor]
        .chars()
        .next_back()
        .map(|c| (cursor - c.len_utf8(), c));
    let at = text[cursor..].chars().next().map(|c| (cursor, c));
    if let Some((pos, ch)) = before {
        if matching_closer(ch).is_some() {
            return match_forward(text, pos, ch).map(|m| (pos..pos + ch.len_utf8(), m));
        } else if matching_opener(ch).is_some() {
            return match_backward(text, pos, ch).map(|m| (m, pos..pos + ch.len_utf8()));
        }
    }
    if let Some((pos, ch)) = at {
        if matching_closer(ch).is_some() {
            return match_forward(text, pos, ch).map(|m| (pos..pos + ch.len_utf8(), m));
        } else if matching_opener(ch).is_some() {
            return match_backward(text, pos, ch).map(|m| (m, pos..pos + ch.len_utf8()));
        }
    }
    None
}

/// 自 `open_pos` 处的左括号向后找配对右括号。
fn match_forward(text: &str, open_pos: usize, open: char) -> Option<Range<usize>> {
    let close = matching_closer(open)?;
    let mut depth = 0;
    let mut offset = open_pos;
    for c in text[open_pos..].chars() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(offset..offset + c.len_utf8());
            }
        }
        offset += c.len_utf8();
    }
    None
}

/// 自 `close_pos` 处的右括号向前找配对左括号。
fn match_backward(text: &str, close_pos: usize, close: char) -> Option<Range<usize>> {
    let open = matching_opener(close)?;
    let mut depth = 0;
    let mut offset = close_pos + close.len_utf8();
    for c in text[..offset].chars().rev() {
        offset -= c.len_utf8();
        if c == close {
            depth += 1;
        } else if c == open {
            depth -= 1;
            if depth == 0 {
                return Some(offset..offset + c.len_utf8());
            }
        }
    }
    None
}

/// 括号匹配高亮的背景色（主题 accent 低透明）。
fn bracket_color(cx: &crate::App) -> Hsla {
    cx.theme().tokens.accent.color.opacity(0.25)
}

impl InputState {
    /// 按当前光标刷新括号匹配高亮（无匹配/关闭/单行时清空）。
    pub(super) fn refresh_bracket_match(&mut self, cx: &mut Context<Self>) {
        if !self.bracket_match_enabled || !self.mode.is_multi_line() {
            self.clear_bracket_match(cx);
            return;
        }
        let style = HighlightStyle {
            background_color: Some(bracket_color(cx)),
            ..Default::default()
        };
        let decorations: Vec<TextDecoration> = find_bracket_match(&self.text, self.cursor())
            .into_iter()
            .flat_map(|(a, b)| [a, b])
            .map(|range| TextDecoration::new(range, style))
            .collect();
        // 原地写（借用中调 `set` 会重入 panic，见模块文档）。
        if let Some(collection) = self.bracket_match_collection.clone() {
            let decorations = normalize(&self.text, decorations);
            if collection.set_in_place(&mut self.decorations, decorations) {
                cx.notify();
            }
        } else if !decorations.is_empty() {
            self.bracket_match_collection =
                Some(self.create_decorations_collection(decorations, cx));
        }
    }

    /// 清除括号匹配高亮（保留集合句柄供复用，原地写以兼容借用中调用）。
    pub(super) fn clear_bracket_match(&mut self, cx: &mut Context<Self>) {
        if let Some(collection) = self.bracket_match_collection.clone() {
            if collection.clear_in_place(&mut self.decorations) {
                cx.notify();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppContext as _;
    use crate::{Entity, Window};

    fn check(text: &str, cursor: usize, expected: Option<(&str, &str)>) {
        let rope = Rope::from(text);
        let found = find_bracket_match(&rope, cursor);
        let actual = found.map(|(a, b)| (rope.slice(a).to_string(), rope.slice(b).to_string()));
        let expected = expected.map(|(a, b)| (a.to_string(), b.to_string()));
        assert_eq!(actual, expected, "text {text:?} cursor {cursor}");
    }

    #[test]
    fn matches_adjacent_brackets() {
        // 光标紧跟左括号后。
        check("(a)", 1, Some(("(", ")")));
        // 光标在右括号前。
        check("(a)", 2, Some(("(", ")")));
        // 光标在左括号前。
        check("(a)", 0, Some(("(", ")")));
        // 嵌套：内层优先。
        check("((x))", 2, Some(("(", ")")));
        check("((x))", 3, Some(("(", ")")));
        // 花括号/方括号。
        check("{a[b]}", 1, Some(("{", "}")));
        check("{a[b]}", 3, Some(("[", "]")));
    }

    #[test]
    fn no_match_cases() {
        // 非括号旁。
        check("ab", 1, None);
        // 未闭合。
        check("(abc", 1, None);
        check("abc)", 4, None);
        // 空文本/越界钳制。
        check("", 0, None);
        check("()", 99, Some(("(", ")")));
    }

    /// 持有输入框状态的测试宿主视图。
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

    #[rgpui::test]
    fn refresh_highlights_match_on_cursor_move(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            state.update(cx, |state, cx| state.replace("(a) b", window, cx));
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        // 光标移到 `(` 之后：两处括号都被标出。
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_selected_range(1..1, cx));
        });
        let ranges = state.read_with(cx, |state, cx| {
            state
                .bracket_match_collection
                .as_ref()
                .map(|collection| collection.get_ranges(cx))
                .unwrap_or_default()
        });
        assert_eq!(ranges, vec![0..1, 2..3]);
        // 光标移到 `b` 之后（前后都不是括号）：高亮清空。
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_selected_range(5..5, cx));
        });
        let ranges = state.read_with(cx, |state, cx| {
            state
                .bracket_match_collection
                .as_ref()
                .map(|collection| collection.get_ranges(cx))
                .unwrap_or_default()
        });
        assert!(ranges.is_empty());
    }
}
