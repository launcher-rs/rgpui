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
//! 结构化能力（都**不引入新的文本空间**，结构只引用文档段落，因此字符坐标编辑与
//! 批注 / 脚注区间对结构内文本同样生效）：
//! - 表格 [`RichTable`]（[`RichTextDoc::add_table`]）；
//! - 节 / 分栏 [`RichSection`]（[`RichTextDoc::add_section`]）；
//! - 批注 [`Annotation`]、脚注 [`Footnote`]（区间随编辑自动平移）；
//! - 修订标记（[`RunProps::inserted`] / [`RunProps::deleted`]）。
//!
//! **本版暂缓**（后续增补）：项目符号字体与层级样式、可编辑光标与选区（现为只读渲染）。

use std::ops::Range;
use std::sync::Arc;

use crate::prelude::ActiveTheme as _;
use crate::{
    AnyElement, App, Div, ElementId, FontWeight, Hsla, InteractiveText, IntoElement,
    ParentElement as _, Pixels, RenderOnce, Rgba, SharedString, StrikethroughStyle, Styled as _,
    StyledText, TextAlign, TextRun, Theme, UnderlineStyle, Window, div, px, relative, v_flex,
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

/// 行内富文本属性：粗体/斜体/下划线/删除线/行内代码 + 修订标记 + 可选前景色。
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
    /// 是否行内代码（等宽字体 + 浅底）。
    pub code: bool,
    /// 是否为修订插入（渲染为主色 + 下划线）。
    pub inserted: bool,
    /// 是否为修订删除（渲染为删除线 + 变暗；文本仍留在文档里）。
    pub deleted: bool,
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

    /// 置为行内代码。
    pub fn code(mut self) -> Self {
        self.code = true;
        self
    }

    /// 置为修订插入（主色 + 下划线）。
    pub fn inserted(mut self) -> Self {
        self.inserted = true;
        self
    }

    /// 置为修订删除（删除线 + 变暗；文本仍留在文档里）。
    pub fn deleted(mut self) -> Self {
        self.deleted = true;
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

/// 段落级样式：对齐、缩进层级、列表标记、标题级别、行高、段前距。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ParagraphStyle {
    /// 水平对齐。
    pub align: TextAlign,
    /// 缩进层级（0..=8，每级 [`INDENT_PX`] 像素）。
    pub indent: u8,
    /// 列表标记。
    pub list: ListKind,
    /// 标题级别（1–6；`None` 表示普通段落）。
    pub heading: Option<u8>,
    /// 行高倍数（`None` 表示用外层默认行高）。
    pub line_height: Option<f32>,
    /// 段前距（`None` 表示无额外间距）。
    pub space_before: Option<Pixels>,
    /// 是否为引用块内容（渲染带左边线与浅底，HTML 输出 `<blockquote>`）。
    pub quote: bool,
}

impl ParagraphStyle {
    /// 设置缩进层级（自动夹取到 0..=8）。
    pub fn indent(mut self, level: u8) -> Self {
        self.indent = level.min(MAX_INDENT);
        self
    }

    /// 设置标题级别（自动夹取到 1..=6）。
    pub fn heading(mut self, level: u8) -> Self {
        self.heading = Some(level.clamp(1, 6));
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
    /// 链接地址（`None` 表示普通文本）。
    pub link: Option<String>,
}

impl Run {
    /// 以默认属性创建文本运行。
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            props: RunProps::default(),
            link: None,
        }
    }

    /// 以指定属性创建文本运行。
    pub fn with(text: impl Into<String>, props: RunProps) -> Self {
        Self {
            text: text.into(),
            props,
            link: None,
        }
    }

    /// 以指定属性创建**链接**文本运行。
    pub fn with_link(text: impl Into<String>, props: RunProps, link: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            props,
            link: Some(link.into()),
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
                link: self.link.clone(),
            }),
            Some(Run {
                text: tail.to_string(),
                props: self.props,
                link: self.link.clone(),
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
            link: None,
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
                // 链接不同的运行不能合并（否则链接地址会丢失）。
                && last.link == run.link
            {
                last.text.push_str(&run.text);
                continue;
            }
            out.push(run);
        }
        self.runs = out;
    }
}

/// 批注：挂在文档字符区间上的备注（不改动正文内容）。
///
/// 区间为文档级字符偏移（左闭右开）；正文插入 / 删除时随编辑自动平移。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Annotation {
    /// 批注标识（文档内唯一，从 1 开始）。
    pub id: u32,
    /// 批注覆盖的字符区间。
    pub range: Range<usize>,
    /// 批注正文。
    pub note: String,
    /// 批注作者（可选）。
    pub author: Option<String>,
}

/// 脚注：正文中的引用标记 + 文末脚注文本。
///
/// 引用标记是正文里的普通字符（插入时一并写入），区间随编辑自动平移。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Footnote {
    /// 脚注序号（文档内唯一，从 1 开始）。
    pub id: u32,
    /// 正文引用标记所在的字符区间。
    pub range: Range<usize>,
    /// 脚注文本。
    pub text: String,
}

/// 文档历史：快照式撤销 / 重做栈。
///
/// 用法：`history.edit(&mut doc, |doc| { …修改… })` 会在修改前压入快照；
/// `undo` / `redo` 用快照整份替换文档。快照是文档拷贝，适合中小文档；
/// 超大文档可改用差分 / 块级快照（后续增补）。
#[derive(Clone, Debug, Default)]
pub struct RichTextHistory {
    /// 撤销栈（栈顶是最近一次编辑**前**的状态）。
    undo: Vec<RichTextDoc>,
    /// 重做栈（栈顶是最近一次撤销前的状态）。
    redo: Vec<RichTextDoc>,
}

/// 历史栈条目上限（超出后丢弃最旧的快照，限制内存占用）。
const HISTORY_LIMIT: usize = 100;

impl RichTextHistory {
    /// 创建历史（此时无可撤销的编辑）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 是否可撤销。
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// 是否可重做。
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// 清空历史（文档被整体替换时调用）。
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// 在历史中执行一次编辑：先压入编辑前的快照，再执行 `f`。
    ///
    /// 返回 `f` 是否真的改动了文档（未改动时不压栈，避免产生空撤销步骤）。
    pub fn edit(&mut self, doc: &mut RichTextDoc, f: impl FnOnce(&mut RichTextDoc)) -> bool {
        let before = doc.clone();
        f(doc);
        if *doc == before {
            return false;
        }
        self.push_undo(before);
        // 新编辑后原重做链失效。
        self.redo.clear();
        true
    }

    /// 手动记录一次编辑前的状态（供在历史之外修改文档的路径使用）。
    pub fn record(&mut self, doc: &RichTextDoc) {
        self.push_undo(doc.clone());
        self.redo.clear();
    }

    /// 撤销一步：撤销栈顶快照写回文档，当前文档压入重做栈。
    pub fn undo(&mut self, doc: &mut RichTextDoc) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(std::mem::replace(doc, previous));
        true
    }

    /// 重做一步：重做栈顶快照写回文档，当前文档压回撤销栈。
    pub fn redo(&mut self, doc: &mut RichTextDoc) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(doc, next));
        true
    }

    /// 压入撤销栈（超出上限时丢弃最旧的一条）。
    fn push_undo(&mut self, snapshot: RichTextDoc) {
        self.undo.push(snapshot);
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
    }
}

/// 文档统计（字符数 / 段落数 / 行数）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DocStats {
    /// 字符数（不含段落分隔符）。
    pub chars: usize,
    /// 段落数。
    pub paragraphs: usize,
    /// 行数（段落内换行另计）。
    pub lines: usize,
}

/// 表格列对齐方式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TableAlign {
    /// 左对齐。
    #[default]
    Left,
    /// 居中。
    Center,
    /// 右对齐。
    Right,
}

/// 表格：单元格内容复用文档段落（段落索引即单元格位置）。
///
/// 表格不引入新的文本空间——单元格就是普通段落，因此字符坐标编辑、批注 / 脚注区间
/// 等既有机制对表格内文本同样生效。渲染时表格出现在其**首个单元格段落**的位置。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RichTable {
    /// 表头行的单元格段落索引。
    pub header: Vec<usize>,
    /// 数据行的单元格段落索引。
    pub rows: Vec<Vec<usize>>,
    /// 各列对齐方式（与列数一致；缺省左对齐）。
    pub alignments: Vec<TableAlign>,
}

/// 表格解析中间态。
#[derive(Default)]
struct TableBuilder {
    /// 表头行的单元格段落索引。
    header: Vec<usize>,
    /// 数据行的单元格段落索引。
    rows: Vec<Vec<usize>>,
    /// 当前行的单元格段落索引。
    current_row: Vec<usize>,
    /// 当前行是否表头行（出现过 `<th>`）。
    current_is_header: bool,
    /// 各列对齐。
    alignments: Vec<TableAlign>,
}

/// 节解析中间态。
#[derive(Default)]
struct SectionBuilder {
    /// 节内首个段落的索引（节内段落连续）。
    start: usize,
    /// 节标题（来自 `data-title` 属性）。
    title: Option<String>,
}
/// 文档节：一组段落 + 分栏排版。
///
/// 与表格同理，节内段落复用文档段落（不引入新的文本空间）：节出现在其**首个段落**
/// 所在位置，其余段落不再单独输出。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RichSection {
    /// 节内段落索引（按顺序）。
    pub paragraphs: Vec<usize>,
    /// 分栏数（1 表示不分栏；上限见 `MAX_SECTION_COLUMNS`）。
    pub columns: u8,
    /// 节标题（可选，渲染在栏上方）。
    pub title: Option<String>,
}

/// 分栏数上限（超出按此夹取）。
const MAX_SECTION_COLUMNS: u8 = 4;

/// 富文本文档：段落序列。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RichTextDoc {
    /// 段落序列，至少一段。
    pub paragraphs: Vec<Paragraph>,
    /// 批注列表（挂在字符区间上的备注）。
    pub annotations: Vec<Annotation>,
    /// 脚注列表（正文引用点 + 文末脚注文本）。
    pub footnotes: Vec<Footnote>,
    /// 表格列表（单元格复用段落；渲染时按首个单元格段落的位置插入）。
    pub tables: Vec<RichTable>,
    /// 节列表（段落复用；渲染时按首个段落的位置插入，可分栏）。
    pub sections: Vec<RichSection>,
}

