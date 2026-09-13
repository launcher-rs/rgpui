//! 富文本渲染：将结构化的富文本块模型渲染为 rgpui 元素。

use crate::code_block::CodeBlock;
use rgpui::prelude::FluentBuilder as _;
use rgpui::*;

/// 表格单元格对齐方式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableAlignment {
    /// 左对齐。
    Left,
    /// 居中。
    Center,
    /// 右对齐。
    Right,
}

/// GitHub 风格标注类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalloutKind {
    /// 提示。
    Note,
    /// 技巧。
    Tip,
    /// 重要。
    Important,
    /// 警告。
    Warning,
    /// 危险。
    Caution,
    /// 无特定类型（默认）。
    Default,
}

impl CalloutKind {
    /// 从字符串解析标注类型。
    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_uppercase().as_str() {
            "NOTE" => CalloutKind::Note,
            "TIP" => CalloutKind::Tip,
            "IMPORTANT" => CalloutKind::Important,
            "WARNING" => CalloutKind::Warning,
            "CAUTION" => CalloutKind::Caution,
            _ => CalloutKind::Default,
        }
    }

    /// 标注类型的显示名称。
    pub fn label(&self) -> &str {
        match self {
            CalloutKind::Note => "Note",
            CalloutKind::Tip => "Tip",
            CalloutKind::Important => "Important",
            CalloutKind::Warning => "Warning",
            CalloutKind::Caution => "Caution",
            CalloutKind::Default => "Note",
        }
    }

    /// 标注类型的图标。
    pub fn icon(&self) -> &str {
        match self {
            CalloutKind::Note => "\u{1f4dd}",
            CalloutKind::Tip => "\u{1f4a1}",
            CalloutKind::Important => "\u{26a0}\u{fe0f}",
            CalloutKind::Warning => "\u{26a0}\u{fe0f}",
            CalloutKind::Caution => "\u{1f6ab}",
            CalloutKind::Default => "\u{1f4dd}",
        }
    }
}

/// 行内富文本元素。
#[derive(Debug, Clone)]
pub enum RichInline {
    /// 普通文本。
    Text(String),
    /// 加粗。
    Bold(Vec<RichInline>),
    /// 斜体。
    Italic(Vec<RichInline>),
    /// 删除线。
    Strikethrough(Vec<RichInline>),
    /// 行内代码。
    Code(String),
    /// 链接。
    Link {
        /// 链接文本。
        text: Vec<RichInline>,
        /// 链接地址。
        url: String,
    },
    /// 图片。
    Image {
        /// 替代文本。
        alt: String,
        /// 图片地址。
        url: String,
    },
    /// 换行。
    LineBreak,
    /// 原始 HTML（渲染时忽略）。
    Html(String),
    /// 带样式包裹的行内内容。
    Styled {
        /// 子元素。
        children: Vec<RichInline>,
        /// 文字颜色。
        color: Option<Hsla>,
        /// 背景颜色。
        background_color: Option<Hsla>,
        /// 是否加粗。
        bold: Option<bool>,
        /// 是否斜体。
        italic: Option<bool>,
        /// 字号倍率。
        font_size: Option<f32>,
    },
}

/// 列表项。
#[derive(Debug, Clone)]
pub struct ListItem {
    /// 任务列表勾选状态（None 表示普通列表项）。
    pub checked: Option<bool>,
    /// 列表项内容。
    pub content: Vec<RichInline>,
    /// 嵌套子列表项。
    pub children: Vec<ListItem>,
}

