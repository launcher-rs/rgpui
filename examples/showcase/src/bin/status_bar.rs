//! 状态栏（StatusBar）示例。
//!
//! `StatusBar` 是纯值驱动的组件：左/右两组 `StatusBarItem`，每项可带图标、提示、
//! 点击回调、置灰（muted）/激活（active）/禁用（disabled）等状态。每次重绘按当前
//! 状态重新组装即可，无需同步。
//!
//! 运行：
//!
//! ```text
//! cargo run -p showcase --bin status_bar
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::components::status_bar::{StatusBar, StatusBarItem};
use rgpui::{
    App, Bounds, Context, IntoElement, ParentElement, Render, Styled, Window, WindowBounds,
    WindowOptions, div, h_flex, prelude::*, px, rgb, size, v_flex,
};
use rgpui_platform::application;

/// 状态栏演示状态（纯值）。
struct StatusBarDemo {
    line: usize,
    column: usize,
    language: &'static str,
    lsp_connected: bool,
    errors: usize,
    warnings: usize,
    branch: &'static str,
}

impl StatusBarDemo {
    fn new() -> Self {
        Self {
            line: 42,
            column: 15,
            language: "Rust",
            lsp_connected: true,
            errors: 2,
            warnings: 5,
            branch: "main",
        }
    }

    /// 按当前值组装状态栏。
    fn status_bar(&self) -> StatusBar {
        let lsp = if self.lsp_connected {
            "rust-analyzer ✓".to_string()
        } else {
            "No LSP".to_string()
        };
        let mut left = vec![StatusBarItem::new(lsp).id("lsp").active(self.lsp_connected)];
        if self.errors > 0 {
            left.push(StatusBarItem::new(format!("{} errors", self.errors)).id("errors"));
        }
        if self.warnings > 0 {
            left.push(StatusBarItem::new(format!("{} warnings", self.warnings)).id("warnings"));
        }
        let right = vec![
            StatusBarItem::new(self.branch).id("branch").muted(true),
            StatusBarItem::new(format!("Ln {}, Col {}", self.line, self.column))
                .id("cursor")
                .muted(true),
            StatusBarItem::new(self.language).id("language").muted(true),
            StatusBarItem::new("UTF-8").id("encoding").muted(true),
        ];
        StatusBar::new().left(left).right(right)
    }
}

impl Render for StatusBarDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status_bar = self.status_bar();

        v_flex()
            .id("status-bar-demo")
            .size_full()
            .bg(rgb(0xffffff))
            .child(
                v_flex()
                    .flex_1()
                    .gap(px(16.0))
                    .p(px(24.0))
                    .child(div().text_2xl().child("状态栏（StatusBar）"))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .flex_wrap()
                            .child(button("sb-line-up", "行号 +1", cx, |this, cx| {
                                this.line += 1;
                                cx.notify();
                            }))
                            .child(button("sb-col-up", "列号 +5", cx, |this, cx| {
                                this.column += 5;
                                cx.notify();
                            }))
                            .child(button("sb-lang", "切换语言", cx, |this, cx| {
                                this.language = if this.language == "Rust" {
                                    "TypeScript"
                                } else {
                                    "Rust"
                                };
                                cx.notify();
                            }))
                            .child(button("sb-lsp", "切换 LSP", cx, |this, cx| {
                                this.lsp_connected = !this.lsp_connected;
                                cx.notify();
                            }))
                            .child(button("sb-error", "错误 +1", cx, |this, cx| {
                                this.errors += 1;
                                cx.notify();
                            }))
                            .child(button("sb-reset", "重置", cx, |this, cx| {
                                *this = StatusBarDemo::new();
                                cx.notify();
                            })),
                    ),
            )
            // 底部状态栏。
            .child(status_bar)
    }
}

/// 一个简单的按钮辅助函数。
fn button(
    id: &'static str,
    label: &'static str,
    cx: &mut Context<StatusBarDemo>,
    on_click: impl Fn(&mut StatusBarDemo, &mut Context<StatusBarDemo>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px(px(12.0))
        .py(px(6.0))
        .rounded_md()
        .bg(rgb(0xf0f0f0))
        .cursor_pointer()
        .child(label)
        .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
}

/// 启动示例。
fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(720.0), px(420.0)), cx);
        let _ = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_window, cx| cx.new(|_cx| StatusBarDemo::new()),
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
