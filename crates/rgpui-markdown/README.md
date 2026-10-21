# rgpui-markdown

独立的 Markdown 渲染库，基于 [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark)，为 rgpui 提供 Markdown 文本渲染能力。

## 功能

- 解析 Markdown CommonMark 语法（表格、删除线、任务列表、脚注）
- 渲染为 rgpui 元素树（支持段落、标题、列表、代码块、表格、链接、标注块等）
- 块模型公开（`RichBlock` / `RichInline`），可自行接管渲染
- 可自定义主题样式与代码块渲染器
- 支持中文排版
- 可选 `latex` feature：LaTeX 公式（`$...$` 行内 / `$$...$$` 展示）渲染成图片
- 可选 `mermaid` feature：` ```mermaid ` 围栏代码块渲染成图表

## 依赖

- `pulldown-cmark = "0.13"`（Markdown 解析）
- `rgpui`（渲染目标）
- `ratex-parser` / `ratex-layout` / `ratex-svg`（仅 `latex` feature，纯 Rust，无系统依赖）
- `mermaid-rs-renderer`（仅 `mermaid` feature，纯 Rust，无系统依赖）

## 示例

```rust
use rgpui_markdown::Markdown;

let md = "# 标题\n\n这是一个 **段落**。";
let element = Markdown::new(md);
```

## feature

- `latex`：启用 LaTeX 公式渲染。整条管线为「ratex 解析 → 排版 → SVG（字形转路径）→
  usvg 光栅化」，不依赖系统字体；公式解析失败时降级为等宽源码文本，不中断整篇渲染。
- `mermaid`：启用 Mermaid 图表渲染。管线为「`mermaid-rs-renderer` → SVG → usvg 光栅化」，
  图表配色跟随主题深浅；语法不合法时退回普通代码块。
