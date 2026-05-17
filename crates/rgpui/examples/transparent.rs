//！ 背景透明

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{App, Bounds, Context, SharedString, Window, WindowBounds, WindowOptions, WindowDecorations, div, prelude::*, px, rgb, size, WindowBackgroundAppearance};
use rgpui_platform::application;

struct HelloWorld {
    text: SharedString,
}

impl Render for HelloWorld {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .size(px(500.0))
            .justify_center()
            .items_center()
            .text_xl()
            .text_color(rgb(0x505050))
            .child(
                div()
                    .child(format!("Hello, {}!", &self.text))
                    .bg(rgb(0xff00ff))
            )

    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // 背景透明
                window_background: WindowBackgroundAppearance::Transparent,
                // 去掉标题栏和边框
                window_decorations: Some(WindowDecorations::Client),

                titlebar: None,
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| HelloWorld {
                    text: "World".into(),
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
