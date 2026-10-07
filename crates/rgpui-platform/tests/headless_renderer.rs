//! Linux 离屏渲染器的端到端验证：不建窗口，走 `TestPlatform` + `HeadlessAppContext` 出图。
//!
//! 这条链路覆盖的是「核心把场景交给平台渲染器 → GPU 画完 → 像素回读」整段，
//! 任何一环（feature 转发、无 surface 设备、图集、回读）断了这里就会红。
//! 没有可用 GPU 驱动的环境（CI 容器常见）直接跳过：渲染器工厂返回 `None` 时
//! 核心会退回到空图集，测不出任何东西，硬失败只会变成环境噪声。
#![cfg(all(
    any(target_os = "linux", target_os = "freebsd"),
    feature = "test-support"
))]

use rgpui::{
    AnyWindowHandle, AppContext, Context, HeadlessAppContext, IntoElement, Render, Window, div,
    prelude::*, px, rgb, size,
};

/// 白底上一块 100×100 的红方块
struct RedBlock;

impl Render for RedBlock {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0xffffff)).child(
            div()
                .absolute()
                .left(px(40.0))
                .top(px(40.0))
                .size(px(100.0))
                .bg(rgb(0xff0000)),
        )
    }
}

#[test]
fn headless_renderer_captures_painted_pixels() {
    let text_system = rgpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(
        text_system,
        std::sync::Arc::new(()),
        rgpui_platform::current_headless_renderer,
    );
    let handle: AnyWindowHandle = cx
        .open_window(size(px(280.0), px(180.0)), |_window, app| {
            app.new(|_cx| RedBlock)
        })
        .expect("离屏窗口应能打开")
        .into();

    cx.update_window(handle, |_view, window, app| {
        let _ = window.draw(app);
    })
    .expect("窗口应可绘制");

    let image = match cx.capture_screenshot(handle) {
        Ok(image) => image,
        Err(error) => {
            // 没有可用 GPU 驱动时工厂返回 None，这条链路根本建立不起来，不算回归
            if rgpui_platform::current_headless_renderer().is_none() {
                eprintln!("跳过：本机没有可用的离屏渲染器（{error:#}）");
                return;
            }
            panic!("离屏截图失败: {error:#}");
        }
    };

    let mut red = 0usize;
    let mut white = 0usize;
    for pixel in image.pixels() {
        let [r, g, b, _a] = pixel.0;
        if r > 200 && g < 60 && b < 60 {
            red += 1;
        } else if r > 200 && g > 200 && b > 200 {
            white += 1;
        }
    }
    // 100×100 的方块在最坏情况下（缩放 1.0）也是 1 万像素，背景远多于它
    assert!(red > 5_000, "红色方块没有画出来，red={red} white={white}");
    assert!(white > red, "背景白色应多于方块，red={red} white={white}");
}
