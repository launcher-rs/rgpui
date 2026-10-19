//! 富文本文档模型与只读渲染元素。
//!
//! 面向文档编辑/文字处理应用的**最小编排外壳**：以「段落（[`Paragraph`]）包含若干行内
//! 富文本运行（[`Run`]）」为核心模型，提供字符坐标下的编辑操作（插入、删除、拆分/合并
//! 段落、批量应用行内样式），以及纯文本 / HTML 双向转换（可对接富剪贴板
//! [`crate::ClipboardItem::new_html`]）。
//!
//! 渲染由 [`RichText`] 元素承担：把每个段落编译成一段 [`crate::StyledText`]（粗体/斜体/
//! 下划线/删除线/颜色映射到 [`crate::TextRun`]），按段落对齐与缩进排版；项目符号 / 编号
//! 以行首标记呈现。**只读**渲染，编辑控件（光标、选区、IME）由应用自行拼接输入组件。
//!
//! ## 用法
//!
//! ```ignore
//! use rgpui::rich_text::{RichText, RichTextDoc, RunProps};
//!
//! // 从纯文本建文档，给首行加粗，再渲染。
//! let mut doc = RichTextDoc::from_plain_text("标题\n正文");
//! doc.apply_props(0..2, |p| p.bold = true);
//! let element = RichText::new(doc);
//! ```
//!
//! **本版暂缓**（后续增补）：节（section）/分栏、表格、批注、修订、文末脚注/尾注、
//! 项目符号字体与层级样式、可编辑光标与选区（现为只读渲染）。

use std::ops::Range;

use crate::{
    App, Hsla, IntoElement, ParentElement as _, RenderOnce, Rgba, StrikethroughStyle, Styled as _,
    StyledText, TextAlign, TextRun, UnderlineStyle, Window, div, px, v_flex,
};

/// 单次解析 HTML 的字节上限（防御恶意片段）。
const MAX_HTML: usize = 1 << 20;
/// 单个文档的段落数上限。
const MAX_PARAS: usize = 2_000;
/// 单个文档的富文本运行数上限。
const MAX_RUNS: usize = 10_000;
/// 一个缩进层级对应的像素数（HTML `margin-left` 与渲染共用）。
const INDENT_PX: f32 = 24.0;
/// 缩进的最大层级。
const MAX_INDENT: u8 = 8;

/// 行内富文本属性：粗体/斜体/下划线/删除线 + 可选前景色。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RunProps {
    /// 是否粗体。
    pub bold: bool,
    /// 是否斜体。
    pub italic: bool,
    /// 是否下划线。
    pub underline: bool,
    /// 是否删除线。
    pub strikethrough: bool,
    /// 前景色覆盖；`None` 表示跟随主题/父级。
    pub color: Option<Hsla>,
}

impl RunProps {
    /// 全部关闭（无覆盖色）的默认属性。
    pub fn new() -> Self {
        Self::default()
    }

    /// 置为粗体。
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// 置为斜体。
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// 置为下划线。
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// 置为删除线。
    pub fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }

    /// 设置前景色。
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// 是否为无任何样式的纯文本属性。
    pub fn is_plain(&self) -> bool {
        *self == Self::default()
    }
}

/// 段落级列表标记。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListKind {
    /// 普通段落。
    #[default]
    None,
    /// 项目符号（无序列表）。
    Bullet,
    /// 编号（有序列表，编号按同类型连续段落自动递增）。
    Number,
}

/// 段落级样式：对齐、缩进层级、列表标记。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParagraphStyle {
    /// 水平对齐。
    pub align: TextAlign,
    /// 缩进层级（0..=8，每级 [`INDENT_PX`] 像素）。
    pub indent: u8,
    /// 列表标记。
    pub list: ListKind,
}

impl ParagraphStyle {
    /// 设置缩进层级（自动夹取到 0..=8）。
    pub fn indent(mut self, level: u8) -> Self {
        self.indent = level.min(MAX_INDENT);
        self
    }
}

/// 一段连续的同属性文本。
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// 文本内容（不含换行，换行表现为段落边界）。
    pub text: String,
    /// 行内属性。
    pub props: RunProps,
}

impl Run {
    /// 以默认属性创建文本运行。
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            props: RunProps::default(),
        }
    }

    /// 以指定属性创建文本运行。
    pub fn with(text: impl Into<String>, props: RunProps) -> Self {
        Self {
            text: text.into(),
            props,
        }
    }

    /// 本运行的字符数。
    pub fn char_len(&self) -> usize {
        self.text.chars().count()
    }

    /// 在字符偏移 `at` 处拆成前后两半；`at` 落在边界时仅一侧存在。
    fn split_at_char(&self, at: usize) -> (Option<Run>, Option<Run>) {
        let len = self.char_len();
        if at == 0 {
            return (None, Some(self.clone()));
        }
        if at >= len {
            return (Some(self.clone()), None);
        }
        let byte = self
            .text
            .char_indices()
            .nth(at)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len());
        let (head, tail) = self.text.split_at(byte);
        (
            Some(Run {
                text: head.to_string(),
                props: self.props,
            }),
            Some(Run {
                text: tail.to_string(),
                props: self.props,
            }),
        )
    }
}