/// 块级富文本元素。
#[derive(Debug, Clone)]
pub enum RichBlock {
    /// 段落。
    Paragraph(Vec<RichInline>),
    /// 标题。
    Heading {
        /// 标题级别（1-6）。
        level: u8,
        /// 标题内容。
        content: Vec<RichInline>,
    },
    /// 代码块。
    CodeBlock {
        /// 语言。
        language: Option<String>,
        /// 代码内容。
        code: String,
    },
    /// 块引用。
    BlockQuote(Vec<RichBlock>),
    /// 有序列表。
    OrderedList {
        /// 起始序号。
        start: u64,
        /// 列表项。
        items: Vec<ListItem>,
    },
    /// 无序列表。
    UnorderedList {
        /// 列表项。
        items: Vec<ListItem>,
    },
    /// 表格。
    Table {
        /// 表头。
        headers: Vec<Vec<RichInline>>,
        /// 各列对齐方式。
        alignments: Vec<TableAlignment>,
        /// 数据行。
        rows: Vec<Vec<Vec<RichInline>>>,
    },
    /// 水平分割线。
    HorizontalRule,
    /// 图片。
    Image {
        /// 替代文本。
        alt: String,
        /// 图片地址。
        url: String,
    },
    /// GitHub 风格的标注块（`> [!NOTE]`、`> [!TIP]` 等）。
    Callout {
        /// 标注类型。
        kind: CalloutKind,
        /// 标注标题（可选，用户自定义）。
        title: Option<String>,
        /// 标注内容块。
        blocks: Vec<RichBlock>,
    },
    /// 脚注定义（`[^1]: content`）。
    FootnoteDefinition {
        /// 脚注标签。
        label: String,
        /// 脚注内容块。
        blocks: Vec<RichBlock>,
    },
}

/// 链接区域信息（文本区间 + URL）。
#[derive(Clone)]
pub struct LinkInfo {
    /// 文本字节区间。
    pub range: std::ops::Range<usize>,
    /// 链接地址。
    pub url: String,
}

/// 链接点击回调。
pub type LinkClickHandler = Box<dyn Fn(&str, &mut Window, &mut App) + 'static>;

/// 代码块自定义渲染回调：(language, code, base_size, theme) -> 可选自定义元素。
/// 返回 `None` 表示走默认 CodeBlock 渲染路径。
pub type CodeBlockRenderer =
    Box<dyn Fn(&str, &str, Pixels, &Theme) -> Option<AnyElement> + 'static>;

/// 文本展平结果。
struct FlattenResult {
    text: String,
    runs: Vec<TextRun>,
    links: Vec<LinkInfo>,
}

/// 将行内富文本展平为带样式区间（TextRun）的纯文本。
struct InlineFlattener {
    text: String,
    runs: Vec<TextRun>,
    links: Vec<LinkInfo>,
    font_family: SharedString,
    font_mono: SharedString,
    text_color: Hsla,
    link_color: Hsla,
    code_bg: Hsla,
    bold: bool,
    italic: bool,
    strikethrough: bool,
    in_link: Option<String>,
    color_override: Option<Hsla>,
    bg_override: Option<Hsla>,
}

impl InlineFlattener {
    /// 创建展平器。
    fn new(
        font_family: SharedString,
        font_mono: SharedString,
        text_color: Hsla,
        link_color: Hsla,
        code_bg: Hsla,
    ) -> Self {
        Self {
            text: String::new(),
            runs: Vec::new(),
            links: Vec::new(),
            font_family,
            font_mono,
            text_color,
            link_color,
            code_bg,
            bold: false,
            italic: false,
            strikethrough: false,
            in_link: None,
            color_override: None,
            bg_override: None,
        }
    }

    /// 执行展平。
    fn flatten(mut self, inlines: &[RichInline]) -> FlattenResult {
        self.walk(inlines);
        FlattenResult {
            text: self.text,
            runs: self.runs,
            links: self.links,
        }
    }

