//! Markdown 渲染组件：将 Markdown 源码解析为富文本块并渲染。

use crate::rich_text::{
    LinkClickHandler, ListItem, RichBlock, RichInline, TableAlignment, render_blocks,
};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use rgpui::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::{Arc, Mutex};

/// 解析缓存：单条目（最近一次文档全文 → 块列表 + 块源码区间）。
///
/// `Markdown::render` 每帧都会调用解析；分屏拖拽等高频重渲染场景下文本不变，
/// 命中缓存可省去 pulldown 全文解析（大文档 debug 下可达数十毫秒）。
/// 值用 `Arc` 共享，避免每帧深拷贝全部块字符串；键为内容哈希，冲突概率可忽略，
/// 且最坏情况也只是某一帧显示旧内容。
static PARSE_CACHE: Mutex<Option<(u64, Arc<(Vec<RichBlock>, Vec<Range<usize>>)>)>> =
    Mutex::new(None);

/// 带单条目缓存的解析：内容不变直接复用上次结果（`Arc` 共享，无深拷贝）。
fn parse_markdown_cached(source: &str) -> Arc<(Vec<RichBlock>, Vec<Range<usize>>)> {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    let hash = hasher.finish();

    if let Ok(guard) = PARSE_CACHE.lock() {
        if let Some((cached_hash, parsed)) = guard.as_ref() {
            if *cached_hash == hash {
                return parsed.clone();
            }
        }
    }

    let parsed = Arc::new(parse_markdown_with_urls(source));
    if let Ok(mut guard) = PARSE_CACHE.lock() {
        *guard = Some((hash, parsed.clone()));
    }
    parsed
}

/// 解析 Markdown 源码为富文本块 + 顶层块源码区间（带单条目缓存）。
///
/// 供虚拟化、WYSIWYG 块编辑等按块处理的场景使用：调用方按需渲染块区间，
/// 用区间把编辑结果拼回全文，避免全文建树。
pub fn parse_markdown(source: &str) -> Arc<(Vec<RichBlock>, Vec<Range<usize>>)> {
    parse_markdown_cached(source)
}

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

        let parsed = parse_markdown_cached(&self.source);
        let elements = render_blocks(&parsed.0, base_size, &self.on_link_click, "md", theme);

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

/// 解析 Markdown 源码为富文本块列表（含顶层块源码区间，一一对应）。
fn parse_markdown_with_urls(source: &str) -> (Vec<RichBlock>, Vec<Range<usize>>) {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(source, options);
    let events: Vec<(Event, Range<usize>)> = parser.into_offset_iter().collect();

    let mut builder = UrlTrackingBlockBuilder::new();
    builder.build(&events);
    let mut spans = builder.spans;
    fill_span_gaps(&mut spans, source.len());
    debug_assert_eq!(spans.len(), builder.blocks.len(), "块与区间必须一一对应");
    (builder.blocks, spans)
}

/// Markdown 事件流 → 富文本块的构建器。
struct UrlTrackingBlockBuilder {
    blocks: Vec<RichBlock>,
    /// 顶层块源码区间，与 `blocks` 一一对应（push 位点同步记录，天然对齐）。
    spans: Vec<Range<usize>>,
    /// 当前顶层块的起始字节：块级 Start 首次出现时置位，push 时消费；
    /// 嵌套块不覆盖外层；单事件块（Rule/行内图片）不用它，直接取自身区间。
    pending_span_start: Option<usize>,
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
            spans: Vec::new(),
            pending_span_start: None,
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

