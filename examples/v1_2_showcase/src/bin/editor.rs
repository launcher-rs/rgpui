//! Editor 演示：CodeEditor + tree-sitter 高亮/折叠 + 行操作 + 键入体验 +
//! 符号大纲 + 多光标（键位来自全局 `init_all` 默认注册）。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div, h_flex,
    highlight::{DocumentSymbol, rust_highlighter},
    input_ui::{Input, InputEvent, InputState},
    prelude::*,
    px, rgb, size, v_flex,
};
use rgpui_platform::application;

const SAMPLE: &str = "fn main() {\n    let name = \"rgpui\";\n    println!(\"hello, {name}\");\n}\n\nstruct Point {\n    x: f32,\n    y: f32,\n}\n\nimpl Point {\n    fn len(&self) -> f32 {\n        (self.x * self.x + self.y * self.y).sqrt()\n    }\n}\n";

const READONLY_SAMPLE: &str = "只读预览：可选可复制，不可编辑。右键菜单的剪切/粘贴/撤销自动禁用。";

struct EditorDemo {
    input: rgpui::Entity<InputState>,
    readonly: rgpui::Entity<InputState>,
    symbols: Vec<DocumentSymbol>,
}

impl EditorDemo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .code_editor("rust")
                .auto_close_pairs(true)
                .bracket_match(true)
                .current_line_highlight(true)
                .line_comment_prefix("//");
            state.replace(SAMPLE, window, cx);
            state.set_highlighter(Some(rust_highlighter()), window, cx);
            state
        });
        let readonly = cx.new(|cx| {
            let mut state = InputState::new(window, cx).multi_line(true);
            state.replace(READONLY_SAMPLE, window, cx);
            state
        });
        let symbols = input.read_with(cx, |state, _| state.document_symbols());

        // 文本一改就刷新大纲。
        cx.subscribe(&input, |this, _, event, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            this.symbols = this
                .input
                .read_with(cx, |state, _| state.document_symbols());
            cx.notify();
        })
        .detach();

        Self {
            input,
            readonly,
            symbols,
        }
    }

    fn goto(&mut self, symbol: &DocumentSymbol, cx: &mut Context<Self>) {
        let symbol = symbol.clone();
        self.input.update(cx, |state, cx| {
            state.goto_symbol(&symbol, cx);
        });
    }
}

impl Render for EditorDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let symbols = self.symbols.clone();
        let demo = cx.entity();
        h_flex().size_full().gap(px(12.0)).p(px(12.0)).child(
            v_flex()
                .w(px(220.0))
                .gap(px(4.0))
                .child(div().text_sm().child("大纲（符号）"))
                .children(symbols.into_iter().map(|symbol| {
                    let label = format!("{} · {}行", symbol.name, symbol.start_row + 1);
                    let demo = demo.clone();
                    div()
                        .id(("outline-symbol", symbol.start_row))
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|this| this.bg(rgb(0x000000).opacity(0.05)))
                        .child(div().text_sm().child(label))
                        .on_click(move |_, _, cx| {
                            demo.update(cx, |this, cx| {
                                this.goto(&symbol, cx);
                            });
                        })
                })),
        )
        .child(
            v_flex()
                .flex_1()
                .gap(px(8.0))
                .child(
                    div().text_xs().child(
                        "行操作：Shift+Alt+↓复制行 Ctrl+Shift+K删行 Alt+↑↓移行 Ctrl+/注释 Ctrl+J合行 ｜ \
                         多光标：Ctrl+Alt+↑↓加光标 Alt+点击 ｜ 键入：自动补括号/电缩进/括号匹配/当前行高亮",
                    ),
                )
                .child(Input::new(&self.input).flex_1())
                .child(div().text_xs().child("只读预览："))
                .child(Input::new(&self.readonly).read_only(true).h(px(72.0))),
        )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(980.0), px(640.0)), cx);
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