impl RichTextDoc {
    /// 空文档（含一个空段落）。
    pub fn new() -> Self {
        Self {
            paragraphs: vec![Paragraph::default()],
            annotations: Vec::new(),
            footnotes: Vec::new(),
            tables: Vec::new(),
            sections: Vec::new(),
        }
    }

    /// 从纯文本构造（按 `\n` 切分段落）。
    pub fn from_plain_text(text: &str) -> Self {
        Self {
            paragraphs: text.split('\n').map(Paragraph::new).collect(),
            annotations: Vec::new(),
            footnotes: Vec::new(),
            tables: Vec::new(),
            sections: Vec::new(),
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

    /// 添加批注：覆盖字符区间 `range`（超界自动夹取），返回批注 id。
    pub fn add_annotation(&mut self, range: Range<usize>, note: impl Into<String>) -> u32 {
        let total = self.char_len();
        let start = range.start.min(total);
        let end = range.end.min(total).max(start);
        let id = self.next_annotation_id();
        self.annotations.push(Annotation {
            id,
            range: start..end,
            note: note.into(),
            author: None,
        });
        id
    }

    /// 按 id 取批注（只读）。
    pub fn annotation(&self, id: u32) -> Option<&Annotation> {
        self.annotations.iter().find(|item| item.id == id)
    }

    /// 按 id 取批注（可写，用于设置作者等）。
    pub fn annotation_mut(&mut self, id: u32) -> Option<&mut Annotation> {
        self.annotations.iter_mut().find(|item| item.id == id)
    }

    /// 取覆盖某字符位置的所有批注（按区间起点排序）。
    pub fn annotations_at(&self, index: usize) -> Vec<&Annotation> {
        let mut hits: Vec<&Annotation> = self
            .annotations
            .iter()
            .filter(|item| item.range.start <= index && index < item.range.end)
            .collect();
        hits.sort_by_key(|item| item.range.start);
        hits
    }

    /// 删除批注记录（正文文字保留），返回是否删到。
    pub fn remove_annotation(&mut self, id: u32) -> bool {
        let before = self.annotations.len();
        self.annotations.retain(|item| item.id != id);
        self.annotations.len() != before
    }

    /// 插入脚注引用标记并登记脚注，返回脚注 id。
    ///
    /// `marker` 是写入正文的引用标记（如 `"*"`、`"¹"`），`text` 是文末脚注内容。
    pub fn insert_footnote(&mut self, index: usize, marker: &str, text: impl Into<String>) -> u32 {
        let total = self.char_len();
        let index = index.min(total);
        let id = self.next_footnote_id();
        // 先写标记文本（会顺带平移既有区间），再登记本脚注的引用区间。
        self.insert_text(index, marker, RunProps::default());
        let len = marker.chars().count();
        self.footnotes.push(Footnote {
            id,
            range: index..index + len,
            text: text.into(),
        });
        id
    }

    /// 按 id 取脚注。
    pub fn footnote(&self, id: u32) -> Option<&Footnote> {
        self.footnotes.iter().find(|item| item.id == id)
    }

    /// 删除脚注记录与正文中的引用标记，返回是否删到。
    pub fn remove_footnote(&mut self, id: u32) -> bool {
        let Some(pos) = self.footnotes.iter().position(|item| item.id == id) else {
            return false;
        };
        let range = self.footnotes[pos].range.clone();
        self.footnotes.remove(pos);
        // 引用标记是正文普通字符，一并删除（其余区间随之平移）。
        self.delete(range);
        true
    }

    /// 追加表格：单元格文本按「表头 → 数据行」建段落，返回表格索引。
    ///
    /// 单元格是普通段落（可继续编辑，字符坐标与批注 / 脚注区间同样生效）；
    /// 表格渲染在其首个单元格段落所在的位置。
    pub fn add_table(
        &mut self,
        header: &[&str],
        rows: &[Vec<&str>],
        alignments: Vec<TableAlign>,
    ) -> usize {
        let mut table = RichTable {
            header: Vec::new(),
            rows: Vec::new(),
            alignments,
        };
        for cell in header {
            table.header.push(self.paragraphs.len());
            self.paragraphs.push(Paragraph::new(cell));
        }
        for row in rows {
            let mut indices = Vec::with_capacity(row.len());
            for cell in row {
                indices.push(self.paragraphs.len());
                self.paragraphs.push(Paragraph::new(cell));
            }
            table.rows.push(indices);
        }
        self.tables.push(table);
        self.tables.len() - 1
    }

    /// 标记区间为修订插入（渲染为主色 + 下划线）。
    pub fn mark_inserted(&mut self, range: Range<usize>) {
        self.apply_props(range, |props| props.inserted = true);
    }

    /// 标记区间为修订删除（文本保留在文档里，渲染为删除线 + 变暗）。
    pub fn mark_deleted(&mut self, range: Range<usize>) {
        self.apply_props(range, |props| props.deleted = true);
    }

    /// 清除区间上的修订标记。
    pub fn clear_revisions(&mut self, range: Range<usize>) {
        self.apply_props(range, |props| {
            props.inserted = false;
            props.deleted = false;
        });
    }

    /// 有效文本（不含修订删除的内容；复制 / 导出用）。
    pub fn effective_text(&self) -> String {
        let mut out = String::new();
        for (index, para) in self.paragraphs.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            for run in &para.runs {
                if run.props.deleted {
                    continue;
                }
                out.push_str(&run.text);
            }
        }
        out
    }

    /// 导出为 Markdown 文本。
    ///
    /// 覆盖标题级别、列表、引用块、行内样式（粗 / 斜 / 删除线 / 行内代码）、链接、
    /// 表格与节（节标题按二级标题输出）；修订标记用内联 HTML（`<ins>` / `<del>`）表达。
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        // 段落 → 表格 / 节归属（与渲染同款：按首个单元格 / 段落插入）。
        let mut table_at: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        let mut table_cells: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for (table_index, table) in self.tables.iter().enumerate() {
            let mut first: Option<usize> = None;
            for para_index in table.header.iter().chain(table.rows.iter().flatten()) {
                table_cells.insert(*para_index);
                first.get_or_insert(*para_index);
            }
            if let Some(first) = first {
                table_at.insert(first, table_index);
            }
        }
        let mut section_at: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        let mut section_paragraphs: std::collections::HashSet<usize> =
            std::collections::HashSet::new();
        for (section_index, section) in self.sections.iter().enumerate() {
            let mut first: Option<usize> = None;
            for para_index in &section.paragraphs {
                section_paragraphs.insert(*para_index);
                first.get_or_insert(*para_index);
            }
            if let Some(first) = first {
                section_at.insert(first, section_index);
            }
        }

        let mut index = 0usize;
        while index < self.paragraphs.len() {
            // 表格：整表输出一次。
            if let Some(table_index) = table_at.get(&index) {
                push_table_markdown(&mut out, &self.tables[*table_index], &self.paragraphs);
                while index < self.paragraphs.len() && table_cells.contains(&index) {
                    index += 1;
                }
                continue;
            }
            // 节：标题按二级标题输出，节内段落照常。
            if let Some(section_index) = section_at.get(&index) {
                if let Some(title) = &self.sections[*section_index].title {
                    out.push_str("## ");
                    out.push_str(title);
                    out.push('\n');
                    out.push('\n');
                }
            }
            if section_paragraphs.contains(&index) {
                index += 1;
                continue;
            }
            // 普通段落。
            let para = &self.paragraphs[index];
            match para.style.list {
                ListKind::Bullet => {
                    out.push_str("- ");
                    out.push_str(&runs_to_markdown(&para.runs));
                    out.push('\n');
                }
                ListKind::Number => {
                    out.push_str("1. ");
                    out.push_str(&runs_to_markdown(&para.runs));
                    out.push('\n');
                }
                ListKind::None => {
                    if para.style.quote {
                        out.push_str("> ");
                    }
                    if let Some(level) = para.style.heading {
                        for _ in 0..level {
                            out.push('#');
                        }
                        out.push(' ');
                    }
                    out.push_str(&runs_to_markdown(&para.runs));
                    out.push('\n');
                }
            }
            index += 1;
        }
        out
    }

    /// 取表格（按索引）。
    pub fn table(&self, index: usize) -> Option<&RichTable> {
        self.tables.get(index)
    }

    /// 追加文档节：段落按顺序建段并登记，返回节索引。
    ///
    /// `columns` 为分栏数（1 表示不分栏，上限 4）；`title` 为节标题（可选）。
    /// 节内段落复用文档段落，出现在其首个段落所在位置。
    pub fn add_section(&mut self, paragraphs: &[&str], columns: u8, title: Option<&str>) -> usize {
        let mut section = RichSection {
            paragraphs: Vec::new(),
            columns: columns.clamp(1, MAX_SECTION_COLUMNS),
            title: title.map(|title| title.to_string()),
        };
        for text in paragraphs {
            section.paragraphs.push(self.paragraphs.len());
            self.paragraphs.push(Paragraph::new(text));
        }
        self.sections.push(section);
        self.sections.len() - 1
    }

    /// 取节（按索引）。
    pub fn section(&self, index: usize) -> Option<&RichSection> {
        self.sections.get(index)
    }

    /// 下一个批注 id（从 1 开始，跳过已用编号）。
    fn next_annotation_id(&self) -> u32 {
        let mut id = 1u32;
        while self.annotations.iter().any(|item| item.id == id) {
            id += 1;
        }
        id
    }

    /// 下一个脚注 id（从 1 开始，跳过已用编号）。
    fn next_footnote_id(&self) -> u32 {
        let mut id = 1u32;
        while self.footnotes.iter().any(|item| item.id == id) {
            id += 1;
        }
        id
    }

