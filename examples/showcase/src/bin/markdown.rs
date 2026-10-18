//! Markdown 渲染示例。
//!
//! 演示两条 Markdown 相关能力：
//! - **块级渲染**：`BlockRenderer::parse_markdown` 把 Markdown 拆成 `BlockElement`
//!   （标题/段落/代码块/引用/分隔线…），再由调用方按 `BlockType` 自行渲染。
//! - **插件式渲染**：`MarkdownRenderer` 注册内置插件（代码高亮/数学/任务列表）后
//!   输出渲染结果字符串，并可自定义样式。
//!
//! 运行：
//!
//! ```text
//! cargo run -p showcase --bin markdown
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::markdown_ext::{CodeHighlightPlugin, MathPlugin, TaskListPlugin};
use rgpui::{
    AnyElement, App, Bounds, Context, IntoElement, ParentElement, Render, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size, v_flex,
};
use rgpui_platform::application;

/// 示例根视图。
struct MarkdownDemo {
    /// 待解析的 Markdown 文本。
    input: String,
    /// 插件式渲染器。
    renderer: MarkdownRenderer,
}

impl MarkdownDemo {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        // 注册内置插件：代码高亮、数学、任务列表。
        let mut renderer = MarkdownRenderer::new();
        renderer.register_plugin(Box::new(CodeHighlightPlugin));
        renderer.register_plugin(Box::new(MathPlugin));
        renderer.register_plugin(Box::new(TaskListPlugin));
        renderer.add_style("highlight", "background: yellow;");

        Self {
            input: "# 标题\n\n这是 **粗体** 和 *斜体* 段落。\n\n> 一段引用\n\n```rust\nfn main() {}\n```\n\n---\n"
                .to_string(),
            renderer,
        }
    }
}

impl Render for MarkdownDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let renderer = BlockRenderer::new();
        let blocks = renderer.parse_markdown(&self.input);
        let plugin_output = self.renderer.render(&self.input);

        v_flex()
            .id("markdown-demo")
            .size_full()
            .gap(px(16.0))
            .p(px(24.0))
            .overflow_scroll()
            .bg(rgb(0xffffff))
            .child(div().text_2xl().child("Markdown 渲染"))
            // 1. 块级渲染。
            .child(section_title("1. 块级渲染（BlockRenderer）"))
            .child(
                v_flex()
                    .gap(px(6.0))
                    .p(px(12.0))
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0xe0e0e0))
                    .children(blocks.into_iter().map(render_block_element)),
            )
            // 2. 插件式渲染。
            .child(section_title("2. 插件式渲染（MarkdownRenderer）"))
            .child(
                div()
                    .rounded_md()
                    .bg(rgb(0x282c34))
                    .p(px(12.0))
                    .text_sm()
                    .text_color(rgb(0xabb2bf))
                    .child(plugin_output),
            )
    }
}

/// 小标题。
fn section_title(text: &str) -> impl IntoElement {
    div().text_lg().child(text.to_string())
}

/// 把单个块元素渲染成 UI。
fn render_block_element(block: BlockElement) -> AnyElement {
    match &block.block_type {
        BlockType::Heading(level) => {
            let base = div()
                .id(format!("md-h-{}", block.content))
                .mb(px(4.0))
                .text_color(rgb(0x1a1a1a));
            let styled = match level {
                1 => base.text_2xl().font_bold(),
                2 => base.text_xl().font_semibold(),
                3 => base.text_lg().font_medium(),
                _ => base.text_base(),
            };
            styled.child(block.content.clone()).into_any_element()
        }
        BlockType::Paragraph => div()
            .id(format!(
                "md-p-{}",
                block.content.chars().take(8).collect::<String>()
            ))
            .mb(px(8.0))
            .text_sm()
            .text_color(rgb(0x333333))
            .child(block.content.clone())
            .into_any_element(),
        BlockType::CodeBlock => {
            let lang = block
                .attributes
                .get("language")
                .map(|s| s.as_str())
                .unwrap_or("");
            div()
                .id("md-code")
                .mb(px(8.0))
                .rounded_md()
                .bg(rgb(0x282c34))
                .p(px(12.0))
                .child(
                    v_flex()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x888888))
                                .child(format!("语言: {lang}")),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0xabb2bf))
                                .child(block.content.clone()),
                        ),
                )
                .into_any_element()
        }
        BlockType::Blockquote => div()
            .id("md-quote")
            .mb(px(8.0))
            .pl(px(12.0))
            .border_l(px(3.0))
            .border_color(rgb(0x0078d4))
            .text_sm()
            .text_color(rgb(0x666666))
            .child(block.content.clone())
            .into_any_element(),
        BlockType::HorizontalRule => div()
            .id("md-hr")
            .my(px(12.0))
            .h(px(1.0))
            .bg(rgb(0xe9ecef))
            .into_any_element(),
        _ => div()
            .text_sm()
            .child(block.content.clone())
            .into_any_element(),
    }
}

/// 启动示例。
fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(720.0), px(640.0)), cx);
        let _ = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| MarkdownDemo::new(window, cx)),
        );
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}
