//! 双向文字（bidi）：段落基准方向的解析规则。
//!
//! 完整 UBA（Unicode 双向算法）的**字形重排**属于平台 shaping 的职责：cosmic-text
//! 后端内部已用 `unicode-bidi` 完成重排，Windows DirectWrite 后端把整行交给
//! `IDWriteTextLayout`（DirectWrite 自己做 bidi）。核心层真正缺的是**段落基准方向**：
//! 谁来回答「这一段文字属于 LTR 还是 RTL」——它决定 `start` / `end` 对齐往哪边靠、
//! 行首行尾标点落在哪一侧，应用外壳（滚动条侧、图标方向）也随之翻转。
//!
//! 方向判定采用 HTML `dir=auto` 的口径（UBA 的 P2 / P3 规则）：取**首个强方向字符**
//! （`L` / `R` / `AL`，见 [`strong_direction`]），全段没有强方向字符时按 LTR 处理 ——
//! 所以「纯阿拉伯数字」段落与浏览器一致地回退为 LTR，而不是让人误以为它是 RTL。
//!
//! ```
//! use rgpui::TextAlign;
//! use rgpui::bidi::TextDirection;
//!
//! assert_eq!(TextDirection::resolve("Hello world"), TextDirection::Ltr);
//! assert_eq!(TextDirection::resolve("שלום עולם"), TextDirection::Rtl);
//! assert_eq!(TextDirection::resolve("مَرْحَبًا"), TextDirection::Rtl);
//! // 数字是弱方向字符，跳过它们继续找首个强方向字符。
//! assert_eq!(TextDirection::resolve("123 שלום"), TextDirection::Rtl);
//! assert_eq!(TextDirection::resolve("123456"), TextDirection::Ltr);
//! // `start` / `end` 随着段落方向翻转。
//! assert_eq!(TextAlign::Start.resolve(TextDirection::Ltr), TextAlign::Left);
//! assert_eq!(TextAlign::Start.resolve(TextDirection::Rtl), TextAlign::Right);
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 段落基准方向（LTR / RTL）。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum TextDirection {
    /// 从左到右（拉丁、希腊、西里尔、CJK、谚文…）。
    #[default]
    Ltr,

    /// 从右到左（希伯来、阿拉伯、叙利亚、它拿、曼达、阿德拉姆…）。
    Rtl,
}

impl TextDirection {
    /// 解析文本的段落基准方向（HTML `dir=auto` 口径：首个强方向字符；找不到则为 LTR）。
    ///
    /// 纯 ASCII 文本恒为 [`Ltr`][TextDirection::Ltr]，走快速路径不做逐字符查表。
    pub fn resolve(text: &str) -> Self {
        if text.is_ascii() {
            return TextDirection::Ltr;
        }
        text.chars()
            .find_map(strong_direction)
            .unwrap_or(TextDirection::Ltr)
    }

    /// 是否为从右到左方向。
    pub fn is_rtl(self) -> bool {
        matches!(self, TextDirection::Rtl)
    }

    /// 语言标签的默认段落方向（`"ar-EG"` → RTL，`"zh-CN"` → LTR）。
    ///
    /// 供 i18n 层在「没有具体文本可判定」时给出基准方向，例如空输入框、
    /// 整段数字，或外壳（滚动条侧、图标方向）需要按语言翻转的场合。
    pub fn for_locale(locale: &str) -> Self {
        if is_rtl_locale(locale) {
            TextDirection::Rtl
        } else {
            TextDirection::Ltr
        }
    }
}

