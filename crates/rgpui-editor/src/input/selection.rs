use std::{char, ops::Range};

use crate::{RopeExt as _, input::InputState};
use rgpui::sum_tree::Bias;
use rgpui::{Context, Window};
use ropey::Rope;

/// 字符类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharType {
    /// a-z, A-Z, 0-9, _
    Word,
    /// '\t', ' ', '\u{00A0}' 等
    Whitespace,
    /// \n, \r
    Newline,
    /// . , ; : ( ) [ ] { } ... 或 CJK 字符：`汉`, `🎉` 等
    Other,
}

/// 实现参考 <https://github.com/zed-industries/zed/blob/main/crates/gpui/src/text_system/line_wrapper.rs>
fn is_word_char(c: char) -> bool {
    matches!(c, '_' ) ||
    // ASCII 字母数字字符，用于英语、数字：`Hello123` 等。
    c.is_ascii_alphanumeric() ||
    // Unicode 中的拉丁语系，用于法语、德语、西班牙语等。
    // Latin-1 Supplement
    // https://en.wikipedia.org/wiki/Latin-1_Supplement
    matches!(c, '\u{00C0}'..='\u{00FF}') ||
    // Latin Extended-A
    // https://en.wikipedia.org/wiki/Latin_Extended-A
    matches!(c, '\u{0100}'..='\u{017F}') ||
    // Latin Extended-B
    // https://en.wikipedia.org/wiki/Latin_Extended-B
    matches!(c, '\u{0180}'..='\u{024F}') ||
    // 西里尔字母，用于俄语、乌克兰语等。
    // https://en.wikipedia.org/wiki/Cyrillic_script_in_Unicode
    matches!(c, '\u{0400}'..='\u{04FF}') ||

    // 越南语 (https://vietunicode.sourceforge.net/charset/)
    matches!(c, '\u{1E00}'..='\u{1EFF}') || // Latin Extended Additional
    matches!(c, '\u{0300}'..='\u{036F}') // 组合变音符号
}

impl From<char> for CharType {
    fn from(c: char) -> Self {
        match c {
            c if is_word_char(c) => CharType::Word,
            c if c == '\n' || c == '\r' => CharType::Newline,
            c if c.is_whitespace() => CharType::Whitespace,
            _ => CharType::Other,
        }
    }
}

impl CharType {
    /// 检查两个 CharType 是否可连接
    fn is_connectable(self, c: char) -> bool {
        let other = CharType::from(c);
        match (self, other) {
            (CharType::Word, CharType::Word) => true,
            (CharType::Whitespace, CharType::Whitespace) => true,
            _ => false,
        }
    }
}

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
        let Some(char) = text.char_at(offset) else {
            return None;
        };

        let char_type = CharType::from(char);
        let mut start = offset;
        let mut end = offset + char.len_utf8();
        let prev_chars = text.chars_at(start).reversed().take(128);
        let next_chars = text.chars_at(end).take(128);

        for ch in prev_chars {
            if char_type.is_connectable(ch) {
                start -= ch.len_utf8();
            } else {
                break;
            }
        }

        for ch in next_chars {
            if char_type.is_connectable(ch) {
                end += ch.len_utf8();
            } else {
                break;
            }
        }

        Some(start..end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    #[test]
    fn test_char_type_from_char() {
        assert_eq!(CharType::from('a'), CharType::Word);
        assert_eq!(CharType::from('Z'), CharType::Word);
        assert_eq!(CharType::from('0'), CharType::Word);
        assert_eq!(CharType::from('_'), CharType::Word);
        assert_eq!(CharType::from('.'), CharType::Other);
        assert_eq!(CharType::from(','), CharType::Other);
        assert_eq!(CharType::from(';'), CharType::Other);
        assert_eq!(CharType::from('!'), CharType::Other);
        assert_eq!(CharType::from('?'), CharType::Other);
        assert_eq!(CharType::from('['), CharType::Other);
        assert_eq!(CharType::from('{'), CharType::Other);
        assert_eq!(CharType::from(' '), CharType::Whitespace);
        assert_eq!(CharType::from('\t'), CharType::Whitespace);
        assert_eq!(CharType::from('\u{00A0}'), CharType::Whitespace);
        assert_eq!(CharType::from('\n'), CharType::Newline);
        assert_eq!(CharType::from('\r'), CharType::Newline);
        assert_eq!(CharType::from('汉'), CharType::Other);
        // 欧洲字母
        assert_eq!(CharType::from('é'), CharType::Word);
        assert_eq!(CharType::from('ä'), CharType::Word);
        assert_eq!(CharType::from('ö'), CharType::Word);
        assert_eq!(CharType::from('ü'), CharType::Word);
        // 西里尔字母
        assert_eq!(CharType::from('д'), CharType::Word);
    }

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