/// 一个段落：段落样式 + 行内运行序列。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Paragraph {
    /// 段落级样式。
    pub style: ParagraphStyle,
    /// 行内运行（按出现顺序）。
    pub runs: Vec<Run>,
}

impl Paragraph {
    /// 以纯文本创建单运行段落（空串得到空段落）。
    pub fn new(text: &str) -> Self {
        let runs = if text.is_empty() {
            Vec::new()
        } else {
            vec![Run::new(text)]
        };
        Self {
            style: ParagraphStyle::default(),
            runs,
        }
    }

    /// 段落纯文本。
    pub fn text(&self) -> String {
        let mut out = String::new();
        for run in &self.runs {
            out.push_str(&run.text);
        }
        out
    }

    /// 段落字符数。
    pub fn char_len(&self) -> usize {
        self.runs.iter().map(Run::char_len).sum()
    }

    /// 段落是否无字符。
    pub fn is_empty(&self) -> bool {
        self.char_len() == 0
    }

    /// 在字符偏移处插入一段带属性的文本（不处理换行，调用方保证）。
    pub fn insert_run(&mut self, offset: usize, text: &str, props: RunProps) {
        if text.is_empty() {
            return;
        }
        let (mut head, tail) = split_runs(std::mem::take(&mut self.runs), offset);
        head.push(Run {
            text: text.to_string(),
            props,
        });
        head.extend(tail);
        self.runs = head;
    }

    /// 删除字符区间 `[start, end)`。
    pub fn remove_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let runs = std::mem::take(&mut self.runs);
        let (mut head, rest) = split_runs(runs, start);
        let (_, tail) = split_runs(rest, end - start);
        head.extend(tail);
        self.runs = head;
    }

    /// 合并相邻且属性相同的运行，并丢弃空运行。
    pub fn normalize(&mut self) {
        let mut out: Vec<Run> = Vec::with_capacity(self.runs.len());
        for run in self.runs.drain(..) {
            if run.text.is_empty() {
                continue;
            }
            if let Some(last) = out.last_mut()
                && last.props == run.props
            {
                last.text.push_str(&run.text);
                continue;
            }
            out.push(run);
        }
        self.runs = out;
    }
}

/// 富文本文档：段落序列。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RichTextDoc {
    /// 段落序列，至少一段。
    pub paragraphs: Vec<Paragraph>,
}

impl RichTextDoc {
    /// 空文档（含一个空段落）。
    pub fn new() -> Self {
        Self {
            paragraphs: vec![Paragraph::default()],
        }
    }

    /// 从纯文本构造（按 `\n` 切分段落）。
    pub fn from_plain_text(text: &str) -> Self {
        Self {
            paragraphs: text.split('\n').map(Paragraph::new).collect(),
        }
    }

