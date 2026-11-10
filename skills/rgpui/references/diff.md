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

`DiffState`（`components/diff.rs:35`），差异状态实体，持有计算后的差异行列表。

| 方法 | 签名 | 位置 |
|------|------|------|
| `new` | `fn new(old: impl Into<String>, new: impl Into<String>) -> Self` | `:44` |
| `show_line_numbers` | `fn show_line_numbers(mut self, show: bool) -> Self` | `:96` |
| `lines` | `fn lines(&self) -> &[DiffLine]` | `:102` |
| `is_show_line_numbers` | `fn is_show_line_numbers(&self) -> bool` | `:107` |
| `added_count` | `fn added_count(&self) -> usize` | `:112` |
| `removed_count` | `fn removed_count(&self) -> usize` | `:120` |
| `unchanged_count` | `fn unchanged_count(&self) -> usize` | `:128` |

`impl Render for DiffState`（`:136`）返回空 `div()`，状态本身不直接渲染，需配合 
`DiffView` 元素。

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

`DiffView`（`components/diff.rs:143`），差异查看器元素，`impl IntoElement`。

| 方法 | 签名 | 位置 |
|------|------|------|
| `new` | `fn new(state: &Entity<DiffState>) -> Self` | `:152` |
| `line_height` | `fn line_height(mut self, height: impl Into<Pixels>) -> Self` | `:161` |

`impl RenderOnce for DiffView`（`:169`）用 `uniform_list` 渲染差异行（只排布视口内的行）：
- 新增行：绿色背景（`theme.tokens.success.color.opacity(0.15)`）+ 绿色文字
- 删除行：红色背景（`theme.tokens.danger.color.opacity(0.15)`）+ 红色文字
- 未变更行：默认背景 + 默认文字色
- 可选行号列（宽 60px，灰色文字）
- 每行前缀：`+ `（新增）/ `- `（删除）/ `  `（未变更）
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
