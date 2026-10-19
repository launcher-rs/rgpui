//! 东亚断行（禁则 / kinsoku）：CJK 文字之间无需空格即可断行，但断行点必须避开
//! 「行首禁则」与「行末禁则」字符，也不得拆开胶连字符。
//!
//! 拉丁文本靠空格切词，换行点天然落在空格处；CJK 没有空格，任何一个汉字之间都能
//! 断，于是「。、」这类标点会跑到行首、「「（」这类开括号会留在行末 —— 这正是
//! 中西文混排最容易看出来的排版错误。本模块给出判定规则，文本系统的换行逻辑
//! （`LineWrapper::wrap_line` 与 `LineLayout::compute_wrap_boundaries`）据此**否决**
//! 不合规的候选断点。
//!
//! 规则面向 CJK 与全角标点，**不改变 ASCII 标点的既有行为**：半角 `)` `]` `,` `.`
//! 仍按拉丁规则处理（`can_break_between('a', ')') == true`），避免影响既有英文排版。
//!
//! ```
//! use rgpui::line_break::{BreakClass, break_class, can_break_between, is_ideographic};
//!
//! // 汉字之间可以断行。
//! assert!(can_break_between('你', '好'));
//! // 句号不可以出现在行首（行首禁则）。
//! assert!(!can_break_between('好', '。'));
//! // 开引号不可以留在行末（行末禁则）。
//! assert!(!can_break_between('「', '好'));
//! // 胶连字符两侧都不可断。
//! assert!(!can_break_between('a', '\u{00A0}')); // 不换行空格
//!
//! assert_eq!(break_class('汉'), BreakClass::Ideographic);
//! assert_eq!(break_class('。'), BreakClass::ClosePunctuation);
//! assert!(is_ideographic('漢'));
//! ```

/// 字符在东亚断行规则中的类别。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BreakClass {
    /// 汉字（表意文字）：两侧任意可断。
    Ideographic,

    /// 假名（平假名 / 片假名 / 半角片假名）：两侧任意可断。
    Kana,

    /// 谚文（韩文音节）：两侧任意可断。
    Hangul,

    /// 小写假名与促音（`ぁぃぅ…っゃゅょゎ`、`ァィ…ッャュョヮ`）：行首禁则。
    SmallKana,

    /// 长音符与重复符号（`ー`、`々`、`〻`）：行首禁则。
    ProlongedSoundMark,

    /// 句读与闭括号（`。、）」』】`…）：行首禁则。
    ClosePunctuation,

    /// 开括号与开引号（`（「『【`…）：行末禁则。
    OpenPunctuation,

    /// 字母数字（拉丁、西里尔、希腊…）：单词内部不可断行。
    Word,

    /// 空白字符。
    Space,

    /// 其他：不附加任何断行约束的符号。
    Other,
}

/// 判断字符的断行类别。
pub fn break_class(c: char) -> BreakClass {
    use BreakClass::*;

    if c.is_whitespace() {
        Space
    } else if is_small_kana(c) {
        SmallKana
    } else if is_prolonged_sound_mark(c) {
        ProlongedSoundMark
    } else if is_close_punctuation(c) {
        ClosePunctuation
    } else if is_open_punctuation(c) {
        OpenPunctuation
    } else if is_ideographic(c) {
        Ideographic
    } else if is_kana(c) {
        Kana
    } else if is_hangul_syllable(c) {
        Hangul
    } else if c.is_alphanumeric() {
        Word
    } else {
        Other
    }
}

/// 行首禁则：这些字符不得出现在行首。
///
/// 含句读 / 闭括号 / 闭引号（[`BreakClass::ClosePunctuation`]）、小写假名与促音
/// （[`BreakClass::SmallKana`]）、长音符（[`BreakClass::ProlongedSoundMark`]）。
pub fn is_line_start_prohibited(c: char) -> bool {
    matches!(
        break_class(c),
        BreakClass::ClosePunctuation | BreakClass::SmallKana | BreakClass::ProlongedSoundMark
    )
}

/// 行末禁则：这些字符不得出现在行末（开括号 / 开引号之后必须还有内容）。
pub fn is_line_end_prohibited(c: char) -> bool {
    matches!(break_class(c), BreakClass::OpenPunctuation)
}

/// 胶连字符：与其相邻的位置都不可断行（不换行空格、词连接符、零宽连字…）。
pub fn is_no_break(c: char) -> bool {
    matches!(
        c,
        '\u{00A0}'      // 不换行空格 NBSP
            | '\u{2007}'    // 数字空格 FIGURE SPACE
            | '\u{2011}'    // 不换行连字符
            | '\u{202F}'    // 窄不换行空格
            | '\u{200C}'    // 零宽不连字 ZWNJ
            | '\u{200D}'    // 零宽连字 ZWJ
            | '\u{2060}'    // 词连接符 WORD JOINER
            | '\u{FEFF}' // 零宽不换行空格
    )
}

