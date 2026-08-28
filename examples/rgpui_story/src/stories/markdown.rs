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
