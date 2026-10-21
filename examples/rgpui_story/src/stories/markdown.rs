//! Markdown 渲染示例：基础语法、代码块、表格、综合文档。

use rgpui::prelude::*;
use rgpui::{Context, IntoElement, Styled, Window, div, px, v_flex};
use rgpui_markdown::{CodeBlock, Markdown};

use super::StoryItem;

/// Markdown 故事注册表。
pub fn stories() -> Vec<StoryItem> {
    vec![
        StoryItem {
            title: "Markdown 基础",
            build: |_, cx| cx.new(|cx| MarkdownBasicStory::new(cx)).into(),
        },
        StoryItem {
            title: "代码块",
            build: |_, cx| cx.new(|cx| CodeBlockStory::new(cx)).into(),
        },
        StoryItem {
            title: "语法全览",
            build: |_, cx| cx.new(|cx| MarkdownShowcaseStory::new(cx)).into(),
        },
        StoryItem {
            title: "LaTeX 公式",
            build: |_, cx| cx.new(|cx| MarkdownMathStory::new(cx)).into(),
        },
        StoryItem {
            title: "Mermaid 图表",
            build: |_, cx| cx.new(|cx| MarkdownMermaidStory::new(cx)).into(),
        },
        StoryItem {
            title: "多语言代码高亮",
            build: |_, cx| cx.new(|cx| MarkdownCodeHighlightStory::new(cx)).into(),
        },
        StoryItem {
            title: "文档大纲",
            build: |_, cx| cx.new(|cx| MarkdownTocStory::new(cx)).into(),
        },
        StoryItem {
            title: "综合文档",
            build: |_, cx| cx.new(|cx| MarkdownFullStory::new(cx)).into(),
        },
    ]
}

/// Markdown 基础语法示例。
struct MarkdownBasicStory;

impl MarkdownBasicStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for MarkdownBasicStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let source = r#"# Markdown 基础语法

## 文本样式

这是一段普通文本。**粗体文本**、*斜体文本*、~~删除线文本~~。

行内代码 `println!("hello")` 也可以使用。

## 列表

### 无序列表

- 第一项
- 第二项
  - 嵌套 A
  - 嵌套 B
- 第三项

### 有序列表

1. 打开文件
2. 编辑内容
3. 保存退出

### 任务列表

- [x] 已完成的事项
- [ ] 待办事项
- [x] 另一件已完成的事

## 引用

> rgpui 是一个 GPU 加速的跨平台 UI 框架。
> 支持 Windows、macOS 和 Linux。

## 分割线

---

以上是最常用的 Markdown 语法。"#;

        v_flex()
            .id("markdown-basic-story")
            .gap(px(12.0))
            .p(px(16.0))
            .child(section_title("Markdown 基础语法"))
            .child(
                v_flex()
                    .p(px(16.0))
                    .rounded(px(8.0))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(Markdown::new(source)),
            )
    }
}

/// 代码块示例。
struct CodeBlockStory;

impl CodeBlockStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for CodeBlockStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let rust_code = r#"fn main() {
    let greeting = "Hello, rgpui!";
    println!("{}", greeting);

    // 计算斐波那契数列
    let n = 10;
    let mut a = 0u64;
    let mut b = 1u64;
    for _ in 0..n {
        let temp = b;
        b = a + b;
        a = temp;
    }
    println!("fib({}) = {}", n, a);
}"#;

        let js_code = r#"const greet = (name) => {
    return `Hello, ${name}!`;
};

const items = [1, 2, 3].map(x => x * 2);
console.log(greet("World"), items);"#;

        v_flex()
            .id("code-block-story")
            .gap(px(12.0))
            .p(px(16.0))
            .child(section_title("代码块（CodeBlock）"))
            .child(section_subtitle("Rust — 带行号"))
            .child(
                CodeBlock::new(rust_code)
                    .language("rust")
                    .show_line_numbers(true),
            )
            .child(section_subtitle("JavaScript — 无行号"))
            .child(CodeBlock::new(js_code).language("javascript"))
    }
}

/// 语法全览示例：集中展示所有受支持的 Markdown 语法，便于核对渲染与复制。
struct MarkdownShowcaseStory;

impl MarkdownShowcaseStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for MarkdownShowcaseStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let source = r#"# 语法全览

## 标题（H1–H6）

# 一级标题
## 二级标题
### 三级标题
#### 四级标题
##### 五级标题
###### 六级标题

## 行内样式

**粗体**、*斜体*、***粗斜体***、~~删除线~~、行内代码 `let x = 1;`。

混合：**粗体中含 *斜体* 与 `代码`**，*斜体中含 **粗体** 与 ~~删除线~~*。

## 链接

