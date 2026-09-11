//! 代码块组件：带行号、复制按钮与简易关键字高亮的代码展示。

use rgpui::*;

/// 复制按钮状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CodeBlockCopyState {
    /// 空闲状态。
    #[default]
    Idle,
    /// 已复制状态。
    Copied,
}

/// 简易分词类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    /// 关键字。
    Keyword,
    /// 字符串字面量。
    StringLiteral,
    /// 注释。
    Comment,
    /// 数字。
    Number,
    /// 普通文本。
    Plain,
}

/// 代码块组件。
#[derive(IntoElement)]
pub struct CodeBlock {
    /// 基础 Div。
    base: Div,
    /// 代码内容。
    code: SharedString,
    /// 语言（用于判断是否为 Rust 以启用关键字高亮）。
    language: Option<SharedString>,
    /// 是否显示行号。
    show_line_numbers: bool,
    /// 是否显示复制按钮。
    show_copy_button: bool,
    /// 需要高亮的行号集合。
    highlight_lines: Vec<usize>,
    /// 最大高度（超出滚动）。
    max_height: Option<Pixels>,
}

impl CodeBlock {
    /// 创建代码块。
    pub fn new(code: impl Into<SharedString>) -> Self {
        Self {
            base: div(),
            code: code.into(),
            language: None,
            show_line_numbers: true,
            show_copy_button: true,
            highlight_lines: Vec::new(),
            max_height: None,
        }
    }

    /// 设置语言。
    pub fn language(mut self, lang: impl Into<SharedString>) -> Self {
        self.language = Some(lang.into());
        self
    }

    /// 设置是否显示行号。
    pub fn show_line_numbers(mut self, show: bool) -> Self {
        self.show_line_numbers = show;
        self
    }

    /// 设置需要高亮的行号。
    pub fn highlight_lines(mut self, lines: Vec<usize>) -> Self {
        self.highlight_lines = lines;
        self
    }

    /// 设置最大高度。
    pub fn max_height(mut self, height: Pixels) -> Self {
        self.max_height = Some(height);
        self
    }

    /// 设置是否显示复制按钮。
    pub fn show_copy_button(mut self, show: bool) -> Self {
        self.show_copy_button = show;
        self
    }
}

impl RenderOnce for CodeBlock {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let lines: Vec<&str> = self.code.split('\n').collect();
        let is_rust = self
            .language
            .as_ref()
            .map(|l| l.as_ref() == "rust" || l.as_ref() == "rs")
            .unwrap_or(false);

        let keyword_color = theme.tokens.primary.color;
        let string_color = hsla(0.4, 0.7, 0.5, 1.0);
        let comment_color = theme.tokens.muted_foreground.color;
        let number_color = hsla(0.08, 0.7, 0.6, 1.0);
        let plain_color = theme.tokens.foreground.color;
        let line_number_color = theme.tokens.muted_foreground.color;
        let highlight_bg = theme.tokens.muted.opacity(0.5);

        let gutter_width = px(40.0);

        let code_for_copy = self.code.clone();
        let show_copy = self.show_copy_button;
        let radius = theme.radius;
        let mono_font_family = theme.mono_font_family.clone();

        let mut outer = self
            .base
            .relative()
            .bg(theme.tokens.muted.opacity(0.3))
            .rounded(radius)
            .font_family(mono_font_family)
            .text_size(px(13.0))
            .overflow_hidden();

        let max_h = self.max_height;

        let copy_btn = if show_copy {
            let copy_id: SharedString =
                format!("code-block-copy-{}", code_id_prefix(&self.code)).into();
            Some(
                div()
                    .id(copy_id)
                    .absolute()
                    .top(px(8.0))
                    .right(px(8.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(radius)
                    .bg(theme.tokens.muted.opacity(0.6))
                    .text_color(theme.tokens.muted_foreground)
                    .text_size(px(11.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.tokens.muted))
                    .active(|s| s.opacity(0.7))
                    .child("Copy")
                    .on_click(move |_, _window, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(code_for_copy.to_string()));
                    }),
            )
        } else {
            None
        };

        let mut content = div().flex().flex_col().py(px(12.0));

