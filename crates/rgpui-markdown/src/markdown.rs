//! Markdown 渲染组件：将 Markdown 源码解析为富文本块并渲染。

use crate::rich_text::{
    LinkClickHandler, ListItem, RichBlock, RichInline, TableAlignment, render_blocks,
};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use rgpui::*;

/// Markdown 渲染组件。
#[derive(IntoElement)]
pub struct Markdown {
    /// 基础 Div。
    base: Div,
    /// Markdown 源码。
    source: SharedString,
    /// 基础字号。
    base_font_size: Option<Pixels>,
    /// 链接点击回调。
    on_link_click: Option<LinkClickHandler>,
}

impl Markdown {
    /// 创建 Markdown 组件。
    pub fn new(source: impl Into<SharedString>) -> Self {
        Self {
            base: div(),
            source: source.into(),
            base_font_size: None,
            on_link_click: None,
        }
    }

    /// 设置基础字号。
    pub fn base_font_size(mut self, size: Pixels) -> Self {
        self.base_font_size = Some(size);
        self
    }

    /// 设置链接点击回调。
    pub fn on_link_click(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_link_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Markdown {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let base_size = self.base_font_size.unwrap_or(px(14.0));

        let blocks = parse_markdown_with_urls(&self.source);
        let elements = render_blocks(&blocks, base_size, &self.on_link_click, "md", theme);

        self.base
            .flex()
            .flex_col()
            .font_family(theme.font_family.clone())
            .text_color(theme.tokens.foreground)
            .children(elements)
    }
}

impl Styled for Markdown {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

/// 将标题级别转为数字（1-6）。
fn heading_level_to_u8(level: &HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// 将行内元素转为纯文本（用于图片替代文本等）。
fn inlines_to_plain_text(inlines: &[RichInline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            RichInline::Text(s) => out.push_str(s),
            RichInline::Bold(children)
            | RichInline::Italic(children)
            | RichInline::Strikethrough(children) => {
                out.push_str(&inlines_to_plain_text(children));
            }
            RichInline::Code(s) => out.push_str(s),
            RichInline::Link { text, .. } => {
                out.push_str(&inlines_to_plain_text(text));
            }
            RichInline::Image { alt, .. } => out.push_str(alt),
            RichInline::LineBreak => out.push('\n'),
            RichInline::Html(_) => {}
            RichInline::Styled { children, .. } => {
                out.push_str(&inlines_to_plain_text(children));
            }
        }
    }
    out
}

/// 列表解析状态。
struct ListState {
    ordered: bool,
    start: u64,
    items: Vec<ListItem>,
    current_item_inlines: Vec<RichInline>,
    current_item_checked: Option<bool>,
}

/// 表格解析状态。
struct TableState {
    headers: Vec<Vec<RichInline>>,
    alignments: Vec<TableAlignment>,
    rows: Vec<Vec<Vec<RichInline>>>,
    current_row: Vec<Vec<RichInline>>,
    in_head: bool,
}

/// 解析 Markdown 源码为富文本块列表。
fn parse_markdown_with_urls(source: &str) -> Vec<RichBlock> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(source, options);
    let events: Vec<Event> = parser.collect();

    let mut builder = UrlTrackingBlockBuilder::new();
    builder.build(&events);
    builder.blocks
}

/// Markdown 事件流 → 富文本块的构建器。
struct UrlTrackingBlockBuilder {
    blocks: Vec<RichBlock>,
    inline_stack: Vec<Vec<RichInline>>,
    list_stack: Vec<ListState>,
    blockquote_depth: usize,
    blockquote_blocks: Vec<Vec<RichBlock>>,
    table_state: Option<TableState>,
    current_heading_level: Option<u8>,
    in_code_block: bool,
    code_block_lang: Option<String>,
    code_block_content: String,
    url_stack: Vec<String>,
}

impl UrlTrackingBlockBuilder {
    /// 创建构建器。
    fn new() -> Self {
        Self {
            blocks: Vec::new(),
            inline_stack: Vec::new(),
            list_stack: Vec::new(),
            blockquote_depth: 0,
            blockquote_blocks: Vec::new(),
            table_state: None,
            current_heading_level: None,
            in_code_block: false,
            code_block_lang: None,
            code_block_content: String::new(),
            url_stack: Vec::new(),
        }
    }