    /// 递归遍历行内元素。
    fn walk(&mut self, inlines: &[RichInline]) {
        for inline in inlines {
            match inline {
                RichInline::Text(s) => self.push_text(s, false),
                RichInline::Bold(children) => {
                    let prev = self.bold;
                    self.bold = true;
                    self.walk(children);
                    self.bold = prev;
                }
                RichInline::Italic(children) => {
                    let prev = self.italic;
                    self.italic = true;
                    self.walk(children);
                    self.italic = prev;
                }
                RichInline::Strikethrough(children) => {
                    let prev = self.strikethrough;
                    self.strikethrough = true;
                    self.walk(children);
                    self.strikethrough = prev;
                }
                RichInline::Code(code) => self.push_text(code, true),
                RichInline::Link { text, url } => {
                    let prev = self.in_link.take();
                    self.in_link = Some(url.clone());
                    self.walk(text);
                    self.in_link = prev;
                }
                RichInline::Image { alt, .. } => {
                    if !alt.is_empty() {
                        self.push_text(alt, false);
                    }
                }
                RichInline::LineBreak => self.push_text("\n", false),
                RichInline::Html(_) => {}
                RichInline::Styled {
                    children,
                    color,
                    background_color,
                    bold,
                    italic,
                    font_size: _,
                } => {
                    let prev_bold = self.bold;
                    let prev_italic = self.italic;
                    let prev_color = self.color_override;
                    let prev_bg = self.bg_override;
                    if let Some(true) = bold {
                        self.bold = true;
                    }
                    if let Some(true) = italic {
                        self.italic = true;
                    }
                    if color.is_some() {
                        self.color_override = *color;
                    }
                    if background_color.is_some() {
                        self.bg_override = *background_color;
                    }
                    self.walk(children);
                    self.bold = prev_bold;
                    self.italic = prev_italic;
                    self.color_override = prev_color;
                    self.bg_override = prev_bg;
                }
            }
        }
    }

    /// 追加一段文本并生成对应 TextRun。
    fn push_text(&mut self, text: &str, is_code: bool) {
        if text.is_empty() {
            return;
        }

        let start = self.text.len();
        self.text.push_str(text);
        let len = text.len();

        let is_link = self.in_link.is_some();

        let font = if is_code {
            Font {
                family: self.font_mono.clone(),
                features: FontFeatures::default(),
                weight: FontWeight::default(),
                style: FontStyle::default(),
                fallbacks: None,
            }
        } else {
            Font {
                family: self.font_family.clone(),
                features: FontFeatures::default(),
                weight: if self.bold {
                    FontWeight::BOLD
                } else {
                    FontWeight::default()
                },
                style: if self.italic {
                    FontStyle::Italic
                } else {
                    FontStyle::Normal
                },
                fallbacks: None,
            }
        };

        let color = if is_link {
            self.link_color
        } else if let Some(c) = self.color_override {
            c
        } else {
            self.text_color
        };

        let underline = if is_link {
            Some(UnderlineStyle {
                thickness: px(1.0),
                color: Some(self.link_color),
                wavy: false,
            })
        } else {
            None
        };

        let strikethrough = if self.strikethrough {
            Some(StrikethroughStyle {
                thickness: px(1.0),
                color: Some(self.text_color),
            })
        } else {
            None
        };

        let background_color = if is_code {
            Some(self.code_bg)
        } else {
            self.bg_override
        };

        self.runs.push(TextRun {
            len,
            font,
            color,
            background_color,
            underline,
            strikethrough,
        });

        if let Some(ref url) = self.in_link {
            self.links.push(LinkInfo {
                range: start..(start + len),
                url: url.clone(),
            });
        }
    }
}

/// 渲染行内元素为可点击链接或纯文本。
pub fn render_inlines(
    inlines: &[RichInline],
    base_size: Pixels,
    link_handler: Option<&LinkClickHandler>,
    element_id: Option<ElementId>,
    theme: &Theme,
) -> AnyElement {
    let font_family = theme.font_family.clone();
    let font_mono = theme.mono_font_family.clone();
    let text_color = theme.tokens.foreground.color;
    let link_color = theme.tokens.primary.color;
    let code_bg = theme.tokens.muted.opacity(0.3);

    let flattened = InlineFlattener::new(font_family, font_mono, text_color, link_color, code_bg)
        .flatten(inlines);

    if flattened.text.is_empty() {
        return div().into_any_element();
    }

    let styled = StyledText::new(SharedString::from(flattened.text)).with_runs(flattened.runs);

    if !flattened.links.is_empty() && link_handler.is_some() {
        let id = element_id.unwrap_or_else(|| ElementId::Name("rich-text-inline".into()));
        let click_ranges: Vec<std::ops::Range<usize>> =
            flattened.links.iter().map(|l| l.range.clone()).collect();
        let urls: Vec<String> = flattened.links.iter().map(|l| l.url.clone()).collect();

        return div()
            .text_size(base_size)
            .line_height(relative(1.5))
            .child(InteractiveText::new(id, styled).on_click(
                click_ranges,
                move |idx, _window, cx| {
                    if let Some(url) = urls.get(idx) {
                        cx.open_url(url);
                    }
                },
            ))
            .into_any_element();
    }

    div()
        .text_size(base_size)
        .line_height(relative(1.5))
        .child(styled)
        .into_any_element()
}

