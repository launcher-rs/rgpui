#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, SharedString, Window, WindowBounds, WindowOptions, div, prelude::*, px,
    rgb, size,
};
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
            .bg(rgb(0x505050))
            .size(px(500.0))
            .justify_center()
            .items_center()
            .shadow_lg()
            .border_1()
            .border_color(rgb(0x0000ff))
            .text_xl()
            .text_color(rgb(0xffffff))
            .child(format!("Hello, {}!", &self.text))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .size_8()
                            .bg(rgpui::red())
                            .border_1()
                            .border_dashed()
                            .rounded_md()
                            .border_color(rgpui::white()),
                    )
                    .child(
                        div()
                            .size_8()
                            .bg(rgpui::green())
                            .border_1()
                            .border_dashed()
                            .rounded_md()
                            .border_color(rgpui::white()),
                    )
                    .child(
                        div()
                            .size_8()
                            .bg(rgpui::blue())
                            .border_1()
                            .border_dashed()
                            .rounded_md()
                            .border_color(rgpui::white()),
                    )
                    .child(
                        div()
                            .size_8()
                            .bg(rgpui::yellow())
                            .border_1()
                            .border_dashed()
                            .rounded_md()
                            .border_color(rgpui::white()),
                    )
                    .child(
                        div()
                            .size_8()
                            .bg(rgpui::black())
                            .border_1()
                            .border_dashed()
                            .rounded_md()
                            .rounded_md()
                            .border_color(rgpui::white()),
                    )
                    .child(
                        div()
                            .size_8()
                            .bg(rgpui::white())
                            .border_1()
                            .border_dashed()
                            .rounded_md()
                            .border_color(rgpui::black()),
                    ),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
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
