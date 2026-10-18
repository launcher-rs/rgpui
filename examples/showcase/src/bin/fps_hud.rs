//! FPS 监控 HUD 示例。
//!
//! `FpsHud` 是一个 `Render` 视图，把它作为根视图的子节点插入即可：它默认持续采样
//! 每帧间隔、经指数移动平均平滑后显示实时帧率（并请求下一帧保证 HUD 实时跳动）。
//! CPU / 内存两行默认关闭，需要时由调用方经 `FpsHudState::update_cpu/update_memory`
//! 喂入后再用 `FpsHud::show_cpu/show_memory` 打开。
//!
//! 运行：
//!
//! ```text
//! cargo run -p showcase --bin fps_hud
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, Entity, IntoElement, ParentElement, Render, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size, v_flex,
};
use rgpui_platform::application;

/// 示例根视图：只负责把 HUD 挂到右上角。
struct FpsDemo {
    /// HUD 视图实体。
    hud: Entity<FpsHud>,
}

impl FpsDemo {
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 状态实体（采样数据）。
        let state = cx.new(|_cx| FpsHudState::new());
        // HUD 视图：显示 CPU 行（默认会持续采样）。
        let hud = cx.new(|_cx| FpsHud::new(state).show_cpu(true).continuous(true));
        Self { hud }
    }
}

impl Render for FpsDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("fps-demo")
            // `.relative()` 让 HUD 的 `.absolute()` 相对本视图定位。
            .relative()
            .size_full()
            .bg(rgb(0x101418))
            .text_color(rgb(0xffffff))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .p(px(24.0))
                    .child(div().text_2xl().child("FPS 监控 HUD"))
                    .child(div().text_sm().child("右上角浮层实时显示帧率与帧时间。"))
                    .child(div().text_sm().child(
                        "FpsHud::new(state).show_cpu(true).show_memory(true).continuous(true)",
                    )),
            )
            // 把 HUD 视图作为子节点插入。
            .child(self.hud.clone())
    }
}

/// 启动示例。
fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(640.0), px(420.0)), cx);
        let _ = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| FpsDemo::new(window, cx)),
        );
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