/// 字符的强方向（UBA 的 `L` / `R` / `AL` 三类）。弱方向与中性字符返回 `None`。
///
/// 数字（`EN` / `AN`）、标点、空白与 bidi 控制符都**不是**强方向字符 ——
/// 「123」与「٣٤٥」都不会决定段落方向，这正是 HTML `dir=auto` 的行为。
/// 方向标记 `LRM` / `RLM` / `ALM` 在 UBA 中分别属于 `L` / `R` / `AL`，
/// 因此这里由字符表自然得出，无需特例。
pub fn strong_direction(c: char) -> Option<TextDirection> {
    match unicode_bidi::bidi_class(c) {
        unicode_bidi::BidiClass::L => Some(TextDirection::Ltr),
        unicode_bidi::BidiClass::R | unicode_bidi::BidiClass::AL => Some(TextDirection::Rtl),
        _ => None,
    }
}

/// 字符是否属于 RTL 文字（`R` / `AL` 类：希伯来、阿拉伯、叙利亚…）。
pub fn is_rtl_char(c: char) -> bool {
    matches!(strong_direction(c), Some(TextDirection::Rtl))
}

/// 语言标签的主语言子标签（`"ar-EG"` / `"ar_EG"` → `"ar"`）。
///
/// 只取第一个子标签，不解析书写系统子标签（如 `"az-Arab"` 仍按 `"az"` 判为 LTR）——
/// 那种场合应由应用显式传入方向，而不是猜。
pub fn primary_language(locale: &str) -> &str {
    locale.split(['-', '_']).next().unwrap_or(locale)
}

/// 该语言是否默认从右到左书写（CLDR 常用 RTL 语言，非穷举）。
///
/// 只看主语言子标签、大小写不敏感：`"ar"`、`"ar-EG"`、`"AR_eg"` 都是 RTL。
/// 覆盖阿拉伯语、希伯来语、波斯语、乌尔都语、普什图语、库尔德语（索拉尼）、
/// 信德语、维吾尔语、迪维希语、叙利亚语、恩科语、意第绪语等常用 RTL 语言；
/// 历史代码 `iw`（希伯来语）/ `ji`（意第绪语）一并认。
pub fn is_rtl_locale(locale: &str) -> bool {
    const RTL_LANGUAGES: &[&str] = &[
        "ar", "arc", "ckb", "dv", "fa", "he", "iw", "ji", "khw", "ks", "nqo", "prs", "ps", "sd",
        "skr", "syr", "ug", "ur", "yi",
    ];
    let primary = primary_language(locale);
    RTL_LANGUAGES
        .iter()
        .any(|language| language.eq_ignore_ascii_case(primary))
}

#[cfg(test)]
mod tests {
    use super::{TextDirection, strong_direction};
    use crate::TextAlign;

    // 基准方向取首个强方向字符；拉丁 / 希腊 / 西里尔 / CJK 都是 LTR。
    #[test]
    fn resolve_takes_first_strong_character() {
        assert_eq!(TextDirection::resolve(""), TextDirection::Ltr);
        assert_eq!(TextDirection::resolve("Hello"), TextDirection::Ltr);
        assert_eq!(TextDirection::resolve("Привет"), TextDirection::Ltr);
        assert_eq!(TextDirection::resolve("你好世界"), TextDirection::Ltr);
        assert_eq!(TextDirection::resolve("שלום"), TextDirection::Rtl);
        assert_eq!(TextDirection::resolve("مرحبا"), TextDirection::Rtl);
        // 首个强方向字符是中性的标点 / 数字时继续往后找。
        assert_eq!(TextDirection::resolve("«שלום»"), TextDirection::Rtl);
        assert_eq!(TextDirection::resolve("12 שלום"), TextDirection::Rtl);
        // 混合段落：先出现的强方向字符说了算。
        assert_eq!(TextDirection::resolve("abc שלום"), TextDirection::Ltr);
        assert_eq!(TextDirection::resolve("שלום abc"), TextDirection::Rtl);
    }

    // 弱方向字符（数字）不决定方向：与 HTML `dir=auto` 一致地回退为 LTR。
    #[test]
    fn weak_characters_do_not_decide_direction() {
        assert_eq!(TextDirection::resolve("123456"), TextDirection::Ltr);
        assert_eq!(TextDirection::resolve("٣٤٥"), TextDirection::Ltr);
        assert_eq!(TextDirection::resolve("12.5%"), TextDirection::Ltr);
        assert_eq!(TextDirection::resolve("！？…"), TextDirection::Ltr);
        assert!(strong_direction('1').is_none());
        assert!(strong_direction('٣').is_none());
    }