        for (idx, line_text) in lines.iter().enumerate() {
            let line_num = idx + 1;
            let is_highlighted = self.highlight_lines.contains(&line_num);

            let mut row = div().flex().flex_row().px(px(12.0));

            if is_highlighted {
                row = row.bg(highlight_bg);
            }

            if self.show_line_numbers {
                row = row.child(
                    div()
                        .w(gutter_width)
                        .flex_shrink_0()
                        .text_color(line_number_color)
                        .text_size(px(12.0))
                        .text_align(TextAlign::Right)
                        .pr(px(12.0))
                        .child(format!("{}", line_num)),
                );
            }

            let mut code_row = div().flex().flex_row().flex_1().min_w_0();
            let tokens = tokenize(line_text, is_rust);

            for (kind, text) in tokens {
                let color = match kind {
                    TokenKind::Keyword => keyword_color,
                    TokenKind::StringLiteral => string_color,
                    TokenKind::Comment => comment_color,
                    TokenKind::Number => number_color,
                    TokenKind::Plain => plain_color,
                };
                code_row = code_row.child(div().text_color(color).child(text.to_string()));
            }

            row = row.child(code_row);
            content = content.child(row);
        }

        if let Some(h) = max_h {
            outer = outer.child(
                div()
                    .id("code-block-scroll")
                    .max_h(h)
                    .overflow_y_scroll()
                    .child(content),
            );
        } else {
            outer = outer.child(content);
        }

        if let Some(btn) = copy_btn {
            outer.child(btn)
        } else {
            outer
        }
    }
}

/// 复制按钮 ID 前缀：按字节截断时向下对齐到字符边界，
/// 避免切在多字节字符（如中文）中间导致 panic。
/// 直接复用 `rgpui::SafeStrSlice::safe_prefix_until`，不本地手写循环。
fn code_id_prefix(code: &str) -> &str {
    code.safe_prefix_until(16)
}

/// 对一行代码做简易分词（注释/字符串/数字/关键字/普通文本）。
///
/// 不变式：每次循环入口 `pos` 恒为字符边界（0 是边界；所有分支要么只推进
/// ASCII 字节，要么落到 `"`/行尾等边界上；fallthrough 向前对齐），因此所有
/// `&line[a..b]` 切片都是安全的。
fn tokenize(line: &str, is_rust: bool) -> Vec<(TokenKind, &str)> {
    let mut tokens = Vec::new();
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        // 防御网：若 pos 不在字符边界（理论上不可达），把所在整个字符
        // 作为普通文本吞入并对齐，避免 panic。
        if !line.is_char_boundary(pos) {
            let mut start = pos;
            while start > 0 && !line.is_char_boundary(start) {
                start -= 1;
            }
            let mut end = pos + 1;
            while end < len && !line.is_char_boundary(end) {
                end += 1;
            }
            tokens.push((TokenKind::Plain, &line[start..end]));
            pos = end;
            continue;
        }
        if pos + 1 < len && bytes[pos] == b'/' && bytes[pos + 1] == b'/' {
            tokens.push((TokenKind::Comment, &line[pos..]));
            return tokens;
        }

        if bytes[pos] == b'"' {
            let start = pos;
            pos += 1;
            while pos < len && bytes[pos] != b'"' {
                if bytes[pos] == b'\\' && pos + 1 < len {
                    pos += 1;
                }
                pos += 1;
            }
            if pos < len {
                pos += 1;
            }
            tokens.push((TokenKind::StringLiteral, &line[start..pos]));
            continue;
        }

        if bytes[pos] == b'\'' && is_rust {
            let start = pos;
            pos += 1;
            if pos < len && bytes[pos] == b'\\' && pos + 1 < len {
                pos += 2;
            } else if pos < len {
                pos += 1;
            }
            if pos < len && bytes[pos] == b'\'' {
                pos += 1;
                tokens.push((TokenKind::StringLiteral, &line[start..pos]));
                continue;
            }
            pos = start + 1;
            tokens.push((TokenKind::Plain, &line[start..start + 1]));
            continue;
        }

        if bytes[pos].is_ascii_digit()
            || (bytes[pos] == b'-' && pos + 1 < len && bytes[pos + 1].is_ascii_digit())
        {
            let start = pos;
            if bytes[pos] == b'-' {
                pos += 1;
            }
            while pos < len
                && (bytes[pos].is_ascii_digit() || bytes[pos] == b'.' || bytes[pos] == b'_')
            {
                pos += 1;
            }
            if pos < len && (bytes[pos] == b'e' || bytes[pos] == b'E') {
                pos += 1;
                if pos < len && (bytes[pos] == b'+' || bytes[pos] == b'-') {
                    pos += 1;
                }
                while pos < len && bytes[pos].is_ascii_digit() {
                    pos += 1;
                }
            }
            tokens.push((TokenKind::Number, &line[start..pos]));
            continue;
        }

        if bytes[pos].is_ascii_alphabetic() || bytes[pos] == b'_' {
            let start = pos;
            while pos < len && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_') {
                pos += 1;
            }
            let word = &line[start..pos];
            if is_rust && is_rust_keyword(word) {
                tokens.push((TokenKind::Keyword, word));
            } else {
                tokens.push((TokenKind::Plain, word));
            }
            continue;
        }

        if bytes[pos] == b' ' {
            let start = pos;
            while pos < len && bytes[pos] == b' ' {
                pos += 1;
            }
            tokens.push((TokenKind::Plain, &line[start..pos]));
            continue;
        }

        // 其余字节（ASCII 标点或多字节字符首字节）：至少吞一个字节，
        // 若落在多字节字符中间则向前对齐到边界，保证切片安全。
        let start = pos;
        pos += 1;
        while pos < len && !line.is_char_boundary(pos) {
            pos += 1;
        }
        tokens.push((TokenKind::Plain, &line[start..pos]));
    }

    tokens
}