    /// 文档纯文本（段落以 `\n` 连接，不含列表标记）。
    pub fn plain_text(&self) -> String {
        self.paragraphs
            .iter()
            .map(Paragraph::text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 文档字符数（段落内字符 + 段落间各计一个分隔符）。
    pub fn char_len(&self) -> usize {
        let n: usize = self.paragraphs.iter().map(Paragraph::char_len).sum();
        n + self.paragraphs.len().saturating_sub(1)
    }

    /// 文档是否无可见字符。
    pub fn is_empty(&self) -> bool {
        self.paragraphs.iter().all(Paragraph::is_empty)
    }

    /// 把字符索引映射到 `(段落索引, 段内偏移)`；越界时归到末段末尾。
    pub fn locate(&self, index: usize) -> (usize, usize) {
        let mut start = 0usize;
        for (i, para) in self.paragraphs.iter().enumerate() {
            let len = para.char_len();
            if index <= start + len {
                return (i, index - start);
            }
            start += len + 1;
        }
        let last = self.paragraphs.len().saturating_sub(1);
        let off = self.paragraphs.get(last).map_or(0, Paragraph::char_len);
        (last, off)
    }

    /// 在字符索引处插入文本；文本中的 `\n` 会把段落拆开。
    pub fn insert_text(&mut self, index: usize, text: &str, props: RunProps) {
        if text.is_empty() {
            return;
        }
        let total = self.char_len();
        let (p, offset) = self.locate(index.min(total));
        let segments: Vec<&str> = text.split('\n').collect();
        if segments.len() == 1 {
            if let Some(para) = self.paragraphs.get_mut(p) {
                para.insert_run(offset, text, props);
            }
            return;
        }

        let style = self.paragraphs[p].style;
        let (head, tail) = split_runs(std::mem::take(&mut self.paragraphs[p].runs), offset);

        // 原段落保留拆分点之前的部分 + 首个文本片段。
        let mut first = Paragraph { style, runs: head };
        if let Some(seg) = segments.first()
            && !seg.is_empty()
        {
            first.runs.push(Run {
                text: (*seg).to_string(),
                props,
            });
        }

        // 末段落 = 末片段 + 原段落拆分点之后的部分（沿用原段落样式）。
        let mut last = Paragraph {
            style,
            runs: Vec::new(),
        };
        if let Some(seg) = segments.last()
            && !seg.is_empty()
        {
            last.runs.push(Run {
                text: (*seg).to_string(),
                props,
            });
        }
        last.runs.extend(tail);

        // 中间片段各成一段（用默认样式）。
        let middles: Vec<Paragraph> = segments[1..segments.len() - 1]
            .iter()
            .map(|seg| Paragraph::new(seg))
            .collect();

        self.paragraphs[p] = first;
        let mut at = p + 1;
        for middle in middles {
            self.paragraphs.insert(at, middle);
            at += 1;
        }
        self.paragraphs.insert(at, last);
    }

    /// 删除字符区间 `[start, end)`，跨段落时合并段落。
    pub fn delete(&mut self, range: Range<usize>) {
        let total = self.char_len();
        let start = range.start.min(total);
        let end = range.end.min(total);
        if start >= end {
            return;
        }
        let (p1, o1) = self.locate(start);
        let (p2, o2) = self.locate(end);

        if p1 == p2 {
            if let Some(para) = self.paragraphs.get_mut(p1) {
                para.remove_range(o1, o2);
            }
            return;
        }

        // 保留末段落删除点之后的尾部，拼到首段段的删除点之后。
        let tail = {
            let runs = std::mem::take(&mut self.paragraphs[p2].runs);
            let (_, tail) = split_runs(runs, o2);
            tail
        };
        {
            let para = &mut self.paragraphs[p1];
            let len = para.char_len();
            para.remove_range(o1, len);
            para.runs.extend(tail);
        }
        self.paragraphs.drain(p1 + 1..=p2);
    }

    /// 在字符索引处拆分段落（等价于插入一个换行）。
    pub fn split_paragraph(&mut self, index: usize) {
        self.insert_text(index, "\n", RunProps::default());
    }

    /// 把第 `index` 段与下一段合并；越界返回 `false`。
    pub fn merge_paragraph(&mut self, index: usize) -> bool {
        if index + 1 >= self.paragraphs.len() {
            return false;
        }
        let sep: usize = self.paragraphs[..=index]
            .iter()
            .map(|p| p.char_len() + 1)
            .sum::<usize>()
            - 1;
        self.delete(sep..sep + 1);
        true
    }

    /// 对字符区间 `[start, end)` 内的运行逐个应用 `f`（跨运行的选区会被拆开）。
    pub fn apply_props(&mut self, range: Range<usize>, f: impl Fn(&mut RunProps)) {
        let total = self.char_len();
        let start = range.start.min(total);
        let end = range.end.min(total);
        if start >= end {
            return;
        }
        let (p1, o1) = self.locate(start);
        let (p2, o2) = self.locate(end);
        for p in p1..=p2 {
            let para_len = self.paragraphs[p].char_len();
            let from = if p == p1 { o1 } else { 0 };
            let to = if p == p2 { o2 } else { para_len };
            if from >= to {
                continue;
            }
            let style = self.paragraphs[p].style;
            let (mut head, rest) = split_runs(std::mem::take(&mut self.paragraphs[p].runs), from);
            let (mid, tail) = split_runs(rest, to - from);
            for mut run in mid {
                f(&mut run.props);
                head.push(run);
            }
            head.extend(tail);
            let mut para = Paragraph { style, runs: head };
            para.normalize();
            self.paragraphs[p] = para;
        }
    }

    /// 规范化：合并同属性相邻运行、丢弃空运行，并保证至少一段。
    pub fn normalize(&mut self) {
        for para in &mut self.paragraphs {
            para.normalize();
        }
        if self.paragraphs.is_empty() {
            self.paragraphs.push(Paragraph::default());
        }
    }

    /// 序列化为 HTML：普通段落用 `<p>`，列表用 `<ul>`/`<ol>` + `<li>`，
    /// 行内样式用 `<b>`/`<i>`/`<u>`/`<s>` 与 `<span style="color:#rrggbb">`。
    pub fn to_html(&self) -> String {
        let mut out = String::new();
        let mut i = 0;
        while i < self.paragraphs.len() {
            let kind = self.paragraphs[i].style.list;
            match kind {
                ListKind::None => {
                    out.push_str("<p");
                    let css = paragraph_css(&self.paragraphs[i].style);
                    if !css.is_empty() {
                        out.push_str(" style=\"");
                        out.push_str(&css);
                        out.push('"');
                    }
                    out.push('>');
                    push_paragraph_html(&mut out, &self.paragraphs[i]);
                    out.push_str("</p>");
                    i += 1;
                }
                ListKind::Bullet | ListKind::Number => {
                    let tag = if kind == ListKind::Number { "ol" } else { "ul" };
                    out.push('<');
                    out.push_str(tag);
                    out.push('>');
                    while i < self.paragraphs.len() && self.paragraphs[i].style.list == kind {
                        out.push_str("<li>");
                        push_paragraph_html(&mut out, &self.paragraphs[i]);
                        out.push_str("</li>");
                        i += 1;
                    }
                    out.push_str("</");
                    out.push_str(tag);
                    out.push('>');
                }
            }
        }
        out
    }

    /// 解析 HTML 片段为文档；无可见文本时返回 `None`（调用方可退回纯文本）。
    ///
    /// 识别 `<b>`/`<strong>`、`<i>`/`<em>`、`<u>`、`<s>`/`<strike>`/`<del>`、`<br>`、
    /// `<p>`/`<div>`/`<h1>`…`<h6>`、`<ul>`/`<ol>`/`<li>` 与 `style` 中的
    /// `font-weight`/`font-style`/`text-decoration`/`color`（`#rrggbb`）/`text-align`。其余标签忽略。
    pub fn from_html(html: &str) -> Option<Self> {
        if html.len() > MAX_HTML {
            return None;
        }
        let bytes = html.as_bytes();
        let mut i = 0usize;
        let mut style = StyleState::default();
        let mut stack: Vec<(&'static str, StyleState)> = Vec::new();
        let mut list: Option<ListKind> = None;
        let mut paragraphs: Vec<Paragraph> = vec![Paragraph::default()];
        let mut runs = 0usize;
        let mut skip = 0i32;

        while i < bytes.len() && paragraphs.len() <= MAX_PARAS && runs <= MAX_RUNS {
            if bytes[i] == b'<' {
                let end = bytes[i + 1..]
                    .iter()
                    .position(|c| *c == b'>')
                    .map(|n| i + 1 + n)
                    .unwrap_or(bytes.len());
                let raw = &html[i + 1..end.min(html.len())];
                i = (end + 1).min(bytes.len());
                let token = raw.trim();
                if token.starts_with('!') || token.starts_with('?') {
                    continue;
                }
                let closing = token.starts_with('/');
                let name_src = token.trim_start_matches('/').trim();
                let name_end = name_src
                    .find(|c: char| c.is_whitespace() || c == '/')
                    .unwrap_or(name_src.len());
                let name = name_src[..name_end].to_ascii_lowercase();

                if matches!(name.as_str(), "script" | "style" | "head") {
                    if closing {
                        skip = (skip - 1).max(0);
                    } else if !token.ends_with('/') {
                        skip += 1;
                    }
                    continue;
                }
                if skip > 0 {
                    continue;
                }

                if closing {
                    match name.as_str() {
                        "ul" | "ol" => {
                            list = None;
                            new_paragraph(&mut paragraphs, list);
                        }
                        "li" => {}
                        _ => {
                            if is_block(&name) {
                                new_paragraph(&mut paragraphs, list);
                            }
                            let kind = tag_kind(&name);
                            if !kind.is_empty()
                                && let Some(pos) = stack.iter().rposition(|(k, _)| *k == kind)
                            {
                                style = stack[pos].1;
                                stack.truncate(pos);
                            }
                        }
                    }
                    continue;
                }

                match name.as_str() {
                    "ul" => {
                        list = Some(ListKind::Bullet);
                        continue;
                    }
                    "ol" => {
                        list = Some(ListKind::Number);
                        continue;
                    }
                    "li" => {
                        new_paragraph(&mut paragraphs, list);
                        if let Some(para) = paragraphs.last_mut() {
                            para.style.list = list.unwrap_or(ListKind::Bullet);
                        }
                        continue;
                    }
                    _ => {}
                }

                if is_break(&name) {
                    new_paragraph(&mut paragraphs, list);
                    continue;
                }

                if is_block(&name) {
                    // 段落级样式（对齐 / 缩进）落在当前段落上。
                    if let Some(css) = attr(name_src, "style")
                        && let Some(para) = paragraphs.last_mut()
                    {
                        apply_paragraph_css(&mut para.style, css);
                    }
                    continue;
                }

                let kind = tag_kind(&name);
                if kind.is_empty() {
                    continue;
                }
                if token.ends_with('/') {
                    continue;
                }
                stack.push((kind, style));
                apply_inline_tag(&name, &mut style, attr(name_src, "style"));
                continue;
            }

            if skip > 0 {
                i += 1;
                continue;
            }
            let end = bytes[i..]
                .iter()
                .position(|c| *c == b'<')
                .map(|n| i + n)
                .unwrap_or(bytes.len());
            let text = decode_text(&html[i..end]);
            i = end;
            if text.is_empty() {
                continue;
            }
            if runs >= MAX_RUNS {
                continue;
            }
            if let Some(para) = paragraphs.last_mut() {
                para.runs.push(Run {
                    text,
                    props: RunProps {
                        bold: style.bold,
                        italic: style.italic,
                        underline: style.underline,
                        strikethrough: style.strikethrough,
                        color: style.color,
                    },
                });
                runs += 1;
            }
        }

        while paragraphs.last().is_some_and(Paragraph::is_empty) && paragraphs.len() > 1 {
            paragraphs.pop();
        }
        if paragraphs.iter().all(Paragraph::is_empty) {
            return None;
        }
        for para in &mut paragraphs {
            para.normalize();
        }
        Some(Self { paragraphs })
    }
}

/// 行内样式的解析状态。
#[derive(Clone, Copy, Debug, Default)]
struct StyleState {
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    color: Option<Hsla>,
}

/// 按字符偏移把运行序列切成前后两半。
fn split_runs(runs: Vec<Run>, offset: usize) -> (Vec<Run>, Vec<Run>) {
    let mut head = Vec::new();
    let mut tail = Vec::new();
    let mut seen = 0usize;
    let mut placed = false;
    for run in runs {
        if placed {
            tail.push(run);
            continue;
        }
        let len = run.char_len();
        if seen + len <= offset {
            seen += len;
            head.push(run);
            continue;
        }
        let (a, b) = run.split_at_char(offset - seen);
        if let Some(a) = a {
            head.push(a);
        }
        if let Some(b) = b {
            tail.push(b);
        }
        placed = true;
    }
    (head, tail)
}

/// 追加一个段落的行内 HTML（不含外层 `<p>`/`<li>`）。
fn push_paragraph_html(out: &mut String, para: &Paragraph) {
    for run in &para.runs {
        let (open, close) = run_tags(&run.props);
        out.push_str(&open);
        escape_html(out, &run.text);
        out.push_str(&close);
    }
}

/// 运行属性对应的开/闭标签（颜色用 `<span style="color:...">`）。
fn run_tags(props: &RunProps) -> (String, String) {
    let mut open = String::new();
    let mut close = String::new();
    if props.bold {
        open.push_str("<b>");
        close.insert_str(0, "</b>");
    }
    if props.italic {
        open.push_str("<i>");
        close.insert_str(0, "</i>");
    }
    if props.underline {
        open.push_str("<u>");
        close.insert_str(0, "</u>");
    }
    if props.strikethrough {
        open.push_str("<s>");
        close.insert_str(0, "</s>");
    }
    if let Some(color) = props.color {
        open.push_str("<span style=\"color:");
        open.push_str(&color_hex(color));
        open.push_str("\">");
        close.insert_str(0, "</span>");
    }
    (open, close)
}

/// 段落级样式对应的内联 CSS。
fn paragraph_css(style: &ParagraphStyle) -> String {
    let mut css = String::new();
    match style.align {
        TextAlign::Center => css.push_str("text-align:center"),
        TextAlign::Right => css.push_str("text-align:right"),
        // CSS 的 `start` / `end` 由浏览器按文本方向解析，导出 HTML 时可直接透传。
        TextAlign::Start => css.push_str("text-align:start"),
        TextAlign::End => css.push_str("text-align:end"),
        TextAlign::Left => {}
    }
    if style.indent > 0 {
        if !css.is_empty() {
            css.push(';');
        }
        css.push_str(&format!(
            "margin-left:{}px",
            u32::from(style.indent) * INDENT_PX as u32
        ));
    }
    css
}

/// `Hsla` 转 `#rrggbb`。
fn color_hex(color: Hsla) -> String {
    let rgba: Rgba = color.into();
    let channel = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(rgba.r),
        channel(rgba.g),
        channel(rgba.b)
    )
}

/// HTML 转义（`&`/`<`/`>`）。
fn escape_html(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}

/// 若上一段非空，则新起一个沿用给定列表类型的空段落。
fn new_paragraph(paragraphs: &mut Vec<Paragraph>, list: Option<ListKind>) {
    if paragraphs.len() >= MAX_PARAS {
        return;
    }
    if paragraphs.last().is_some_and(|p| !p.is_empty()) {
        let mut para = Paragraph::default();
        para.style.list = list.unwrap_or(ListKind::None);
        paragraphs.push(para);
    }
}

/// 应用一个行内标签（`<b>`/`<i>`/`<u>`/`<s>` 及其 `style`）。
fn apply_inline_tag(name: &str, style: &mut StyleState, css: Option<&str>) {
    match name {
        "b" | "strong" => style.bold = true,
        "i" | "em" => style.italic = true,
        "u" => style.underline = true,
        "s" | "strike" | "del" => style.strikethrough = true,
        _ => {}
    }
    if let Some(css) = css {
        let lower = css.to_ascii_lowercase();
        if let Some(v) = css_value(&lower, "font-weight") {
            if v.starts_with("bold") {
                style.bold = true;
            } else if v.starts_with("normal") || v.starts_with("lighter") {
                style.bold = false;
            } else {
                let digits: String = v.chars().take_while(char::is_ascii_digit).collect();
                if let Ok(weight) = digits.parse::<u32>() {
                    style.bold = weight >= 600;
                }
            }
        }
        if lower.contains("font-style:italic") || lower.contains("font-style: italic") {
            style.italic = true;
        }
        if lower.contains("line-through") {
            style.strikethrough = true;
        }
        if lower.contains("text-decoration:underline")
            || lower.contains("text-decoration: underline")
            || lower.contains("text-decoration-line:underline")
            || lower.contains("text-decoration-line: underline")
        {
            style.underline = true;
        }
        if let Some(v) = css_value(&lower, "color")
            && let Some(color) = parse_hex_color(v)
        {
            style.color = Some(color);
        }
    }
}

/// 应用段落级内联 CSS（对齐 / 缩进）。
fn apply_paragraph_css(style: &mut ParagraphStyle, css: &str) {
    let lower = css.to_ascii_lowercase();
    if let Some(v) = css_value(&lower, "text-align") {
        style.align = match v {
            "center" => TextAlign::Center,
            "right" => TextAlign::Right,
            _ => TextAlign::Left,
        };
    }
    if let Some(v) = css_value(&lower, "margin-left")
        && let Some(px) = v
            .strip_suffix("px")
            .and_then(|n| n.trim().parse::<f32>().ok())
    {
        let level = (px / INDENT_PX).round().clamp(0.0, f32::from(MAX_INDENT)) as u8;
        style.indent = level;
    }
}

/// 在 CSS 声明串里取某个属性值（按 `;` 分隔，忽略大小写，避免匹配子串）。
fn css_value<'a>(css: &'a str, key: &str) -> Option<&'a str> {
    let bytes = css.as_bytes();
    let mut from = 0usize;
    while let Some(found) = css[from..].find(key) {
        let at = from + found;
        let before_ok = at == 0 || matches!(bytes[at - 1], b';' | b' ' | b'\t' | b',');
        let after = css[at + key.len()..].trim_start();
        if before_ok && let Some(rest) = after.strip_prefix(':') {
            let value = rest.trim_start();
            let end = value.find(';').unwrap_or(value.len());
            return Some(value[..end].trim());
        }
        from = at + key.len();
        if from >= css.len() {
            break;
        }
    }
    None
}