/// 渲染行内元素，链接点击走自定义回调。
pub fn render_inlines_with_handler(
    inlines: &[RichInline],
    base_size: Pixels,
    _link_urls: &[LinkInfo],
    on_link_click: &Option<LinkClickHandler>,
    element_id: Option<ElementId>,
    theme: &Theme,
) -> AnyElement {
    let font_family = theme.font_family.clone();
    let font_mono = theme.mono_font_family.clone();
    let text_color = theme.tokens.foreground.color;
    let link_color = theme.tokens.primary.color;
    let code_bg = theme.tokens.muted.opacity(0.3);

    let flattened = InlineFlattener::new(font_family, font_mono, text_color, link_color, code_bg)
        .flatten(inlines);

    if flattened.text.is_empty() {
        return div().into_any_element();
    }

    let styled = StyledText::new(SharedString::from(flattened.text)).with_runs(flattened.runs);

    if !flattened.links.is_empty() {
        let id = element_id.unwrap_or_else(|| ElementId::Name("rich-text-inline".into()));
        let click_ranges: Vec<std::ops::Range<usize>> =
            flattened.links.iter().map(|l| l.range.clone()).collect();

        if on_link_click.is_some() {
            let urls: Vec<String> = flattened.links.iter().map(|l| l.url.clone()).collect();
            return div()
                .text_size(base_size)
                .line_height(relative(1.5))
                .child(InteractiveText::new(id, styled).on_click(
                    click_ranges,
                    move |idx, _window, cx| {
                        if let Some(url) = urls.get(idx) {
                            cx.open_url(url);
                        }
                    },
                ))
                .into_any_element();
        } else {
            let urls: Vec<String> = flattened.links.iter().map(|l| l.url.clone()).collect();
            return div()
                .text_size(base_size)
                .line_height(relative(1.5))
                .child(InteractiveText::new(id, styled).on_click(
                    click_ranges,
                    move |idx, _window, cx| {
                        if let Some(url) = urls.get(idx) {
                            cx.open_url(url);
                        }
                    },
                ))
                .into_any_element();
        }
    }

    div()
        .text_size(base_size)
        .line_height(relative(1.5))
        .child(styled)
        .into_any_element()
}

/// 渲染块列表，返回元素列表。
pub fn render_blocks(
    blocks: &[RichBlock],
    base_size: Pixels,
    on_link_click: &Option<LinkClickHandler>,
    id_prefix: &str,
    theme: &Theme,
) -> Vec<AnyElement> {
    render_blocks_with(blocks, base_size, on_link_click, id_prefix, theme, &None)
}

/// 渲染块列表（可指定代码块自定义渲染器）。
pub fn render_blocks_with(
    blocks: &[RichBlock],
    base_size: Pixels,
    on_link_click: &Option<LinkClickHandler>,
    id_prefix: &str,
    theme: &Theme,
    code_block_renderer: &Option<CodeBlockRenderer>,
) -> Vec<AnyElement> {
    let mut elements = Vec::new();
    let mut block_idx = 0u32;

    for block in blocks {
        let el = render_block(
            block,
            base_size,
            on_link_click,
            id_prefix,
            &mut block_idx,
            theme,
            code_block_renderer,
        );
        elements.push(el);
    }

    elements
}

