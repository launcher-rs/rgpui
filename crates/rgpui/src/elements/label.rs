//! 标签组件，支持文本高亮、选择和自定义渲染的文本元素。

use std::ops::Range;

use crate::prelude::FluentBuilder;
use crate::{
    ActiveTheme, App, HighlightStyle, IntoElement, ParentElement, RenderOnce, SharedString,
    StyleRefinement, Styled, StyledExt, StyledText, Window, div, rems,
};

const MASKED: &'static str = "•";

/// 表示标签文本高亮的匹配类型。
#[derive(Clone)]
pub enum HighlightsMatch {
    /// 前缀匹配（仅匹配文本开头的搜索词）
    Prefix(SharedString),
    /// 全文匹配（匹配所有出现的位置）
    Full(SharedString),
}

impl HighlightsMatch {
    /// 返回匹配字符串。
    pub fn as_str(&self) -> &str {
        match self {
            Self::Prefix(s) => s.as_str(),
            Self::Full(s) => s.as_str(),
        }
    }

    /// 是否为前缀匹配。
    #[inline]
    pub fn is_prefix(&self) -> bool {
        matches!(self, Self::Prefix(_))
    }
}

impl From<&str> for HighlightsMatch {
    fn from(value: &str) -> Self {
        Self::Full(value.to_string().into())
    }
}

impl From<String> for HighlightsMatch {
    fn from(value: String) -> Self {
        Self::Full(value.into())
    }
}

impl From<SharedString> for HighlightsMatch {
    fn from(value: SharedString) -> Self {
        Self::Full(value)
    }
}

/// 文本标签元素，支持可选的次要文本、掩码与高亮功能。
#[derive(IntoElement)]
pub struct Label {
    /// 样式精炼
    style: StyleRefinement,
    /// 主标签文本
    label: SharedString,
    /// 次要文本
    secondary: Option<SharedString>,
    /// 是否掩码显示
    masked: bool,
    /// 高亮匹配文本
    highlights_text: Option<HighlightsMatch>,
}

impl Label {
    /// 创建带有主文本的新标签。
    pub fn new(label: impl Into<SharedString>) -> Self {
        let label: SharedString = label.into();
        Self {
            style: Default::default(),
            label,
            secondary: None,
            masked: false,
            highlights_text: None,
        }
    }

    /// 设置标签的次要文本，次要文本将以 `muted` 颜色显示在主文本之后。
    pub fn secondary(mut self, secondary: impl Into<SharedString>) -> Self {
        self.secondary = Some(secondary.into());
        self
    }

    /// 设置是否掩码显示标签文本。
    pub fn masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }

    /// 设置标签中要高亮的匹配文本。
    pub fn highlights(mut self, text: impl Into<HighlightsMatch>) -> Self {
        self.highlights_text = Some(text.into());
        self
    }

    fn full_text(&self) -> SharedString {
        match &self.secondary {
            Some(secondary) => format!("{} {}", self.label, secondary).into(),
            None => self.label.clone(),
        }
    }

    fn highlight_ranges(&self, total_length: usize) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();
        let full_text = self.full_text();

        if self.secondary.is_some() {
            ranges.push(0..self.label.len());
            ranges.push(self.label.len()..total_length);
        }

        if let Some(matched) = &self.highlights_text {
            let matched_str = matched.as_str();
            if !matched_str.is_empty() {
                let search_lower = matched_str.to_lowercase();
                let full_text_lower = full_text.to_lowercase();

                if matched.is_prefix() {
                    // 对于前缀匹配，只检查文本是否以搜索词开头
                    if full_text_lower.starts_with(&search_lower) {
                        ranges.push(0..matched_str.len());
                    }
                } else {
                    // 对于全文匹配，查找所有出现的位置
                    let mut search_start = 0;
                    while let Some(pos) = full_text_lower[search_start..].find(&search_lower) {
                        let match_start = search_start + pos;
                        let match_end = match_start + matched_str.len();

                        if match_end <= full_text.len() {
                            ranges.push(match_start..match_end);
                        }

                        search_start = match_start + 1;
                        while !full_text.is_char_boundary(search_start)
                            && search_start < full_text.len()
                        {
                            search_start += 1;
                        }

                        if search_start >= full_text.len() {
                            break;
                        }
                    }
                }
            }
        }

        ranges
    }

    fn measure_highlights(
        &self,
        length: usize,
        cx: &mut App,
    ) -> Option<Vec<(Range<usize>, HighlightStyle)>> {
        let ranges = self.highlight_ranges(length);
        if ranges.is_empty() {
            return None;
        }

        let mut highlights = Vec::new();
        let mut highlight_ranges_added = 0;

        if self.secondary.is_some() {
            highlights.push((ranges[0].clone(), HighlightStyle::default()));
            highlights.push((
                ranges[1].clone(),
                HighlightStyle {
                    color: Some(cx.theme().muted_foreground),
                    ..Default::default()
                },
            ));
            highlight_ranges_added = 2;
        }

        for range in ranges.iter().skip(highlight_ranges_added) {
            highlights.push((
                range.clone(),
                HighlightStyle {
                    color: Some(cx.theme().blue),
                    ..Default::default()
                },
            ));
        }

        Some(crate::combine_highlights(vec![], highlights).collect())
    }
}

