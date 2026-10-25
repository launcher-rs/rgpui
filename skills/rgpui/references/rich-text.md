# 富文本（rich_text）

**目录：** [概述](#概述) · [文档模型](#文档模型) · [编辑操作](#编辑操作) · [查找替换与统计](#查找替换与统计) · [撤销重做](#撤销重做) · [批注与脚注](#批注与脚注) · [表格与节](#表格与节) · [修订标记](#修订标记) · [文本序列化](#文本序列化) · [渲染元素 RichText](#渲染元素-richtext) · [示例与最佳实践](#示例与最佳实践)

## 概述

`crates/rgpui/src/rich_text.rs`（`pub mod rich_text;`，`rgpui.rs:292`），核心子系统、
无 feature 门控。**crate 根与 prelude 均不重导出**，导入一律走
`rgpui::rich_text::{...}`。模型是「段落 `Paragraph` 含若干行内运行 `Run`」；
编辑按**文档级字符坐标**（UTF-8 字符数，不是字节），段落分隔符每处计 1 字符；
渲染是**只读**元素 `RichText`，光标/选区/IME 由应用用输入组件自行拼接。

## 文档模型

- `RichTextDoc`（`:537`）：公开字段 `paragraphs: Vec<Paragraph>`、`annotations`、
  `footnotes`、`tables`、`sections`；`new()`（`:552`，含一个空段落）、
  `from_plain_text(&str)`（`:563`，按 `\n` 切段）、`plain_text()`（`:574`）、
  `char_len()`（`:583`，段落字符 + 段间各 1 分隔符）、`is_empty()`（`:589`）。
- `Paragraph`（`:256`）：`style: ParagraphStyle` + `runs: Vec<Run>`；
  `new(text)`（`:265`，空串得空段落）、`text()`（`:278`）、`char_len()`（`:287`）、
  `insert_run(offset, text, props)`（`:297`）、`remove_range(start, end)`（`:312`）、
  `normalize()`（`:324`，合并相邻同属性运行）。
- `Run`（`:181`）：`text: String`、`props: RunProps`、`link: Option<String>`；
  `new(text)`（`:192`）、`with(text, props)`（`:201`）、
  `with_link(text, props, link)`（`:210`）、`char_len()`（`:219`）。
- `RunProps`（`:55`，Copy）：字段 `bold`/`italic`/`underline`/`strikethrough`/`code`/
  `inserted`/`deleted`/`color: Option<Hsla>`；同名链式构造器（`RunProps::new().bold()`，
  `:76`–`:126`）与字段赋值两种写法都可；`is_plain()`（`:129`）。
- `ParagraphStyle`（`:148`，Copy）：`align: TextAlign`、`indent: u8`（0..=8，每级 24px）、
  `list: ListKind`、`heading: Option<u8>`（1–6）、`line_height: Option<f32>`（倍数）、
  `space_before: Option<Pixels>`、`quote: bool`；链式 `indent(level)`（`:167`，自动夹取）、
  `heading(level)`（`:173`，夹取 1..=6）。
- `ListKind`（`:136`）：`None` / `Bullet` / `Number`。
- `TextAlign`（crate 根 `rgpui::TextAlign`）含 `Start` / `End`，渲染时按段落自身 bidi
  方向解析（`bidi::TextDirection::resolve` 口径）。

## 编辑操作

均为 `RichTextDoc` 方法，坐标是文档级字符索引：

| 方法 | 位置 | 行为 |
|------|------|------|
| `insert_text(index, text, props)` | `:968` | 插入文本；`text` 内的 `\n` 会拆段；批注/脚注区间随之平移 |
| `delete(range)` | `:1032` | 删除 `[start,end)`；跨段时合并段落 |
| `split_paragraph(index)` | `:1093` | 等价于在该处插入 `"\n"` |
| `merge_paragraph(index)` | `:1098` | 第 `index` 段与下一段合并；越界返回 `false` |
| `apply_props(range, f)` | `:1112` | 对区间内运行逐个应用 `f: Fn(&mut RunProps)`，跨运行自动拆开 |
| `normalize()` | `:1143` | 合并同属性相邻运行、丢空运行、保证至少一段 |
| `locate(index)` | `:953` | 文档索引 → `(段落索引, 段内偏移)`；越界归末段末尾 |

## 查找替换与统计

- `find(needle, case_sensitive) -> Vec<Range<usize>>`（`:876`）：按字符匹配，返回文档级
  命中区间（起点升序），可直接喂给 `RichText::highlight_ranges`。
- `replace_all(needle, with, case_sensitive) -> usize`（`:908`）：替换文本沿用命中处
  的行内属性，区间自动平移。
- `props_at(index) -> Option<RunProps>`（`:923`）：段落分隔符处返回 `None`。
- `stats() -> DocStats`（`:938`）：`DocStats { chars, paragraphs, lines }`（`:460`）。

## 撤销重做

`RichTextHistory`（`:378`，快照式，上限 100 条）：
`edit(&mut doc, f)`（`:413`，先压编辑前快照再执行 `f`；**未实际改动不压栈**，返回是否改动）、
`record(&doc)`（`:426`，历史外改文档时手动记快照）、`undo(&mut doc)`（`:432`）、
`redo(&mut doc)`（`:441`）、`can_undo()` / `can_redo()`（`:395` / `:400`）、`clear()`（`:405`）。

## 批注与脚注

- `Annotation`（`:348`）：`id`（文档内唯一，从 1 起）、`range`、`note`、`author: Option<String>`。
  `add_annotation(range, note) -> u32`（`:594`，区间超界自动夹取）、`annotation(id)`（`:609`）、
  `annotation_mut(id)`（`:614`，设作者用）、`annotations_at(index)`（`:619`）、
  `remove_annotation(id) -> bool`（`:630`，只删记录，正文保留）。
- `Footnote`（`:363`）：`id`、`range`（正文引用标记区间）、`text`（文末脚注文本）。
  `insert_footnote(index, marker, text) -> u32`（`:639`，`marker` 是写入正文的引用字符）、
  `footnote(id)`（`:655`）、`remove_footnote(id) -> bool`（`:660`，连正文标记一起删）。
- 两者区间都随 `insert_text` / `delete` 自动平移、收缩。

## 表格与节

结构化能力**不引入新文本空间**——单元格与节内段落都是普通段落，字符编辑与批注/脚注
对它们同样生效；渲染/导出时结构插在其**首个段落**所在位置。

- `RichTable`（`:486`）：`header: Vec<usize>`、`rows: Vec<Vec<usize>>`（单元格段落索引）、
  `alignments: Vec<TableAlign>`（`Left`/`Center`/`Right`，`:471`）。
  `add_table(header: &[&str], rows: &[Vec<&str>], alignments) -> usize`（`:675`）、
  `table(index)`（`:828`）。
- `RichSection`（`:523`）：`paragraphs: Vec<usize>`、`columns: u8`（1..=4 自动夹取）、
  `title: Option<String>`。`add_section(paragraphs: &[&str], columns, title) -> usize`（`:836`）、
  `section(index)`（`:851`）。

## 修订标记

`mark_inserted(range)`（`:703`，渲染主色 + 下划线）、`mark_deleted(range)`（`:708`，
删除线 + 变暗，**文本仍留在文档里**）、`clear_revisions(range)`（`:713`）、
`effective_text()`（`:721`，剔除 deleted 运行后的文本，复制/导出用）。
底层就是 `RunProps.inserted` / `RunProps.deleted` 两个标志。

## 文本序列化

- `to_html() -> String`（`:1156`）/ `from_html(&str) -> Option<Self>`（`:1277`）双向：
  `<p>`、`<ul>/<ol>/<li>`、`<b>/<i>/<u>/<s>`、`<span style="color:#rrggbb">`、
  批注 `<mark data-annotation data-note>`、脚注 `<sup data-footnote data-text>`、
  表格 `<table>`、节 `<section>`；输入上限 1MB，超限 `from_html` 返回 `None`。
  段落样式（对齐/标题/行高/段前距/引用）在往返中保留。可对接富剪贴板
  `ClipboardItem::new_html`。
- `to_markdown() -> String`（`:741`）：标题 `#`、列表 `- `/`1. `、引用 `> `、
  行内样式、链接、表格、节标题（按 `##` 输出）；修订用 `<ins>`/`<del>` 内联 HTML。

## 渲染元素 RichText

`RichText`（`:2325`，`impl RenderOnce` `:2406`，`From<RichTextDoc>` `:2400`）：
逐段编译为 `StyledText::new(text).with_runs(Vec<TextRun>)`——粗/斜/下划线/删除线/
行内代码（等宽 + 浅底）/颜色映射到 `TextRun`；列表段以「• 」/「N. 」开头；
标题按级别放大（1→1.8 倍 … 6→1.0 倍）。批注区间 `warning` 色背景、脚注标记与链接走
主题主色（链接带下划线）、查找命中背景 `warning` 浅底。

配置方法：`new(doc)`（`:2340`）、`doc()`（`:2351`）、
`on_annotation_click(impl Fn(u32, &mut Window, &mut App) + Send + Sync)`（`:2356`）、
`on_link_click(impl Fn(&str, &mut Window, &mut App) + Send + Sync)`（`:2365`）——
设置后对应区间包进 `InteractiveText` 可点击，未设置则纯展示；
`highlight_ranges(Vec<Range<usize>>)`（`:2376`，直接传 `find` 结果）、
`base_font_size(Pixels)`（`:2382`，未设置时取窗口默认字号，标题仅加粗不放大）。

## 示例与最佳实践

示例：`cargo run -p showcase --bin rich_text`（`examples/showcase/src/bin/rich_text.rs`，
演示选词套样式、段落对齐/列表、HTML 双向 + 剪贴板对接、历史撤销）。

```rust
use rgpui::rich_text::{RichText, RichTextDoc, RichTextHistory, RunProps};

let mut doc = RichTextDoc::from_plain_text("标题\n正文");
let mut history = RichTextHistory::new();
history.edit(&mut doc, |doc| doc.apply_props(0..2, |p| p.bold = true));
let ranges = doc.find("正文", true);
// 渲染：RichText::new(doc).highlight_ranges(ranges)
```

- **坐标按字符不按字节**：中文 1 字 = 1 索引；换算段内偏移用 `locate`。
- **编辑走 `history.edit`**，历史外改文档（如反序列化替换）先 `record` 或用 `clear` 重置。
- **表格/节内文本照常编辑**：拿 `add_table` 返回的索引再 `table(i)` 查单元格段落号。
- **渲染只读**：要光标/选区请拼 `InputState` 或自绘，本模块负责模型与展示。
