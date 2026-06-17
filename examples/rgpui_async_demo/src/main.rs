use rgpui::*;
use rgpui_component::{button::*, *};
use rgpui_component_assets::Assets;
use rgpui_tokio;

#[derive(Default, Debug)]
pub struct Example {
    result: String,
}

impl Render for Example {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .child(format!("Result: {}", self.result))
            .child(
                Button::new("ok")
                    .primary()
                    .label("Let's Go!")
                    .on_click(cx.listener(|_this, _, _, cx| {
                        cx.spawn(async move |this, cx| {
                            // 在使用 tokion
                            let text = rgpui_tokio::Tokio::spawn(cx, async move {
                                reqwest::get("https://httpbin.org/ip")
                                    .await
                                    .unwrap()
                                    .text()
                                    .await
                                    .unwrap()
                            })
                            .await
                            .unwrap();
                            // 更新界面
                            this.update(cx, |this, _| {
                                this.result = text;
                            })
                            .ok();
                        })
                        .detach();
                    })),
            )
    }
}

fn main() {
    let app = rgpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        // This must be called before using any GPUI Component features.
        rgpui_component::init(cx);
        rgpui_tokio::init(cx);

        let bounds = Bounds::centered(None, size(px(1000.), px(600.)), cx);

        cx.spawn(async move |cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("异步测试".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|_| Example::default());
                    // This first level on the window, should be a Root.
                    cx.new(|cx| {
                        // You can refine the root view style by yourself.
                        Root::new(view, window, cx).bg(cx.theme().background)
                    })
                },
            )
            .expect("Failed to open window");
        })
        .detach();
    });
}
