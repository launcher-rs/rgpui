//! 数字滚动计数器：每一位数字以滚轮方式滚动到目标值。

use std::time::Duration;

use rgpui::{prelude::FluentBuilder as _, *};

/// 数字滚动计数器组件。
///
/// 每一位数字是一列 0~9，切换到目标值时长列表整体滚动到对应位置。
#[derive(IntoElement)]
pub struct NumberTicker {
    _id: ElementId,
    base: Div,
    value: i64,
    separator: Option<char>,
    prefix: Option<SharedString>,
    suffix: Option<SharedString>,
    duration: Duration,
}

impl NumberTicker {
    /// 创建计数器，默认 600ms 滚动时长。
    pub fn new(id: impl Into<ElementId>, value: i64) -> Self {
        Self {
            _id: id.into(),
            base: div(),
            value,
            separator: None,
            prefix: None,
            suffix: None,
            duration: Duration::from_millis(600),
        }
    }

    /// 设置千分位分隔符（如 `,`）。
    pub fn separator(mut self, sep: char) -> Self {
        self.separator = Some(sep);
        self
    }

    /// 设置前缀文本。
    pub fn prefix(mut self, prefix: impl Into<SharedString>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// 设置后缀文本。
    pub fn suffix(mut self, suffix: impl Into<SharedString>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }

    /// 设置滚动时长。
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }
}

impl Styled for NumberTicker {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

/// 单个数字或分隔符。
#[derive(Clone, Copy)]
enum DigitOrSeparator {
    /// 数字。
    Digit(u8),
    /// 分隔符。
    Separator(char),
}

/// 把数字按千分位格式化为数字/分隔符序列。
fn format_with_separator(value: i64, separator: Option<char>) -> Vec<DigitOrSeparator> {
    let is_negative = value < 0;
    let abs_str = value.unsigned_abs().to_string();
    let mut result = Vec::new();

    if is_negative {
        result.push(DigitOrSeparator::Separator('-'));
    }

    let digits: Vec<u8> = abs_str.bytes().map(|b| b - b'0').collect();
    let len = digits.len();

    for (i, &digit) in digits.iter().enumerate() {
        result.push(DigitOrSeparator::Digit(digit));
        if let Some(sep) = separator {
            let remaining = len - 1 - i;
            if remaining > 0 && remaining % 3 == 0 {
                result.push(DigitOrSeparator::Separator(sep));
            }
        }
    }

    result
}

impl RenderOnce for NumberTicker {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let chars = format_with_separator(self.value, self.separator);
        let duration = self.duration;
        let digit_height = px(24.0);
        let column_height = digit_height * 10.0;

        self.base
            .flex()
            .flex_row()
            .items_center()
            .text_color(theme.tokens.foreground)
            .font_family(theme.mono_font_family.clone())
            .when_some(self.prefix.clone(), |el, prefix| {
                el.child(div().child(prefix))
            })
            .children(chars.iter().enumerate().map(move |(pos, item)| {
                match *item {
                    DigitOrSeparator::Separator(ch) => div()
                        .flex_shrink_0()
                        .child(SharedString::from(String::from(ch)))
                        .into_any_element(),
                    DigitOrSeparator::Digit(digit) => {
                        let target_offset = -(digit_height * digit as f32);

                        div()
                            .flex_shrink_0()
                            .h(digit_height)
                            .overflow_hidden()
                            .child(
                                div()
                                    .id(("digit-col", pos as u32))
                                    .flex()
                                    .flex_col()
                                    .h(column_height)
                                    .children((0..10u8).map(|d| {
                                        div()
                                            .h(digit_height)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(SharedString::from(d.to_string()))
                                    }))
                                    .with_animation(
                                        ("digit-roll", pos as u32),
                                        Animation::new(duration).with_easing(ease_out_cubic),
                                        move |el, delta| {
                                            let offset = target_offset * delta;
                                            el.top(offset)
                                        },
                                    ),
                            )
                            .into_any_element()
                    }
                }
            }))
            .when_some(self.suffix, |el, suffix| el.child(div().child(suffix)))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{DigitOrSeparator, format_with_separator};

    /// 验证千分位格式化：无分隔符时负数带负号。
    #[test]
    fn test_format_without_separator() {
        let chars = format_with_separator(-123, None);
        let mut kinds = chars.iter();
        assert!(matches!(
            kinds.next(),
            Some(DigitOrSeparator::Separator('-'))
        ));
        assert!(matches!(kinds.next(), Some(DigitOrSeparator::Digit(1))));
        assert!(matches!(kinds.next(), Some(DigitOrSeparator::Digit(2))));
        assert!(matches!(kinds.next(), Some(DigitOrSeparator::Digit(3))));
    }

    /// 验证千分位分隔符在正确位置插入。
    #[test]
    fn test_format_with_separator() {
        let chars = format_with_separator(1234567, Some(','));
        let kinds: Vec<u8> = chars
            .iter()
            .filter_map(|k| match k {
                DigitOrSeparator::Digit(d) => Some(*d),
                DigitOrSeparator::Separator(_) => None,
            })
            .collect();
        assert_eq!(kinds, vec![1, 2, 3, 4, 5, 6, 7]);
        // 共应有 2 个分隔符（千分位与百万位之间）
        let sep_count = chars
            .iter()
            .filter(|k| matches!(k, DigitOrSeparator::Separator(',')))
            .count();
        assert_eq!(sep_count, 2);
    }
}
