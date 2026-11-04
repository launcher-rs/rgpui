use std::fmt::{Display, Formatter};

/// 从标题文本生成 URL 友好的 slug（例如 "Hello World" → "hello-world"）。
pub fn generate_heading_slug(text: &str) -> String {
    text.trim()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                Some(c.to_lowercase().next().unwrap_or(c))
            } else if c == ' ' {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

/// 如果 URL 以 URI 方案开头则返回 true（RFC 3986 §3.1）。
fn has_uri_scheme(url: &str) -> bool {
    let mut chars = url.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    for c in chars {
        if c == ':' {
            return true;
        }
        if !(c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
            return false;
        }
    }
    false
}

/// 将相对 URL 分割为其路径和 `#fragment` 部分。
/// 绝对 URL 按原样返回，没有 fragment。
pub fn split_local_url_fragment(url: &str) -> (&str, Option<&str>) {
    if has_uri_scheme(url) {
        return (url, None);
    }
    match url.find('#') {
        Some(pos) => {
            let path = &url[..pos];
            let fragment = &url[pos + 1..];
            (
                path,
                if fragment.is_empty() {
                    None
                } else {
                    Some(fragment)
                },
            )
        }
        None => (url, None),
    }
}

/// 表示包装的 `String` 是 markdown 文本。
#[derive(Debug, Clone)]
pub struct MarkdownString(pub String);

impl Display for MarkdownString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 转义 markdown 文本块中的 markdown 特殊字符。Markdown 代码块遵循
/// 不同的规则，在这种情况下应使用 `MarkdownInlineCode` 或 `MarkdownCodeBlock`。
///
/// 行内记号一律转义：`\`、`` ` ``、`*`、`_`、`[`、`^`（上标）、`$`（行内数学）、
/// `~`（删除线）、`&`；`<` / `>` 改成 HTML 实体（避免被当成标签或引用块）。
///
/// 块级记号只在**行首**（其前只有空白）才有语法意义，所以 `#`、`+`、`-`、`=`
/// 只在该位置转义，行内的连字符、加号保持原样（更干净，且不影响语义）。
/// 行首「数字 + `.` / `)`」是有序列表记号，点/括号转义成 `1\.` / `1\)`，
/// 否则 `1. 文本` 会被解析成列表项。
///
/// 某些字符的转义是不必要的，因为虽然它们参与 markdown 语法，
/// 但参与的其他字符已被转义：
///
/// * `!`、`]`、`(` 和 `)` 用于链接语法，但 `[` 被转义，因此它们被解析为纯文本。
///
/// * `;` 用于 HTML 实体语法，但 `&` 被转义，因此它们被解析为纯文本。
pub struct MarkdownEscaped<'a>(pub &'a str);

/// 实现 `Display` 以格式化 markdown 行内代码（用反引号包裹），处理包含
/// 反引号和空格的代码。所有空白字符都被视为单个空格字符。对于不包含
/// 除 ' ' 之外的空白字符的文本，此转义通过 pulldown-cmark 往返。
///
/// 在表中使用时，应在提供给此函数的文本中将 `|` 转义为 `\|`。
pub struct MarkdownInlineCode<'a>(pub &'a str);

/// 实现 `Display` 以格式化 markdown 代码块，根据需要用 3 个或更多反引号包裹。
pub struct MarkdownCodeBlock<'a> {
    pub tag: &'a str,
    pub text: &'a str,
}

impl Display for MarkdownEscaped<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let mut out = String::with_capacity(self.0.len() + 8);
        // 行首状态：块级记号（`#` `+` `-` `=` 与 `1.`）只在行首有语法意义。
        let mut line_start = true;
        // 行首连续数字：紧跟的 `.` / `)` 是有序列表记号。
        let mut digit_run = false;
        for c in self.0.chars() {
            match c {
                '\n' => {
                    out.push(c);
                    line_start = true;
                    digit_run = false;
                }
                // 行内记号：任何位置都要转义。
                '\\' | '`' | '*' | '_' | '[' | '^' | '$' | '~' | '&' => {
                    out.push('\\');
                    out.push(c);
                    line_start = false;
                    digit_run = false;
                }
                // `<` / `>` 用 HTML 实体：既能当纯文本，也不会被当成标签或引用块。
                '<' => {
                    out.push_str("&lt;");
                    line_start = false;
                    digit_run = false;
                }
                '>' => {
                    out.push_str("&gt;");
                    line_start = false;
                    digit_run = false;
                }
                // 块级记号：只在行首（前面只有空白）成语法，行内保持原样。
                '#' | '+' | '=' | '-' if line_start => {
                    out.push('\\');
                    out.push(c);
                    line_start = false;
                    digit_run = false;
                }
                // 行首 `1.` / `1)` 会被解析成有序列表，转义点/括号。
                '.' | ')' if digit_run => {
                    out.push('\\');
                    out.push(c);
                    line_start = false;
                    digit_run = false;
                }
                c if line_start && c.is_whitespace() => {
                    out.push(c);
                }
                c if line_start && c.is_ascii_digit() => {
                    out.push(c);
                    digit_run = true;
                }
                c => {
                    out.push(c);
                    line_start = false;
                    digit_run = false;
                }
            }
        }
        formatter.write_str(&out)
    }
}

