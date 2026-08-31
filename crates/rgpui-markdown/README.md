# rgpui-markdown

独立的 Markdown 渲染库，基于 [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark)，为 rgpui 提供 Markdown 文本渲染能力。

## 功能

- 解析 Markdown CommonMark 语法
- 渲染为 rgpui 元素树（支持段落、标题、列表、代码块、表格、链接等）
- 可自定义主题样式
- 支持中文排版

## 依赖

- `pulldown-cmark = "0.12"`（Markdown 解析）
- `rgpui`（渲染目标）

## 示例

```rust
use rgpui_markdown::MarkdownElement;

let md = "# 标题\n\n这是一个 **段落**。";
let element = MarkdownElement::new(md, theme);
```
