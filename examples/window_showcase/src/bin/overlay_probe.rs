//! 透明浮层探针：用纯色判断 Overlay/普通窗口 × 透明/不透明背景 在 X11 上到底画没画出来。
//!
//! 用法：`overlay_probe [mode]`，mode 取
//! `ov_tr`（Overlay+Transparent）、`ov_op`（Overlay+Opaque）、
//! `fl_tr`（Floating+Transparent）、`fl_op`（Floating+Opaque）、
//! `ov_tr_win`（Overlay+Transparent，但只占半屏 Windowed，便于看边界）、
//! `fl_hid`（Floating+Opaque+show=false，验证隐藏启动不该上屏）。

use rgpui::{
    App, Bounds, Context, Pixels, Render, Window, WindowBackgroundAppearance, WindowBounds,
    WindowKind, WindowOptions, div, prelude::*, px, rgb, size,
};
use rgpui_platform::application;

/// 探针根视图：整屏铺红、左上偏中一块 200px 绿方，两种纯色在截图里极易判别。
struct Probe;

impl Render for Probe {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().relative().bg(rgb(0xFF0000)).child(
            div()
                .absolute()
                .left(px(100.))
                .top(px(100.))
                .size(px(200.))
                .bg(rgb(0x00FF00)),
        )
    }
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "ov_tr".into());
    let (kind, background, bounds_kind, show) = match mode.as_str() {
        "ov_tr" => (
            WindowKind::Overlay,
            WindowBackgroundAppearance::Transparent,
            "full",
            true,
        ),
        "ov_op" => (
            WindowKind::Overlay,
            WindowBackgroundAppearance::Opaque,
            "full",
            true,
        ),
        "fl_tr" => (
            WindowKind::Floating,
            WindowBackgroundAppearance::Transparent,
            "full",
            true,
        ),
        "fl_op" => (
            WindowKind::Floating,
            WindowBackgroundAppearance::Opaque,
            "full",
            true,
        ),
        "fl_hid" => (
            WindowKind::Floating,
            WindowBackgroundAppearance::Opaque,
            "full",
            false,
        ),
        "ov_tr_win" => (
            WindowKind::Overlay,
            WindowBackgroundAppearance::Transparent,
            "half",
            true,
        ),
        other => panic!("未知模式 {other}"),
    };
    application().run(move |cx: &mut App| {
        let bounds: Bounds<Pixels> = if bounds_kind == "half" {
            Bounds::new(
                rgpui::Point {
                    x: px(300.),
                    y: px(200.),
                },
                size(px(600.), px(400.)),
            )
        } else {
            cx.primary_display()
                .map(|d| d.bounds())
                .unwrap_or_else(|| Bounds::centered(None, size(px(1280.), px(800.)), cx))
        };
        cx.open_window(
            WindowOptions {
                titlebar: None,
                window_bounds: Some(if bounds_kind == "half" {
                    WindowBounds::Windowed(bounds)
                } else {
                    WindowBounds::Fullscreen(bounds)
                }),
                window_background: background,
                kind,
                focus: true,
                show,
                ..Default::default()
            },
            |_, cx| cx.new(|_| Probe),
        )
        .unwrap();
        cx.activate(true);
    });
}
