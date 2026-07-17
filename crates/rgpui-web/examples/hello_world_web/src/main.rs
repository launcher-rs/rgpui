use rgpui::*;
use rgpui_component::{button::*, *};
use rgpui_component_assets::Assets;

pub struct Example {
    click_count: u32,
}

impl Example {
    fn new(_: &mut Context<Self>) -> Self {
        Self { click_count: 0 }
    }
}

impl Render for Example {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg = cx.theme().tokens.background;
        let fg = cx.theme().foreground;

        div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .bg(bg)
            .text_color(fg)
            .child(
                div()
                    .text_xl()
                    .text_color(rgb(0xcdd6f4))
                    .child("Hello, World! (Web)"),
            )
            .child(
                div()
                    .id("counter")
                    .text_color(rgb(0xcdd6f4))
                    .child(format!("点击次数: {}", self.click_count)),
            )
            .child(
                Button::new("ok")
                    .primary()
                    .label("Let's Go!")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.click_count += 1;
                        cx.notify();
                    })),
            )
    }
}

fn main() {
    #[cfg(target_family = "wasm")]
    console_error_panic_hook::set_once();
    #[cfg(target_family = "wasm")]
    rgpui_platform::web_init();
    rgpui_platform::application().with_assets(Assets::default()).run(move |cx| {
        rgpui_component::init(cx);

        let bounds = Bounds::centered(None, size(px(480.), px(360.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(Example::new);
                cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background).bordered(false))
            },
        )
        .expect("Failed to open window");

        cx.activate(true);
    });
}