impl Display for MarkdownInlineCode<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        // Apache License 2.0, same as this crate.
        //
        // Copied from `pulldown-cmark-to-cmark-20.0.0` with modifications:
        //
        // * Handling of all whitespace. pulldown-cmark-to-cmark is anticipating
        // `Code` events parsed by pulldown-cmark.
        //
        // https://github.com/Byron/pulldown-cmark-to-cmark/blob/3c850de2d3d1d79f19ca5f375e1089a653cf3ff7/src/lib.rs#L290

        let mut all_whitespace = true;
        let text = self
            .0
            .chars()
            .map(|c| {
                if c.is_whitespace() {
                    ' '
                } else {
                    all_whitespace = false;
                    c
                }
            })
            .collect::<String>();

        // When inline code has leading and trailing ' ' characters, additional space is needed
        // to escape it, unless all characters are space.
        if all_whitespace {
            write!(formatter, "`{text}`")
        } else {
            // More backticks are needed to delimit the inline code than the maximum number of
            // backticks in a consecutive run.
            let backticks = "`".repeat(count_max_consecutive_chars(&text, '`') + 1);
            let space = match text.as_bytes() {
                &[b'`', ..] | &[.., b'`'] => " ", // Space needed to separate backtick.
                &[b' ', .., b' '] => " ",         // Space needed to escape inner space.
                _ => "",                          // No space needed.
            };
            write!(formatter, "{backticks}{space}{text}{space}{backticks}")
        }
    }
}

impl Display for MarkdownCodeBlock<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let tag = self.tag;
        let text = self.text;
        let backticks = "`".repeat(3.max(count_max_consecutive_chars(text, '`') + 1));
        write!(formatter, "{backticks}{tag}\n{text}\n{backticks}\n")
    }
}

