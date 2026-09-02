//! 图表组件示例（`charts` feature 门控）。
//!
//! 展示柱状图、折线图、饼图/环形图的使用方法。
//!
//! 运行：
//!
//! ```text
//! cargo run -p rgpui --example charts --features charts
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::components::charts::{
    BarChart, BarChartData, LineChart, LineChartPoint, LineChartSeries, PieChart, PieChartSegment,
};
use rgpui::{
    App, AppContext as _, Bounds, Context, IntoElement, ParentElement, Render, Styled, Window,
    WindowBounds, WindowOptions, div, px, size,
};
use rgpui_platform::application;

/// 图表示例根视图：纵向排列多个图表。
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

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(900.), px(1200.0)), cx);
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