- 外部链接：[rgpui 仓库](https://github.com/launcher-rs/rgpui)
- 自动识别的裸链接在部分解析器支持，本渲染器以标准 `[文本](url)` 为准。

## 图片

![示例图片](https://via.placeholder.com/300x120.png)

## 列表

### 无序列表（含嵌套）

- 水果
  - 苹果
  - 香蕉
- 蔬菜
  - 西红柿
  - 黄瓜

### 有序列表

1. 准备食材
2. 热锅下油
3. 翻炒均匀
4. 装盘出锅

### 任务列表

- [x] 需求评审
- [x] 技术方案
- [ ] 编码实现
- [ ] 单元测试
- [ ] 集成联调

## 引用

> 引用内容：rgpui 是 GPU 加速的跨平台 UI 框架。
>
> 同一引用块内的多段落。

嵌套引用：

> 外层引用
> > 内层引用
> > > 更深的引用

## 表格

| 左对齐 | 居中对齐 | 右对齐 | 说明 |
|:-------|:--------:|--------:|------|
| A1 | B1 | C1 | 第一行 |
| A2 | B2 | C2 | 第二行 |
| A3 | B3 | C3 | 第三行 |

## 代码块（多语言）

```rust
fn main() {
    println!("Hello, rgpui!");
}
```

```python
def greet(name: str) -> str:
    return f"Hello, {name}!"
```

```json
{
  "name": "rgpui",
  "version": "1.0.0",
  "edition": "2021"
}
```

## 分割线

上面的表格与下面的内容由分割线隔开。

---

## 脚注

脚注引用[^1] 与脚注定义都能渲染[^2]。

[^1]: 这是第一条脚注。
[^2]: 这是第二条脚注。

## 综合段落

本段包含 **粗体**、*斜体*、~~删除线~~、行内代码 `cargo build` 以及
[一个链接](https://github.com/launcher-rs/rgpui)，用于验证混合排版与复制效果。"#;

        v_flex()
            .id("markdown-showcase-story")
            .gap(px(12.0))
            .p(px(16.0))
            .child(section_title("Markdown 语法全览"))
            .child(
                v_flex()
                    .p(px(16.0))
                    .rounded(px(8.0))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(Markdown::new(source)),
            )
    }
}

/// LaTeX 公式示例：行内公式与展示公式。
struct MarkdownMathStory;

impl MarkdownMathStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for MarkdownMathStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let source = r#"# LaTeX 公式

公式由 `rgpui-markdown` 的 `latex` feature 支持：纯 Rust 解析 → 排版 → SVG → 光栅化，
不依赖系统字体与外部服务。

## 行内公式

勾股定理写作 $a^2 + b^2 = c^2$，欧拉恒等式则是 $e^{i\pi} + 1 = 0$。
行内公式与正文同基线排布。

## 展示公式

$$
\int_{-\infty}^{\infty} e^{-x^2} \, dx = \sqrt{\pi}
$$

$$
\frac{\partial \mathcal{L}}{\partial w} = \sum_{i=1}^{n} \left(y_i - \hat{y}_i\right) x_i
$$

## 降级显示

无法解析的公式（如 $\frac{a}$）会退回为等宽源码文本，不会中断整篇渲染。"#;

        v_flex()
            .id("markdown-math-story")
            .gap(px(12.0))
            .p(px(16.0))
            .child(section_title("LaTeX 公式"))
            .child(
                v_flex()
                    .p(px(16.0))
                    .rounded(px(8.0))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(Markdown::new(source)),
            )
    }
}

/// Mermaid 图表示例：围栏代码块渲染成图表。
struct MarkdownMermaidStory;

impl MarkdownMermaidStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for MarkdownMermaidStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let source = r#"# Mermaid 图表

````markdown
```mermaid
flowchart TD
    A[开始] --> B{判断}
    B -->|是| C[执行]
    B -->|否| D[结束]
```
````

围栏信息串为 `mermaid`（或 `mmd`）时，代码块会被渲染成图表；
语法不合法时自动退回普通代码块，不中断整篇渲染。"#;

        v_flex()
            .id("markdown-mermaid-story")
            .gap(px(12.0))
            .p(px(16.0))
            .child(section_title("Mermaid 图表"))
            .child(
                v_flex()
                    .p(px(16.0))
                    .rounded(px(8.0))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(Markdown::new(source)),
            )
    }
}

/// 多语言代码高亮示例：按围栏信息串选择语言。
struct MarkdownCodeHighlightStory;

impl MarkdownCodeHighlightStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for MarkdownCodeHighlightStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let source = r#"# 多语言代码高亮

代码块的语言由围栏信息串决定，命中已注册的语言包时使用真实语法高亮，
否则回退到简易分词高亮。

## Rust

```rust
pub fn add(a: i32, b: i32) -> i32 {
    // 注释
    let sum = a + b;
    sum
}
```

## Python

```python
def add(a, b):
    # 注释
    return a + b
```

## JavaScript

```javascript
function add(a, b) {
  // 注释
  return a + b;
}
```

## JSON

```json
{
  "name": "rgpui",
  "version": 1
}
```

## 未注册语言