    /// 查找子串，返回全部命中的**字符**区间（按起点升序）。
    ///
    /// `case_sensitive` 为 `false` 时按字符做大小写无关比较。
    pub fn find(&self, needle: &str, case_sensitive: bool) -> Vec<Range<usize>> {
        if needle.is_empty() {
            return Vec::new();
        }
        let haystack: Vec<char> = self.plain_text().chars().collect();
        let pattern: Vec<char> = needle.chars().collect();
        if pattern.len() > haystack.len() {
            return Vec::new();
        }
        let matches_char = |a: char, b: char| -> bool {
            if case_sensitive {
                a == b
            } else {
                a.to_lowercase().eq(b.to_lowercase())
            }
        };
        let mut out = Vec::new();
        for start in 0..=haystack.len() - pattern.len() {
            if haystack[start..start + pattern.len()]
                .iter()
                .zip(&pattern)
                .all(|(a, b)| matches_char(*a, *b))
            {
                out.push(start..start + pattern.len());
            }
        }
        out
    }

    /// 替换全部命中，返回替换次数。
    ///
    /// 替换文本沿用命中区间的行内属性；批注 / 脚注区间随编辑自动平移。
    pub fn replace_all(&mut self, needle: &str, with: &str, case_sensitive: bool) -> usize {
        let matches = self.find(needle, case_sensitive);
        if matches.is_empty() {
            return 0;
        }
        // 从后往前替换：前面的命中偏移不受影响。
        for range in matches.iter().rev() {
            let props = self.props_at(range.start).unwrap_or_default();
            self.delete(range.clone());
            self.insert_text(range.start, with, props);
        }
        matches.len()
    }

    /// 取某字符位置的行内属性（越界或段落分隔符返回 `None`）。
    pub fn props_at(&self, index: usize) -> Option<RunProps> {
        let (para_index, offset) = self.locate(index);
        let para = self.paragraphs.get(para_index)?;
        let mut remaining = offset;
        for run in &para.runs {
            let len = run.char_len();
            if remaining < len {
                return Some(run.props);
            }
            remaining -= len;
        }
        None
    }