/// 解析 `#rgb` / `#rrggbb` 颜色。
fn parse_hex_color(value: &str) -> Option<Hsla> {
    let hex = value.trim().strip_prefix('#')?;
    let (r, g, b) = match hex.len() {
        3 => {
            let mut chars = hex.chars();
            let r = dup_nibble(chars.next()?)?;
            let g = dup_nibble(chars.next()?)?;
            let b = dup_nibble(chars.next()?)?;
            (r, g, b)
        }
        6 => (
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
        ),
        _ => return None,
    };
    Some(
        Rgba {
            r: f32::from(r) / 255.0,
            g: f32::from(g) / 255.0,
            b: f32::from(b) / 255.0,
            a: 1.0,
        }
        .into(),
    )
}

/// 单个十六进制位展开成两位（`f` → `0xff`）。
fn dup_nibble(c: char) -> Option<u8> {
    let v = c.to_digit(16)? as u8;
    Some(v * 16 + v)
}

/// 标签名到「同类可嵌套标签」的归并键；不参与样式时为 `""`。
fn tag_kind(name: &str) -> &'static str {
    match name {
        "b" | "strong" => "b",
        "i" | "em" => "i",
        "u" => "u",
        "s" | "strike" | "del" => "s",
        "span" | "font" => "span",
        _ => "",
    }
}

