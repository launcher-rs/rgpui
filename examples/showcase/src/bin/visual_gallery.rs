//! 图表 / 动效 / 二维码 示例集合。
//!
//! 用法：cargo run -p showcase --bin visual_gallery -- <demo>
//! 可用 demo：charts / effects / qr_code
//! 不带参数时默认运行 charts，传 --list 打印清单。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    AnyView, App, Bounds, Button, ButtonVariants, Context, IntoElement, ParentElement, Render,
    Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use rgpui_platform::application;

mod charts {
    //! 图表组件示例（`charts` feature 门控）。
    //!
    //! 展示柱状图、折线图、饼图/环形图的使用方法。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin charts --features charts
    //! ```

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::components::charts::{
        BarChart, BarChartData, LineChart, LineChartPoint, LineChartSeries, PieChart,
        PieChartSegment,
    };
    use rgpui::{
        App, AppContext as _, Bounds, Context, IntoElement, ParentElement, Render,
        ScrollableElement, Styled, Window, WindowBounds, WindowOptions, div, px, size,
    };
    use rgpui_platform::application;

    struct ChartsApp;

    impl Render for ChartsApp {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let bars = vec![
                BarChartData::new("一月", 30.0),
                BarChartData::new("二月", 55.0),
                BarChartData::new("三月", 42.0),
                BarChartData::new("四月", 78.0),
                BarChartData::new("五月", 63.0),
            ];

            let line_series = LineChartSeries::new(
                "访问量",
                vec![
                    LineChartPoint::new(0.0, 10.0),
                    LineChartPoint::new(1.0, 18.0),
                    LineChartPoint::new(2.0, 12.0),
                    LineChartPoint::new(3.0, 30.0),
                    LineChartPoint::new(4.0, 24.0),
                    LineChartPoint::new(5.0, 40.0),
                ],
            )
            .show_points(true);

            let pie_segments = vec![
                PieChartSegment::new("Chrome", 45.0),
                PieChartSegment::new("Safari", 25.0),
                PieChartSegment::new("Firefox", 18.0),
                PieChartSegment::new("其他", 12.0),
            ];

            div()
                .size_full()
                .overflow_y_scrollbar()
                .p(px(24.0))
                .flex()
                .flex_col()
                .gap_6()
                .child(div().text_2xl().child("图表组件示例"))
                .child(
                    div().text_lg().child("柱状图（BarChart）").child(
                        BarChart::new(bars)
                            .chart_height(px(260.0))
                            .show_values(true),
                    ),
                )
                .child(
                    div()
                        .text_lg()
                        .child("折线图（LineChart）")
                        .child(LineChart::new(vec![line_series]).show_grid(true)),
                )
                .child(
                    div().text_lg().child("饼图（PieChart）").child(
                        PieChart::pie(pie_segments)
                            .size_px(220)
                            .show_percentages(true),
                    ),
                )
                .child(
                    div().text_lg().child("环形图（PieChart donut）").child(
                        PieChart::donut(vec![
                            PieChartSegment::new("产品 A", 40.0),
                            PieChartSegment::new("产品 B", 35.0),
                            PieChartSegment::new("产品 C", 25.0),
                        ])
                        .size_px(200)
                        .center_label("总计")
                        .show_percentages(true),
                    ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(900.), px(800.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| ChartsApp),
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| ChartsApp).into()
    }
}

mod effects {
    //! 特效组件示例（`effects` feature 门控）。
    //!
    //! 展示极光背景、彩带、粒子发射器、涟漪、微光、跑马灯、脉冲指示器。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin effects --features effects
    //! ```

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::components::{
        Aurora, Confetti, ConfettiState, Marquee, ParticleEmitter, ParticleEmitterState,
        PulseIndicator, Ripple, Shimmer,
    };
    use rgpui::{
        App, AppContext as _, Bounds, Context, Entity, InteractiveElement, IntoElement,
        ParentElement, Render, StatefulInteractiveElement, Styled, Window, WindowBounds,
        WindowOptions, div, point, px, size,
    };
    use rgpui_platform::application;

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
                                            this.confetti_state
                                                .update(cx, |state, cx| state.burst(cx));
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
                                            div()
                                                .child("反向滚动：从左到右展示内容")
                                                .into_any_element()
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

    pub fn run_example() {
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

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| EffectsApp::new(window, cx)).into()
    }
}

mod qr_code {
    //! 二维码组件示例（`qr-code` feature 门控）。
    //!
    //! 展示二维码的生成与自定义样式。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin qr_code --features qr-code
    //! ```

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::components::QRCodeComponent;
    use rgpui::{
        App, AppContext as _, Bounds, Context, IntoElement, ParentElement, Render, Styled, Window,
        WindowBounds, WindowOptions, div, px, size,
    };
    use rgpui_platform::application;

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

    pub fn run_example() {
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

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| QrCodeApp).into()
    }
}

/// Gallery 根视图：左侧竖行 tab + 右侧示例内容。
struct GalleryApp {
    titles: Vec<&'static str>,
    /// 当前示例的介绍与用法。
    active: usize,
    views: Vec<AnyView>,
}

impl GalleryApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let titles: Vec<&'static str> = vec!["charts", "effects", "qr_code"];
        let builds: &[fn(&mut Window, &mut App) -> AnyView] =
            &[charts::build, effects::build, qr_code::build];
        let views = builds.iter().map(|build| build(window, cx)).collect();
        Self {
            titles,
            active: 0,
            views,
        }
    }
}

impl Render for GalleryApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let theme = cx.theme();
        div()
            .id("gallery-app")
            .size_full()
            .flex()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                div()
                    .id("gallery-tabs")
                    .h_full()
                    .w(px(180.0))
                    .flex_shrink_0()
                    .flex_col()
                    .overflow_y_scroll()
                    .border_r_1()
                    .border_color(theme.border)
                    .children(self.titles.iter().enumerate().map(|(ix, title)| {
                        let this = this.clone();
                        Button::new(("gallery-tab", ix))
                            .label(*title)
                            .w_full()
                            .when(ix == self.active, |b| b.primary())
                            .when(ix != self.active, |b| b.ghost())
                            .on_click(move |_, _, cx| {
                                this.update(cx, |gallery, cx| {
                                    gallery.active = ix;
                                    cx.notify();
                                });
                            })
                    })),
            )
            .child(
                div()
                    .id("gallery-content")
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .overflow_hidden()
                    .child(self.views[self.active].clone()),
            )
    }
}

fn run() {
    // 传 -- <demo> 时保留原有独立窗口行为；不带参数时打开 gallery 导航界面。
    if let Some(name) = std::env::args().nth(1) {
        match name.as_str() {
            "charts" => charts::run_example(),
            "effects" => effects::run_example(),
            "qr_code" => qr_code::run_example(),
            _ => eprintln!("未知 demo：{name}"),
        }
        return;
    }
    application().run(|cx: &mut App| {
        rgpui::theme::init(cx);
        rgpui::input_ui::init(cx);
        rgpui::menu::init(cx);
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| GalleryApp::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run();
}
