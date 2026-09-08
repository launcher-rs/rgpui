//! Editor 演示：`EditorState` + `Editor` 组件（P3b 迁移）。
//!
//! 主窗格走编辑器状态（多行 + 行号/折叠配置/大纲缓存/符号跳转/高亮接入内聚在
//! `EditorState` 里，渲染走 `Editor` 自带行号列号状态行）；只读窗格保留表单
//! `Input` 做对照。tree-sitter 高亮/折叠 + 行操作 + 多光标（键位来自全局
//! `init_all` 默认注册）。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div, h_flex,
    input_ui::{Editor, EditorState, InputEvent, InputState, TextArea},
    prelude::*,
    px, rgb, size, v_flex,
};
use rgpui_platform::application;

const SAMPLE: &str = "fn main() {\n    let name = \"rgpui\";\n    println!(\"hello, {name}\");\n}\n\nstruct Point {\n    x: f32,\n    y: f32,\n}\n\nimpl Point {\n    fn len(&self) -> f32 {\n        (self.x * self.x + self.y * self.y).sqrt()\n    }\n}\n";

const READONLY_SAMPLE: &str = "只读预览：可选可复制，不可编辑。右键菜单的剪切/粘贴/撤销自动禁用。";

struct EditorDemo {
    editor: rgpui::Entity<EditorState>,
    input: rgpui::Entity<InputState>,
    readonly: rgpui::Entity<InputState>,
}

impl EditorDemo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 编辑器状态一次配好（多行 + 行号 + 折叠 + 键入体验 + 大纲订阅）。
        let editor = cx.new(|cx| EditorState::new(window, cx, SAMPLE));
        editor.update(cx, |state, cx| {
            // 高亮器经编辑器状态透传接入（大纲同步刷新）。
            state.set_highlighter(Some(rgpui::highlight::rust_highlighter()), window, cx);
        });
        let input = editor.read_with(cx, |state, _| state.input().clone());
        let readonly = cx.new(|cx| {
            let mut state = InputState::new(window, cx).multi_line(true);
            // 初始内容不进撤销栈（与 `EditorState::new` 同理）。
            state.set_value(READONLY_SAMPLE, window, cx);
            state
        });

        // 大纲缓存由编辑器状态维护，这里只在文本变更时重渲染。
        cx.subscribe(&input, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();

        Self {
            editor,
            input,
            readonly,
        }
    }

    fn goto(&mut self, symbol: &rgpui::highlight::DocumentSymbol, cx: &mut Context<Self>) {
        let symbol = symbol.clone();
        self.editor.update(cx, |state, cx| {
            state.goto_symbol(&symbol, cx);
        });
    }
}

impl Render for EditorDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let symbols = self
            .editor
            .read_with(cx, |state, _| state.outline().to_vec());
        let demo = cx.entity();
        h_flex()
            .size_full()
            .items_stretch()
            .gap(px(12.0))
            .p(px(12.0)).child(
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
                .child(Editor::new(self.input.clone()).flex_1())
                .child(div().text_xs().child("只读预览（TextArea）："))
                .child(TextArea::new(&self.readonly).read_only(true)),
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
