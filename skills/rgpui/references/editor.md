# 编辑器（Editor）

**目录：** [概述](#概述) · [feature 门控](#feature-门控) · [快速入门](#快速入门) · [EditorState](#editorstate) · [Editor 组件](#editor-组件) · [行操作/多光标](#行操作多光标) · [LSP 接线](#lsp-接线) · [片段](#片段) · [行内提示/CodeLens](#行内提示codelens) · [粘性顶栏](#粘性顶栏) · [多语言高亮](#多语言高亮) · [自定义 gutter](#自定义-gutter) · [Minimap/Vim/标尺](#minimapvim标尺) · [扩展与 on_edit](#扩展与-on_edit) · [最佳实践](#最佳实践)

## 概述

代码编辑器位于 `crates/rgpui/src/input_ui/editor/`，由 `EditorState`（编排外壳，
内含 `Entity<InputState>` + 大纲缓存）与 `Editor`（消费 `EditorState` 实体 + 粘性
大纲顶栏 + 状态行）组成。许多特性挂在 `impl InputState`（行操作/多光标/自动闭合/
括号匹配/当前行）。

## feature 门控

整个 `editor` 模块 `#[cfg(feature = "editor")]`（`input_ui/mod.rs:26-27`，重导出
`:44-50`），`editor` 蕴含 `lsp`。启用后 `pub use input_ui::*`（`rgpui.rs:529`）
在根导出 `Editor`、`EditorState` 等。Vim 键位由 `input_ui::init`（`mod.rs:76`）经
`editor::vim::init(cx)` 注册（仅在 `editor` feature 下，`:79-80`）。

## 快速入门

```rust
// 需 cargo 依赖开启 rgpui 的 "editor" feature
use rgpui::{Editor, EditorState, prelude::*};

let editor = cx.new(|cx| EditorState::new(window, cx, "fn main() {}\n"));
editor.update(cx, |ed, cx| ed.set_language("rust", window, cx));

// 渲染
Editor::new(&editor)
```

示例：`cargo run -p showcase --bin editor`（需 `--features editor` 或
`cargo run -p showcase --all-features --bin editor`）。

## EditorState

`EditorState`（`editor/state.rs:22`）。`new(window, cx, initial: &str)`（`:55`）。
唯一真相源是内部 `Entity<InputState>`。

| 方法 | 位置 | 说明 |
|------|------|------|
| `input() -> &Entity<InputState>` | `:107` | 透传底层状态 |
| `text(cx) -> String` | `:112` | 全文 |
| `set_value(...)` | `:122` | 设置文本 |
| `set_selected_range(range, cx)` | `:135` | 选区 |
| `reveal_offset(offset, cx)` / `reveal_range(range, cx)` | `:144` / `:151` | 滚动可见 |
| `set_folding(bool, window, cx)` | `:158` | 折叠 |
| `set_read_only(bool, cx)` | `:167` | 只读 |
| `set_line_comment_prefix(prefix, cx)` | `:283` | 行注释前缀 |
| `cursor(cx) -> usize` | `:290` | 主光标字节偏移 |
| `set_highlighter(Option<Box<dyn Highlighter>>, window, cx)` | `:298` | 语法高亮 |
| `set_language(&str, window, cx)` | `:314` | 查注册表 + 联动大纲/高亮 |
| `language() -> Option<SharedString>` | `:325` | 当前语言 |
| `set_font_size(Option<Pixels>, cx)` / `font_size()` | `:335` / `:330` | 字号 |
| `outline() -> &[DocumentSymbol]` | `:343` | 大纲缓存 |
| `goto_symbol(&DocumentSymbol, cx)` | `:348` | 跳符号 |
| `set_vim_enabled(bool, cx)` / `vim_enabled(cx)` / `vim_mode(cx)` | `:198`/`:214`/`:219` | Vim |
| `set_rulers(Vec<usize>, cx)` / `set_ruler_color` | `:174` / `:181` | 标尺 |
| `set_bracket_rainbow_enabled(bool, cx)` | `:188` | 括号彩虹 |
| `chrome_geometry(cx) -> Option<ChromeGeometry>` | `:251` | gutter/行号几何 |

`ChromeGeometry`（`:227`）：`gutter_end`/`numbers_end`/`fold_end`/`text_start`。

## Editor 组件

`Editor`（`editor/editor_ui.rs:303`）：`new(&Entity<EditorState>)`（`:318`）、
`font_size(Pixels)`（`:366`）、`show_context_menu(bool)`（`:360`）、
`context_menu_extra`（`:330`）、`context_menu_override`（`:345`），`impl Styled`。
渲染含粘性大纲顶栏与状态行。

## 行操作/多光标

- **行操作**（`editor/line_ops.rs`，`impl InputState`）：`MoveLineUp/Down`、
  `CopyLine`、`DeleteLine`、`JoinLines` 等 action（`input_ui/mod.rs:55-58` 重导出，
  仅 `editor` feature）。
- **多光标**（`editor/multicursor.rs`）：`has_multiple_cursors()`（`:24`）、
  `extra_selections()`（`:29`）、`clear_extra_cursors(cx)`（`:34`）；
  action `AddCursorAbove`/`AddCursorBelow`。
- **自动闭合**：`auto_close.rs`；`InputState::auto_close_pairs(bool)`（`state.rs:758`）。
- **括号匹配**：`bracket_match.rs`；`bracket_match(bool)`（`:772`）。
- **当前行**：`current_line.rs`；`current_line_highlight(bool)`（`:810`）。

## LSP 接线

`editor/lsp_attach.rs`（`impl EditorState`）：provider 注入 + 防抖 + 诊断下划线。

| 方法 | 位置 |
|------|------|
| `set_completion_provider(...)` | `:153` |
| `set_diagnostics_provider(...)` | `:247` |
| `set_hover_provider(...)` | `:261` |
| `set_document_uri(Option<Uri>, cx)` / `document_uri()` | `:275` / `:281` |
| `set_auto_completion_enabled(bool, cx)` / `auto_completion_enabled()` | `:167` / `:177` |
| `set_auto_completion_min_prefix_len(usize, cx)` | `:184` |
| `request_completions(window, cx)` / `accept_completion(...)` / `dismiss_completion(cx)` | `:310`/`:390`/`:419` |
| `completion_menu_active()` / `select_next_completion(cx)` / `select_previous_completion(cx)` | `:428`/`:433`/`:443` |
| `diagnostics() -> &[DiagnosticEntry]` | `:296` |
| `request_diagnostics(window, cx)` | `:453` |
| `request_hover(offset, window, cx)` / `dismiss_hover(cx)` / `hover_state()` | `:533`/`:570`/`:301` |

> stdio LSP 传输在 `lsp/stdio.rs`（子进程 + JSON-RPC）。

## 片段

`editor/snippets.rs`（`impl EditorState`）：`expand_snippet(template, window, cx)`（`:158`）、
`next_placeholder(cx)`（`:263`）、`prev_placeholder(cx)`（`:276`）、
`snippet_active()`（`:288`）、`exit_snippet(cx)`（`:293`）。

## 行内提示/CodeLens

- **行内提示**（`editor/inlay_hints.rs`，`impl EditorState`）：`set_inlay_provider(...)`（`:84`）、
  `set_inlay_hints_enabled(bool, cx)`（`:99`）、`inlay_hints_enabled(cx)`（`:110`）、
  `inlay_hint_list(cx)`（`:116`）、`request_inlay_hints(window, cx)`（`:125`）。
  类型 `InlayHint`/`InlayProvider`（`editor/mod.rs:42`）。
- **CodeLens**（`editor/codelens.rs`）：`set_codelens_provider(...)`（`:105`）、
  `codelenses()`（`:119`）、`request_codelenses(window, cx)`（`:127`）；
  `CodeLensOverlay::new(&Entity<EditorState>)`（`:199`）+ `on_lens(...)`（`:207`）。
  类型 `CodeLens/CodeLensOverlay/CodeLensProvider/ResolvedCodeLens`（`mod.rs:36`）。

## 粘性顶栏

`editor/sticky_scroll.rs`（`impl EditorState`）：`sticky_stack(cx)`（`:28`）、
`set_sticky_scroll(bool, cx)`（`:47`）、`sticky_scroll_enabled()`（`:53`）、
`set_sticky_position(StickyPosition, cx)`（`:59`）、`sticky_position()`（`:65`）。
`StickyPosition`（`mod.rs:45`）。

## 多语言高亮

注册表在 `crate::highlight`（`pub mod highlight;` `rgpui.rs:257`）：

- `highlight::register_highlighter(language: &'static str, factory)`（`highlight/mod.rs:208`）
- `highlight::highlighter_for(language: &str) -> Option<Box<dyn Highlighter>>`（`:218`）
- 内置：`highlight::rust_highlighter()`（需 `tree-sitter` feature）、
  `json_highlighter()` / `toml_highlighter()`（`highlight/mod.rs:31/34/36`）。

用法：`editor.update(cx, |ed, cx| ed.set_language("rust", window, cx))`；未注册语言
静默降级为纯文本。

## 自定义 gutter

`editor/gutter.rs`（`impl EditorState`）：`add_gutter_provider(...)`（`:50`）、
`remove_gutter_provider(id, cx)`（`:72`）、`set_gutter_provider_enabled(id, bool, cx)`（`:83`）、
`set_gutter_column_enabled(bool, cx)`（`:97`）、`gutter_column_enabled(cx)`（`:105`）、
`gutter_providers(cx)`（`:111`）。

## Minimap/Vim/标尺

- **Minimap**（`editor/minimap.rs`）：`set_minimap_enabled(bool, cx)`（`:118`）、
  `minimap_enabled()`（`:124`）。
- **Vim**（`editor/vim.rs`，三态）：`VimMode::{Normal, Insert, Visual}`（`:43`，
  默认 Normal）；`VimKey`（`mod.rs:46`）；`EditorState::set_vim_enabled` 等。
- **标尺 + 括号彩虹**：标尺在 `input_ui/indent.rs`；括号彩虹在 `editor/bracket_match.rs`；
  `EditorState::set_rulers`/`set_ruler_color`/`set_bracket_rainbow_enabled`。

## 扩展与 on_edit

`editor/extensions.rs`：

- `EditorExtension` trait（`:30`）：`name()` + `on_edit(&EditEvent)`。
- `EditorExtensionFactory`（`:38`）：`Box<dyn Fn() -> Box<dyn EditorExtension> + Send + Sync>`。
- `EditEvent`（`:22`）：`{ cursor, text_len }`。
- `EditHandler`（`:44`）：`Rc<dyn Fn(&EditEvent, &mut Window, &mut Context<EditorState>)>`。
- `register_editor_extension(name, factory)`（`:51`）、`editor_extension(name)`（`:58`）。
  `EditorState` 侧按名挂载扩展（`attach_extension`），文本变更时逐个调 `on_edit`。

> **重入警告：** `EditHandler`/`on_edit` 回调内禁止直接改文本（会重入管线），
> 需 `cx.spawn`/`defer` 延后或只读。

## 最佳实践

- **启用 `editor` feature** 才能用 `Editor`/`EditorState`；内置 Rust 高亮还需
  `tree-sitter`（wasm 不可用）。
- **`EditorState` 是编排外壳**，需要底层能力时取 `ed.input()` 操作 `InputState`。
- **换语言用 `set_language`**（自动查注册表 + 刷新大纲），或 `set_highlighter` 直接注入。
- **LSP 能力一律通过 provider 注入**，核心不内置网络/子进程协议实现细节。
- **`on_edit` 回调只读**，编辑副作用一律延后到 `defer`/`spawn`。