/// 是否为换行标签。
fn is_break(name: &str) -> bool {
    name == "br" || name == "hr"
}

/// 是否为块级标签（关闭时新起段落）。
fn is_block(name: &str) -> bool {
    matches!(
        name,
        "p" | "div" | "li" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "blockquote"
    )
}

/// 读取标签中某个布尔/字符串属性（带引号）的值。
fn attr<'a>(tag: &'a str, key: &str) -> Option<&'a str> {
    let lower = tag.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut from = 0usize;
    while let Some(found) = lower[from..].find(key) {
        let at = from + found;
        from = at + key.len();
        // 属性名必须前接空白（或位于标签名之后），避免匹配到别的属性名的子串。
        let before_ok = at == 0 || matches!(bytes[at - 1], b' ' | b'\t' | b'\n');
        if !before_ok {
            continue;
        }
        let after = tag.get(at + key.len()..)?.trim_start();
        let after = after.strip_prefix('=')?.trim_start();
        let quote = after.chars().next()?;
        if quote != '"' && quote != '\'' {
            continue;
        }
        let rest = after.get(quote.len_utf8()..)?;
        let end = rest.find(quote)?;
        return rest.get(..end);
    }
    None
}

/// 解码 HTML 文本中的实体，并去掉换行（换行由段落边界表达）。
fn decode_text(raw: &str) -> String {
    let mut out = String::new();
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at].replace(['\n', '\r'], ""));
        rest = &rest[at..];
        let Some(end) = rest.find(';') else {
            out.push_str(&rest.replace(['\n', '\r'], ""));
            break;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => entity.strip_prefix('#').and_then(numeric_char),
        };
        if let Some(c) = decoded {
            out.push(c);
            rest = &rest[end + 1..];
        } else {
            // 非规范实体（例如裸 `&`）：按原样保留。
            out.push('&');
            rest = &rest[1..];
        }
    }
    if !rest.is_empty() {
        out.push_str(&rest.replace(['\n', '\r'], ""));
    }
    out
}

