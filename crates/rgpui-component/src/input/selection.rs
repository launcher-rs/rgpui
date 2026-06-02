use std::ops::Range;

use crate::{RopeExt as _, input::InputState, text::selection::word_range_from_chars};
use rgpui::sum_tree::Bias;
use rgpui::{Context, Window};
use ropey::Rope;

impl InputState {
    /// 在给定偏移量处双击选中单词。
    ///
    /// 偏移量为 UTF-8 偏移量。
    pub(super) fn select_word(&mut self, offset: usize, _: &mut Window, cx: &mut Context<Self>) {
        let Some(range) = TextSelector::word_range(&self.text, offset) else {
            return;
        };

        self.selected_range = (range.start..range.end).into();
        self.selected_word_range = Some(self.selected_range);
        cx.notify()
    }

    /// 在给定偏移量处三击选中整行。
    ///
    /// 偏移量为 UTF-8 偏移量。
    pub(super) fn select_line(&mut self, offset: usize, _: &mut Window, cx: &mut Context<Self>) {
        let range = TextSelector::line_range(&self.text, offset);
        self.selected_range = (range.start..range.end).into();
        self.selected_word_range = None;
        cx.notify()
    }
}

/// 文本选择器辅助结构
struct TextSelector;
impl TextSelector {
    /// 在给定文本的指定偏移量处选中一行。
    ///
    /// 偏移量为 UTF-8 偏移量。
    ///
    /// 返回选中行的起始和结束偏移量。
    pub fn line_range(text: &Rope, offset: usize) -> Range<usize> {
        let offset = text.clip_offset(offset, Bias::Left);
        let row = text.offset_to_point(offset).row;
        let start = text.line_start_offset(row);
        let end = text.line_end_offset(row);

        start..end
    }

    /// 在给定文本的指定偏移量处选中一个单词。
    ///
    /// 偏移量为 UTF-8 偏移量。
    ///
    /// 返回选中单词的起始和结束偏移量。
    pub fn word_range(text: &Rope, offset: usize) -> Option<Range<usize>> {
        let offset = text.clip_offset(offset, Bias::Left);
        let char = text.char_at(offset)?;
        let end = offset + char.len_utf8();
        let prev_chars = text.chars_at(offset).reversed().take(128);
        let next_chars = text.chars_at(end).take(128);
        Some(word_range_from_chars(offset, char, prev_chars, next_chars))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    #[test]
    fn test_word_range() {
        use indoc::indoc;

        let rope = Rope::from(indoc! {
            r#"
            test text:
            abcde 中文🎉 test
            hello[()]
            test_connector ____
            Rope
            rök
            grande île
            "#
        });

        let tests = vec![
            (0, 0, Some("test")),
            (0, 4, Some(" ")),
            (1, 0, Some("abcde")),
            (1, 4, Some("abcde")),
            (1, 5, Some(" ")),
            (1, 6, Some("中")),
            (1, 9, Some("文")),
            (1, 13, Some("🎉")),
            (1, 20, Some("test")),
            (2, 5, Some("[")),
            (2, 6, Some("(")),
            (2, 7, Some(")")),
            (2, 8, Some("]")),
            (3, 5, Some("test_connector")),
            (3, 14, Some(" ")),
            (3, 16, Some("____")),
            (4, 0, Some("Rope")),
            (5, 0, Some("rök")),
            (6, 8, Some("île")),
        ];

        for (line, column, expected) in tests {
            let line_start_offset = rope.line_start_offset(line);
            let offset = line_start_offset + column;
            let range = TextSelector::word_range(&rope, offset);

            let actual = range.map(|r| rope.slice(r).to_string());
            let expect = expected.map(|s| s.to_string());
            assert_eq!(actual, expect, "行 {}, 列 {}", line, column);
        }
    }

    #[test]
    fn test_line_range() {
        let rope = Rope::from("first line\nsecond line\nthird");
        let tests = vec![
            (0, 0, "first line"),
            (0, 5, "first line"),
            (1, 3, "second line"),
            (2, 1, "third"),
        ];

        for (line, column, expected) in tests {
            let line_start_offset = rope.line_start_offset(line);
            let offset = line_start_offset + column;
            let range = TextSelector::line_range(&rope, offset);

            let actual = rope.slice(range).to_string();
            assert_eq!(actual, expected, "行 {}, 列 {}", line, column);
        }
    }
}
