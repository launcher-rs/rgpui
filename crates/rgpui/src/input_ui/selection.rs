use std::ops::Range;

use rgpui::sum_tree::Bias;
use ropey::Rope;

use super::rope_ext::RopeExt as _;
use super::word_selection::word_range_from_chars;

/// 文本选择器辅助结构体。
///
/// 提供在给定文本中按单词或按行计算选择范围的方法。
struct TextSelector;

impl TextSelector {
    /// 在给定文本中选中指定偏移量所在的一行。
    ///
    /// 偏移量是 UTF-8 偏移量。
    ///
    /// 返回选中行的起始和结束偏移量。
    pub fn line_range(text: &Rope, offset: usize) -> Range<usize> {
        let offset = text.clip_offset(offset, Bias::Left);
        let row = text.offset_to_point(offset).row;
        let start = text.line_start_offset(row);
        let end = text.line_end_offset(row);

        start..end
    }

    /// 在给定文本中选中指定偏移量所在的单词。
    ///
    /// 偏移量是 UTF-8 偏移量。
    ///
    /// 返回选中单词的起始和结束偏移量。
    pub fn word_range(text: &Rope, offset: usize) -> Option<Range<usize>> {
        let offset = text.clip_offset(offset, Bias::Left);
        let Some(char) = text.char_at(offset) else {
            return None;
        };

        let end = offset + char.len_utf8();
        let prev_chars = text.chars_at(offset).reversed().take(128);
        let next_chars = text.chars_at(end).take(128);
        Some(word_range_from_chars(offset, char, prev_chars, next_chars))
    }
}

#[cfg(test)]
mod tests {
    use super::TextSelector;
    use crate::input_ui::RopeExt as _;
    use ropey::Rope;

    #[test]
    fn test_word_range() {
        let rope = Rope::from(
            "\ntest text:\nabcde 中文🎉 test\nhello[()]\ntest_connector ____\nRope\nrök\ngrande île\n",
        );

        let tests = vec![
            (1, 0, Some("test")),
            (1, 4, Some(" ")),
            (2, 0, Some("abcde")),
            (2, 4, Some("abcde")),
            (2, 5, Some(" ")),
            (2, 6, Some("中")),
            (2, 9, Some("文")),
            (2, 13, Some("🎉")),
            (2, 20, Some("test")),
            (3, 5, Some("[")),
            (3, 6, Some("(")),
            (3, 7, Some(")")),
            (3, 8, Some("]")),
            (4, 5, Some("test_connector")),
            (4, 14, Some(" ")),
            (4, 16, Some("____")),
            (5, 0, Some("Rope")),
            (6, 0, Some("rök")),
            (7, 8, Some("île")),
        ];

        for (line, column, expected) in tests {
            let line_start_offset = rope.line_start_offset(line);
            let offset = line_start_offset + column;
            let range = TextSelector::word_range(&rope, offset);

            let actual = range.map(|r| rope.slice(r).to_string());
            let expect = expected.map(|s| s.to_string());
            assert_eq!(actual, expect, "line {}, column {}", line, column);
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
            assert_eq!(actual, expected, "line {}, column {}", line, column);
        }
    }
}