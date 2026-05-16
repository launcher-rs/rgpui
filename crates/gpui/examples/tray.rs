//! System tray example
//!
//! This example demonstrates how to create a system tray icon with a context menu.

#![cfg_attr(target_family = "wasm", no_main)]

use gpui::{
    actions, App, Bounds, Context, MenuItem, Render, SharedString, Tray, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size,
};
use gpui_platform::application;

actions!(tray, [Quit, ToggleWindow]);

struct TrayExample {
    message: SharedString,
}

impl Render for TrayExample {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .size(px(400.0))
            .justify_center()
            .items_center()
            .text_xl()
            .text_color(rgb(0x505050))
            .child(div().child(self.message.clone()))
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x808080))
                    .child("Check the system tray for the menu"),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(400.), px(300.0)), cx);

        // Register actions
        cx.on_action(|_: &Quit, cx| {
            cx.quit();
        });

        // Create the tray
        let tray = Tray::new()
            .tooltip("GPUI Tray Example")
            .menu(|_cx| {
                vec![
                    MenuItem::action("Show Window", ToggleWindow),
                    MenuItem::separator(),
                    MenuItem::action("Quit", Quit),
                ]
            });

        cx.set_tray(tray, None);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| TrayExample {
                    message: "Hello from GPUI Tray!".into(),
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

fn main() {
    run_example();
}
