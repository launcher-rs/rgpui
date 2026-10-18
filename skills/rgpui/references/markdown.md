# Markdown 渲染（rgpui-markdown）

**目录：** [概述](#概述) · [Markdown 组件](#markdown-组件) · [富文本块模型](#富文本块模型) · [CodeBlock](#codeblock) · [markdown_ext 插件](#markdown_ext-插件) · [最佳实践](#最佳实践)

## 概述

`rgpui-markdown` 是**独立库 crate**（`crates/rgpui-markdown/`），重依赖
`pulldown-cmark = "0.13"` 不污染核心默认构建。入口 `lib.rs`：
`pub mod code_block;` / `markdown;` / `rich_text;`，重导出
`CodeBlock`/`CodeBlockCopyState`/`Markdown`/`CalloutKind`/`CodeBlockRenderer`/
`LinkClickHandler`/`ListItem`/`RichBlock`/`RichInline`/`TableAlignment`/`render_blocks_with`。

核心侧另有 `crates/rgpui/src/markdown_ext/`（`pub mod markdown_ext;` `rgpui.rs:299`），
prelude 重导出 `MarkdownPlugin`/`MarkdownRenderer`/`PluginManager`（`prelude.rs:88`）。

示例：`cargo run -p showcase --bin markdown`。

## Markdown 组件

`Markdown`（`rgpui-markdown/src/markdown.rs:53`）：`new(source: impl Into<SharedString>)`（`:66`）、
`base_font_size(Pixels)`（`:76`）、`on_link_click(handler)`（`:82`，`impl IntoElement`）。

`parse_markdown(source: &str) -> Arc<(Vec<RichBlock>, Vec<Range<usize>>)>`（`:47`）。

## 富文本块模型

`RichBlock`（`rich_text.rs:134`）：`Paragraph(Vec<RichInline>)`、`Heading { level: u8, content }`、
`CodeBlock { language: Option<String>, code: String }`、`BlockQuote(Vec<RichBlock>)`、
`OrderedList { start: u64, items: Vec<ListItem> }`、`UnorderedList { items: Vec<ListItem> }`、
`Table { headers, alignments, rows }`、水平分割线等。

配套：`RichInline`（`:75`）、`CalloutKind`（`:20`）、`ListItem`、`TableAlignment`（`:9`）、
`LinkClickHandler = Box<dyn Fn(&str, &mut Window, &mut App)>`（`:211`）、
`CodeBlockRenderer`、`render_blocks_with(...)`（`:569`）。

## CodeBlock

`CodeBlock`（`code_block.rs:32`）：`new(code)`（`:51`）、`language(lang)`（`:64`）、
`show_line_numbers(bool)`（`:70`）、`highlight_lines(Vec<usize>)`（`:76`）、
`max_height(Pixels)`（`:82`）、`show_copy_button(bool)`（`:88`）。
`CodeBlockCopyState`（lib.rs:17）。

## markdown_ext 插件

`MarkdownRenderer`（`markdown_ext/mod.rs:49`）：`new()`（`:58`）、
`register_plugin(Box<dyn MarkdownPlugin>)`（`:66`）、`add_style(class, style)`（`:71`）、
`render(&str) -> String`（`:77`）。
`MarkdownPlugin`（`:25`，trait）。`PluginManager`（`:177`）：`new()`（`:184`）、
`register`（`:191`）、`plugins()`（`:196`）、`find_plugin(name)`（`:201`）、`remove(name)`（`:209`）。
内置插件：`CodeHighlightPlugin`（`:233`）、`MathPlugin`（`:253`）、`TaskListPlugin`（`:278`）。

## 最佳实践

- **独立 crate**：依赖 `rgpui-markdown`，不启用核心即可用。
- **`Markdown::new` 一行渲染**；需自定义块渲染用 `RichBlock` + `render_blocks_with`。
- **链接点击**用 `on_link_click`；代码块用 `CodeBlock`（行号/高亮/复制）。