/// 是否允许在 `prev` 与 `next` 之间断行（东亚禁则 + 胶连字符）。
///
/// 返回 `false` 的三种情形：`prev` 是行末禁则字符、`next` 是行首禁则字符、
/// 两者中有一个是胶连字符。拉丁单词内部的断行判定不在此列（由文本系统的
/// `is_word_char` 负责），因此本函数只用于「非单词字符处」的候选断点。
pub fn can_break_between(prev: char, next: char) -> bool {
    !is_no_break(prev)
        && !is_no_break(next)
        && !is_line_end_prohibited(prev)
        && !is_line_start_prohibited(next)
}

/// 是否为表意文字（汉字及其扩展区、兼容区）。
pub fn is_ideographic(c: char) -> bool {
    is_han(c)
}

/// 汉字（含扩展区 A–G 与兼容表意文字）。
fn is_han(c: char) -> bool {
    matches!(c,
        '\u{3400}'..='\u{4DBF}'      // 扩展 A
            | '\u{4E00}'..='\u{9FFF}'    // 基本区
            | '\u{F900}'..='\u{FAFF}'    // 兼容表意文字
            | '\u{20000}'..='\u{2A6DF}'  // 扩展 B
            | '\u{2A700}'..='\u{2EBEF}'  // 扩展 C–F
            | '\u{2F800}'..='\u{2FA1F}'  // 兼容表意文字补充
            | '\u{30000}'..='\u{3134F}' // 扩展 G
    )
}

/// 假名（平假名 / 片假名 / 片假名语音扩展 / 半角片假名）。
fn is_kana(c: char) -> bool {
    matches!(c,
        '\u{3040}'..='\u{309F}'      // 平假名
            | '\u{30A0}'..='\u{30FF}'    // 片假名
            | '\u{31F0}'..='\u{31FF}'    // 片假名语音扩展
            | '\u{FF66}'..='\u{FF9D}' // 半角片假名
    )
}

/// 谚文音节（韩文预组合音节与兼容字母）。
fn is_hangul_syllable(c: char) -> bool {
    matches!(c,
        '\u{3130}'..='\u{318F}'      // 谚文兼容字母
            | '\u{A960}'..='\u{A97F}'    // 谚文扩展 A
            | '\u{AC00}'..='\u{D7AF}' // 谚文音节
    )
}

/// 小写假名与促音：行首禁则（`ぁぃぅぇぉっゃゅょゎゕゖ` 与对应片假名）。
fn is_small_kana(c: char) -> bool {
    matches!(
        c,
        'ぁ' | 'ぃ'
            | 'ぅ'
            | 'ぇ'
            | 'ぉ'
            | 'っ'
            | 'ゃ'
            | 'ゅ'
            | 'ょ'
            | 'ゎ'
            | 'ゕ'
            | 'ゖ'
            | 'ァ'
            | 'ィ'
            | 'ゥ'
            | 'ェ'
            | 'ォ'
            | 'ッ'
            | 'ャ'
            | 'ュ'
            | 'ョ'
            | 'ヮ'
            | 'ヵ'
            | 'ヶ'
            | 'ヷ'
            | 'ヸ'
            | 'ヹ'
            | 'ヺ'
    )
}

/// 长音符与重复符号：行首禁则。
fn is_prolonged_sound_mark(c: char) -> bool {
    matches!(
        c,
        'ー' | 'ｰ' | '〜' | '～' | '々' | '〻' | 'ゝ' | 'ゞ' | 'ヽ' | 'ヾ'
    )
}

/// 句读与闭括号：行首禁则。
fn is_close_punctuation(c: char) -> bool {
    matches!(
        c,
        '、' | '。'
            | '，'
            | '．'
            | '：'
            | '；'
            | '！'
            | '？'
            | '…'
            | '‥'
            | '・'
            | '）'
            | '］'
            | '｝'
            | '〉'
            | '》'
            | '」'
            | '』'
            | '】'
            | '〕'
            | '〗'
            | '〙'
            | '〛'
            | '｡'
            | '､'
            | '｣'
            | '･'
            | '％'
            | '°'
            | '′'
            | '″'
            | '℃'
            | '‰'
    )
}

/// 开括号与开引号：行末禁则。
fn is_open_punctuation(c: char) -> bool {
    matches!(
        c,
        '（' | '［' | '｛' | '〈' | '《' | '「' | '『' | '【' | '〔' | '〖' | '〘' | '〚' | '｢'
    )
}