/// 解码 `&#38;` / `&#x26;` 形式的数字实体。
fn numeric_char(digits: &str) -> Option<char> {
    let hex = digits
        .strip_prefix('x')
        .or_else(|| digits.strip_prefix('X'));
    let n = if let Some(hex) = hex {
        u32::from_str_radix(hex, 16).ok()?
    } else {
        digits.parse().ok()?
    };
    char::from_u32(n).filter(|c| !c.is_control() || *c == '\n')
}

/// 富文本文档的只读渲染元素。
///
/// 逐段落排版：对齐与缩进取自 [`ParagraphStyle`]，列表段落以「• 」/「N. 」标记开头，
/// 行内属性映射到 [`crate::TextRun`]。
#[derive(IntoElement)]
pub struct RichText {
    /// 待渲染文档。
    doc: RichTextDoc,
}

impl RichText {
    /// 创建渲染元素。
    pub fn new(doc: RichTextDoc) -> Self {
        Self { doc }
    }

    /// 取文档引用。
    pub fn doc(&self) -> &RichTextDoc {
        &self.doc
    }
}

impl From<RichTextDoc> for RichText {
    fn from(doc: RichTextDoc) -> Self {
        Self::new(doc)
    }
}

impl RenderOnce for RichText {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let base = window.text_style();
        let mut column = v_flex().w_full().gap_1();
        let mut number = 0usize;

