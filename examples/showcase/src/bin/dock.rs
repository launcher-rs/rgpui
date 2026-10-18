//! Dock 布局演示：四区域标签页、拖拽跨区、关闭/显隐、布局持久化。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, Render, Window, WindowBounds, WindowOptions,
    components::{DockArea, DockAreaState, DockPanel, DockPosition},
    div, h_flex,
    prelude::*,
    px, size, v_flex,
};
use rgpui_platform::application;

struct DockDemo {
    dock: rgpui::Entity<DockAreaState>,
    saved_len: usize,
}

impl DockDemo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dock = cx.new(|_| DockAreaState::new());
        dock.update(cx, |dock, cx| {
            dock.add_panel(
                DockPanel::new("files", "文件树", DockPosition::Left, |_, _| {
                    div()
                        .p(px(12.0))
                        .child("src/\n  main.rs\n  lib.rs")
                        .into_any_element()
                }),
                cx,
            );
            dock.add_panel(
                DockPanel::new("outline", "大纲", DockPosition::Left, |_, _| {
                    div()
                        .p(px(12.0))
                        .child("fn main\nfn render")
                        .into_any_element()
                }),
                cx,
            );
            dock.add_panel(
                DockPanel::new("editor", "编辑器", DockPosition::Center, |_, _| {
                    div()
                        .p(px(12.0))
                        .child("fn main() {\n    println!(\"hello\");\n}")
                        .into_any_element()
                }),
                cx,
            );
            dock.add_panel(
                DockPanel::new("props", "属性", DockPosition::Right, |_, _| {
                    div()
                        .p(px(12.0))
                        .child("width: 240\nheight: 自适应")
                        .into_any_element()
                }),
                cx,
            );
            dock.add_panel(
                DockPanel::new("terminal", "终端", DockPosition::Bottom, |_, _| {
                    div().p(px(12.0)).child("$ cargo run").into_any_element()
                }),
                cx,
            );
        });
        let _ = window;
        Self { dock, saved_len: 0 }
    }
}

impl Render for DockDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let saved = self.saved_len;
        v_flex()
            .size_full()
            .child(
                h_flex()
                    .gap(px(8.0))
                    .p(px(8.0))
                    .child(
                        rgpui::Button::new("dock-save")
                            .label("保存布局")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let json = this.dock.read(cx).layout_json();
                                this.saved_len = json.len();
                                cx.notify();
                            })),
                    )
                    .child(
                        rgpui::Button::new("dock-hide-right")
                            .label("显隐右侧")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let visible = this.dock.read(cx).is_visible(DockPosition::Right);
                                this.dock.update(cx, |dock, cx| {
                                    dock.set_visible(DockPosition::Right, !visible, cx);
                                });
                            })),
                    )
                    .child(div().text_sm().child(format!("已保存布局 {saved} 字节"))),
            )
            .child(div().flex_1().child(DockArea::new(self.dock.clone())))
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(960.0), px(640.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| DockDemo::new(window, cx)),
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