    // 方向标记（LRM / RLM / ALM）在 UBA 中即为强方向字符。
    #[test]
    fn direction_marks_are_strong() {
        assert_eq!(strong_direction('\u{200E}'), Some(TextDirection::Ltr));
        assert_eq!(strong_direction('\u{200F}'), Some(TextDirection::Rtl));
        assert_eq!(strong_direction('\u{061C}'), Some(TextDirection::Rtl));
        // 零宽连接符之类的中性字符不决定方向。
        assert!(strong_direction('\u{200D}').is_none());
        assert!(strong_direction(' ').is_none());
    }

    // `start` / `end` 按段落方向解析成绝对方向；`left` / `center` / `right` 原样返回。
    #[test]
    fn start_and_end_flip_with_direction() {
        let ltr = TextDirection::Ltr;
        let rtl = TextDirection::Rtl;

        assert_eq!(TextAlign::Start.resolve(ltr), TextAlign::Left);
        assert_eq!(TextAlign::Start.resolve(rtl), TextAlign::Right);
        assert_eq!(TextAlign::End.resolve(ltr), TextAlign::Right);
        assert_eq!(TextAlign::End.resolve(rtl), TextAlign::Left);
        assert_eq!(TextAlign::Center.resolve(rtl), TextAlign::Center);
        assert_eq!(TextAlign::Left.resolve(rtl), TextAlign::Left);
        assert_eq!(TextAlign::Right.resolve(ltr), TextAlign::Right);
    }

    // RTL 字符判定：希伯来 / 阿拉伯字母为真，拉丁 / CJK 与标点为假。
    #[test]
    fn rtl_char_detection() {
        use super::is_rtl_char;

        assert!(is_rtl_char('ש'));
        assert!(is_rtl_char('م'));
        assert!(is_rtl_char('ܐ'));
        assert!(!is_rtl_char('a'));
        assert!(!is_rtl_char('你'));
        assert!(!is_rtl_char('.'));
        assert!(!is_rtl_char('1'));
        assert_eq!(TextDirection::Rtl.is_rtl(), true);
        assert_eq!(TextDirection::Ltr.is_rtl(), false);
    }

    // 语言标签 → 默认方向：只看主语言子标签、大小写不敏感。
    #[test]
    fn locale_direction_lookup() {
        use super::{is_rtl_locale, primary_language};

        assert_eq!(TextDirection::for_locale("ar"), TextDirection::Rtl);
        assert_eq!(TextDirection::for_locale("ar-EG"), TextDirection::Rtl);
        assert_eq!(TextDirection::for_locale("AR_eg"), TextDirection::Rtl);
        assert_eq!(TextDirection::for_locale("he-IL"), TextDirection::Rtl);
        assert_eq!(TextDirection::for_locale("fa-IR"), TextDirection::Rtl);
        assert_eq!(TextDirection::for_locale("ur-PK"), TextDirection::Rtl);
        assert_eq!(TextDirection::for_locale("ckb-IQ"), TextDirection::Rtl);
        // 历史代码仍然认。
        assert!(is_rtl_locale("iw"));
        assert!(is_rtl_locale("ji"));

        assert_eq!(TextDirection::for_locale("zh-CN"), TextDirection::Ltr);
        assert_eq!(TextDirection::for_locale("ja-JP"), TextDirection::Ltr);
        assert_eq!(TextDirection::for_locale("en-US"), TextDirection::Ltr);
        assert_eq!(TextDirection::for_locale(""), TextDirection::Ltr);

        assert_eq!(primary_language("zh-Hant-TW"), "zh");
        assert_eq!(primary_language("ar_EG"), "ar");
        assert_eq!(primary_language(""), "");
    }
}
