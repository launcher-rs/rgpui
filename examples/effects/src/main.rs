//! 特效组件示例（`effects` feature 门控）。
//!
//! 展示极光背景、彩带、粒子发射器、涟漪、微光、跑马灯、脉冲指示器。
//!
//! 运行：
//!
//! ```text
//! cargo run -p rgpui --example effects --features effects
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::components::{
    Aurora, Confetti, ConfettiState, Marquee, ParticleEmitter, ParticleEmitterState,
    PulseIndicator, Ripple, Shimmer,
};
use rgpui::{
    App, AppContext as _, Bounds, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Render, StatefulInteractiveElement, Styled, Window, WindowBounds, WindowOptions, div, point,
    px, size,
};
use rgpui_platform::application;

/// 特效示例根视图。
struct EffectsApp {
    /// 彩带状态实体。
    confetti_state: Entity<ConfettiState>,
    /// 粒子发射器状态实体。
    particle_state: Entity<ParticleEmitterState>,
}

impl EffectsApp {
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let confetti_state = cx.new(ConfettiState::new);
        let particle_state = cx.new(ParticleEmitterState::new);
        Self {
            confetti_state,
            particle_state,
        }
    }
}

impl Render for EffectsApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let confetti_state = self.confetti_state.clone();
        let particle_state = self.particle_state.clone();

        Aurora::new()
            .colors(vec![
                rgpui::hsla(0.55, 0.9, 0.6, 0.6),
                rgpui::hsla(0.8, 0.9, 0.6, 0.5),
                rgpui::hsla(0.1, 0.9, 0.7, 0.5),
            ])
            .child(
                div()
                    .size_full()
                    .p(px(24.0))
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(div().text_2xl().child("特效组件示例"))
                    .child(
                        div().child("彩带（Confetti，点击触发）").child(
                            div()
                                .id("confetti-trigger")
                                .w(px(200.0))
                                .h(px(60.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(8.0))
                                .bg(rgpui::hsla(0.0, 0.0, 0.0, 0.25))
                                .text_color(rgpui::white())
                                .cursor_pointer()
                                .on_click(cx.listener(
                                    move |this: &mut EffectsApp,
                                          _: &rgpui::ClickEvent,
                                          _window,
                                          cx| {
                                        this.confetti_state.update(cx, |state, cx| state.burst(cx));
                                    },
                                ))
                                .child("点击放彩带"),
                        ),
                    )
                    .child(
                        Confetti::new("confetti", confetti_state)
                            .particle_count(80, cx)
                            .gravity(0.6, cx),
                    )
                    .child(
                        div().child("粒子发射器（ParticleEmitter）").child(
                            ParticleEmitter::new("particles", particle_state)
                                .spawn_rate(20.0, cx)
                                .lifetime(std::time::Duration::from_secs(2), cx),
                        ),
                    )
                    .child(
                        div().child("微光（Shimmer）").child(
                            div()
                                .w(px(260.0))
                                .h(px(48.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(8.0))
                                .bg(rgpui::hsla(0.0, 0.0, 0.0, 0.2))
                                .child(Shimmer::new().child("加载中…")),
                        ),
                    )
                    .child(
                        div().child("跑马灯（Marquee）").child(
                            div()
                                .w(px(400.0))
                                .child(Marquee::new("marquee", || {
                                    div()
                                        .child("这是一条跑马灯文本，用于展示滚动横幅效果")
                                        .into_any_element()
                                }))
                                .child(div().h(px(8.0)))
                                .child(
                                    Marquee::new("marquee-reverse", || {
                                        div().child("反向滚动：从左到右展示内容").into_any_element()
                                    })
                                    .direction(rgpui::components::MarqueeDirection::Right),
                                ),
                        ),
                    )
                    .child(
                        div().child("脉冲指示器（PulseIndicator）").child(
                            div()
                                .flex()
                                .gap_4()
                                .child(PulseIndicator::new("pulse-1"))
                                .child(
                                    PulseIndicator::new("pulse-2")
                                        .color(rgpui::red())
                                        .size(px(24.0)),
                                ),
                        ),
                    )
                    .child(div().child("涟漪（Ripple）").child(Ripple::new(
                        "ripple",
                        point(px(20.0), px(20.0)),
                        rgpui::white(),
                    ))),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(900.), px(1200.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| EffectsApp::new(window, cx)),
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