impl Styled for Label {
    fn style(&mut self) -> &mut crate::StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Label {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut text = self.full_text();
        let chars_count = text.chars().count();

        if self.masked {
            text = SharedString::from(MASKED.repeat(chars_count))
        };

        let highlights = self.measure_highlights(text.len(), cx);

        div()
            .line_height(rems(1.25))
            .text_color(cx.theme().foreground)
            .refine_style(&self.style)
            .child(
                StyledText::new(&text).when_some(highlights, |this, hl| this.with_highlights(hl)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_ranges() {
        // 基本功能

        // 无高亮
        let label = Label::new("Hello World");
        let result = label.highlight_ranges("Hello World".len());
        assert_eq!(result, Vec::<Range<usize>>::new());

        // 仅次要文本区间
        let label = Label::new("Hello").secondary("World");
        let total_length = "Hello World".len();
        let result = label.highlight_ranges(total_length);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], 0..5); // "Hello"
        assert_eq!(result[1], 5..11); // " World"

        // 文本高亮

        // 不区分大小写的单个匹配
        let label = Label::new("Hello World").highlights("WORLD");
        let result = label.highlight_ranges("Hello World".len());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 6..11); // "World"

        // 多个匹配
        let label = Label::new("Hello Hello Hello").highlights("Hello");
        let result = label.highlight_ranges("Hello Hello Hello".len());
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], 0..5); // 第一个 "Hello"
        assert_eq!(result[1], 6..11); // 第二个 "Hello"
        assert_eq!(result[2], 12..17); // 第三个 "Hello"

        // 无匹配与空搜索词
        let label = Label::new("Hello World").highlights("xyz");
        let result = label.highlight_ranges("Hello World".len());
        assert_eq!(result, Vec::<Range<usize>>::new());

        let label = Label::new("Hello World").highlights("");
        let result = label.highlight_ranges("Hello World".len());
        assert_eq!(result, Vec::<Range<usize>>::new());

        // 组合功能

        // 次要文本 + 主文本高亮
        let label = Label::new("Hello").secondary("World").highlights("llo");
        let total_length = "Hello World".len();
        let result = label.highlight_ranges(total_length);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], 0..5); // 主文本区间
        assert_eq!(result[1], 5..11); // 次要文本区间
        assert_eq!(result[2], 2..5); // 主文本中的 "llo"

        // 次要文本中的高亮
        let label = Label::new("Hello").secondary("World").highlights("World");
        let total_length = "Hello World".len();
        let result = label.highlight_ranges(total_length);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], 0..5); // 主文本区间
        assert_eq!(result[1], 5..11); // 次要文本区间
        assert_eq!(result[2], 6..11); // 次要文本中的 "World"

        // 跨边界高亮
        let label = Label::new("Hello").secondary("World").highlights("o W");
        let total_length = "Hello World".len();
        let result = label.highlight_ranges(total_length);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], 0..5); // 主文本区间
        assert_eq!(result[1], 5..11); // 次要文本区间
        assert_eq!(result[2], 4..7); // 跨边界的 "o W"

        // 边界情况

        // 重叠匹配
        let label = Label::new("aaaa").highlights("aa");
        let result = label.highlight_ranges("aaaa".len());
        assert!(result.len() >= 2);
        assert_eq!(result[0], 0..2); // 第一个 "aa"
        assert_eq!(result[1], 1..3); // 重叠的 "aa"

        // Unicode 文本
        let label = Label::new("你好世界，Hello World").highlights("世界");
        let result = label.highlight_ranges("你好世界，Hello World".len());
        assert_eq!(result.len(), 1);
        let text = "你好世界，Hello World";
        let start = text.find("世界").unwrap();
        let end = start + "世界".len();
        assert_eq!(result[0], start..end);
    }

    #[test]
    fn test_highlight_ranges_prefix() {
        // 测试前缀匹配 - 应只匹配第一次出现
        let label = Label::new("aaaa").highlights(HighlightsMatch::Prefix("aa".into()));
        let result = label.highlight_ranges("aaaa".len());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 0..2); // 仅第一个 "aa"

        // 测试前缀与全文匹配行为
        let label_full =
            Label::new("Hello Hello").highlights(HighlightsMatch::Full("Hello".into()));
        let result_full = label_full.highlight_ranges("Hello Hello".len());
        assert_eq!(result_full.len(), 2); // 两个 "Hello" 都匹配

        let label_prefix =
            Label::new("Hello Hello").highlights(HighlightsMatch::Prefix("Hello".into()));
        let result_prefix = label_prefix.highlight_ranges("Hello Hello".len());
        assert_eq!(result_prefix.len(), 1); // 仅第一个 "Hello"
        assert_eq!(result_prefix[0], 0..5);

        // 测试不区分大小写的前缀匹配
        let label =
            Label::new("Hello hello HELLO").highlights(HighlightsMatch::Prefix("hello".into()));
        let result = label.highlight_ranges("Hello hello HELLO".len());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 0..5); // 第一个 "Hello"（不区分大小写）

        // 测试无匹配的前缀
        let label = Label::new("Hello World").highlights(HighlightsMatch::Prefix("xyz".into()));
        let result = label.highlight_ranges("Hello World".len());
        assert_eq!(result.len(), 0);

        // 测试空前缀
        let label = Label::new("Hello World").highlights(HighlightsMatch::Prefix("".into()));
        let result = label.highlight_ranges("Hello World".len());
        assert_eq!(result.len(), 0);

        // 带次要文本的前缀匹配 - 在主文本中匹配
        let label = Label::new("Hello")
            .secondary("Hello World")
            .highlights(HighlightsMatch::Prefix("Hello".into()));
        let total_length = "Hello Hello World".len();
        let result = label.highlight_ranges(total_length);
        assert_eq!(result.len(), 3); // 2 个次要文本 + 1 个前缀匹配
        assert_eq!(result[0], 0..5); // 主文本区间
        assert_eq!(result[1], 5..17); // 次要文本区间
        assert_eq!(result[2], 0..5); // 主文本中第一个 "Hello" 前缀匹配

        // 带次要文本的前缀匹配 - 跨边界不匹配
        let label = Label::new("abc")
            .secondary("def abc def")
            .highlights(HighlightsMatch::Prefix("abc".into()));
        let total_length = "abc def abc def".len();
        let result = label.highlight_ranges(total_length);
        assert_eq!(result.len(), 3); // 2 个次要文本 + 1 个前缀匹配
        assert_eq!(result[0], 0..3); // 主文本区间
        assert_eq!(result[1], 3..15); // 次要文本区间
        assert_eq!(result[2], 0..3); // "abc" 在全文开头匹配

        // 测试 Unicode 字符的前缀匹配
        let label = Label::new("你好世界你好").highlights(HighlightsMatch::Prefix("你好".into()));
        let result = label.highlight_ranges("你好世界你好".len());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 0..6); // 第一个 "你好"（UTF-8 中 6 字节）

        // 测试重叠模式的前缀匹配
        let label = Label::new("abababab").highlights(HighlightsMatch::Prefix("abab".into()));
        let result = label.highlight_ranges("abababab".len());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 0..4); // 仅第一个 "abab"

        // 前缀匹配在不同位置（"Hello" 不在开头则不匹配）
        let label =
            Label::new("xyz Hello abc Hello").highlights(HighlightsMatch::Prefix("Hello".into()));
        let result = label.highlight_ranges("xyz Hello abc Hello".len());
        assert_eq!(result.len(), 0); // 无匹配，因为 "Hello" 不在开头

        // 测试 is_prefix 方法
        let prefix_match = HighlightsMatch::Prefix("test".into());
        let full_match = HighlightsMatch::Full("test".into());
        assert!(prefix_match.is_prefix());
        assert!(!full_match.is_prefix());

        // 测试 as_str 方法
        let prefix_match = HighlightsMatch::Prefix("test".into());
        assert_eq!(prefix_match.as_str(), "test");
    }
}