    /// 文档统计（字符数 / 段落数 / 行数）。
    pub fn stats(&self) -> DocStats {
        let mut lines = 0usize;
        let mut chars = 0usize;
        for para in &self.paragraphs {
            lines += para.text().matches('\n').count() + 1;
            chars += para.char_len();
        }
        DocStats {
            chars,
            paragraphs: self.paragraphs.len(),
            lines,
        }
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
        let index = index.min(total);
        let (p, offset) = self.locate(index);
        let segments: Vec<&str> = text.split('\n').collect();
        if segments.len() == 1 {
            if let Some(para) = self.paragraphs.get_mut(p) {
                para.insert_run(offset, text, props);
            }
            self.shift_ranges_for_insert(index, text.chars().count());
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
                link: None,
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
                link: None,
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
        self.shift_ranges_for_insert(index, text.chars().count());
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
            self.shift_ranges_for_delete(start, end);
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
        self.shift_ranges_for_delete(start, end);
    }

    /// 插入后平移批注 / 脚注区间。
    fn shift_ranges_for_insert(&mut self, index: usize, delta: usize) {
        if delta == 0 {
            return;
        }
        for annotation in &mut self.annotations {
            shift_range_for_insert(&mut annotation.range, index, delta);
        }
        for footnote in &mut self.footnotes {
            shift_range_for_insert(&mut footnote.range, index, delta);
        }
    }

    /// 删除后收缩 / 平移批注 / 脚注区间。
    fn shift_ranges_for_delete(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        for annotation in &mut self.annotations {
            shift_range_for_delete(&mut annotation.range, start, end);
        }
        for footnote in &mut self.footnotes {
            shift_range_for_delete(&mut footnote.range, start, end);
        }
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
    /// 行内样式用 `<b>`/`<i>`/`<u>`/`<s>` 与 `<span style="color:#rrggbb">`，
    /// 批注用 `<mark data-annotation="id" data-note="…">`，脚注标记用
    /// `<sup data-footnote="id" data-text="…">`。
    pub fn to_html(&self) -> String {
        let mut out = String::new();
        let mut i = 0;
        // 段落起点的文档级偏移（把批注 / 脚注区间换算到段内）。
        let mut offsets = Vec::with_capacity(self.paragraphs.len());
        let mut acc = 0usize;
        for para in &self.paragraphs {
            offsets.push(acc);
            acc += para.char_len() + 1;
        }
        // 表格归属：首个单元格段落即表格插入点，其余单元格段落由表格输出。
        let mut table_at: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        let mut table_cells: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for (table_index, table) in self.tables.iter().enumerate() {
            let mut first: Option<usize> = None;
            for para_index in table.header.iter().chain(table.rows.iter().flatten()) {
                table_cells.insert(*para_index);
                first.get_or_insert(*para_index);
            }
            if let Some(first) = first {
                table_at.insert(first, table_index);
            }
        }

        // 节归属：首个段落即插入点，其余段落由节输出。
        let mut section_at: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        let mut section_paragraphs: std::collections::HashSet<usize> =
            std::collections::HashSet::new();
        for (section_index, section) in self.sections.iter().enumerate() {
            let mut first: Option<usize> = None;
            for para_index in &section.paragraphs {
                section_paragraphs.insert(*para_index);
                first.get_or_insert(*para_index);
            }
            if let Some(first) = first {
                section_at.insert(first, section_index);
            }
        }

        while i < self.paragraphs.len() {
            // 节：整节输出一次（节内段落不再单独输出）。
            if let Some(section_index) = section_at.get(&i) {
                let section = &self.sections[*section_index];
                // 节标题走 `data-title` 属性（内容段落另行输出，避免标题重复）。
                out.push_str("<section");
                if let Some(title) = &section.title {
                    out.push_str(" data-title=\"");
                    escape_html_attr(&mut out, title);
                    out.push('"');
                }
                out.push('>');
                for para_index in &section.paragraphs {
                    if let Some(para) = self.paragraphs.get(*para_index) {
                        let spans: Vec<Span> = Vec::new();
                        push_paragraph_html(&mut out, para, &spans, Some(self));
                    }
                }
                out.push_str("</section>");
                while i < self.paragraphs.len() && section_paragraphs.contains(&i) {
                    i += 1;
                }
                continue;
            }
            // 表格：整表输出一次（单元格段落不再单独输出）。
            if let Some(table_index) = table_at.get(&i) {
                push_table_html(
                    &mut out,
                    &self.tables[*table_index],
                    &self.paragraphs,
                    &offsets,
                );
                while i < self.paragraphs.len() && table_cells.contains(&i) {
                    i += 1;
                }
                continue;
            }
            let kind = self.paragraphs[i].style.list;
            let spans = collect_spans(self, offsets[i], self.paragraphs[i].char_len());
            // 连续的引用段落合并为一个 `<blockquote>`。
            if self.paragraphs[i].style.quote {
                out.push_str("<blockquote>");
                while i < self.paragraphs.len() && self.paragraphs[i].style.quote {
                    let spans = collect_spans(self, offsets[i], self.paragraphs[i].char_len());
                    push_paragraph_block(&mut out, &self.paragraphs[i], &spans, self);
                    i += 1;
                }
                out.push_str("</blockquote>");
                continue;
            }
            match kind {
                ListKind::None => {
                    push_paragraph_block(&mut out, &self.paragraphs[i], &spans, self);
                    i += 1;
                }
                ListKind::Bullet | ListKind::Number => {
                    let tag = if kind == ListKind::Number { "ol" } else { "ul" };
                    out.push('<');
                    out.push_str(tag);
                    out.push('>');
                    while i < self.paragraphs.len() && self.paragraphs[i].style.list == kind {
                        out.push_str("<li>");
                        push_paragraph_html(&mut out, &self.paragraphs[i], &spans, Some(self));
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
        // 批注 / 脚注：解析期先记段内区间，收尾时换算成文档级区间。
        let mut annotations: Vec<(usize, Range<usize>, u32, String)> = Vec::new();
        let mut footnotes: Vec<(usize, Range<usize>, u32, String)> = Vec::new();
        let mut open_annotations: Vec<(usize, usize, u32, String)> = Vec::new();
        let mut open_footnotes: Vec<(usize, usize, u32, String)> = Vec::new();
        // 引用块嵌套深度（`<blockquote>`）。
        let mut quote_depth = 0usize;
        // 表格 / 节：解析期收集单元格与节内段落，收尾时登记到文档。
        let mut table: Option<TableBuilder> = None;
        let mut section: Option<SectionBuilder> = None;
        let mut tables: Vec<RichTable> = Vec::new();
        let mut sections: Vec<RichSection> = Vec::new();

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
                // 表格 / 节：块级结构（单元格与节内段落都是普通段落）。
                match name.as_str() {
                    "table" => {
                        if !closing {
                            table = Some(TableBuilder::default());
                        } else if let Some(builder) = table.take() {
                            tables.push(RichTable {
                                header: builder.header,
                                rows: builder.rows,
                                alignments: builder.alignments,
                            });
                        }
                        continue;
                    }
                    "tr" => {
                        if let Some(builder) = table.as_mut() {
                            if !closing {
                                builder.current_row.clear();
                                builder.current_is_header = false;
                            } else if !builder.current_row.is_empty() {
                                if builder.current_is_header {
                                    builder.header = builder.current_row.clone();
                                } else {
                                    builder.rows.push(builder.current_row.clone());
                                }
                                builder.current_row.clear();
                            }
                        }
                        continue;
                    }
                    "th" | "td" => {
                        if let Some(builder) = table.as_mut() {
                            if !closing {
                                // 新单元格：新建段落承载内容。
                                paragraphs.push(Paragraph::default());
                                builder.current_row.push(paragraphs.len() - 1);
                                if name == "th" {
                                    builder.current_is_header = true;
                                }
                                // 列对齐：`align` 属性或 `style` 里的 `text-align`。
                                let align = attr(name_src, "align")
                                    .map(|value| value.to_string())
                                    .or_else(|| {
                                        let css = attr(name_src, "style")?.to_ascii_lowercase();
                                        css_value(&css, "text-align").map(|value| value.to_string())
                                    });
                                if let Some(align) = align {
                                    let value = match align.trim() {
                                        "center" => Some(TableAlign::Center),
                                        "right" => Some(TableAlign::Right),
                                        "left" => Some(TableAlign::Left),
                                        _ => None,
                                    };
                                    if let Some(value) = value {
                                        let col = builder.current_row.len() - 1;
                                        while builder.alignments.len() <= col {
                                            builder.alignments.push(TableAlign::Left);
                                        }
                                        builder.alignments[col] = value;
                                    }
                                }
                            }
                            continue;
                        }
                    }
                    "section" => {
                        if !closing {
                            // 新段落承载节内内容（节内段落连续，便于收尾时取范围）。
                            paragraphs.push(Paragraph::default());
                            let start = paragraphs.len() - 1;
                            let title = attr(name_src, "data-title").map(decode_text);
                            section = Some(SectionBuilder { start, title });
                        } else if let Some(builder) = section.take() {
                            let mut end = paragraphs.len();
                            // 去掉尾部空段。
                            while end > builder.start
                                && paragraphs.get(end - 1).is_some_and(Paragraph::is_empty)
                            {
                                end -= 1;
                                paragraphs.remove(end);
                            }
                            let indices: Vec<usize> = (builder.start..end).collect();
                            sections.push(RichSection {
                                paragraphs: indices,
                                columns: 1,
                                title: builder.title,
                            });
                        }
                        continue;
                    }
                    _ => {}
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
                        // 批注 / 脚注闭合：区间到当前段落的文本末尾。
                        "mark" => {
                            if let Some((para_index, start, id, note)) = open_annotations.pop() {
                                let end = paragraphs.last().map_or(0, Paragraph::char_len);
                                annotations.push((para_index, start..end, id, note));
                            }
                        }
                        "sup" => {
                            if let Some((para_index, start, id, text)) = open_footnotes.pop() {
                                let end = paragraphs.last().map_or(0, Paragraph::char_len);
                                footnotes.push((para_index, start..end, id, text));
                            }
                        }
                        // 引用块闭合：没有 `<p>` 包裹时内容就在当前段落里。
                        "blockquote" => {
                            if quote_depth > 0 {
                                if let Some(para) = paragraphs.last_mut() {
                                    para.style.quote = true;
                                }
                                quote_depth -= 1;
                            }
                        }
                        _ => {
                            if is_block(&name) {
                                // 标题标签闭合时给承载标题文本的段落记级别。
                                if let Some(level) = heading_level(&name)
                                    && let Some(para) = paragraphs.last_mut()
                                {
                                    para.style.heading = Some(level);
                                }
                                // 引用块内的段落标记为引用样式。
                                if quote_depth > 0
                                    && let Some(para) = paragraphs.last_mut()
                                {
                                    para.style.quote = true;
                                }
                                new_paragraph(&mut paragraphs, list);
                            }
                            let kind = tag_kind(&name);
                            if !kind.is_empty()
                                && let Some(pos) = stack.iter().rposition(|(k, _)| *k == kind)
                            {
                                style = stack[pos].1.clone();
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
                    // 批注：`<mark data-annotation="id" data-note="…">`。
                    "mark" => {
                        let id = attr(name_src, "data-annotation")
                            .and_then(|value| value.parse::<u32>().ok())
                            .unwrap_or(0);
                        let note = attr(name_src, "data-note")
                            .map(decode_text)
                            .unwrap_or_default();
                        let para_index = paragraphs.len().saturating_sub(1);
                        let start = paragraphs.last().map_or(0, Paragraph::char_len);
                        open_annotations.push((para_index, start, id, note));
                        continue;
                    }
                    // 脚注引用：`<sup data-footnote="id" data-text="…">`。
                    "sup" => {
                        let id = attr(name_src, "data-footnote")
                            .and_then(|value| value.parse::<u32>().ok())
                            .unwrap_or(0);
                        let text = attr(name_src, "data-text")
                            .map(decode_text)
                            .unwrap_or_default();
                        let para_index = paragraphs.len().saturating_sub(1);
                        let start = paragraphs.last().map_or(0, Paragraph::char_len);
                        open_footnotes.push((para_index, start, id, text));
                        continue;
                    }
                    _ => {}
                }

                if is_break(&name) {
                    new_paragraph(&mut paragraphs, list);
                    continue;
                }

                // 引用块：进入时记深度，内容段落按引用样式渲染。
                if name == "blockquote" {
                    quote_depth += 1;
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
                stack.push((kind, style.clone()));
                apply_inline_tag(&name, &mut style, attr(name_src, "style"));
                // 链接：`<a href="…">` 的地址挂在样式状态上（关闭时随栈恢复）。
                if name == "a"
                    && let Some(href) = attr(name_src, "href")
                {
                    style.link = Some(decode_text(href));
                }
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
                let link = style.link.clone();
                para.runs.push(Run {
                    text,
                    props: RunProps {
                        bold: style.bold,
                        italic: style.italic,
                        underline: style.underline,
                        strikethrough: style.strikethrough,
                        code: style.code,
                        inserted: false,
                        deleted: false,
                        color: style.color,
                    },
                    link,
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
        // 段内区间 → 文档级区间（段落起点按字符累计）。
        let mut offsets: Vec<usize> = Vec::with_capacity(paragraphs.len());
        let mut acc = 0usize;
        for para in &paragraphs {
            offsets.push(acc);
            acc += para.char_len() + 1;
        }
        let to_doc_range = |para_index: usize, range: Range<usize>| -> Range<usize> {
            let base = offsets.get(para_index).copied().unwrap_or(0);
            (base + range.start)..(base + range.end)
        };
        let annotations = annotations
            .into_iter()
            .map(|(para_index, range, id, note)| Annotation {
                id,
                range: to_doc_range(para_index, range),
                note,
                author: None,
            })
            .collect();
        let footnotes = footnotes
            .into_iter()
            .map(|(para_index, range, id, text)| Footnote {
                id,
                range: to_doc_range(para_index, range),
                text,
            })
            .collect();
        Some(Self {
            paragraphs,
            annotations,
            footnotes,
            tables,
            sections,
        })
    }
}

/// 行内样式的解析状态。
#[derive(Clone, Debug, Default)]
struct StyleState {
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    /// 行内代码（`<code>`）。
    code: bool,
    color: Option<Hsla>,
    /// 当前链接地址（`<a href="…">`）。
    link: Option<String>,
}

/// 插入后平移区间：插入点之前的区间不动，插入点之后的整体后移，
/// 覆盖插入点的区间长度增加。
fn shift_range_for_insert(range: &mut Range<usize>, index: usize, delta: usize) {
    if range.start >= index {
        range.start += delta;
        range.end += delta;
    } else if range.end > index {
        range.end += delta;
    }
}

/// 删除后调整区间：完全在删除点之前的保持不变，完全之后的整体前移，
/// 与删除点相交的按存活文本收缩（删除后前后两段相邻）。
fn shift_range_for_delete(range: &mut Range<usize>, start: usize, end: usize) {
    if range.end <= start {
        return;
    }
    let removed = end.saturating_sub(start);
    if range.start >= end {
        range.start -= removed;
        range.end -= removed;
        return;
    }
    // 相交：起点夹到删除点，长度取删除前后的存活字符数。
    let before = start.saturating_sub(range.start);
    let after = range.end.saturating_sub(end);
    range.start = range.start.min(start);
    range.end = range.start + before + after;
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
/// 输出表格 HTML（`<table>` + 表头行 + 数据行）。
fn push_table_html(
    out: &mut String,
    table: &RichTable,
    paragraphs: &[Paragraph],
    offsets: &[usize],
) {
    out.push_str("<table>");
    if !table.header.is_empty() {
        out.push_str("<tr>");
        for para_index in &table.header {
            out.push_str("<th>");
            if let Some(para) = paragraphs.get(*para_index) {
                let spans: Vec<Span> = Vec::new();
                let _ = offsets;
                push_paragraph_html(out, para, &spans, None);
            }
            out.push_str("</th>");
        }
        out.push_str("</tr>");
    }
    for row_indices in &table.rows {
        out.push_str("<tr>");
        for para_index in row_indices {
            out.push_str("<td>");
            if let Some(para) = paragraphs.get(*para_index) {
                let spans: Vec<Span> = Vec::new();
                push_paragraph_html(out, para, &spans, None);
            }
            out.push_str("</td>");
        }
        out.push_str("</tr>");
    }
    out.push_str("</table>");
}

/// 把行内运行序列转成 Markdown。
fn runs_to_markdown(runs: &[Run]) -> String {
    let mut out = String::new();
    for run in runs {
        if run.text.is_empty() {
            continue;
        }
        let text = &run.text;
        if run.props.deleted {
            out.push_str("<del>");
            out.push_str(text);
            out.push_str("</del>");
            continue;
        }
        if run.props.inserted {
            out.push_str("<ins>");
            out.push_str(text);
            out.push_str("</ins>");
            continue;
        }
        if let Some(href) = &run.link {
            out.push('[');
            out.push_str(text);
            out.push_str("](");
            out.push_str(href);
            out.push(')');
            continue;
        }
        if run.props.code {
            out.push('`');
            out.push_str(text);
            out.push('`');
            continue;
        }
        if run.props.bold {
            out.push_str("**");
            out.push_str(text);
            out.push_str("**");
            continue;
        }
        if run.props.italic {
            out.push('*');
            out.push_str(text);
            out.push('*');
            continue;
        }
        if run.props.strikethrough {
            out.push_str("~~");
            out.push_str(text);
            out.push_str("~~");
            continue;
        }
        out.push_str(text);
    }
    out
}

/// 把表格输出为 Markdown（表头行 + 分隔行 + 数据行）。
fn push_table_markdown(out: &mut String, table: &RichTable, paragraphs: &[Paragraph]) {
    let cell_text = |para_index: usize| -> String {
        paragraphs
            .get(para_index)
            .map(|para| runs_to_markdown(&para.runs))
            .unwrap_or_default()
    };
    if !table.header.is_empty() {
        out.push('|');
        for para_index in &table.header {
            out.push(' ');
            out.push_str(&cell_text(*para_index));
            out.push_str(" |");
        }
        out.push('\n');
        // 分隔行：按表头列数输出 `---`。
        out.push('|');
        for _ in &table.header {
            out.push_str(" --- |");
        }
        out.push('\n');
    }
    for row in &table.rows {
        out.push('|');
        for para_index in row {
            out.push(' ');
            out.push_str(&cell_text(*para_index));
            out.push_str(" |");
        }
        out.push('\n');
    }
    out.push('\n');
}
/// 输出一个段落的 HTML（`<p>` / `<h1>`…`<h6>` + 内联样式 + 内容）。
fn push_paragraph_block(out: &mut String, para: &Paragraph, spans: &[Span], doc: &RichTextDoc) {
    let tag = match para.style.heading {
        Some(level) => format!("h{}", level.clamp(1, 6)),
        None => "p".to_string(),
    };
    out.push('<');
    out.push_str(&tag);
    let css = paragraph_css(&para.style);
    if !css.is_empty() {
        out.push_str(" style=\"");
        out.push_str(&css);
        out.push('"');
    }
    out.push('>');
    push_paragraph_html(out, para, spans, Some(doc));
    out.push_str("</");
    out.push_str(&tag);
    out.push('>');
}

/// 输出一段段落 HTML；批注 / 脚注区间按 `<mark>` / `<sup>` 包裹。
fn push_paragraph_html(
    out: &mut String,
    para: &Paragraph,
    spans: &[Span],
    doc: Option<&RichTextDoc>,
) {
    let mut run_offset = 0usize;
    for run in &para.runs {
        let (open, close) = run_tags(&run.props);
        for (piece, span) in split_run_by_spans(&run.text, run_offset, spans) {
            out.push_str(&open);
            // 链接：`<a href="…">` 包裹（放在批注 / 脚注标签之外）。
            if let Some(href) = &run.link {
                out.push_str("<a href=\"");
                escape_html_attr(out, href);
                out.push_str("\">");
            }
            match span {
                Some((SpanKind::Annotation, id)) => {
                    let note = doc
                        .and_then(|doc| doc.annotation(id))
                        .map(|item| item.note.clone())
                        .unwrap_or_default();
                    out.push_str(&format!("<mark data-annotation=\"{id}\" data-note=\""));
                    escape_html_attr(out, &note);
                    out.push_str("\">");
                    escape_html(out, &piece);
                    out.push_str("</mark>");
                }
                Some((SpanKind::Footnote, id)) => {
                    let text = doc
                        .and_then(|doc| doc.footnote(id))
                        .map(|item| item.text.clone())
                        .unwrap_or_default();
                    out.push_str(&format!("<sup data-footnote=\"{id}\" data-text=\""));
                    escape_html_attr(out, &text);
                    out.push_str("\">");
                    escape_html(out, &piece);
                    out.push_str("</sup>");
                }
                // 查找命中：只是渲染高亮，HTML 里不额外标记。
                Some((SpanKind::Highlight, _)) => {
                    escape_html(out, &piece);
                }
                None => {
                    escape_html(out, &piece);
                }
            }
            if run.link.is_some() {
                out.push_str("</a>");
            }
            out.push_str(&close);
        }
        run_offset += run.char_len();
    }
}

/// HTML 属性值转义（额外转义引号）。
fn escape_html_attr(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
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
    if props.inserted {
        open.push_str("<ins>");
        close.insert_str(0, "</ins>");
    }
    if props.deleted {
        open.push_str("<del>");
        close.insert_str(0, "</del>");
    }
    if props.code {
        open.push_str("<code>");
        close.insert_str(0, "</code>");
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
    if let Some(line_height) = style.line_height {
        if !css.is_empty() {
            css.push(';');
        }
        css.push_str(&format!("line-height:{line_height}"));
    }
    if let Some(space_before) = style.space_before {
        if !css.is_empty() {
            css.push(';');
        }
        css.push_str(&format!("margin-top:{}px", space_before.0.round() as i32));
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
        "code" => style.code = true,
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
    // 行高：`1.5`（倍数）或 `24px`（绝对值）。
    if let Some(v) = css_value(&lower, "line-height") {
        let value = v.trim();
        if let Ok(multiple) = value.parse::<f32>() {
            if multiple > 0.0 {
                style.line_height = Some(multiple);
            }
        } else if let Some(px) = value
            .strip_suffix("px")
            .and_then(|n| n.trim().parse::<f32>().ok())
            && px > 0.0
        {
            // 绝对行高换算成倍数（以 16px 为基准字号）。
            style.line_height = Some(px / 16.0);
        }
    }
    // 段前距：`margin-top: 12px`。
    if let Some(v) = css_value(&lower, "margin-top")
        && let Some(value) = v
            .strip_suffix("px")
            .and_then(|n| n.trim().parse::<f32>().ok())
        && value > 0.0
    {
        style.space_before = Some(px(value));
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
        "a" => "a",
        "code" => "code",
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

/// 标题标签名（`h1`…`h6`）转级别；其余标签返回 `None`。
fn heading_level(name: &str) -> Option<u8> {
    let level = name.strip_prefix('h')?;
    match level {
        "1" => Some(1),
        "2" => Some(2),
        "3" => Some(3),
        "4" => Some(4),
        "5" => Some(5),
        "6" => Some(6),
        _ => None,
    }
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
/// 行内属性映射到 [`crate::TextRun`]；批注区间给背景高亮、脚注引用标记给主题色。
/// 设置 [`RichText::on_annotation_click`] 后，点击批注高亮会回传批注 id。
#[derive(IntoElement)]
pub struct RichText {
    /// 待渲染文档。
    doc: RichTextDoc,
    /// 批注点击回调（批注 id → 回调）。
    on_annotation_click: Option<Arc<dyn Fn(u32, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 链接点击回调（链接地址 → 回调）。
    on_link_click: Option<Arc<dyn Fn(&str, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 基础字号（`None` 表示继承外层字号；标题按级别倍率放大）。
    base_font_size: Option<Pixels>,
    /// 查找命中区间（文档级字符区间；渲染为搜索高亮）。
    highlight_ranges: Vec<Range<usize>>,
}

impl RichText {
    /// 创建渲染元素。
    pub fn new(doc: RichTextDoc) -> Self {
        Self {
            doc,
            on_annotation_click: None,
            on_link_click: None,
            base_font_size: None,
            highlight_ranges: Vec::new(),
        }
    }

    /// 取文档引用。
    pub fn doc(&self) -> &RichTextDoc {
        &self.doc
    }

    /// 设置批注点击回调（回传被点击的批注 id）。
    pub fn on_annotation_click(
        mut self,
        handler: impl Fn(u32, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_annotation_click = Some(Arc::new(handler));
        self
    }

    /// 设置链接点击回调（回传被点击的链接地址）。
    pub fn on_link_click(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_link_click = Some(Arc::new(handler));
        self
    }

    /// 设置查找命中区间（文档级字符区间；渲染为搜索高亮）。
    ///
    /// 区间可用 [`RichTextDoc::find`] 的结果直接传入。
    pub fn highlight_ranges(mut self, ranges: Vec<Range<usize>>) -> Self {
        self.highlight_ranges = ranges;
        self
    }

    /// 设置基础字号（标题按级别倍率放大；未设置时标题沿用外层字号，仅加粗）。
    pub fn base_font_size(mut self, size: Pixels) -> Self {
        self.base_font_size = Some(size);
        self
    }
}

/// 标题级别的字号倍率。
fn heading_scale(level: u8) -> f32 {
    match level {
        1 => 1.8,
        2 => 1.5,
        3 => 1.3,
        4 => 1.15,
        5 => 1.05,
        _ => 1.0,
    }
}

impl From<RichTextDoc> for RichText {
    fn from(doc: RichTextDoc) -> Self {
        Self::new(doc)
    }
}

impl RenderOnce for RichText {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let base = window.text_style();
        let theme = cx.theme();
        // 批注背景色与脚注标记色（跟随主题）。
        let annotation_bg = theme.tokens.warning.opacity(0.28);
        let footnote_color = theme.tokens.primary.color;
        // 链接颜色（主题主色）。
        let link_color = theme.tokens.primary.color;
        // 修订配色：插入用主色，删除用弱化前景色。
        let insert_color = theme.tokens.primary.color;
        let delete_color = theme.tokens.muted_foreground.color;
        // 查找命中高亮（主题 warning 浅底）。
        let highlight_bg = theme.tokens.warning.opacity(0.3);
        // 标题基准字号：显式设置优先，否则取窗口默认文本字号（rem 字号无法换算时留空）。
        let base_px = self
            .base_font_size
            .or_else(|| match window.text_style().font_size {
                crate::AbsoluteLength::Pixels(px) => Some(px),
                crate::AbsoluteLength::Rems(_) => None,
            });
        let mut column = v_flex().w_full().gap_1();
        let mut number = 0usize;
        // 当前段落的文档级起始字符偏移（区间换算用）。
        let mut doc_offset = 0usize;

        // 表格归属：首个单元格段落即表格插入点，其余单元格段落由表格渲染。
        let mut table_at: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        let mut table_cells: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for (table_index, table) in self.doc.tables.iter().enumerate() {
            let mut first: Option<usize> = None;
            for para_index in table.header.iter().chain(table.rows.iter().flatten()) {
                table_cells.insert(*para_index);
                first.get_or_insert(*para_index);
            }
            if let Some(first) = first {
                table_at.insert(first, table_index);
            }
        }

        // 节归属：首个段落即插入点，其余段落由节渲染。
        let mut section_at: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        let mut section_paragraphs: std::collections::HashSet<usize> =
            std::collections::HashSet::new();
        for (section_index, section) in self.doc.sections.iter().enumerate() {
            let mut first: Option<usize> = None;
            for para_index in &section.paragraphs {
                section_paragraphs.insert(*para_index);
                first.get_or_insert(*para_index);
            }
            if let Some(first) = first {
                section_at.insert(first, section_index);
            }
        }

        for (para_index, para) in self.doc.paragraphs.iter().enumerate() {
            // 节插在首个段落处（内部按分栏排布）。
            if let Some(section_index) = section_at.get(&para_index) {
                let section = &self.doc.sections[*section_index];
                column = column.child(render_section(&self.doc, section, &base, &theme));
            }
            if section_paragraphs.contains(&para_index) {
                // 节内段落由节渲染，正文不再单独输出。
                continue;
            }
            // 表格插在首个单元格段落处。
            if let Some(table_index) = table_at.get(&para_index) {
                let table = &self.doc.tables[*table_index];
                column = column.child(render_table(&self.doc, table, &base, &theme));
            }
            if table_cells.contains(&para_index) {
                // 单元格段落由表格渲染，正文不再单独输出。
                continue;
            }
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
            let mut spans = collect_spans(&self.doc, doc_offset, para.char_len());
            // 查找命中区间（文档级 → 段内）。
            for range in &self.highlight_ranges {
                let start = range.start.max(doc_offset);
                let end = range.end.min(doc_offset + para.char_len());
                if start < end {
                    spans.push(Span {
                        range: (start - doc_offset)..(end - doc_offset),
                        kind: SpanKind::Highlight,
                        id: 0,
                    });
                }
            }
            let heading = para.style.heading;
            let built = build_paragraph(
                para,
                &base,
                theme.mono_font_family.clone(),
                theme.tokens.muted.opacity(0.3),
                marker.as_deref(),
                &spans,
                annotation_bg,
                footnote_color,
                link_color,
                insert_color,
                delete_color,
                highlight_bg,
                heading.is_some(),
            );
            let styled = StyledText::new(built.text).with_runs(built.runs);
            // 批注 / 链接可点击：把两类区间合并到一个 InteractiveText（按序号分发）。
            let clickable = !built.annotations.is_empty() || !built.links.is_empty();
            let content = match (&self.on_annotation_click, &self.on_link_click, clickable) {
                (Some(annotation_handler), Some(link_handler), true) => {
                    let annotation_ranges = built.annotations.clone();
                    let link_ranges = built.links.clone();
                    let ranges: Vec<Range<usize>> = annotation_ranges
                        .iter()
                        .map(|(range, _)| range.clone())
                        .chain(link_ranges.iter().map(|(range, _)| range.clone()))
                        .collect();
                    let annotation_count = annotation_ranges.len();
                    let annotation_handler = annotation_handler.clone();
                    let link_handler = link_handler.clone();
                    InteractiveText::new(
                        ElementId::Name(format!("rich-text-link-{doc_offset}").into()),
                        styled,
                    )
                    .on_change(ranges, move |index, window, cx| {
                        if index < annotation_count {
                            if let Some((_, id)) = annotation_ranges.get(index) {
                                annotation_handler(*id, window, cx);
                            }
                        } else if let Some((_, href)) = link_ranges.get(index - annotation_count) {
                            link_handler(href, window, cx);
                        }
                    })
                    .into_any_element()
                }
                _ => styled.into_any_element(),
            };
            let mut paragraph = div()
                .w_full()
                .text_align(para.style.align)
                .pl(px(f32::from(para.style.indent) * INDENT_PX));
            // 标题段落按级别放大字号（基准字号取显式设置或窗口默认值）。
            if let (Some(level), Some(base_px)) = (heading, base_px) {
                paragraph = paragraph.text_size(px(base_px.as_f32() * heading_scale(level)));
            }
            // 段落级行高与段前距。
            if let Some(line_height) = para.style.line_height {
                paragraph = paragraph.line_height(relative(line_height));
            }
            if let Some(space_before) = para.style.space_before {
                paragraph = paragraph.mt(space_before);
            }
            // 引用块：左边线 + 浅底 + 内缩。
            if para.style.quote {
                paragraph = paragraph
                    .pl(px(12.0))
                    .border_l(px(3.0))
                    .border_color(theme.tokens.border)
                    .bg(theme.tokens.muted.opacity(0.15))
                    .text_color(theme.tokens.muted_foreground);
            }
            column = column.child(paragraph.child(content));
            doc_offset += para.char_len() + 1;
        }

        column
    }
}

/// 段落内的高亮区间类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpanKind {
    /// 批注覆盖区间（背景高亮）。
    Annotation,
    /// 脚注引用标记（主题色）。
    Footnote,
    /// 查找命中区间（搜索高亮）。
    Highlight,
}

/// 段落内的高亮区间（段内字符偏移，左闭右开）。
#[derive(Clone, Debug)]
struct Span {
    /// 段内字符区间。
    range: Range<usize>,
    /// 区间类型。
    kind: SpanKind,
    /// 对应的批注 / 脚注 id。
    id: u32,
}

/// 收集落在某段落内的高亮区间（批注背景 / 脚注标记）。
fn collect_spans(doc: &RichTextDoc, para_start: usize, para_len: usize) -> Vec<Span> {
    let para_end = para_start + para_len;
    let mut spans: Vec<Span> = Vec::new();
    for annotation in &doc.annotations {
        let start = annotation.range.start.max(para_start);
        let end = annotation.range.end.min(para_end);
        if start < end {
            spans.push(Span {
                range: (start - para_start)..(end - para_start),
                kind: SpanKind::Annotation,
                id: annotation.id,
            });
        }
    }
    for footnote in &doc.footnotes {
        let start = footnote.range.start.max(para_start);
        let end = footnote.range.end.min(para_end);
        if start < end {
            spans.push(Span {
                range: (start - para_start)..(end - para_start),
                kind: SpanKind::Footnote,
                id: footnote.id,
            });
        }
    }
    spans.sort_by_key(|span| span.range.start);
    spans
}

/// 按高亮区间切分一段运行文本，返回（片段，区间类型与 id）。
fn split_run_by_spans(
    text: &str,
    run_start: usize,
    spans: &[Span],
) -> Vec<(String, Option<(SpanKind, u32)>)> {
    let chars: Vec<char> = text.chars().collect();
    let run_end = run_start + chars.len();
    // 与本运行相交的区间（夹到运行范围内）。
    let mut hits: Vec<(usize, usize, SpanKind, u32)> = spans
        .iter()
        .filter_map(|span| {
            let start = span.range.start.max(run_start);
            let end = span.range.end.min(run_end);
            (start < end).then_some((start, end, span.kind, span.id))
        })
        .collect();
    if hits.is_empty() {
        return vec![(text.to_string(), None)];
    }
    hits.sort_by_key(|(start, _, _, _)| *start);

    let mut out: Vec<(String, Option<(SpanKind, u32)>)> = Vec::new();
    let mut cursor = run_start;
    while cursor < run_end {
        // 当前光标处生效的区间（后开的覆盖先开的）。
        let mut active: Option<(SpanKind, u32)> = None;
        for (start, end, kind, id) in &hits {
            if *start <= cursor && cursor < *end {
                active = Some((*kind, *id));
            }
        }
        // 下一处边界（区间起止）。
        let mut next = run_end;
        for (start, end, _, _) in &hits {
            if *start > cursor {
                next = next.min(*start);
            }
            if *end > cursor {
                next = next.min(*end);
            }
        }
        let take = (next - cursor).min(chars.len());
        let from = cursor - run_start;
        let piece: String = chars[from..from + take].iter().collect();
        out.push((piece, active));
        cursor += take;
    }
    out
}

/// 渲染文档节（标题 + 分栏排布；单栏时与普通段落同款）。
fn render_section(
    doc: &RichTextDoc,
    section: &RichSection,
    base: &crate::TextStyle,
    _theme: &Theme,
) -> AnyElement {
    let mut block = div().w_full().mb(px(12.0));
    if let Some(title) = &section.title {
        block = block.child(
            div()
                .mb(px(4.0))
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.clone()),
        );
    }
    let columns = section.columns.max(1) as usize;
    if columns == 1 {
        // 单栏：段落依次输出。
        let mut column = div().flex().flex_col().gap(px(6.0));
        for para_index in &section.paragraphs {
            if let Some(para) = doc.paragraphs.get(*para_index) {
                let (text, runs) = build_cell_runs(para, base);
                column = column.child(StyledText::new(text).with_runs(runs));
            }
        }
        return block.child(column).into_any_element();
    }
    // 多栏：段落按列序轮流分配。
    let mut row = div().flex().flex_row().gap(px(16.0));
    for col in 0..columns {
        let mut column = div().flex_1().min_w_0().flex().flex_col().gap(px(6.0));
        for (index, para_index) in section.paragraphs.iter().enumerate() {
            if index % columns != col {
                continue;
            }
            if let Some(para) = doc.paragraphs.get(*para_index) {
                let (text, runs) = build_cell_runs(para, base);
                column = column.child(StyledText::new(text).with_runs(runs));
            }
        }
        row = row.child(column);
    }
    block.child(row).into_any_element()
}
/// 渲染表格（单元格内容取对应段落）。
fn render_table(
    doc: &RichTextDoc,
    table: &RichTable,
    base: &crate::TextStyle,
    theme: &Theme,
) -> AnyElement {
    let mut table_el = div()
        .w_full()
        .mb(px(12.0))
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.tokens.border)
        .overflow_hidden();
    if !table.header.is_empty() {
        let mut row = div().flex().flex_row().bg(theme.tokens.muted.opacity(0.3));
        for (col, para_index) in table.header.iter().enumerate() {
            row = row.child(render_table_cell(
                doc,
                *para_index,
                table,
                col,
                base,
                theme,
                true,
            ));
        }
        table_el = table_el.child(row);
    }
    for row_indices in &table.rows {
        let mut row = div().flex().flex_row();
        for (col, para_index) in row_indices.iter().enumerate() {
            row = row.child(render_table_cell(
                doc,
                *para_index,
                table,
                col,
                base,
                theme,
                false,
            ));
        }
        table_el = table_el.child(row);
    }
    table_el.into_any_element()
}

/// 渲染表格单元格（内容为对应段落的行内文本）。
fn render_table_cell(
    doc: &RichTextDoc,
    para_index: usize,
    table: &RichTable,
    col: usize,
    base: &crate::TextStyle,
    theme: &Theme,
    header: bool,
) -> Div {
    let (text, runs) = match doc.paragraphs.get(para_index) {
        Some(para) => build_cell_runs(para, base),
        None => (String::new(), Vec::new()),
    };
    let mut cell = div()
        .flex_1()
        .px(px(12.0))
        .py(px(8.0))
        .border_b_1()
        .border_color(theme.tokens.border);
    cell = match table.alignments.get(col).copied().unwrap_or_default() {
        TableAlign::Center => cell.items_center(),
        TableAlign::Right => cell.items_end(),
        TableAlign::Left => cell,
    };
    if header {
        cell = cell.font_weight(FontWeight::SEMIBOLD);
    }
    cell.child(StyledText::new(text).with_runs(runs))
}

/// 把单元格段落编译成 `(文本, 运行)`（不含列表标记与批注区间）。
fn build_cell_runs(para: &Paragraph, base: &crate::TextStyle) -> (String, Vec<TextRun>) {
    let mut text = String::new();
    let mut runs = Vec::new();
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
        let underline = (run.props.underline || run.props.inserted).then(|| UnderlineStyle {
            thickness: px(1.0),
            color: Some(color),
            wavy: false,
        });
        let strikethrough =
            (run.props.strikethrough || run.props.deleted).then(|| StrikethroughStyle {
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
    (text, runs)
}
/// 把段落编译成 `(文本, 运行, 批注点击区间)`，可选的列表标记作为首个运行。
///
/// `spans` 是段内高亮区间（批注背景 / 脚注标记），会按其边界切分运行；
/// 第三个返回值是批注片段的**字节**区间与批注 id（供点击回调用）。
fn build_paragraph(
    para: &Paragraph,
    base: &crate::TextStyle,
    mono_font: SharedString,
    code_bg: Hsla,
    marker: Option<&str>,
    spans: &[Span],
    annotation_bg: Hsla,
    footnote_color: Hsla,
    link_color: Hsla,
    insert_color: Hsla,
    delete_color: Hsla,
    highlight_bg: Hsla,
    bold_all: bool,
) -> ParagraphBuild {
    let mut text = String::new();
    let mut runs = Vec::new();
    let mut annotation_ranges: Vec<(Range<usize>, u32)> = Vec::new();
    let mut link_ranges: Vec<(Range<usize>, String)> = Vec::new();

    if let Some(marker) = marker {
        text.push_str(marker);
        runs.push(TextRun {
            len: marker.len(),
            font: base.font(),
            color: base.color,
            ..Default::default()
        });
    }

    // 当前运行的段内起始字符偏移（区间按字符计）。
    let mut run_offset = 0usize;
    for run in &para.runs {
        if run.text.is_empty() {
            continue;
        }
        let pieces = split_run_by_spans(&run.text, run_offset, spans);
        run_offset += run.char_len();
        for (piece, kind) in pieces {
            if piece.is_empty() {
                continue;
            }
            let mut font = base.font();
            if run.props.code {
                font.family = mono_font.clone();
            }
            if run.props.bold || bold_all {
                font = font.bold();
            }
            if run.props.italic {
                font = font.italic();
            }
            let mut color = run.props.color.unwrap_or(base.color);
            let mut background_color = None;
            // 行内代码：浅底 + 等宽字体。
            if run.props.code {
                background_color = Some(code_bg);
            }
            // 链接：主题主色 + 下划线。
            let is_link = run.link.is_some();
            if is_link {
                color = link_color;
            }
            // 修订：插入为主色 + 下划线；删除为变暗 + 删除线。
            if run.props.inserted {
                color = insert_color;
            }
            if run.props.deleted {
                color = delete_color;
            }
            match kind {
                Some((SpanKind::Annotation, _)) => {
                    background_color = Some(annotation_bg);
                }
                Some((SpanKind::Footnote, _)) => {
                    color = footnote_color;
                }
                Some((SpanKind::Highlight, _)) => {
                    background_color = Some(highlight_bg);
                }
                None => {}
            }
            let underline =
                (run.props.underline || is_link || run.props.inserted).then(|| UnderlineStyle {
                    thickness: px(1.0),
                    color: Some(color),
                    wavy: false,
                });
            let strikethrough =
                (run.props.strikethrough || run.props.deleted).then(|| StrikethroughStyle {
                    thickness: px(1.0),
                    color: Some(color),
                });
            runs.push(TextRun {
                len: piece.len(),
                font,
                color,
                background_color,
                underline,
                strikethrough,
            });
            let byte_start = text.len();
            text.push_str(&piece);
            if let Some((SpanKind::Annotation, id)) = kind {
                annotation_ranges.push((byte_start..text.len(), id));
            }
            if let Some(href) = &run.link {
                link_ranges.push((byte_start..text.len(), href.clone()));
            }
        }
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

    ParagraphBuild {
        text,
        runs,
        annotations: annotation_ranges,
        links: link_ranges,
    }
}

/// 段落编译结果。
struct ParagraphBuild {
    /// 拼接后的文本。
    text: String,
    /// 行内运行。
    runs: Vec<TextRun>,
    /// 批注片段的字节区间与批注 id。
    annotations: Vec<(Range<usize>, u32)>,
    /// 链接片段的字节区间与地址。
    links: Vec<(Range<usize>, String)>,
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
            annotations: Vec::new(),
            footnotes: Vec::new(),
            tables: Vec::new(),
            sections: Vec::new(),
        };
        doc.normalize();
        assert_eq!(doc.paragraphs.len(), 1);
    }

    /// 批注：添加 / 按位置查询 / 删除。
    #[test]
    fn annotation_add_query_remove() {
        let mut doc = RichTextDoc::from_plain_text("hello world");
        let id = doc.add_annotation(0..5, "问候语");
        assert_eq!(id, 1);
        let annotation = doc.annotation(id).expect("批注");
        assert_eq!(annotation.range, 0..5);
        assert_eq!(annotation.note, "问候语");
        // 命中区间内，区间外不命中。
        assert_eq!(doc.annotations_at(2).len(), 1);
        assert!(doc.annotations_at(7).is_empty());
        // 作者可后写。
        doc.annotation_mut(id).expect("批注").author = Some("小明".to_string());
        assert_eq!(
            doc.annotation(id).and_then(|item| item.author.clone()),
            Some("小明".to_string())
        );
        assert!(doc.remove_annotation(id));
        assert!(doc.annotation(id).is_none());
    }

    /// 批注区间随正文插入 / 删除平移。
    #[test]
    fn annotation_range_follows_edits() {
        let mut doc = RichTextDoc::from_plain_text("hello world");
        let id = doc.add_annotation(6..11, "宾语");
        // 在批注之前插入：区间整体后移。
        doc.insert_text(0, ">> ", RunProps::default());
        assert_eq!(doc.annotation(id).expect("批注").range, 9..14);
        // 在批注内部插入：区间变长。
        doc.insert_text(10, "*", RunProps::default());
        assert_eq!(doc.annotation(id).expect("批注").range, 9..15);
        // 删除批注覆盖的文本：区间收缩到删除点。
        doc.delete(9..12);
        assert_eq!(doc.annotation(id).expect("批注").range, 9..12);
    }

    /// 脚注：插入引用标记 + 登记文末脚注；删除时标记文本一并移除。
    #[test]
    fn footnote_insert_and_remove() {
        let mut doc = RichTextDoc::from_plain_text("正文");
        let id = doc.insert_footnote(2, "*", "脚注内容");
        assert_eq!(id, 1);
        // 标记文本写进正文，脚注文本单独登记。
        assert_eq!(doc.plain_text(), "正文*");
        let footnote = doc.footnote(id).expect("脚注");
        assert_eq!(footnote.text, "脚注内容");
        assert_eq!(footnote.range, 2..3);
        // 删除脚注：记录与正文标记一并消失。
        assert!(doc.remove_footnote(id));
        assert_eq!(doc.plain_text(), "正文");
        assert!(doc.footnote(id).is_none());
    }

    /// HTML 往返：批注 / 脚注可序列化并还原。
    #[test]
    fn annotations_and_footnotes_round_trip_html() {
        let mut doc = RichTextDoc::from_plain_text("hello world");
        let annotation_id = doc.add_annotation(0..5, "问候语");
        let footnote_id = doc.insert_footnote(11, "*", "脚注内容");
        let html = doc.to_html();
        assert!(html.contains("data-annotation=\"1\""));
        assert!(html.contains("data-footnote=\"1\""));

        let parsed = RichTextDoc::from_html(&html).expect("解析");
        assert_eq!(parsed.paragraphs.len(), 1);
        let annotation = parsed.annotation(annotation_id).expect("批注");
        assert_eq!(annotation.range, 0..5);
        assert_eq!(annotation.note, "问候语");
        let footnote = parsed.footnote(footnote_id).expect("脚注");
        assert_eq!(footnote.text, "脚注内容");
        assert_eq!(footnote.range, 11..12);
    }

    /// 查找：命中区间按字符计，大小写可选。
    #[test]
    fn find_returns_character_ranges() {
        let doc = RichTextDoc::from_plain_text("hello HELLO world");
        let hits = doc.find("hello", true);
        assert_eq!(hits, vec![0..5]);
        let insensitive = doc.find("hello", false);
        assert_eq!(insensitive, vec![0..5, 6..11]);
        assert!(doc.find("", true).is_empty());
    }

    /// 替换全部：文本替换 + 命中区间的行内属性沿用。
    #[test]
    fn replace_all_updates_text_and_keeps_props() {
        let mut doc = RichTextDoc::from_plain_text("aaa bbb aaa");
        doc.apply_props(0..3, |props| props.bold = true);
        let count = doc.replace_all("aaa", "x", true);
        assert_eq!(count, 2);
        assert_eq!(doc.plain_text(), "x bbb x");
        // 替换文本沿用原区间的属性。
        assert!(doc.props_at(0).is_some_and(|props| props.bold));
    }

    /// 替换后批注区间随编辑平移。
    #[test]
    fn replace_all_keeps_annotation_ranges_aligned() {
        let mut doc = RichTextDoc::from_plain_text("hello world");
        doc.add_annotation(6..11, "宾语");
        doc.replace_all("hello", "hi", true);
        // 文本变短，批注区间随之后移。
        assert_eq!(doc.plain_text(), "hi world");
        let annotation = doc.annotations.first().expect("批注");
        assert_eq!(annotation.range, 3..8);
    }

    /// 文档统计：字符 / 段落 / 行。
    #[test]
    fn stats_counts_chars_paragraphs_and_lines() {
        let doc = RichTextDoc::from_plain_text("第一段\n第二段\n\n第四段");
        let stats = doc.stats();
        assert_eq!(stats.paragraphs, 4);
        assert_eq!(stats.chars, "第一段第二段第四段".chars().count());
        assert_eq!(stats.lines, 4);
    }

    /// 标题级别往返：`<h1>`/`<h2>` 解析后保留级别，序列化时还原。
    #[test]
    fn html_heading_levels_round_trip() {
        let doc = RichTextDoc::from_html("<h1>一级</h1><p>正文</p><h2>二级</h2>").expect("解析");
        assert_eq!(doc.paragraphs.len(), 3);
        assert_eq!(doc.paragraphs[0].style.heading, Some(1));
        assert_eq!(doc.paragraphs[1].style.heading, None);
        assert_eq!(doc.paragraphs[2].style.heading, Some(2));
        let html = doc.to_html();
        assert!(html.contains("<h1>一级</h1>"));
        assert!(html.contains("<p>正文</p>"));
        assert!(html.contains("<h2>二级</h2>"));
    }

    /// 撤销 / 重做：编辑压栈、撤销回退、重做前进，新编辑清空重做链。
    #[test]
    fn history_undo_redo_round_trip() {
        let mut doc = RichTextDoc::from_plain_text("第一段");
        let mut history = RichTextHistory::new();
        assert!(!history.can_undo());
        assert!(!history.can_redo());

        // 编辑一：追加文字。
        assert!(history.edit(&mut doc, |doc| {
            doc.insert_text(3, "（改）", RunProps::default());
        }));
        assert_eq!(doc.plain_text(), "第一段（改）");
        assert!(history.can_undo());

        // 编辑二：再加粗（属性变化也算改动）。
        assert!(history.edit(&mut doc, |doc| {
            doc.apply_props(0..3, |props| props.bold = true);
        }));

        // 撤销两步回到初始状态。
        assert!(history.undo(&mut doc));
        assert!(history.undo(&mut doc));
        assert_eq!(doc.plain_text(), "第一段");
        assert!(!history.can_undo());
        assert!(history.can_redo());

        // 重做恢复。
        assert!(history.redo(&mut doc));
        assert_eq!(doc.plain_text(), "第一段（改）");
        assert!(history.redo(&mut doc));
        assert!(doc.paragraphs[0].runs[0].props.bold);

        // 新编辑清空重做链。
        assert!(history.edit(&mut doc, |doc| {
            doc.insert_text(0, "新", RunProps::default());
        }));
        assert!(!history.can_redo());
    }

    /// 历史：未产生改动的编辑不压栈；手动记录也可撤销。
    #[test]
    fn history_skips_noop_edits() {
        let mut doc = RichTextDoc::from_plain_text("内容");
        let mut history = RichTextHistory::new();
        assert!(!history.edit(&mut doc, |_| {}));
        assert!(!history.can_undo());

        history.record(&doc);
        assert!(history.can_undo());
        assert!(history.undo(&mut doc));
    }

    /// 链接往返：`<a href="…">` 解析出链接地址，序列化时还原。
    #[test]
    fn html_links_round_trip() {
        let doc = RichTextDoc::from_html(
            "<p>见 <a href=\"https://example.com\">示例站点</a> 了解更多</p>",
        )
        .expect("解析");
        let link_run = doc.paragraphs[0]
            .runs
            .iter()
            .find(|run| run.link.is_some())
            .expect("链接运行");
        assert_eq!(link_run.link.as_deref(), Some("https://example.com"));
        let html = doc.to_html();
        assert!(html.contains("<a href=\"https://example.com\">"));
        assert!(html.contains("示例站点"));
    }

    /// 段落行高 / 段前距往返：CSS 进 → 样式出 → CSS 回。
    #[test]
    fn paragraph_line_height_and_spacing_round_trip() {
        let doc = RichTextDoc::from_html(
            "<p style=\"line-height:1.8;margin-top:12px;text-align:center\">正文</p>",
        )
        .expect("解析");
        let style = doc.paragraphs[0].style;
        assert_eq!(style.line_height, Some(1.8));
        assert_eq!(style.space_before, Some(px(12.0)));
        assert_eq!(style.align, TextAlign::Center);
        let html = doc.to_html();
        assert!(html.contains("line-height:1.8"));
        assert!(html.contains("margin-top:12px"));
        // 往返后再解析，样式保持一致。
        let reparsed = RichTextDoc::from_html(&html).expect("再解析");
        assert_eq!(reparsed.paragraphs[0].style.line_height, Some(1.8));
        assert_eq!(reparsed.paragraphs[0].style.space_before, Some(px(12.0)));
    }

    /// 引用块往返：`<blockquote>` 解析为引用段落，序列化还原。
    #[test]
    fn blockquote_round_trip() {
        let doc = RichTextDoc::from_html("<blockquote><p>引用内容</p></blockquote>").expect("解析");
        assert!(
            doc.paragraphs.iter().any(|para| para.style.quote),
            "引用段落应标记 quote：{doc:?}"
        );
        let html = doc.to_html();
        assert!(html.contains("<blockquote>"));
        assert!(html.contains("引用内容"));
    }

    /// Markdown 导出：标题 / 列表 / 引用 / 行内样式 / 链接 / 表格。
    #[test]
    fn exports_markdown() {
        let mut doc = RichTextDoc::new();
        let mut title = Paragraph::new("标题");
        title.style.heading = Some(2);
        doc.paragraphs.push(title);
        let mut quote = Paragraph::new("引用内容");
        quote.style.quote = true;
        doc.paragraphs.push(quote);
        doc.paragraphs
            .push(Paragraph::new("带 **加粗** 与 `代码` 的段落"));
        let mut link_para = Paragraph::default();
        link_para.runs = vec![Run::with_link(
            "链接",
            RunProps::default(),
            "https://example.com",
        )];
        doc.paragraphs.push(link_para);
        doc.add_table(&["列"], &[vec!["值"]], vec![TableAlign::Left]);
        let markdown = doc.to_markdown();
        assert!(markdown.contains("## 标题"));
        assert!(markdown.contains("> 引用内容"));
        assert!(markdown.contains("带 **加粗** 与 `代码` 的段落"));
        assert!(markdown.contains("[链接](https://example.com)"));
        assert!(markdown.contains("| 列 |"));
        assert!(markdown.contains("| --- |"));
        assert!(markdown.contains("| 值 |"));
    }
    /// 表格 / 节的 HTML 往返：解析回来仍是表格 / 节（含列对齐与节标题）。
    #[test]
    fn html_tables_and_sections_round_trip() {
        let html = "<table><tr><th>甲</th><th align=\"right\">乙</th></tr><tr><td>一</td><td>二</td></tr></table><section data-title=\"小节\"><p>节内段落。</p></section>";
        let doc = RichTextDoc::from_html(html).expect("解析");
        assert_eq!(doc.tables.len(), 1);
        let table = &doc.tables[0];
        assert_eq!(table.header.len(), 2);
        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.alignments.get(1), Some(&TableAlign::Right));
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.sections[0].title.as_deref(), Some("小节"));
        assert_eq!(doc.sections[0].paragraphs.len(), 1);
        // 再序列化仍带表格与节。
        let out = doc.to_html();
        assert!(out.contains("<table>"));
        assert!(out.contains("<th>"));
        assert!(out.contains("data-title=\"小节\""));
    }
    /// 修订标记：插入 / 删除区间 + 有效文本（不含删除内容）+ HTML `<ins>` / `<del>`。
    #[test]
    fn revisions_mark_ranges_and_exclude_deleted_text() {
        let mut doc = RichTextDoc::from_plain_text("hello world");
        doc.mark_inserted(0..5);
        doc.mark_deleted(6..11);
        let runs = &doc.paragraphs[0].runs;
        assert!(runs.iter().any(|run| run.props.inserted));
        assert!(runs.iter().any(|run| run.props.deleted));
        // 有效文本不含删除内容（复制 / 导出用）。
        assert_eq!(doc.effective_text(), "hello ");
        // HTML 输出 ins / del。
        let html = doc.to_html();
        assert!(html.contains("<ins>"));
        assert!(html.contains("<del>"));
        // 清除标记后恢复完整文本。
        doc.clear_revisions(0..11);
        assert_eq!(doc.effective_text(), "hello world");
    }
    /// 文档节：段落复用 + 分栏 + HTML 输出。
    #[test]
    fn section_reuses_paragraphs_and_columns() {
        let mut doc = RichTextDoc::new();
        let index = doc.add_section(&["第一栏", "第二栏"], 2, Some("小节"));
        assert_eq!(index, 0);
        let first_para = doc.section(0).expect("节").paragraphs[0];
        assert_eq!(doc.section(0).expect("节").columns, 2);
        assert_eq!(doc.section(0).expect("节").paragraphs.len(), 2);
        // 分栏数上限夹取。
        doc.add_section(&["x"], 9, None);
        assert_eq!(doc.section(1).expect("节").columns, MAX_SECTION_COLUMNS);
        // 段落确实落在文档里。
        assert_eq!(doc.paragraphs[first_para].text(), "第一栏");
        // HTML 输出为 section。
        let html = doc.to_html();
        assert!(html.contains("<section"));
        assert!(html.contains("小节"));
    }
    /// 表格：单元格复用段落（可编辑），HTML 输出为 `<table>`。
    #[test]
    fn table_cells_reuse_paragraphs() {
        let mut doc = RichTextDoc::new();
        let index = doc.add_table(
            &["列一", "列二"],
            &[vec!["甲", "乙"]],
            vec![TableAlign::Left, TableAlign::Center],
        );
        assert_eq!(index, 0);
        let table = doc.table(0).expect("表格");
        assert_eq!(table.header.len(), 2);
        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0].len(), 2);
        // 单元格内容就是文档段落（字符坐标与批注区间同样生效）。
        let first_cell = table.header[0];
        assert_eq!(doc.paragraphs[first_cell].text(), "列一");
        assert_eq!(doc.paragraphs[table.rows[0][1]].text(), "乙");
        // HTML 输出为表格。
        let html = doc.to_html();
        assert!(html.contains("<table>"));
        assert!(html.contains("列一"));
        assert!(html.contains("<th>"));
    }
    /// 行内代码往返：`<code>` 解析出行内代码属性，序列化还原。
    #[test]
    fn inline_code_round_trip() {
        let doc = RichTextDoc::from_html("<p>看 <code>let x = 1;</code> 这行</p>").expect("解析");
        let code_run = doc.paragraphs[0]
            .runs
            .iter()
            .find(|run| run.props.code)
            .expect("代码运行");
        assert_eq!(code_run.text, "let x = 1;");
        let html = doc.to_html();
        assert!(html.contains("<code>let x = 1;</code>"));
    }
}
