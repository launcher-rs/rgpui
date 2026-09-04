#![cfg_attr(target_family = "wasm", no_main)]

//! View 示例 —— 基于 `View` 原语组合文本输入组件。
//!
//! 核心思路：文本输入看似简单实则复杂，`View` 让组合变得简单。
//! 以下三个组件分别展示在独立的区块中：
//!
//! * `Editor` —— 核心实体：光标、闪烁、焦点、键盘输入以及专用文本渲染器，
//!   所有复杂逻辑都封装在这里。
//! * `String` —— 数据层。通过 `editor.text(cx)` / `value.read(cx)` 获取内容。
//! * `Input` / `TextArea` —— 布局层。各自接受一个 `String`（内部自行创建 Editor）
//!   或一个已有的 `Editor`（可读取其光标状态）。
//!
//! 运行方式：`cargo run -p view_example`

mod example_editor;
mod example_input;
mod example_text_area;

#[cfg(test)]
mod example_tests;

use example_editor::Editor;
use example_input::Input;
use example_text_area::TextArea;

use rgpui::{
    App, Bounds, Context, Div, Entity, IntoElement, KeyBinding, Render, SharedString, Window,
    WindowBounds, WindowOptions, actions, div, hsla, prelude::*, px, rgb, size,
};
use rgpui_platform::application;

actions!(
    view_example,
    [Backspace, Delete, Left, Right, Home, End, Enter, Quit]
);

/// 一个无状态的轻量视图，读取编辑器的光标位置并组合在编辑器旁边
/// —— 两个视图共享同一个实体，无需额外连接。
#[derive(IntoElement)]
struct CursorReadout {
    editor: Entity<Editor>,
}

impl CursorReadout {
    fn new(editor: Entity<Editor>) -> Self {
        Self { editor }
    }
}

impl rgpui::RenderOnce for CursorReadout {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let cursor = self.editor.read(cx).cursor;
        div()
            .text_sm()
            .text_color(hsla(0., 0., 0.45, 1.))
            .child(SharedString::from(format!("cursor @ {cursor}")))
    }
}

struct ViewExample;

impl ViewExample {
    fn new() -> Self {
        Self
    }
}

impl Render for ViewExample {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 数据层：纯字符串，通过 hook 在顶部分配。
        let name = window.use_state(cx, |_, _| String::new());
        let email = window.use_state(cx, |_, _| String::from("me@example.com"));
        let bio = window.use_state(cx, |_, _| String::new());
        // 内部拥有字符串的编辑器 —— 顶部无需额外的字段。
        let notes = window.use_state(cx, |window, cx| Editor::new("multi\nline", window, cx));
        let owned = window.use_state(cx, |window, cx| Editor::new("editable", window, cx));

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0xf0f0f0))
            .p(px(24.))
            .gap(px(24.))
            .child(
                section("输入框 —— 基于 String（光标在内部管理）")
                    .child(Input::new(name).width(px(320.)))
                    .child(
                        Input::new(email)
                            .width(px(320.))
                            .color(hsla(0., 0., 0.3, 1.)),
                    ),
            )
            .child(
                section("输入框 —— 基于 Editor（可读取其光标）").child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(Input::editor(owned.clone()).width(px(320.)))
                        .child(CursorReadout::new(owned)),
                ),
            )
            .child(
                section("文本域 —— 基于 String 或 Editor")
                    .child(TextArea::new(bio, 3))
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap(px(12.))
                            .child(TextArea::editor(notes.clone(), 3).color(hsla(
                                250. / 360.,
                                0.7,
                                0.4,
                                1.,
                            )))
                            .child(CursorReadout::new(notes)),
                    ),
            )
    }
}

/// 创建带标签的垂直区块。
fn section(title: &str) -> Div {
    div().flex().flex_col().gap(px(8.)).child(
        div()
            .text_sm()
            .text_color(hsla(0., 0., 0.3, 1.))
            .child(SharedString::from(title.to_string())),
    )
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(560.0), px(480.0)), cx);
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, None),
            KeyBinding::new("delete", Delete, None),
            KeyBinding::new("left", Left, None),
            KeyBinding::new("right", Right, None),
            KeyBinding::new("home", Home, None),
            KeyBinding::new("end", End, None),
            KeyBinding::new("enter", Enter, None),
            KeyBinding::new("cmd-q", Quit, None),
        ]);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| ViewExample::new()),
        )
        .unwrap();

        cx.on_action(|_: &Quit, cx| cx.quit());
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