    /// 处理全部事件（带源码区间）。
    fn build(&mut self, events: &[(Event<'_>, Range<usize>)]) {
        for (event, range) in events {
            self.process_event(event, range);
        }
    }

    /// 取出待定的块起始位置（配对块用；缺失时退化为空区间，不断对齐）。
    fn take_pending_span(&mut self, range_end: usize) -> Range<usize> {
        let start = self.pending_span_start.take().unwrap_or(range_end);
        start..range_end
    }

    /// 处理单个事件。
    fn process_event(&mut self, event: &Event, range: &Range<usize>) {
        match event {
            Event::Start(tag) => self.start_tag(tag, range),
            Event::End(tag) => self.end_tag(tag, range),
            Event::Text(text) => self.text(text),
            Event::Code(code) => self.push_inline(RichInline::Code(code.to_string())),
            Event::SoftBreak => self.push_inline(RichInline::Text(" ".to_string())),
            Event::HardBreak => self.push_inline(RichInline::LineBreak),
            // 单事件块：区间即自身范围，不碰 pending（可能正处在外层块内部）。
            Event::Rule => {
                let span = range.clone();
                self.push_block(RichBlock::HorizontalRule, span);
            }
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
    fn start_tag(&mut self, tag: &Tag, range: &Range<usize>) {
        // 块级 Start 首次出现时记录区间起点（嵌套块不覆盖外层；行内标签不在集合内）。
        match tag {
            Tag::Paragraph
            | Tag::Heading { .. }
            | Tag::BlockQuote(_)
            | Tag::CodeBlock(_)
            | Tag::List(_)
            | Tag::Table(_) => {
                if self.pending_span_start.is_none() {
                    self.pending_span_start = Some(range.start);
                }
            }
            _ => {}
        }
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
    fn end_tag(&mut self, tag: &TagEnd, range: &Range<usize>) {
        match tag {
            TagEnd::Paragraph => {
                let inlines = self.inline_stack.pop().unwrap_or_default();
                let span = self.take_pending_span(range.end);
                self.push_block(RichBlock::Paragraph(inlines), span);
            }
            TagEnd::Heading(_level) => {
                let inlines = self.inline_stack.pop().unwrap_or_default();
                let lvl = self.current_heading_level.take().unwrap_or(1);
                let span = self.take_pending_span(range.end);
                self.push_block(
                    RichBlock::Heading {
                        level: lvl,
                        content: inlines,
                    },
                    span,
                );
            }
            TagEnd::BlockQuote(_) => {
                self.blockquote_depth -= 1;
                let inner = self.blockquote_blocks.pop().unwrap_or_default();
                let span = self.take_pending_span(range.end);
                self.push_block(RichBlock::BlockQuote(inner), span);
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                let code = std::mem::take(&mut self.code_block_content);
                let lang = self.code_block_lang.take();
                let span = self.take_pending_span(range.end);
                self.push_block(
                    RichBlock::CodeBlock {
                        language: lang,
                        code,
                    },
                    span,
                );
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
                    let span = self.take_pending_span(range.end);
                    self.push_block(block, span);
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
                // 行内图片也可能抽成独立块：用自身区间，不碰外层 pending。
                let span = range.clone();
                self.push_block(RichBlock::Image { alt, url }, span);
            }
            TagEnd::Table => {
                if let Some(ts) = self.table_state.take() {
                    let span = self.take_pending_span(range.end);
                    self.push_block(
                        RichBlock::Table {
                            headers: ts.headers,
                            alignments: ts.alignments,
                            rows: ts.rows,
                        },
                        span,
                    );
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
    /// 顶层块同步记录源码区间，与 `blocks` 天然对齐；引用内块不记录。
    fn push_block(&mut self, block: RichBlock, span: Range<usize>) {
        if self.blockquote_depth > 0 {
            if let Some(blocks) = self.blockquote_blocks.last_mut() {
                blocks.push(block);
                return;
            }
        }
        self.spans.push(span);
        self.blocks.push(block);
    }
}

/// 块区间后处理：向前吞掉块间空白（上一块末尾→本块开头），首块扩展到 0，
/// 末块扩展到文末。已重叠的不动（如行内图片抽出的块）。保证可编辑区域
/// 全覆盖、无缝隙。
fn fill_span_gaps(spans: &mut [Range<usize>], doc_len: usize) {
    if spans.is_empty() {
        return;
    }
    if spans[0].start > 0 {
        spans[0].start = 0;
    }
    for ix in 1..spans.len() {
        let prev_end = spans[ix - 1].end;
        if prev_end < spans[ix].start {
            spans[ix].start = prev_end;
        }
    }
    if let Some(last) = spans.last_mut() {
        if last.end < doc_len {
            last.end = doc_len;
        }
    }
}

#[cfg(test)]
mod tests {
    // 显式导入：父模块 `use rgpui::*` 引入的 `test` 模块名会遮蔽内置属性。
    use super::{parse_markdown_cached, parse_markdown_with_urls};

    /// 缓存正确性：相同内容复用解析结果，不同内容重新解析（含中文）。
    #[test]
    fn parse_cache_reuses_same_content() {
        let doc = "# 标题\n\n段落运\n\n```rust\nfn f() {}\n```\n";
        let first = parse_markdown_cached(doc);
        assert!(!first.0.is_empty());
        let second = parse_markdown_cached(doc);
        assert_eq!(format!("{first:?}"), format!("{second:?}"));
        // 不同内容必须重新解析（块数不同）。
        let other = parse_markdown_cached("# 只有一个标题\n");
        assert!(format!("{other:?}") != format!("{first:?}"));
        // 切回原文仍正确。
        let third = parse_markdown_cached(doc);
        assert_eq!(format!("{first:?}"), format!("{third:?}"));
    }

    /// 块与区间一一对应；区间覆盖全文、无缝隙、切片合法（含嵌套/表格/代码/图片）。
    #[test]
    fn block_spans_align_and_tile() {
        let doc = concat!(
            "# 标题一\n",
            "\n",
            "段落一运。\n",
            "\n",
            "> 引用行一\n> 引用行二\n",
            "\n",
            "```rust\nfn f() {}\n```\n",
            "\n",
            "- 列表项一\n- 列表项二\n",
            "\n",
            "1. 有序一\n2. 有序二\n",
            "\n",
            "| 甲 | 乙 |\n| --- | --- |\n| 1 | 2 |\n",
            "\n",
            "文字 ![图](u.png) 更多\n",
            "\n",
            "---\n",
            "\n",
            "尾段。\n",
        );
        let (blocks, spans) = parse_markdown_with_urls(doc);
        assert!(!blocks.is_empty());
        assert_eq!(spans.len(), blocks.len(), "块与区间必须一一对应");
        // 首块从 0 开始、末块到文末、相邻块无缝隙（允许重叠：行内图片抽出）。
        assert_eq!(spans[0].start, 0);
        assert_eq!(spans.last().unwrap().end, doc.len());
        for pair in spans.windows(2) {
            // 相邻块无缝隙（允许重叠：行内图片抽出的块与其段落重叠）。
            assert!(pair[1].start <= pair[0].end, "块间不应有缝隙：{pair:?}");
        }
        // 每个区间都是合法 UTF-8 切片。
        for span in &spans {
            assert!(doc.is_char_boundary(span.start));
            assert!(doc.is_char_boundary(span.end));
            let _ = &doc[span.clone()];
        }
    }
}