/// 渲染单个块。
fn render_block(
    block: &RichBlock,
    base_size: Pixels,
    on_link_click: &Option<LinkClickHandler>,
    id_prefix: &str,
    block_idx: &mut u32,
    theme: &Theme,
    code_block_renderer: &Option<CodeBlockRenderer>,
) -> AnyElement {
    *block_idx += 1;
    let idx = *block_idx;

    match block {
        RichBlock::Paragraph(inlines) => {
            let id = ElementId::Name(format!("{}-p-{}", id_prefix, idx).into());
            let el = render_inline_element(inlines, base_size, on_link_click, Some(id), theme);
            div().mb(px(12.0)).child(el).into_any_element()
        }

        RichBlock::Heading { level, content } => {
            let (size, weight) = heading_style(*level);
            let id = ElementId::Name(format!("{}-h{}-{}", id_prefix, level, idx).into());

            let top_margin = match level {
                1 => px(24.0),
                2 => px(20.0),
                3 => px(16.0),
                _ => px(12.0),
            };

            let el = render_inline_element(content, size, on_link_click, Some(id), theme);
            div()
                .mt(top_margin)
                .mb(px(8.0))
                .font_weight(weight)
                .child(el)
                .into_any_element()
        }

        RichBlock::CodeBlock { language, code } => {
            if let Some(renderer) = code_block_renderer {
                if let Some(lang) = language {
                    if let Some(el) = renderer(lang, code, base_size, theme) {
                        return el;
                    }
                }
            }
            let mut cb = CodeBlock::new(code.clone())
                .show_line_numbers(true)
                .show_copy_button(true);
            if let Some(lang) = language {
                cb = cb.language(lang.clone());
            }
            div().mb(px(12.0)).child(cb).into_any_element()
        }

        RichBlock::BlockQuote(inner_blocks) => {
            let children = render_blocks_with(
                inner_blocks,
                base_size,
                on_link_click,
                id_prefix,
                theme,
                code_block_renderer,
            );
            div()
                .mb(px(12.0))
                .pl(px(16.0))
                .border_l(px(4.0))
                .border_color(theme.tokens.border)
                .bg(theme.tokens.muted.opacity(0.15))
                .py(px(4.0))
                .text_color(theme.tokens.muted_foreground)
                .children(children)
                .into_any_element()
        }

        RichBlock::UnorderedList { items } => {
            let children = render_list_items(
                items,
                base_size,
                on_link_click,
                id_prefix,
                block_idx,
                false,
                1,
                theme,
            );
            div().mb(px(12.0)).children(children).into_any_element()
        }

        RichBlock::OrderedList { start, items } => {
            let children = render_list_items(
                items,
                base_size,
                on_link_click,
                id_prefix,
                block_idx,
                true,
                *start,
                theme,
            );
            div().mb(px(12.0)).children(children).into_any_element()
        }

        RichBlock::Table {
            headers,
            alignments,
            rows,
        } => render_table(
            headers,
            alignments,
            rows,
            base_size,
            on_link_click,
            id_prefix,
            block_idx,
            theme,
        ),

        RichBlock::HorizontalRule => div()
            .my(px(16.0))
            .child(Separator::horizontal())
            .into_any_element(),

        RichBlock::Image { alt: _, url } => div()
            .mb(px(12.0))
            .child(img(SharedString::from(url.clone())).max_w(px(600.0)))
            .into_any_element(),

        RichBlock::Callout {
            kind,
            title,
            blocks,
        } => {
            let label = title.as_deref().unwrap_or(kind.label());
            let header_text = format!("{} {}", kind.icon(), label);
            let children = render_blocks_with(
                blocks,
                base_size,
                on_link_click,
                id_prefix,
                theme,
                code_block_renderer,
            );
            let bg = match kind {
                CalloutKind::Note => theme.tokens.info,
                CalloutKind::Tip => theme.tokens.success,
                CalloutKind::Important => theme.tokens.warning,
                CalloutKind::Warning => theme.tokens.warning,
                CalloutKind::Caution => theme.tokens.danger,
                CalloutKind::Default => theme.tokens.info,
            };
            div()
                .mb(px(12.0))
                .pl(px(16.0))
                .pr(px(12.0))
                .py(px(8.0))
                .border_l(px(4.0))
                .border_color(bg)
                .bg(bg.opacity(0.1))
                .child(
                    div()
                        .mb(px(4.0))
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .child(header_text),
                )
                .children(children)
                .into_any_element()
        }

        RichBlock::FootnoteDefinition { label, blocks } => {
            let children = render_blocks_with(
                blocks,
                base_size,
                on_link_click,
                id_prefix,
                theme,
                code_block_renderer,
            );
            div()
                .mb(px(8.0))
                .pl(px(16.0))
                .border_l(px(3.0))
                .border_color(theme.tokens.muted)
                .text_color(theme.tokens.muted_foreground)
                .child(div().mb(px(4.0)).text_sm().child(format!("[^{label}]")))
                .children(children)
                .into_any_element()
        }
    }
}

