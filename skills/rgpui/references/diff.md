# 差异查看器（Diff）

**目录：** [概述](#概述) · [快速入门](#快速入门) · [DiffState](#diffstate) · [DiffView](#diffview) · [最佳实践](#最佳实践)

## 概述

差异查看器组件提供行级文本对比能力，基于 `similar` crate 计算差异并以颜色高亮显示。
定义在 `crates/rgpui/src/components/diff.rs`，经 `pub use diff::*` 在 `rgpui::components` 
导出。**需要 `diff` feature 门控**，不在 prelude，需显式 import。

> **注意：** 当前为 MVP 版本，仅支持行级差异。字符级差异、并排视图、hunk 折叠等高级功能
> 待后续扩展。

## 快速入门

```rust
use rgpui::components::{DiffState, DiffView};
use rgpui::prelude::*;

// 创建差异状态
let diff_state = cx.new(|_| DiffState::new(old_text, new_text));

// 渲染差异视图
DiffView::new(&diff_state)
    .line_height(px(20.0))
```

示例：`cargo run -p showcase --bin diff --features rgpui/diff`
（`examples/showcase/src/bin/diff.rs`）。

## DiffState

`DiffState`（`components/diff.rs:58`），差异状态实体，持有计算后的差异行列表。

| 方法 | 签名 | 位置 |
|------|------|------|
| `new` | `fn new(old: impl Into<String>, new: impl Into<String>) -> Self` | `:73` |
| `show_line_numbers` | `fn show_line_numbers(mut self, show: bool) -> Self` | `:132` |
| `lines` | `fn lines(&self) -> &[DiffLine]` | `:138` |
| `is_show_line_numbers` | `fn is_show_line_numbers(&self) -> bool` | `:143` |
| `added_count` | `fn added_count(&self) -> usize` | `:148` |
| `removed_count` | `fn removed_count(&self) -> usize` | `:156` |
| `unchanged_count` | `fn unchanged_count(&self) -> usize` | `:164` |
| `set_span_highlights` | `fn set_span_highlights(&mut self, highlights: impl IntoIterator<Item = DiffSpanHighlight>, cx: &mut Context<Self>)` | `:172` |
| `clear_span_highlights` | `fn clear_span_highlights(&mut self, cx: &mut Context<Self>)` | `:182` |
| `span_highlights` | `fn span_highlights(&self) -> &[DiffSpanHighlight]` | `:190` |
| `highlights_for_query` | `fn highlights_for_query(&self, query: &str) -> Vec<DiffSpanHighlight>` | `:198` |
| `scroll_to_line` | `fn scroll_to_line(&mut self, line: usize, cx: &mut Context<Self>)` | `:231` |
| `scroll_handle` | `fn scroll_handle(&self) -> &UniformListScrollHandle` | `:237` |

`impl Render for DiffState`（`:270`）返回空 `div()`，状态本身不直接渲染，需配合 
`DiffView` 元素。

### 字符级高亮与滚动定位

`DiffSpanHighlight`（`components/diff.rs:29`）标出某一行内的一段字符区间
（如搜索命中），`DiffView` 渲染时以主题强调色背景显示：

```rust
use rgpui::components::DiffSpanHighlight;

// 搜索联动：生成命中区间 → 高亮 → 跳到首个命中行（一次性）。
let spans = diff_state.read(cx).highlights_for_query(query);
diff_state.update(cx, |state, cx| {
    state.set_span_highlights(spans, cx);
    if let Some(first) = state.span_highlights().first() {
        state.scroll_to_line(first.line, cx);
    }
});
```

- `highlights_for_query` 大小写不敏感，按字符边界对齐（多字节字符安全）；
  区间是**行内容**的字节区间（不含行号与 `+`/`-` 前缀，渲染时自动平移）。
- `scroll_to_line` 是一次性语义：下次布局按 `Nearest` 策略定位，越界收敛到
  末行；`DiffState` 持有滚动句柄，跨帧保持滚动位置。
- 手工构造区间时须落在字符边界上，非法区间渲染时忽略。

### DiffLine

`DiffLine`（`components/diff.rs:22`），单行差异数据。

| 字段 | 类型 | 说明 |
|------|------|------|
| `text` | `SharedString` | 行内容（不含换行符） |
| `kind` | `DiffLineKind` | 行类型（`Unchanged` / `Added` / `Removed`） |
| `old_line` | `Option<usize>` | 旧文件行号（从 1 开始，`None` 表示新增行） |
| `new_line` | `Option<usize>` | 新文件行号（从 1 开始，`None` 表示删除行） |

### DiffLineKind

`DiffLineKind`（`components/diff.rs:11`），差异行类型枚举。

| 变体 | 说明 |
|------|------|
| `Unchanged` | 未变更行 |
| `Added` | 新增行 |
| `Removed` | 删除行 |

## DiffView

`DiffView`（`components/diff.rs:278`），差异查看器元素，`impl IntoElement`。

| 方法 | 签名 | 位置 |
|------|------|------|
| `new` | `fn new(state: &Entity<DiffState>) -> Self` | `:287` |
| `line_height` | `fn line_height(mut self, height: impl Into<Pixels>) -> Self` | `:295` |

`impl RenderOnce for DiffView`（`:301`）用 `uniform_list` 渲染差异行（只排布视口内的行）：
- 新增行：绿色背景（`theme.tokens.success.color.opacity(0.15)`）+ 绿色文字
- 删除行：红色背景（`theme.tokens.danger.color.opacity(0.15)`）+ 红色文字
- 未变更行：默认背景 + 默认文字色
- 可选行号列（宽 60px，灰色文字）
- 每行前缀：`+ `（新增）/ `- `（删除）/ `  `（未变更）
- 字符高亮区间（`DiffState::set_span_highlights`）以主题强调色背景
  （`theme.tokens.accent.color.opacity(0.35)`）渲染
- 行内容不折行（`whitespace_nowrap`），长行由列表横向滚动（`ListHorizontalSizingBehavior::Unconstrained`）；
  行高固定为 `line_height`，`DiffState::new` 会去掉行尾的 `\n` / `\r`（CRLF 文本不会残留回车）。

## 最佳实践

### 统计信息展示

```rust
let added = diff_state.read(cx).added_count();
let removed = diff_state.read(cx).removed_count();
let unchanged = diff_state.read(cx).unchanged_count();

h_flex()
    .gap(px(16.0))
    .child(div().text_sm().child(format!("新增: {added} 行")))
    .child(div().text_sm().child(format!("删除: {removed} 行")))
    .child(div().text_sm().child(format!("未变更: {unchanged} 行")))
```

### 容器高度

`DiffView` 本身不设置高度，需父容器提供确定高度：

```rust
div()
    .h(px(500.0))  // 或 .flex_1() 撑满父容器
    .child(DiffView::new(&diff_state))
```

### 滚动

`DiffView` 内部是 `uniform_list`：只渲染视口内的行，垂直滚动虚拟化；
长行不折行，可横向滚动。父容器仍须提供确定高度（见上一节）。

### Feature 门控

在 `Cargo.toml` 中启用：

```toml
[dependencies]
rgpui = { version = "1.6", features = ["diff"] }
```

运行示例时指定 feature：

```bash
cargo run -p showcase --bin diff --features rgpui/diff
```