/// 判断是否为 Rust 关键字。
fn is_rust_keyword(word: &str) -> bool {
    matches!(
        word,
        "fn" | "let"
            | "mut"
            | "pub"
            | "struct"
            | "enum"
            | "impl"
            | "use"
            | "mod"
            | "if"
            | "else"
            | "for"
            | "while"
            | "match"
            | "return"
            | "self"
            | "Self"
            | "crate"
            | "super"
            | "true"
            | "false"
            | "async"
            | "await"
            | "move"
            | "ref"
            | "where"
            | "type"
            | "trait"
            | "const"
            | "static"
            | "loop"
            | "break"
            | "continue"
            | "in"
            | "as"
            | "unsafe"
            | "dyn"
            | "extern"
    )
}

impl Styled for CodeBlock {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for CodeBlock {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for CodeBlock {}

impl ParentElement for CodeBlock {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements)
    }
}

#[cfg(test)]
mod tests {
    // 注意：不用 `use super::*`，否则父模块 `use rgpui::*` 引入的 `test`
    // 模块名会遮蔽内置 `#[test]` 属性，导致宏展开递归溢出。
    use super::{code_id_prefix, tokenize};

    /// ID 前缀截断必须落在字符边界（复现：字节 16 切在 `运` 中间导致 panic）。
    #[test]
    fn code_id_prefix_stays_on_char_boundary() {
        // 14 个 ASCII + `运`（bytes 14..17）：旧代码 `&code[..16]` 直接 panic。
        let code = "0123456789abcd运xyz";
        assert_eq!(code_id_prefix(code), "0123456789abcd");
        assert_eq!(code_id_prefix("运"), "运");
        assert_eq!(code_id_prefix(""), "");
        assert_eq!(code_id_prefix("short"), "short");
        assert_eq!(code_id_prefix("0123456789abcdef"), "0123456789abcdef");
        // 16 字节恰好是字符边界时取满。
        assert_eq!(code_id_prefix("0123456789abcde运"), "0123456789abcde");
    }

    /// 分词永不 panic，且 tokens 拼回必须与原行完全一致（含中文注释/字符串/标识符）。
    #[test]
    fn tokenize_cjk_never_panics_and_is_lossless() {
        let lines = [
            "// 注释运",
            "\"字符串运\"",
            "let 变量_运1 = 运;",
            "运",
            "a运b",
            "'运'",
            "'a'",
            "\"unterminated 运",
            "123运456",
            "//",
            "",
            "fn main() { println!(\"你好，世界\"); } // 尾注释",
        ];
        for line in lines {
            for is_rust in [false, true] {
                let tokens = tokenize(line, is_rust);
                let joined: String = tokens.iter().map(|(_, t)| *t).collect();
                assert_eq!(joined, line, "分词丢失内容: {line:?} (rust={is_rust})");
            }
        }
    }
}