/// 根据标题级别返回字号与字重。
fn heading_style(level: u8) -> (Pixels, FontWeight) {
    match level {
        1 => (px(32.0), FontWeight::BOLD),
        2 => (px(28.0), FontWeight::SEMIBOLD),
        3 => (px(24.0), FontWeight::SEMIBOLD),
        4 => (px(20.0), FontWeight::SEMIBOLD),
        5 => (px(18.0), FontWeight::MEDIUM),
        _ => (px(16.0), FontWeight::MEDIUM),
    }
}

/// 渲染行内元素为单个元素。
fn render_inline_element(
    inlines: &[RichInline],
    base_size: Pixels,
    on_link_click: &Option<LinkClickHandler>,
    element_id: Option<ElementId>,
    theme: &Theme,
) -> AnyElement {
    let font_family = theme.font_family.clone();
    let font_mono = theme.mono_font_family.clone();
    let text_color = theme.tokens.foreground.color;
    let link_color = theme.tokens.primary.color;
    let code_bg = theme.tokens.muted.opacity(0.3);

    let flattened = InlineFlattener::new(font_family, font_mono, text_color, link_color, code_bg)
        .flatten(inlines);

    if flattened.text.is_empty() {
        return div().into_any_element();
    }

    let styled = StyledText::new(SharedString::from(flattened.text)).with_runs(flattened.runs);

    if !flattened.links.is_empty() {
        let id = element_id.unwrap_or_else(|| ElementId::Name("rich-inline".into()));
        let click_ranges: Vec<std::ops::Range<usize>> =
            flattened.links.iter().map(|l| l.range.clone()).collect();

        if on_link_click.is_some() {
            let urls: Vec<String> = flattened.links.iter().map(|l| l.url.clone()).collect();
            return div()
                .text_size(base_size)
                .line_height(relative(1.5))
                .child(InteractiveText::new(id, styled).on_click(
                    click_ranges,
                    move |idx, _window, cx| {
                        if let Some(url) = urls.get(idx) {
                            cx.open_url(url);
                        }
                    },
                ))
                .into_any_element();
        } else {
            let urls: Vec<String> = flattened.links.iter().map(|l| l.url.clone()).collect();
            return div()
                .text_size(base_size)
                .line_height(relative(1.5))
                .child(InteractiveText::new(id, styled).on_click(
                    click_ranges,
                    move |idx, _window, cx| {
                        if let Some(url) = urls.get(idx) {
                            cx.open_url(url);
                        }
                    },
                ))
                .into_any_element();
        }
    }

    div()
        .text_size(base_size)
        .line_height(relative(1.5))
        .child(styled)
        .into_any_element()
}

