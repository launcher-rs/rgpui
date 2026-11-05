//! 差异查看器示例。
//!
//! 演示 `DiffView` 组件的行级差异显示能力：对比两段文本，以颜色高亮显示
//! 新增（绿色）、删除（红色）与未变更行，并可选显示行号。
//!
//! 运行：
//!
//! ```text
//! cargo run -p showcase --bin diff --features rgpui/diff
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::components::{DiffState, DiffView};
use rgpui::{
    App, Bounds, Context, Entity, IntoElement, ParentElement, Render, Window, WindowBounds,
    WindowOptions, div, h_flex, prelude::*, px, v_flex,
};
use rgpui_platform::application;

/// 差异演示根视图。
struct DiffDemo {
    /// 差异状态。
    diff_state: Entity<DiffState>,
}

impl DiffDemo {
    /// 创建视图并初始化差异状态。
    fn new(cx: &mut Context<Self>) -> Self {
        let old_text = r#"fn main() {
    println!("Hello, world!");
    let x = 5;
    let y = 10;
    println!("Sum: {}", x + y);
}
"#;

        let new_text = r#"fn main() {
    println!("Hello, rgpui!");
    let x = 5;
    let z = 15;
    println!("Sum: {}", x + z);
    println!("Done!");
}
"#;

        let diff_state = cx.new(|_| DiffState::new(old_text, new_text));

        Self { diff_state }
    }
}

impl Render for DiffDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let added = self.diff_state.read(cx).added_count();
        let removed = self.diff_state.read(cx).removed_count();
        let unchanged = self.diff_state.read(cx).unchanged_count();

        div()
            .id("diff-root")
            .size_full()
            .overflow_scroll()
            .bg(cx.theme().tokens.background.color)
            .child(
                v_flex()
                    .max_w(px(900.0))
                    .mx_auto()
                    .gap(px(16.0))
                    .p(px(24.0))
                    .child(div().text_2xl().child("差异查看器示例"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().tokens.muted_foreground.color)
                            .child("对比两段 Rust 代码，显示行级差异（新增、删除、未变更）。"),
                    )
                    .child(
                        h_flex()
                            .gap(px(16.0))
                            .child(div().text_sm().child(format!("新增: {added} 行")))
                            .child(div().text_sm().child(format!("删除: {removed} 行")))
                            .child(div().text_sm().child(format!("未变更: {unchanged} 行"))),
                    )
                    .child(
                        div()
                            .h(px(500.0))
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().tokens.border.color)
                            .bg(cx.theme().tokens.background.color)
                            .child(DiffView::new(&self.diff_state)),
                    ),
            )
    }
}

/// 启动示例窗口。
fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::theme::init(cx);

        let bounds = Bounds::centered(None, rgpui::size(px(960.0), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_window, cx| cx.new(DiffDemo::new),
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
/// WASM 入口：初始化 Web 环境后启动示例。
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}
