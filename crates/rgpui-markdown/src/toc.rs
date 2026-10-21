//! 文档大纲：提取 Markdown 标题（级别 / 文本 / 源码区间 / 锚点）。
//!
//! 供阅读器渲染目录（TOC）、做「点击标题跳转」与生成锚点链接使用。

use std::collections::HashMap;
use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// 大纲条目（一个标题）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// 标题级别（1–6）。
    pub level: u8,
    /// 标题纯文本（已去掉行内标记）。
    pub text: String,
    /// 标题在源码中的字节区间（含 `#` 前缀与行尾换行）。
    pub range: Range<usize>,
    /// 锚点 slug（同名标题依次追加 `-1`、`-2` 去重）。
    pub anchor: String,
}

/// 提取文档大纲（按出现顺序）。
///
/// 标题文本取行内纯文本（`**加粗**` 之类标记被去掉）；锚点 slug 保留字母、数字与
/// CJK，其余字符转连字符；同名标题的锚点依次追加 `-1`、`-2`。
pub fn extract_headings(source: &str) -> Vec<Heading> {
    let parser = Parser::new_ext(source, Options::empty()).into_offset_iter();
    let mut headings: Vec<Heading> = Vec::new();
    let mut current: Option<(u8, String, Range<usize>)> = None;
    let mut counts: HashMap<String, usize> = HashMap::new();

    for (event, range) in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some((heading_level_to_u8(&level), String::new(), range));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text, range)) = current.take() {
                    let anchor = unique_anchor(&text, &mut counts);
                    headings.push(Heading {
                        level,
                        text,
                        range,
                        anchor,
                    });
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, buffer, _)) = current.as_mut() {
                    buffer.push_str(&text);
                }
            }
            Event::SoftBreak => {
                if let Some((_, buffer, _)) = current.as_mut() {
                    buffer.push(' ');
                }
            }
            _ => {}
        }
    }

    headings
}

/// 标题级别转数字（1–6）。
fn heading_level_to_u8(level: &pulldown_cmark::HeadingLevel) -> u8 {
    match level {
        pulldown_cmark::HeadingLevel::H1 => 1,
        pulldown_cmark::HeadingLevel::H2 => 2,
        pulldown_cmark::HeadingLevel::H3 => 3,
        pulldown_cmark::HeadingLevel::H4 => 4,
        pulldown_cmark::HeadingLevel::H5 => 5,
        pulldown_cmark::HeadingLevel::H6 => 6,
    }
}

/// 生成锚点 slug：小写，空白与标点转连字符，保留字母数字与 CJK。
fn slugify(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// 生成去重后的锚点（同名标题追加 `-1`、`-2`）。
fn unique_anchor(text: &str, counts: &mut HashMap<String, usize>) -> String {
    let base = slugify(text);
    let count = counts.entry(base.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base
    } else {
        format!("{base}-{}", *count - 1)
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::extract_headings;

    /// 提取标题：级别 / 文本 / 区间 / 锚点。
    #[test]
    fn extracts_headings_with_levels_and_anchors() {
        let source = "# 标题一\n\n正文\n\n## 标题 二\n\n### 标题三\n";
        let headings = extract_headings(source);
        assert_eq!(headings.len(), 3);
        assert_eq!(headings[0].level, 1);
        assert_eq!(headings[0].text, "标题一");
        assert_eq!(headings[1].level, 2);
        assert_eq!(headings[1].text, "标题 二");
        assert_eq!(headings[2].level, 3);
        // 锚点：小写 + 连字符。
        assert_eq!(headings[0].anchor, "标题一");
        assert_eq!(headings[1].anchor, "标题-二");
        // 区间指向源码中的标题行。
        // 区间指向源码中的标题行（含 `#` 前缀与行尾换行）。
        assert_eq!(&source[headings[1].range.clone()], "## 标题 二\n");
    }

    /// 行内标记被去掉（标题文本取纯文本）。
    #[test]
    fn heading_text_strips_inline_marks() {
        let headings = extract_headings("# **加粗** 与 `代码`\n");
        assert_eq!(headings.len(), 1);
        assert_eq!(headings[0].text, "加粗 与 代码");
    }

    /// 同名标题的锚点去重。
    #[test]
    fn duplicate_anchors_are_suffixed() {
        let headings = extract_headings("# Overview\n\n## Overview\n\n### Overview\n");
        assert_eq!(headings[0].anchor, "overview");
        assert_eq!(headings[1].anchor, "overview-1");
        assert_eq!(headings[2].anchor, "overview-2");
    }

    /// 无标题时返回空列表。
    #[test]
    fn no_headings_returns_empty() {
        assert!(extract_headings("只有正文\n").is_empty());
    }
}