```text
plain text stays plain
```
"#;

        v_flex()
            .id("markdown-code-highlight-story")
            .gap(px(12.0))
            .p(px(16.0))
            .child(section_title("多语言代码高亮"))
            .child(
                v_flex()
                    .p(px(16.0))
                    .rounded(px(8.0))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(Markdown::new(source)),
            )
    }
}

/// 文档大纲示例：提取标题并按层级缩进展示。
struct MarkdownTocStory;

impl MarkdownTocStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for MarkdownTocStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let source = r#"# rgpui 文档

## 快速开始

添加依赖并创建窗口。

### 平台后端

Windows / macOS / Linux 各自实现。

## 组件

- 基础元素
- 输入与表单
- 菜单与通知

### 图表

按 feature 门控。

## 主题

内置浅色 / 深色两套主题。
"#;

        let headings = rgpui_markdown::extract_headings(source);
        let mut toc = v_flex().gap(px(4.0));
        for heading in &headings {
            toc = toc.child(
                div()
                    .pl(px(f32::from(heading.level.saturating_sub(1)) * 16.0))
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("H{}", heading.level)),
                    )
                    .child(div().text_sm().child(heading.text.clone()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("#{}", heading.anchor)),
                    ),
            );
        }

        v_flex()
            .id("markdown-toc-story")
            .gap(px(12.0))
            .p(px(16.0))
            .child(section_title("文档大纲（extract_headings）"))
            .child(
                v_flex()
                    .p(px(16.0))
                    .rounded(px(8.0))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(toc),
            )
    }
}

/// 综合文档示例：模拟一篇完整的技术文档。
struct MarkdownFullStory;

impl MarkdownFullStory {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl rgpui::Render for MarkdownFullStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let doc = r#"# rgpui 开发指南

## 快速开始

在 [`Cargo.toml`](https://doc.rust-lang.org/cargo/reference/manifest.html) 中添加依赖：

```toml
[dependencies]
rgpui = "1.0"
```

然后创建第一个窗口：

```rust
use rgpui::prelude::*;

fn main() {
    rgpui_platform::application().run(|cx| {
        cx.open_window(|window, cx| {
            // 你的 UI 代码
        });
    });
}
```

## 文本样式

这是一段普通文本。**粗体文本**、*斜体文本*、***粗斜体***、~~删除线文本~~。

行内代码 `println!("hello")` 也可以使用。

混合样式：**粗体中的 *斜体* 和 `代码`**，以及 *斜体中的 **粗体** 和 ~~删除线~~*。

## 链接与引用

访问 [rgpui 仓库](https://github.com/launcher-rs/rgpui) 了解更多信息。

> rgpui 是一个 GPU 加速的跨平台 UI 框架。
> 支持 Windows、macOS 和 Linux。

嵌套引用示例：

> 第一层引用
> > 第二层引用
> > > 第三层引用，支持多层嵌套。

## 列表

### 无序列表

- 第一项
- 第二项
  - 嵌套 A
  - 嵌套 B
- 第三项

### 有序列表

1. 打开文件
2. 编辑内容
3. 保存退出

### 任务列表

- [x] 已完成：实现基础组件
- [x] 已完成：添加主题系统
- [ ] 进行中：优化滚动性能
- [ ] 待办：支持动画系统

## 表格

| 组件 | 说明 | 状态 | 星标 |
|------|------|------|------|
| Button | 按钮 | 稳定 | ⭐ |
| Input | 输入框 | 稳定 | ⭐ |
| TabBar | 标签栏 | 稳定 | |
| Dialog | 对话框 | 稳定 | ⭐ |
| Table | 表格 | 稳定 | |
| List | 列表 | 稳定 | |

## 性能对比

与传统 CPU 渲染方案相比：

- **首帧速度**：GPU 直接绘制，无需布局树重建
- **内存占用**：增量更新，仅同步变化节点
- **滚动性能**：60fps 稳定，无卡顿

---

*本文档由 rgpui-markdown 组件渲染*"#;

        v_flex()
            .id("markdown-full-story")
            .gap(px(12.0))
            .p(px(16.0))
            .child(section_title("综合文档示例"))
            .child(
                v_flex()
                    .p(px(16.0))
                    .rounded(px(8.0))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(Markdown::new(doc)),
            )
    }
}

/// 章节标题辅助函数。
fn section_title(text: impl Into<rgpui::SharedString> + IntoElement) -> impl IntoElement {
    div()
        .text_size(px(14.0))
        .text_color(rgpui::hsla(0.0, 0.0, 0.55, 1.0))
        .child(text)
}

/// 章节副标题辅助函数。
fn section_subtitle(text: impl Into<rgpui::SharedString> + IntoElement) -> impl IntoElement {
    div()
        .text_size(px(12.0))
        .mt(px(8.0))
        .text_color(rgpui::hsla(0.0, 0.0, 0.45, 1.0))
        .child(text)
}
