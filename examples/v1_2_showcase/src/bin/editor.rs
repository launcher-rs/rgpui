//! Editor 演示：CodeEditor 模式 + tree-sitter 高亮/折叠（需 `--features tree-sitter`）。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, KeyBinding, Render, Window, WindowBounds, WindowOptions, div,
    highlight::rust_highlighter,
    input_ui::{
        AddCursorAbove, AddCursorBelow, Backspace, Copy, CopyLine, Cut, Delete, DeleteLine, Enter,
        Escape, Input, InputState, JoinLines, MoveDown, MoveEnd, MoveHome, MoveLeft, MoveLineDown,
        MoveLineUp, MoveRight, MoveUp, Paste, Redo, SelectAll, ToggleLineComment, Undo,
    },
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
            .child(Input::new(&self.input).flex_1())
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        // 输入框编辑键位（应用级注册；secondary = macOS Cmd / 其他平台 Ctrl）。
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, None),
            KeyBinding::new("delete", Delete, None),
            KeyBinding::new("left", MoveLeft, None),
            KeyBinding::new("right", MoveRight, None),
            KeyBinding::new("up", MoveUp, None),
            KeyBinding::new("down", MoveDown, None),
            KeyBinding::new("home", MoveHome, None),
            KeyBinding::new("end", MoveEnd, None),
            KeyBinding::new(
                "enter",
                Enter {
                    secondary: false,
                    shift: false,
                },
                None,
            ),
            KeyBinding::new("escape", Escape, None),
            KeyBinding::new("secondary-a", SelectAll, None),
            KeyBinding::new("secondary-c", Copy, None),
            KeyBinding::new("secondary-x", Cut, None),
            KeyBinding::new("secondary-v", Paste, None),
            KeyBinding::new("secondary-z", Undo, None),
            KeyBinding::new("secondary-shift-z", Redo, None),
            // 行操作（§I，与 `Input` 上下文默认键位一致，这里显式注册以便单测外手动验证）。
            KeyBinding::new("shift-alt-down", CopyLine, None),
            KeyBinding::new("secondary-shift-k", DeleteLine, None),
            KeyBinding::new("alt-up", MoveLineUp, None),
            KeyBinding::new("alt-down", MoveLineDown, None),
            KeyBinding::new("secondary-/", ToggleLineComment, None),
            KeyBinding::new("secondary-j", JoinLines, None),
            // 多光标（§L）：Alt+点击加光标，上下加光标动作，Esc 坍缩。
            KeyBinding::new("secondary-alt-up", AddCursorAbove, None),
            KeyBinding::new("secondary-alt-down", AddCursorBelow, None),
        ]);
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