    /// 处理全部事件。
    fn build(&mut self, events: &[Event]) {
        for event in events {
            self.process_event(event);
        }
    }

    /// 处理单个事件。
    fn process_event(&mut self, event: &Event) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.text(text),
            Event::Code(code) => self.push_inline(RichInline::Code(code.to_string())),
            Event::SoftBreak => self.push_inline(RichInline::Text(" ".to_string())),
            Event::HardBreak => self.push_inline(RichInline::LineBreak),
            Event::Rule => self.push_block(RichBlock::HorizontalRule),
            Event::Html(html) => self.push_inline(RichInline::Html(html.to_string())),
            Event::TaskListMarker(checked) => {
                if let Some(list) = self.list_stack.last_mut() {
                    list.current_item_checked = Some(*checked);
                }
            }
            _ => {}
        }
    }

    /// 处理开始标签。
    fn start_tag(&mut self, tag: &Tag) {
        match tag {
            Tag::Paragraph => {
                self.inline_stack.push(Vec::new());
            }
            Tag::Heading { level, .. } => {
                self.current_heading_level = Some(heading_level_to_u8(level));
                self.inline_stack.push(Vec::new());
            }
            Tag::BlockQuote(_) => {
                self.blockquote_depth += 1;
                self.blockquote_blocks.push(Vec::new());
            }
            Tag::CodeBlock(kind) => {
                self.in_code_block = true;
                self.code_block_content.clear();
                match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                        let lang_str = lang.to_string();
                        self.code_block_lang = if lang_str.is_empty() {
                            None
                        } else {
                            Some(lang_str)
                        };
                    }
                    pulldown_cmark::CodeBlockKind::Indented => {
                        self.code_block_lang = None;
                    }
                }
            }
            Tag::List(start) => {
                self.list_stack.push(ListState {
                    ordered: start.is_some(),
                    start: start.unwrap_or(1),
                    items: Vec::new(),
                    current_item_inlines: Vec::new(),
                    current_item_checked: None,
                });
            }
            Tag::Item => {
                if let Some(list) = self.list_stack.last_mut() {
                    list.current_item_inlines.clear();
                    list.current_item_checked = None;
                }
                self.inline_stack.push(Vec::new());
            }
            Tag::Strong => {
                self.inline_stack.push(Vec::new());
            }
            Tag::Emphasis => {
                self.inline_stack.push(Vec::new());
            }
            Tag::Strikethrough => {
                self.inline_stack.push(Vec::new());
            }
            Tag::Link { dest_url, .. } => {
                self.url_stack.push(dest_url.to_string());
                self.inline_stack.push(Vec::new());
            }
            Tag::Image { dest_url, .. } => {
                self.url_stack.push(dest_url.to_string());
                self.inline_stack.push(Vec::new());
            }
            Tag::Table(alignments) => {
                self.table_state = Some(TableState {
                    headers: Vec::new(),
                    alignments: alignments
                        .iter()
                        .map(|a| match a {
                            pulldown_cmark::Alignment::Left => TableAlignment::Left,
                            pulldown_cmark::Alignment::Center => TableAlignment::Center,
                            pulldown_cmark::Alignment::Right => TableAlignment::Right,
                            pulldown_cmark::Alignment::None => TableAlignment::Left,
                        })
                        .collect(),
                    rows: Vec::new(),
                    current_row: Vec::new(),
                    in_head: false,
                });
            }
            Tag::TableHead => {
                if let Some(ref mut ts) = self.table_state {
                    ts.in_head = true;
                    ts.current_row.clear();
                }
            }
            Tag::TableRow => {
                if let Some(ref mut ts) = self.table_state {
                    ts.current_row.clear();
                }
            }
            Tag::TableCell => {
                self.inline_stack.push(Vec::new());
            }
            _ => {}
        }
    }

    /// 处理结束标签。
    fn end_tag(&mut self, tag: &TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                let inlines = self.inline_stack.pop().unwrap_or_default();
                self.push_block(RichBlock::Paragraph(inlines));
            }
            TagEnd::Heading(_level) => {
                let inlines = self.inline_stack.pop().unwrap_or_default();
                let lvl = self.current_heading_level.take().unwrap_or(1);
                self.push_block(RichBlock::Heading {
                    level: lvl,
                    content: inlines,
                });
            }
            TagEnd::BlockQuote(_) => {
                self.blockquote_depth -= 1;
                let inner = self.blockquote_blocks.pop().unwrap_or_default();
                self.push_block(RichBlock::BlockQuote(inner));
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                let code = std::mem::take(&mut self.code_block_content);
                let lang = self.code_block_lang.take();
                self.push_block(RichBlock::CodeBlock {
                    language: lang,
                    code,
                });
            }
            TagEnd::List(_ordered) => {
                if let Some(list) = self.list_stack.pop() {
                    let block = if list.ordered {
                        RichBlock::OrderedList {
                            start: list.start,
                            items: list.items,
                        }
                    } else {
                        RichBlock::UnorderedList { items: list.items }
                    };
                    self.push_block(block);
                }
            }
            TagEnd::Item => {
                let inlines = self.inline_stack.pop().unwrap_or_default();
                if let Some(list) = self.list_stack.last_mut() {
                    list.items.push(ListItem {
                        checked: list.current_item_checked,
                        content: inlines,
                        children: Vec::new(),
                    });
                }
            }
            TagEnd::Strong => {
                let children = self.inline_stack.pop().unwrap_or_default();
                self.push_inline(RichInline::Bold(children));
            }
            TagEnd::Emphasis => {
                let children = self.inline_stack.pop().unwrap_or_default();
                self.push_inline(RichInline::Italic(children));
            }
            TagEnd::Strikethrough => {
                let children = self.inline_stack.pop().unwrap_or_default();
                self.push_inline(RichInline::Strikethrough(children));
            }
            TagEnd::Link => {
                let children = self.inline_stack.pop().unwrap_or_default();
                let url = self.url_stack.pop().unwrap_or_default();
                self.push_inline(RichInline::Link {
                    text: children,
                    url,
                });
            }
            TagEnd::Image => {
                let alt_inlines = self.inline_stack.pop().unwrap_or_default();
                let alt = inlines_to_plain_text(&alt_inlines);
                let url = self.url_stack.pop().unwrap_or_default();
                self.push_block(RichBlock::Image { alt, url });
            }
            TagEnd::Table => {
                if let Some(ts) = self.table_state.take() {
                    self.push_block(RichBlock::Table {
                        headers: ts.headers,
                        alignments: ts.alignments,
                        rows: ts.rows,
                    });
                }
            }
            TagEnd::TableHead => {
                if let Some(ref mut ts) = self.table_state {
                    ts.headers = std::mem::take(&mut ts.current_row);
                    ts.in_head = false;
                }
            }
            TagEnd::TableRow => {
                if let Some(ref mut ts) = self.table_state {
                    if !ts.in_head {
                        let row = std::mem::take(&mut ts.current_row);
                        ts.rows.push(row);
                    }
                }
            }
            TagEnd::TableCell => {
                let inlines = self.inline_stack.pop().unwrap_or_default();
                if let Some(ref mut ts) = self.table_state {
                    ts.current_row.push(inlines);
                }
            }
            _ => {}
        }
    }

    /// 处理文本事件。
    fn text(&mut self, text: &str) {
        if self.in_code_block {
            self.code_block_content.push_str(text);
            return;
        }
        self.push_inline(RichInline::Text(text.to_string()));
    }

    /// 压入行内元素。
    fn push_inline(&mut self, inline: RichInline) {
        if let Some(stack) = self.inline_stack.last_mut() {
            stack.push(inline);
        }
    }

    /// 压入块元素（区分是否在块引用内）。
    fn push_block(&mut self, block: RichBlock) {
        if self.blockquote_depth > 0 {
            if let Some(blocks) = self.blockquote_blocks.last_mut() {
                blocks.push(block);
                return;
            }
        }
        self.blocks.push(block);
    }
}