        for para in &self.doc.paragraphs {
            let marker = match para.style.list {
                ListKind::Bullet => {
                    number = 0;
                    Some("•  ".to_string())
                }
                ListKind::Number => {
                    number += 1;
                    Some(format!("{number}.  "))
                }
                ListKind::None => {
                    number = 0;
                    None
                }
            };
            let (text, runs) = build_paragraph(para, &base, marker.as_deref());
            column = column.child(
                div()
                    .w_full()
                    .text_align(para.style.align)
                    .pl(px(f32::from(para.style.indent) * INDENT_PX))
                    .child(StyledText::new(text).with_runs(runs)),
            );
        }

        column
    }
}

/// 把段落编译成 `(文本, 运行)`，可选的列表标记作为首个运行。
fn build_paragraph(
    para: &Paragraph,
    base: &crate::TextStyle,
    marker: Option<&str>,
) -> (String, Vec<TextRun>) {
    let mut text = String::new();
    let mut runs = Vec::new();

    if let Some(marker) = marker {
        text.push_str(marker);
        runs.push(TextRun {
            len: marker.len(),
            font: base.font(),
            color: base.color,
            ..Default::default()
        });
    }

    for run in &para.runs {
        if run.text.is_empty() {
            continue;
        }
        let mut font = base.font();
        if run.props.bold {
            font = font.bold();
        }
        if run.props.italic {
            font = font.italic();
        }
        let color = run.props.color.unwrap_or(base.color);
        let underline = run.props.underline.then(|| UnderlineStyle {
            thickness: px(1.0),
            color: Some(color),
            wavy: false,
        });
        let strikethrough = run.props.strikethrough.then(|| StrikethroughStyle {
            thickness: px(1.0),
            color: Some(color),
        });
        runs.push(TextRun {
            len: run.text.len(),
            font,
            color,
            background_color: None,
            underline,
            strikethrough,
        });
        text.push_str(&run.text);
    }

    // 空段落占一行高度（标记已提供高度时无需补位）。
    if text.is_empty() {
        text.push(' ');
        runs.push(TextRun {
            len: 1,
            font: base.font(),
            color: base.color,
            ..Default::default()
        });
    }

    (text, runs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hsla;

    #[test]
    fn plain_text_round_trip() {
        let doc = RichTextDoc::from_plain_text("第一段\n第二段\n\n第四段");
        assert_eq!(doc.plain_text(), "第一段\n第二段\n\n第四段");
        assert_eq!(doc.paragraphs.len(), 4);
        assert_eq!(doc.char_len(), "第一段第二段第四段".chars().count() + 3);
    }

    #[test]
    fn insert_text_in_middle_splits_runs() {
        let mut doc = RichTextDoc::from_plain_text("abcd");
        doc.apply_props(0..4, |p| p.bold = true);
        doc.insert_text(2, "XY", RunProps::new().italic());
        assert_eq!(doc.plain_text(), "abXYcd");
        let runs = &doc.paragraphs[0].runs;
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0].text, "ab");
        assert!(runs[0].props.bold);
        assert_eq!(runs[1].text, "XY");
        assert!(runs[1].props.italic && !runs[1].props.bold);
        assert_eq!(runs[2].text, "cd");
        assert!(runs[2].props.bold);
    }

    #[test]
    fn insert_newline_creates_paragraphs() {
        let mut doc = RichTextDoc::from_plain_text("hello world");
        doc.insert_text(5, "\n", RunProps::default());
        assert_eq!(doc.plain_text(), "hello\n world");
        assert_eq!(doc.paragraphs.len(), 2);

        let mut doc = RichTextDoc::from_plain_text("abc");
        doc.insert_text(0, "x\ny\nz", RunProps::new().bold());
        assert_eq!(doc.plain_text(), "x\ny\nzabc");
        assert_eq!(doc.paragraphs.len(), 3);
        assert!(doc.paragraphs[0].runs[0].props.bold);
        assert_eq!(doc.paragraphs[1].text(), "y");
        // 末段继承原段落内容，且末尾保留原内容。
        assert_eq!(doc.paragraphs[2].text(), "zabc");
    }

    #[test]
    fn delete_merges_paragraphs() {
        let mut doc = RichTextDoc::from_plain_text("hello\nworld");
        // 删除 "hello" 与 `\n` 之后的首字符 "w"："helloworld" → "helloorld"
        doc.delete(1..7);
        assert_eq!(doc.plain_text(), "horld");
        assert_eq!(doc.paragraphs.len(), 1);
    }

    #[test]
    fn split_and_merge_paragraph() {
        let mut doc = RichTextDoc::from_plain_text("abcdef");
        doc.split_paragraph(3);
        assert_eq!(doc.plain_text(), "abc\ndef");
        assert_eq!(doc.paragraphs.len(), 2);
        assert!(doc.merge_paragraph(0));
        assert_eq!(doc.plain_text(), "abcdef");
        assert_eq!(doc.paragraphs.len(), 1);
        assert!(!doc.merge_paragraph(0));
    }

    #[test]
    fn apply_props_across_paragraph_boundary() {
        let mut doc = RichTextDoc::from_plain_text("ab\ncd");
        doc.apply_props(1..4, |p| p.underline = true);
        assert_eq!(doc.plain_text(), "ab\ncd");
        assert!(!doc.paragraphs[0].runs[0].props.underline);
        assert_eq!(doc.paragraphs[0].runs[1].text, "b");
        assert!(doc.paragraphs[0].runs[1].props.underline);
        assert!(doc.paragraphs[1].runs[0].props.underline);
        assert_eq!(doc.paragraphs[1].runs[0].text, "c");
    }

    #[test]
    fn html_round_trip_keeps_runs_and_escapes() {
        let mut doc = RichTextDoc::from_plain_text("A & B");
        doc.paragraphs[0].runs = vec![Run::with("A", RunProps::new().bold()), Run::new(" & B")];
        let html = doc.to_html();
        assert!(html.contains("<b>A</b>"), "{html}");
        assert!(html.contains("&amp;"), "{html}");
        let back = RichTextDoc::from_html(&html).unwrap();
        assert_eq!(back.plain_text(), "A & B");
        assert!(back.paragraphs[0].runs[0].props.bold);
    }

    #[test]
    fn html_keeps_underline_strike_and_color() {
        let mut doc = RichTextDoc::from_plain_text("x");
        doc.paragraphs[0].runs = vec![Run::with(
            "x",
            RunProps::new()
                .underline()
                .strikethrough()
                .color(hsla(0.0, 1.0, 0.5, 1.0)),
        )];
        let html = doc.to_html();
        assert!(html.contains("<u>"));
        assert!(html.contains("<s>"));
        assert!(html.contains("color:#ff0000"), "{html}");
        let back = RichTextDoc::from_html(&html).unwrap();
        let props = back.paragraphs[0].runs[0].props;
        assert!(props.underline && props.strikethrough);
        assert!(props.color.is_some());
    }

    #[test]
    fn html_lists_and_alignment_round_trip() {
        let mut doc = RichTextDoc::new();
        doc.paragraphs[0].runs = vec![Run::new("标题")];
        doc.paragraphs[0].style.align = TextAlign::Center;
        doc.paragraphs[0].style.indent = 1;
        let mut item = Paragraph::new("第一项");
        item.style.list = ListKind::Bullet;
        let mut item2 = Paragraph::new("第二项");
        item2.style.list = ListKind::Number;
        doc.paragraphs.push(item);
        doc.paragraphs.push(item2);

        let html = doc.to_html();
        assert!(html.contains("text-align:center"), "{html}");
        assert!(html.contains("margin-left:24px"), "{html}");
        assert!(html.contains("<ul><li>第一项</li></ul>"), "{html}");
        assert!(html.contains("<ol><li>第二项</li></ol>"), "{html}");

        let back = RichTextDoc::from_html(&html).unwrap();
        assert_eq!(back.plain_text(), "标题\n第一项\n第二项");
        assert_eq!(back.paragraphs[0].style.align, TextAlign::Center);
        assert_eq!(back.paragraphs[0].style.indent, 1);
        assert_eq!(back.paragraphs[1].style.list, ListKind::Bullet);
        assert_eq!(back.paragraphs[2].style.list, ListKind::Number);
    }

    #[test]
    fn html_parses_browser_style_fragments() {
        let html = "<html><head><style>p{}</style></head><body>\
            <p><span style=\"font-weight:bold\">A &amp; B</span></p>\
            <p><span style=\"font-style: italic\">C</span></p>\
            <p style=\"text-align: right\">D</p></body></html>";
        let doc = RichTextDoc::from_html(html).unwrap();
        assert_eq!(doc.plain_text(), "A & B\nC\nD");
        assert!(doc.paragraphs[0].runs[0].props.bold);
        assert!(doc.paragraphs[1].runs[0].props.italic);
        assert_eq!(doc.paragraphs[2].style.align, TextAlign::Right);
        // 正常字重（font-weight:normal）不应被当作粗体。
        let light = RichTextDoc::from_html("<b style=\"font-weight:normal\">x</b>").unwrap();
        assert!(!light.paragraphs[0].runs[0].props.bold);
    }

    #[test]
    fn empty_and_oversize_html() {
        assert!(RichTextDoc::from_html("<p></p><br>").is_none());
        assert!(RichTextDoc::from_html(&"x".repeat(MAX_HTML + 1)).is_none());
    }

    #[test]
    fn normalize_merges_equal_runs_and_keeps_one_paragraph() {
        let mut para = Paragraph {
            style: ParagraphStyle::default(),
            runs: vec![
                Run::with("a", RunProps::new().bold()),
                Run::with("b", RunProps::new().bold()),
                Run::new(""),
                Run::new("c"),
            ],
        };
        para.normalize();
        assert_eq!(para.runs.len(), 2);
        assert_eq!(para.runs[0].text, "ab");
        assert!(para.runs[0].props.bold);
        assert_eq!(para.runs[1].text, "c");

        let mut doc = RichTextDoc {
            paragraphs: Vec::new(),
        };
        doc.normalize();
        assert_eq!(doc.paragraphs.len(), 1);
    }
}
