//! 对话框示例。
//!
//! 演示 `Dialog`（经 Root 打开/挂载）、`AlertDialog`（触发器式）和
//! `FocusTrapElement`（Tab 焦点锁定在容器内循环）。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, FocusHandle, FocusTrapElement, Render, Root, Window, WindowBounds,
    WindowOptions, div, h_flex, prelude::*, px, rgb, size, v_flex,
};
use rgpui_platform::application;

struct DialogDemo {
    container_focus: FocusHandle,
    last_result: String,
}

impl DialogDemo {
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            container_focus: cx.focus_handle(),
            last_result: "尚未操作".to_string(),
        }
    }

    /// 打开一个确认对话框。
    fn open_confirm(window: &mut Window, cx: &mut App) {
        let Some(Some(root)) = window.root::<Root>() else {
            return;
        };
        root.update(cx, |root, cx| {
            root.open_dialog(
                |dialog, _window, _cx| {
                    dialog
                        .title("确认删除？")
                        .content(|content, _, _| {
                            content.child(div().child("删除后无法恢复，请确认是否继续。"))
                        })
                        .width(px(360.0))
                },
                window,
                cx,
            );
        });
    }
}

impl Render for DialogDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let result = self.last_result.clone();

        v_flex()
            .size_full()
            .gap(px(16.0))
            .p(px(24.0))
            .bg(rgb(0xffffff))
            // 1. Dialog（经 Root 打开）
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(div().text_lg().child("1. Dialog（Root.open_dialog）"))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .id("open-dialog")
                                    .px(px(12.0))
                                    .py(px(6.0))
                                    .rounded_md()
                                    .bg(rgb(0x0078d4))
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .child("打开确认对话框")
                                    .on_click(|_, window, cx| {
                                        Self::open_confirm(window, cx);
                                    }),
                            )
                            .child(div().text_sm().child(format!("上次结果：{result}"))),
                    ),
            )
            // 2. AlertDialog（触发器式）
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(div().text_lg().child("2. AlertDialog（触发器）"))
                    .child(
                        rgpui::AlertDialog::new(cx)
                            .trigger(
                                div()
                                    .id("open-alert")
                                    .px(px(12.0))
                                    .py(px(6.0))
                                    .rounded_md()
                                    .bg(rgb(0x107c10))
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .child("打开警告框"),
                            )
                            .content(|content, _, _| {
                                content.child(
                                    div()
                                        .p(px(16.0))
                                        .child("这是一个 AlertDialog，点遮罩或按钮关闭。"),
                                )
                            }),
                    ),
            )
            // 3. 焦点陷阱：Tab 只在三个按钮间循环
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(div().text_lg().child("3. 焦点陷阱（Tab 循环）"))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .p(px(12.0))
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(0xe0e0e0))
                            .child(
                                div()
                                    .id("trap-btn-1")
                                    .px(px(12.0))
                                    .py(px(6.0))
                                    .rounded_sm()
                                    .bg(rgb(0xf0f0f0))
                                    .cursor_pointer()
                                    .child("按钮 1"),
                            )
                            .child(
                                div()
                                    .id("trap-btn-2")
                                    .px(px(12.0))
                                    .py(px(6.0))
                                    .rounded_sm()
                                    .bg(rgb(0xf0f0f0))
                                    .cursor_pointer()
                                    .child("按钮 2"),
                            )
                            .child(
                                div()
                                    .id("trap-btn-3")
                                    .px(px(12.0))
                                    .py(px(6.0))
                                    .rounded_sm()
                                    .bg(rgb(0xf0f0f0))
                                    .cursor_pointer()
                                    .child("按钮 3"),
                            )
                            .focus_trap("trap-demo", &self.container_focus),
                    )
                    .child(div().text_sm().child("在三个按钮上按 Tab，焦点不出容器。")),
            )
            // 对话框层挂载
            .when_some(Root::render_dialog_layer(window, cx), |this, layer| {
                this.child(layer)
            })
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(560.0), px(560.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| DialogDemo::new(window, cx)),
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
