//! 二维码组件示例（`qr-code` feature 门控）。
//!
//! 展示二维码的生成与自定义样式。
//!
//! 运行：
//!
//! ```text
//! cargo run -p rgpui --example qr_code --features qr-code
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::components::QRCodeComponent;
use rgpui::{
    App, AppContext as _, Bounds, Context, IntoElement, ParentElement, Render, Styled, Window,
    WindowBounds, WindowOptions, div, px, size,
};
use rgpui_platform::application;

/// 二维码示例根视图。
struct QrCodeApp;

impl Render for QrCodeApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p(px(24.0))
            .flex()
            .flex_col()
            .gap_6()
            .child(div().text_2xl().child("二维码组件示例"))
            .child(
                div()
                    .text_lg()
                    .child("默认二维码")
                    .child(QRCodeComponent::new("https://github.com/").size(px(180.0))),
            )
            .child(
                div().text_lg().child("自定义颜色").child(
                    QRCodeComponent::new("rgpui 二维码")
                        .size(px(180.0))
                        .fg_color(rgpui::hsla(0.6, 0.8, 0.45, 1.0))
                        .bg_color(rgpui::white()),
                ),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(600.), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| QrCodeApp),
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
