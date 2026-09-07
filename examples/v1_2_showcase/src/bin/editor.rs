//! Editor 演示：CodeEditor 模式 + tree-sitter 高亮/折叠（需 `--features tree-sitter`）。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div,
    highlight::rust_highlighter,
    input_ui::{Input, InputState},
    prelude::*,
    px, size, v_flex,
};
use rgpui_platform::application;

const SAMPLE: &str = "fn main() {\n    let name = \"rgpui\";\n    println!(\"hello, {name}\");\n}\n\nstruct Point {\n    x: f32,\n    y: f32,\n}\n";

struct EditorDemo {
    input: rgpui::Entity<InputState>,
}

impl EditorDemo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx).code_editor("rust");
            state.replace(SAMPLE, window, cx);
            state.set_highlighter(Some(rust_highlighter()), window, cx);
            state
        });
        Self { input }
    }
}

impl Render for EditorDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .gap(px(8.0))
            .p(px(12.0))
            .child(
                div()
                    .text_sm()
                    .child("Rust 高亮 + 折叠（行号栏三角）由 tree-sitter 驱动"),
            )
            .child(div().flex_1().child(Input::new(&self.input)))
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(760.0), px(560.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| EditorDemo::new(window, cx)),
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
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}