/// 渲染列表项（支持嵌套）。
fn render_list_items(
    items: &[ListItem],
    base_size: Pixels,
    on_link_click: &Option<LinkClickHandler>,
    id_prefix: &str,
    block_idx: &mut u32,
    ordered: bool,
    start: u64,
    theme: &Theme,
) -> Vec<AnyElement> {
    let mut elements = Vec::new();

    for (i, item) in items.iter().enumerate() {
        *block_idx += 1;
        let idx = *block_idx;

        let bullet = if let Some(checked) = item.checked {
            if checked {
                SharedString::from("[x] ")
            } else {
                SharedString::from("[ ] ")
            }
        } else if ordered {
            SharedString::from(format!("{}. ", start + i as u64))
        } else {
            SharedString::from("\u{2022} ")
        };

        let id = ElementId::Name(format!("{}-li-{}", id_prefix, idx).into());
        let content_el =
            render_inline_element(&item.content, base_size, on_link_click, Some(id), theme);

        let row = div()
            .flex()
            .flex_row()
            .pl(px(20.0))
            .mb(px(4.0))
            .child(
                div()
                    .flex_shrink_0()
                    .w(px(24.0))
                    .text_size(base_size)
                    .text_color(theme.tokens.muted_foreground)
                    .child(bullet),
            )
            .child(div().flex_1().child(content_el));

        elements.push(row.into_any_element());

        if !item.children.is_empty() {
            let children = render_list_items(
                &item.children,
                base_size,
                on_link_click,
                id_prefix,
                block_idx,
                false,
                1,
                theme,
            );
            elements.push(div().pl(px(20.0)).children(children).into_any_element());
        }
    }

    elements
}

/// 渲染表格。
fn render_table(
    headers: &[Vec<RichInline>],
    alignments: &[TableAlignment],
    rows: &[Vec<Vec<RichInline>>],
    base_size: Pixels,
    on_link_click: &Option<LinkClickHandler>,
    id_prefix: &str,
    block_idx: &mut u32,
    theme: &Theme,
) -> AnyElement {
    let col_count = headers.len();
    if col_count == 0 {
        return div().into_any_element();
    }

    let mut table = div()
        .mb(px(12.0))
        .w_full()
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.tokens.border)
        .overflow_hidden();

    *block_idx += 1;
    let header_row = {
        let mut row = div()
            .flex()
            .flex_row()
            .bg(theme.tokens.muted.opacity(0.3))
            .border_b_1()
            .border_color(theme.tokens.border);

        for (ci, header) in headers.iter().enumerate() {
            *block_idx += 1;
            let id = ElementId::Name(format!("{}-th-{}", id_prefix, *block_idx).into());
            let el = render_inline_element(header, base_size, on_link_click, Some(id), theme);
            let mut cell = div()
                .flex_1()
                .px(px(12.0))
                .py(px(8.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(base_size)
                .child(el);

            if ci < alignments.len() {
                cell = match &alignments[ci] {
                    TableAlignment::Center => cell.items_center(),
                    TableAlignment::Right => cell.items_end(),
                    TableAlignment::Left => cell,
                };
            }

            row = row.child(cell);
        }
        row
    };
    table = table.child(header_row);

    for (ri, row_data) in rows.iter().enumerate() {
        *block_idx += 1;
        let mut row = div()
            .flex()
            .flex_row()
            .when(ri < rows.len() - 1, |this| {
                this.border_b_1().border_color(theme.tokens.border)
            })
            .when(ri % 2 == 1, |this| this.bg(theme.tokens.muted.opacity(0.1)));

        for (ci, cell_data) in row_data.iter().enumerate() {
            *block_idx += 1;
            let id = ElementId::Name(format!("{}-td-{}", id_prefix, *block_idx).into());
            let el = render_inline_element(cell_data, base_size, on_link_click, Some(id), theme);
            let mut cell = div()
                .flex_1()
                .px(px(12.0))
                .py(px(6.0))
                .text_size(base_size)
                .child(el);

            if ci < alignments.len() {
                cell = match &alignments[ci] {
                    TableAlignment::Center => cell.items_center(),
                    TableAlignment::Right => cell.items_end(),
                    TableAlignment::Left => cell,
                };
            }

            row = row.child(cell);
        }
        table = table.child(row);
    }

    table.into_any_element()
}
