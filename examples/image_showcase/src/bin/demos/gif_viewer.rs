use rgpui::{
    App, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div, img, prelude::*, px,
    size,
};
use rgpui_platform::application;
use std::path::PathBuf;

struct GifViewer {
    gif_path: PathBuf,
}

impl GifViewer {
    fn new(gif_path: PathBuf) -> Self {
        Self { gif_path }
    }
}

impl Render for GifViewer {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            img(self.gif_path.clone())
                .size_full()
                .object_fit(rgpui::ObjectFit::Contain)
                .id("gif"),
        )
    }
}

pub fn run_example() {
    env_logger::init();
    application().run(|cx: &mut App| {
        let gif_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/black-cat-typing.gif");
        let bounds = Bounds::centered(None, size(px(800.), px(600.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                ..Default::default()
            },
            |_, cx| cx.new(|_| GifViewer::new(gif_path)),
        )
        .unwrap();
        cx.activate(true);
    });
}