#[cfg(test)]
mod tests {
    use super::{
        BreakClass, break_class, can_break_between, is_ideographic, is_line_end_prohibited,
        is_line_start_prohibited, is_no_break,
    };

    // 字符分类覆盖汉字 / 假名 / 小写假名 / 长音符 / 句读 / 开括号 / 谚文 / 单词 / 空白 / 其他。
    #[test]
    fn break_class_covers_cjk_and_latin() {
        assert_eq!(break_class('汉'), BreakClass::Ideographic);
        assert_eq!(break_class('お'), BreakClass::Kana);
        assert_eq!(break_class('ア'), BreakClass::Kana);
        assert_eq!(break_class('ぁ'), BreakClass::SmallKana);
        assert_eq!(break_class('ー'), BreakClass::ProlongedSoundMark);
        assert_eq!(break_class('。'), BreakClass::ClosePunctuation);
        assert_eq!(break_class('」'), BreakClass::ClosePunctuation);
        assert_eq!(break_class('「'), BreakClass::OpenPunctuation);
        assert_eq!(break_class('한'), BreakClass::Hangul);
        assert_eq!(break_class('a'), BreakClass::Word);
        assert_eq!(break_class('你'), BreakClass::Ideographic);
        assert_eq!(break_class(' '), BreakClass::Space);
        assert_eq!(break_class('+'), BreakClass::Other);
        assert_eq!(break_class('\n'), BreakClass::Space);
    }

    // 禁则集合：CJK 标点命中，ASCII 标点不受影响。
    #[test]
    fn kinsoku_sets_do_not_touch_ascii() {
        for c in [
            '。', '、', '，', '）', '」', '！', '？', 'ぁ', 'ッ', 'ー', '｡',
        ] {
            assert!(is_line_start_prohibited(c), "{c:?} 应属行首禁则");
        }
        for c in [')', ']', '.', ',', '!', '?', 'a', '1', '你'] {
            assert!(!is_line_start_prohibited(c), "{c:?} 不应属行首禁则");
        }
        for c in ['「', '『', '（', '【', '｢', '〈'] {
            assert!(is_line_end_prohibited(c), "{c:?} 应属行末禁则");
        }
        for c in ['(', '[', 'a', '你', '」'] {
            assert!(!is_line_end_prohibited(c), "{c:?} 不应属行末禁则");
        }
    }

    // 断行判定：汉字之间可断，禁则两侧不可断，胶连字符两侧不可断。
    #[test]
    fn can_break_between_applies_kinsoku_and_glue() {
        // 汉字 / 假名 / 谚文之间可断。
        assert!(can_break_between('你', '好'));
        assert!(can_break_between('あ', 'い'));
        assert!(can_break_between('안', '녕'));
        assert!(can_break_between('。', '你'));
        assert!(can_break_between('」', '你'));

        // 行首禁则：`。`、`」`、小写假名、长音符不可起行。
        assert!(!can_break_between('好', '。'));
        assert!(!can_break_between('话', '」'));
        assert!(!can_break_between('あ', 'ぁ'));
        assert!(!can_break_between('カ', 'ー'));

        // 行末禁则：开括号 / 开引号之后不可断。
        assert!(!can_break_between('「', '你'));
        assert!(!can_break_between('（', '好'));
        assert!(!can_break_between('【', '注'));

        // 胶连字符：两侧都不可断。
        assert!(!can_break_between('a', '\u{00A0}'));
        assert!(!can_break_between('\u{00A0}', 'b'));
        assert!(!can_break_between('你', '\u{200D}'));
        assert!(!can_break_between('\u{200D}', '你'));

        // ASCII 标点保持既有行为（半角按拉丁规则处理）。
        assert!(can_break_between('a', ')'));
        assert!(can_break_between('a', ','));
        assert!(can_break_between('(', 'a'));
    }

    // 胶连字符集合。
    #[test]
    fn glue_characters() {
        assert!(is_no_break('\u{00A0}'));
        assert!(is_no_break('\u{2011}'));
        assert!(is_no_break('\u{200D}'));
        assert!(is_no_break('\u{2060}'));
        assert!(!is_no_break(' '));
        assert!(!is_no_break('\n'));
        assert!(!is_no_break('\u{200B}'), "零宽空格是断行点，不是胶连字符");
    }

    // 表意文字范围（基本区 / 扩展区 / 兼容区）。
    #[test]
    fn ideographic_ranges() {
        assert!(is_ideographic('你'));
        assert!(is_ideographic('龍'));
        assert!(is_ideographic('\u{3400}'));
        assert!(is_ideographic('\u{20000}'));
        assert!(!is_ideographic('あ'));
        assert!(!is_ideographic('한'));
        assert!(!is_ideographic('a'));
    }
}
