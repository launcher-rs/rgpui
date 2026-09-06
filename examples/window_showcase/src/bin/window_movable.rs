#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, FocusHandle, Window, WindowBounds, WindowOptions, div, prelude::*, px,
    rgb, size,
};
use rgpui::{SharedString, TitlebarOptions};
use rgpui_platform::application;

struct ExampleWindow {
    label: SharedString,
    focus_handle: FocusHandle,
}

impl Render for ExampleWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(0x2e2e2e))
            .size_full()
            .justify_center()
            .items_center()
            .p_8()
            .text_lg()
            .text_color(rgb(0xffffff))
            .child(self.label.clone())
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0xb0b0b0))
                    .child("Try to drag the titlebar, and check the Window menu."),
            )
    }
}

fn open_test_window(
    cx: &mut App,
    bounds: Bounds<rgpui::Pixels>,
    label: &str,
    is_movable: bool,
    appears_transparent: bool,
) {
    let label = SharedString::from(format!(
        "{label}\nis_movable: {is_movable}\n\
         appears_transparent: {appears_transparent}"
    ));

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            is_movable,
            titlebar: Some(TitlebarOptions {
                title: Some(label.clone()),
                appears_transparent,
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            cx.new(|cx| {
                let focus_handle = cx.focus_handle();
                focus_handle.focus(window, cx);
                ExampleWindow {
                    label,
                    focus_handle,
                }
            })
        },
    )
    .unwrap();
}

fn run_example() {
    application().run(|cx: &mut App| {
        let window_size = size(px(420.), px(280.0));
        let base = Bounds::centered(None, window_size, cx);

        // (label, is_movable, appears_transparent, col, row)
        let windows = [
            ("Native titlebar, movable", true, false, 0.0, 0.0),
            ("Native titlebar, NOT movable", false, false, 1.0, 0.0),
            ("Custom titlebar, movable", true, true, 0.0, 1.0),
            ("Custom titlebar, NOT movable", false, true, 1.0, 1.0),
        ];

        for (label, is_movable, appears_transparent, col, row) in windows {
            let mut bounds = base;
            bounds.origin.x += window_size.width * col;
            bounds.origin.y += window_size.height * row;
            open_test_window(cx, bounds, label, is_movable, appears_transparent);
        }
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
