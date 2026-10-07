#![cfg_attr(target_family = "wasm", no_main)]
#![allow(unexpected_cfgs)]

fn run_example() {
    #[cfg(target_os = "linux")]
    example::main();

    #[cfg(not(target_os = "linux"))]
    panic!("This example requires a linux system.");
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

/// Linux 面板类窗口：Wayland 走 layer-shell 协议，X11 走 EWMH（DOCK 窗口 + strut）
#[cfg(target_os = "linux")]
mod example {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use rgpui::{
        App, Bounds, Context, FontWeight, Pixels, Size, Window, WindowBackgroundAppearance,
        WindowBounds, WindowKind, WindowOptions, div, layer_shell::*, point, prelude::*, px, rems,
        rgba, white,
    };
    use rgpui_platform::application;

    /// 面板高度（逻辑像素）
    const PANEL_HEIGHT: Pixels = px(200.);
    /// 面板与屏幕上边缘的间距（逻辑像素）
    const TOP_GAP: Pixels = px(20.);
    /// 独占区域宽度：间距 + 面板高度（逻辑像素）
    const EXCLUSIVE_ZONE: Pixels = px(220.);

    struct LayerShellExample;

    impl LayerShellExample {
        fn new(cx: &mut Context<Self>) -> Self {
            cx.spawn(async move |this, cx| {
                loop {
                    let _ = this.update(cx, |_, cx| cx.notify());
                    cx.background_executor()
                        .timer(Duration::from_millis(500))
                        .await;
                }
            })
            .detach();

            LayerShellExample
        }
    }

    impl Render for LayerShellExample {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let hours = (now / 3600) % 24;
            let minutes = (now / 60) % 60;
            let seconds = now % 60;

            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(rems(4.5))
                .font_weight(FontWeight::EXTRA_BOLD)
                .text_color(white())
                .bg(rgba(0x0000044))
                .rounded_xl()
                .child(format!("{:02}:{:02}:{:02}", hours, minutes, seconds))
        }
    }

    pub fn main() {
        application().run(|cx: &mut App| {
            let window = cx.open_window(
                WindowOptions {
                    titlebar: None,
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(0.), TOP_GAP),
                        size: Size::new(px(500.), PANEL_HEIGHT),
                    })),
                    app_id: Some("gpui-layer-shell-example".to_string()),
                    window_background: WindowBackgroundAppearance::Transparent,
                    kind: WindowKind::LayerShell(LayerShellOptions {
                        namespace: "gpui".to_string(),
                        anchor: Anchor::LEFT | Anchor::RIGHT | Anchor::TOP,
                        margin: Some((TOP_GAP, px(0.), px(0.), px(0.))),
                        keyboard_interactivity: KeyboardInteractivity::None,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(LayerShellExample::new),
            );

            let Ok(handle) = window else {
                return;
            };
            let _ = handle.update(cx, |_, window, _| {
                // 独占区域：从屏幕上边缘起让出「间距 + 面板高度」，
                // 其他窗口（Wayland 的表面、X11 的窗口）都不会压到面板上。
                // 两个后端都是运行时调用，所以创建选项里没有写 exclusive_zone
                window.set_exclusive_edge(Anchor::TOP);
                window.set_exclusive_zone(EXCLUSIVE_ZONE);
            });
        });
    }
}