// Copied from `pulldown-cmark-to-cmark-20.0.0` with changed names.
// https://github.com/Byron/pulldown-cmark-to-cmark/blob/3c850de2d3d1d79f19ca5f375e1089a653cf3ff7/src/lib.rs#L1063
// Apache License 2.0, same as this code.
fn count_max_consecutive_chars(text: &str, search: char) -> usize {
    let mut in_search_chars = false;
    let mut max_count = 0;
    let mut cur_count = 0;

    for ch in text.chars() {
        if ch == search {
            cur_count += 1;
            in_search_chars = true;
        } else if in_search_chars {
            max_count = max_count.max(cur_count);
            cur_count = 0;
            in_search_chars = false;
        }
    }
    max_count.max(cur_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_escaped() {
        let input = r#"
        # Heading

        Another heading
        ===

        Another heading variant
        ---

        Paragraph with [link](https://example.com) and `code`, *emphasis*, and ~strikethrough~.

        ```
        code block
        ```

        List with varying leaders:
          - Item 1
          * Item 2
          + Item 3

        Some math:  $`\sqrt{3x-1}+(1+x)^2`$

        HTML entity: &nbsp;
        "#;

        let expected = r#"
        \# Heading

        Another heading
        \===

        Another heading variant
        \---

        Paragraph with \[link](https://example.com) and \`code\`, \*emphasis\*, and \~strikethrough\~.

        \`\`\`
        code block
        \`\`\`

        List with varying leaders:
          \- Item 1
          \* Item 2
          \+ Item 3

        Some math:  \$\`\\sqrt{3x-1}+(1+x)\^2\`\$

        HTML entity: \&nbsp;
        "#;

        assert_eq!(MarkdownEscaped(input).to_string(), expected);
    }

    /// 块级记号只在行首转义；行内的连字符/加号保持原样。
    #[test]
    fn test_markdown_escaped_only_at_line_start() {
        assert_eq!(
            MarkdownEscaped("a - b + c # d = e").to_string(),
            "a - b + c # d = e",
            "行内的块级记号没有语法意义，不该被转义"
        );
        assert_eq!(
            MarkdownEscaped("  # Heading").to_string(),
            "  \\# Heading",
            "前导空白后仍是行首"
        );
        assert_eq!(MarkdownEscaped("- item").to_string(), "\\- item");
        assert_eq!(MarkdownEscaped("=").to_string(), "\\=");
        assert_eq!(
            MarkdownEscaped("line\n# next").to_string(),
            "line\n\\# next",
            "换行后重新回到行首状态"
        );
    }

    /// 行首「数字 + `.` / `)`」按有序列表转义，避免被解析成列表项。
    #[test]
    fn test_markdown_escaped_ordered_list_marker() {
        assert_eq!(MarkdownEscaped("1. step").to_string(), "1\\. step");
        assert_eq!(MarkdownEscaped("12) step").to_string(), "12\\) step");
        assert_eq!(
            MarkdownEscaped("v1.2.3").to_string(),
            "v1.2.3",
            "不在行首的数字点号是普通文本"
        );
        assert_eq!(
            MarkdownEscaped("1.5 倍").to_string(),
            "1\\.5 倍",
            "行首小数也按列表记号处理"
        );
    }

    #[test]
    fn test_markdown_inline_code() {
        assert_eq!(MarkdownInlineCode(" ").to_string(), "` `");
        assert_eq!(MarkdownInlineCode("text").to_string(), "`text`");
        assert_eq!(MarkdownInlineCode("text ").to_string(), "`text `");
        assert_eq!(MarkdownInlineCode(" text ").to_string(), "`  text  `");
        assert_eq!(MarkdownInlineCode("`").to_string(), "`` ` ``");
        assert_eq!(MarkdownInlineCode("``").to_string(), "``` `` ```");
        assert_eq!(MarkdownInlineCode("`text`").to_string(), "`` `text` ``");
        assert_eq!(
            MarkdownInlineCode("some `text` no leading or trailing backticks").to_string(),
            "``some `text` no leading or trailing backticks``"
        );
    }

    #[test]
    fn test_count_max_consecutive_chars() {
        assert_eq!(
            count_max_consecutive_chars("``a```b``", '`'),
            3,
            "the highest seen consecutive segment of backticks counts"
        );
        assert_eq!(
            count_max_consecutive_chars("```a``b`", '`'),
            3,
            "it can't be downgraded later"
        );
    }

    #[test]
    fn test_split_local_url_fragment() {
        assert_eq!(split_local_url_fragment("#heading"), ("", Some("heading")));
        assert_eq!(
            split_local_url_fragment("./file.md#heading"),
            ("./file.md", Some("heading"))
        );
        assert_eq!(split_local_url_fragment("./file.md"), ("./file.md", None));
        assert_eq!(
            split_local_url_fragment("https://example.com#frag"),
            ("https://example.com#frag", None)
        );
        assert_eq!(
            split_local_url_fragment("mailto:user@example.com"),
            ("mailto:user@example.com", None)
        );
        assert_eq!(split_local_url_fragment("#"), ("", None));
        assert_eq!(
            split_local_url_fragment("../other.md#section"),
            ("../other.md", Some("section"))
        );
        assert_eq!(
            split_local_url_fragment("123:not-a-scheme#frag"),
            ("123:not-a-scheme", Some("frag"))
        );
    }

    #[test]
    fn test_generate_heading_slug() {
        assert_eq!(generate_heading_slug("Hello World"), "hello-world");
        assert_eq!(generate_heading_slug("Hello  World"), "hello--world");
        assert_eq!(generate_heading_slug("Hello-World"), "hello-world");
        assert_eq!(
            generate_heading_slug("Some **bold** text"),
            "some-bold-text"
        );
        assert_eq!(generate_heading_slug("Let's try with Ü"), "lets-try-with-ü");
        assert_eq!(
            generate_heading_slug("heading with 123 numbers"),
            "heading-with-123-numbers"
        );
        assert_eq!(
            generate_heading_slug("What about (parens)?"),
            "what-about-parens"
        );
        assert_eq!(
            generate_heading_slug("  leading spaces  "),
            "leading-spaces"
        );
    }
}
