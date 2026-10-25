# Markdown 渲染（rgpui-markdown）

**目录：** [概述](#概述) · [Markdown 组件](#markdown-组件) · [富文本块模型](#富文本块模型) · [公式与图表（latex / mermaid）](#公式与图表latex--mermaid) · [文档大纲（toc）](#文档大纲toc) · [CodeBlock](#codeblock) · [markdown_ext 插件](#markdown_ext-插件) · [最佳实践](#最佳实践)

## 概述

`rgpui-markdown` 是**独立库 crate**（`crates/rgpui-markdown/`），重依赖
`pulldown-cmark = "0.13"` 不污染核心默认构建。入口 `lib.rs`：常开模块
`pub mod code_block;` / `markdown;` / `rich_text;` / `toc;`，另有私有助手
`mod svg_image;`（`any(latex, mermaid)` 下编译，`lib.rs:26-27`）与 feature 门控的
`pub mod latex;`（`lib.rs:30-31`）/ `pub mod mermaid;`（`:34-35`）。crate 根重导出
`CodeBlock`/`CodeBlockCopyState`/`Markdown`/`CalloutKind`/`CodeBlockRenderer`/
`LinkClickHandler`/`ListItem`/`RichBlock`/`RichInline`/`TableAlignment`/`render_blocks_with`
（`lib.rs:37-46`）、`Heading`/`extract_headings`（`:47`），门控侧
`latex::{LatexImage, latex_image, render_latex_svg}`（`:38-39`）与
`mermaid::{MermaidImage, is_mermaid_language, mermaid_image, render_mermaid_svg}`（`:41-42`）。

核心侧另有 `crates/rgpui/src/markdown_ext/`（`pub mod markdown_ext;` `rgpui.rs:299`），
prelude 重导出 `MarkdownPlugin`/`MarkdownRenderer`/`PluginManager`（`prelude.rs:88`）。

示例：本 crate 的渲染示例在 Storybook —— `cargo run -p rgpui_story`（Markdown 栏目含
基础/语法全览/公式/图表/代码高亮/大纲/综合文档故事；`examples/rgpui_story/Cargo.toml:21`
直接声明 `features = ["latex", "mermaid", "syntax-highlight"]`，无需命令行加 feature）。
注意 showcase 的 `cargo run -p showcase --bin markdown` 演示的是**核心 `markdown_ext`
插件**，不依赖本 crate（该 bin 没有 `required-features`）。

## Markdown 组件

`Markdown`（`rgpui-markdown/src/markdown.rs:57`）：`new(source: impl Into<SharedString>)`（`:70`）、
`base_font_size(Pixels)`（`:80`）、`on_link_click(handler)`（`:86`，`impl RenderOnce` `:95`）。

`parse_markdown(source: &str) -> Arc<(Vec<RichBlock>, Vec<Range<usize>>)>`（`:51`）。

## 富文本块模型

`RichBlock`（`rich_text.rs:140`）：`Paragraph(Vec<RichInline>)`、`Heading { level: u8, content }`、
`CodeBlock { language: Option<String>, code: String }`、`BlockQuote(Vec<RichBlock>)`、
`OrderedList { start: u64, items: Vec<ListItem> }`、`UnorderedList { items: Vec<ListItem> }`、
`Table { headers, alignments, rows }`、`HorizontalRule`、`Image { alt, url }`、
`Callout { kind, title, blocks }`（GitHub 风格 `> [!NOTE]`）、`Math(String)`（展示公式
`:185`，`$$...$$`）、`FootnoteDefinition { label, blocks }`（`:203`）。

`RichInline`（`:75`）除文本/加粗/斜体/删除线/代码/链接/图片/换行/`Styled` 外，新增：
`Math(String)`（`:89`，行内公式 `$...$`，未启用 `latex` 时降级显示源码文本）、
`FootnoteRef(String)`（`:107`，脚注引用标记 `[^label]`，按链接色渲染）。

任务列表：`ListItem { checked: Option<bool>, content, children }`（`:129`），
`checked` 为 `Some(_)` 时渲染**真复选框**——勾选后边框与填充取主题主色、文本加删除线
并变暗（渲染见 `:1220` 起），`None` 为普通列表项。

配套：`CalloutKind`（`:20`）、`TableAlignment`（`:9`）、
`LinkClickHandler = Box<dyn Fn(&str, &mut Window, &mut App) + 'static>`（`:221`）、
`CodeBlockRenderer`（`:225`）。渲染入口：`render_blocks`（`:756`）、
`render_blocks_with(...)`（`:767`，可指定代码块自定义渲染器）、
`render_blocks_with_svg(..., svg: &SvgRenderer)`（`:789`，传入 SVG 渲染器后 `latex`
门控下公式渲染成图片）。注意 crate 根只重导出 `render_blocks_with`，另两个经
`rgpui_markdown::rich_text::{render_blocks, render_blocks_with_svg}` 访问。

## 公式与图表（latex / mermaid）

启用方式（两者纯 Rust、无系统依赖）：

```toml
rgpui-markdown = { workspace = true, features = ["latex", "mermaid"] }
```

（可再加 `syntax-highlight` 接核心 tree-sitter 高亮器。）运行示例即
`cargo run -p rgpui_story`——`rgpui_story` 已默认声明这些 feature，不需要
`cargo run -p rgpui_story --features ...` 这类命令行开关。

### `latex` feature（`src/latex.rs`）

管线：ratex 解析 → 排版 → SVG（`embed_glyphs` 字形转路径，不依赖字体）→ usvg 光栅化。

- `render_latex_svg(latex: &str, color: Hsla, font_size: f32) -> Result<String, String>`
  （`latex.rs:48`）：渲染为 SVG 文本（不含 `$` 分隔符的源码；解析失败返回错误文本，
  调用方降级显示源码）。
- `latex_image(latex, color, font_size, renderer: &SvgRenderer) -> Option<LatexImage>`
  （`:81`）：渲染并缓存（键 = 源码+颜色+字号，`latex_cache_key` `:69`；上限 64 条）。
- `LatexImage`（`:22`）：`image: Arc<RenderImage>` + `width`/`height: Pixels`，可直接交给
  `img` 元素。

接线：启用 `latex` 后 `Markdown` 渲染自动走 `render_blocks_with_svg`
（`markdown.rs:102-114`，内部取 `cx.svg_renderer()`），`$...$` → `RichInline::Math`、
`$$...$$` → `RichBlock::Math`（解析在 `markdown.rs:304/309`）；未启用则显示源码文本。

### `mermaid` feature（`src/mermaid.rs`）

管线：`mermaid-rs-renderer` → SVG → usvg 光栅化。

- `is_mermaid_language(language: Option<&str>) -> bool`（`mermaid.rs:42`）：围栏信息串
  首词为 `mermaid`/`mmd`（大小写不敏感）即判真。
- `render_mermaid_svg(source: &str, dark: bool) -> Result<String, String>`（`:51`）：
  `dark` 决定 dark/modern 主题（**配色跟随主题深浅**）。
- `mermaid_image(source, renderer: &SvgRenderer, dark) -> Option<MermaidImage>`（`:75`）：
  渲染并缓存（键 = 源码+深浅，`mermaid_cache_key` `:64`；上限 32 条）。
- `MermaidImage`（`:26`）：与 `LatexImage` 同形（`image`/`width`/`height`）。

接线：` ```mermaid ` 围栏代码块在渲染时自动识别成图（`rich_text.rs:905-906`），
失败降级为普通代码块。与核心 `components::MermaidDiagram` 的分工：后者是 flowchart
子集 + div 渲染，本管线是完整 Mermaid 语法 + SVG 渲染。

## 文档大纲（toc）

`toc.rs`（**无 feature 门控**，crate 根重导出）：

- `Heading`（`:12`）：`level: u8`（1–6）、`text: String`（行内标记已去除的纯文本）、
  `range: Range<usize>`（源码字节区间，含 `#` 前缀与行尾换行）、`anchor: String`
  （锚点 slug，保留字母数字与 CJK；同名标题依次追加 `-1`、`-2`）。
- `extract_headings(source: &str) -> Vec<Heading>`（`:27`）：按出现顺序提取大纲，供
  阅读器渲染目录、点击跳转与锚点链接。

## CodeBlock

`CodeBlock`（`code_block.rs:36`）：`new(code)`（`:55`）、`language(lang)`（`:68`）、
`show_line_numbers(bool)`（`:74`）、`highlight_lines(Vec<usize>)`（`:80`）、
`max_height(Pixels)`（`:86`）、`show_copy_button(bool)`（`:92`）。
`CodeBlockCopyState`（enum，`code_block.rs:11`；crate 根重导出 `lib.rs:37`）。

## markdown_ext 插件

`MarkdownRenderer`（`markdown_ext/mod.rs:49`）：`new()`（`:58`）、
`register_plugin(Box<dyn MarkdownPlugin>)`（`:66`）、`add_style(class, style)`（`:71`）、
`render(&str) -> String`（`:77`）。
`MarkdownPlugin`（`:25`，trait）。`PluginManager`（`:177`）：`new()`（`:184`）、
`register`（`:191`）、`plugins()`（`:196`）、`find_plugin(name)`（`:201`）、`remove(name)`（`:209`）。
内置插件：`CodeHighlightPlugin`（`:233`）、`MathPlugin`（`:253`）、`TaskListPlugin`（`:278`）。

## 最佳实践

- **独立 crate**：依赖 `rgpui-markdown`，不启用核心即可用；公式/图表按
  `features = ["latex", "mermaid"]` 开启，均为纯 Rust 依赖，失败自动降级（公式显示
  源码、图表显示代码块），不会渲染崩溃。
- **`Markdown::new` 一行渲染**（含 `$...$`/`$$...$$` 与 ` ```mermaid ` 围栏自动接线）；
  需自定义块渲染用 `RichBlock` + `render_blocks_with`；需公式图片需
  `render_blocks_with_svg` 传入 `SvgRenderer`。
- **目录/跳转**用 `extract_headings`（`Heading.anchor` 同名自动去重）。
- **链接点击**用 `on_link_click`；代码块用 `CodeBlock`（行号/高亮/复制）。
- **区分两套 Markdown**：本 crate 渲染真实 UI 元素；核心 `markdown_ext` 是
  HTML 字符串插件管线（`cargo run -p showcase --bin markdown`）。
