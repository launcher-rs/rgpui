//! 柱状图组件（单系列/多系列、分组/堆叠、横向/纵向）。

use crate::{prelude::FluentBuilder as _, *};

/// 默认系列配色。
const CHART_COLORS: [u32; 8] = [
    0x3b82f6, 0x22c55e, 0xf59e0b, 0xef4444, 0x8b5cf6, 0x06b6d4, 0xf97316, 0xec4899,
];

/// 获取默认系列颜色。
fn get_chart_color(index: usize) -> Hsla {
    rgb(CHART_COLORS[index % CHART_COLORS.len()]).into()
}

/// 像素转 f32。
fn pixels_to_f32(p: Pixels) -> f32 {
    p / px(1.0)
}

/// 柱状图数据项。
#[derive(Clone)]
pub struct BarChartData {
    /// 标签。
    pub label: SharedString,
    /// 数值。
    pub value: f64,
    /// 颜色（None 为自动分配）。
    pub color: Option<Hsla>,
}

impl BarChartData {
    /// 创建数据项。
    pub fn new(label: impl Into<SharedString>, value: f64) -> Self {
        Self {
            label: label.into(),
            value,
            color: None,
        }
    }

    /// 设置颜色。
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

/// 多系列柱状图系列。
#[derive(Clone)]
pub struct BarChartSeries {
    /// 系列名称。
    pub name: SharedString,
    /// 系列数据（与标签一一对应）。
    pub data: Vec<f64>,
    /// 颜色（None 为自动分配）。
    pub color: Option<Hsla>,
}

impl BarChartSeries {
    /// 创建系列。
    pub fn new(name: impl Into<SharedString>, data: Vec<f64>) -> Self {
        Self {
            name: name.into(),
            data,
            color: None,
        }
    }

    /// 设置颜色。
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

/// 柱状图方向。
#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub enum BarChartOrientation {
    /// 纵向（柱子向上）。
    #[default]
    Vertical,
    /// 横向（柱子向右）。
    Horizontal,
}

/// 柱状图模式。
#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub enum BarChartMode {
    /// 单系列。
    #[default]
    Single,
    /// 分组多系列。
    Grouped,
    /// 堆叠多系列。
    Stacked,
}

/// 柱状图组件。
#[derive(IntoElement)]
pub struct BarChart {
    data: Vec<BarChartData>,
    series: Vec<BarChartSeries>,
    labels: Vec<SharedString>,
    orientation: BarChartOrientation,
    mode: BarChartMode,
    show_values: bool,
    show_grid: bool,
    show_legend: bool,
    show_axis_labels: bool,
    bar_width: Option<Pixels>,
    gap: Pixels,
    height: Pixels,
    style: StyleRefinement,
}

impl BarChart {
    /// 创建单系列柱状图。
    pub fn new(data: Vec<BarChartData>) -> Self {
        Self {
            data,
            series: Vec::new(),
            labels: Vec::new(),
            orientation: BarChartOrientation::default(),
            mode: BarChartMode::Single,
            show_values: false,
            show_grid: false,
            show_legend: false,
            show_axis_labels: true,
            bar_width: None,
            gap: px(8.0),
            height: px(300.0),
            style: StyleRefinement::default(),
        }
    }

    /// 创建多系列柱状图。
    pub fn multi_series(labels: Vec<impl Into<SharedString>>, series: Vec<BarChartSeries>) -> Self {
        Self {
            data: Vec::new(),
            series,
            labels: labels.into_iter().map(|l| l.into()).collect(),
            orientation: BarChartOrientation::default(),
            mode: BarChartMode::Grouped,
            show_values: false,
            show_grid: false,
            show_legend: true,
            show_axis_labels: true,
            bar_width: None,
            gap: px(8.0),
            height: px(300.0),
            style: StyleRefinement::default(),
        }
    }

    /// 设置纵向方向。
    pub fn vertical(mut self) -> Self {
        self.orientation = BarChartOrientation::Vertical;
        self
    }

    /// 设置横向方向。
    pub fn horizontal(mut self) -> Self {
        self.orientation = BarChartOrientation::Horizontal;
        self
    }

    /// 设置方向。
    pub fn orientation(mut self, orientation: BarChartOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// 设置堆叠模式。
    pub fn stacked(mut self) -> Self {
        self.mode = BarChartMode::Stacked;
        self
    }

    /// 设置分组模式。
    pub fn grouped(mut self) -> Self {
        self.mode = BarChartMode::Grouped;
        self
    }

    /// 设置模式。
    pub fn mode(mut self, mode: BarChartMode) -> Self {
        self.mode = mode;
        self
    }

    /// 设置是否显示数值。
    pub fn show_values(mut self, show: bool) -> Self {
        self.show_values = show;
        self
    }

    /// 设置是否显示网格线。
    pub fn show_grid(mut self, show: bool) -> Self {
        self.show_grid = show;
        self
    }

    /// 设置是否显示图例。
    pub fn show_legend(mut self, show: bool) -> Self {
        self.show_legend = show;
        self
    }

    /// 设置是否显示轴标签。
    pub fn show_axis_labels(mut self, show: bool) -> Self {
        self.show_axis_labels = show;
        self
    }

    /// 设置柱子宽度/高度。
    pub fn bar_width(mut self, width: Pixels) -> Self {
        self.bar_width = Some(width);
        self
    }

    /// 设置柱间距。
    pub fn gap(mut self, gap: Pixels) -> Self {
        self.gap = gap;
        self
    }

    /// 设置图表高度。
    pub fn chart_height(mut self, height: Pixels) -> Self {
        self.height = height;
        self
    }

    /// 渲染单系列纵向柱状图。
    fn render_single_vertical(self, theme: &Theme) -> Div {
        let max_value = self
            .data
            .iter()
            .map(|d| d.value)
            .fold(0.0_f64, |a, b| a.max(b));

        let chart_height = self.height;
        let bar_width = self.bar_width.unwrap_or(px(40.0));
        let gap = self.gap;
        let show_values = self.show_values;
        let show_grid = self.show_grid;
        let show_axis_labels = self.show_axis_labels;

        let grid_lines = if show_grid {
            Some(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .children((0..5).map(|_| {
                        div()
                            .w_full()
                            .h(px(1.0))
                            .bg(theme.tokens.border.opacity(0.3))
                    })),
            )
        } else {
            None
        };

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .relative()
                    .h(chart_height)
                    .w_full()
                    .when_some(grid_lines, |this, grid| this.child(grid))
                    .child(
                        div()
                            .h_full()
                            .w_full()
                            .flex()
                            .items_end()
                            .justify_around()
                            .gap(gap)
                            .px(px(16.0))
                            .children(self.data.iter().enumerate().map(|(i, item)| {
                                let height_percent = if max_value > 0.0 {
                                    (item.value / max_value) as f32
                                } else {
                                    0.0
                                };
                                let bar_color = item.color.unwrap_or_else(|| get_chart_color(i));
                                let value = item.value;
                                let bar_height = chart_height * height_percent;

                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .justify_end()
                                    .h_full()
                                    .gap(px(4.0))
                                    .when(show_values, |this| {
                                        this.child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.tokens.muted_foreground)
                                                .child(format!("{:.0}", value)),
                                        )
                                    })
                                    .child(
                                        div()
                                            .w(bar_width)
                                            .h(bar_height)
                                            .bg(bar_color)
                                            .rounded_t(theme.radius),
                                    )
                            })),
                    ),
            )
            .when(show_axis_labels, |this| {
                this.child(div().flex().justify_around().px(px(16.0)).children(
                    self.data.iter().map(|item| {
                        div()
                            .text_xs()
                            .text_color(theme.tokens.muted_foreground)
                            .text_center()
                            .child(item.label.clone())
                    }),
                ))
            })
    }

    /// 渲染单系列横向柱状图。
    fn render_single_horizontal(self, theme: &Theme) -> Div {
        let max_value = self
            .data
            .iter()
            .map(|d| d.value)
            .fold(0.0_f64, |a, b| a.max(b));

        let bar_height = self.bar_width.unwrap_or(px(24.0));
        let gap = self.gap;
        let show_values = self.show_values;
        let show_grid = self.show_grid;
        let show_axis_labels = self.show_axis_labels;
        let grid_color = theme.tokens.border.opacity(0.3);

        div()
            .flex()
            .flex_col()
            .gap(gap)
            .children(self.data.iter().enumerate().map(|(i, item)| {
                let width_percent = if max_value > 0.0 {
                    (item.value / max_value) as f32
                } else {
                    0.0
                };
                let bar_color = item.color.unwrap_or_else(|| get_chart_color(i));
                let value = item.value;
                let label = item.label.clone();

                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .when(show_axis_labels, |this| {
                        this.child(
                            div()
                                .w(px(80.0))
                                .text_xs()
                                .text_color(theme.tokens.muted_foreground)
                                .text_align(TextAlign::Right)
                                .overflow_hidden()
                                .child(label),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .relative()
                            .h(bar_height)
                            .when(show_grid, |this| {
                                this.child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .flex()
                                        .justify_between()
                                        .children(
                                            (0..5)
                                                .map(|_| div().h_full().w(px(1.0)).bg(grid_color)),
                                        ),
                                )
                            })
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(width_percent))
                                    .bg(bar_color)
                                    .rounded_r(theme.radius),
                            ),
                    )
                    .when(show_values, |this| {
                        this.child(
                            div()
                                .w(px(50.0))
                                .text_xs()
                                .text_color(theme.tokens.muted_foreground)
                                .child(format!("{:.0}", value)),
                        )
                    })
            }))
    }

    /// 渲染多系列纵向分组柱状图。
    fn render_multi_vertical_grouped(self, theme: &Theme) -> Div {
        let max_value = self
            .series
            .iter()
            .flat_map(|s| s.data.iter())
            .fold(0.0_f64, |a, &b| a.max(b));

        let chart_height = self.height;
        let series_count = self.series.len();
        let bar_width = self
            .bar_width
            .unwrap_or(px(24.0 / series_count.max(1) as f32 * series_count as f32));
        let single_bar_width = px(pixels_to_f32(bar_width) / series_count.max(1) as f32);
        let gap = self.gap;
        let show_values = self.show_values;
        let show_grid = self.show_grid;
        let show_axis_labels = self.show_axis_labels;
        let show_legend = self.show_legend;

        let grid_lines = if show_grid {
            Some(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .children((0..5).map(|_| {
                        div()
                            .w_full()
                            .h(px(1.0))
                            .bg(theme.tokens.border.opacity(0.3))
                    })),
            )
        } else {
            None
        };

        let labels = self.labels.clone();
        let series_for_legend = self.series.clone();
        let label_count = labels.len();

        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .relative()
                    .h(chart_height)
                    .w_full()
                    .when_some(grid_lines, |this, grid| this.child(grid))
                    .child(
                        div()
                            .h_full()
                            .w_full()
                            .flex()
                            .items_end()
                            .justify_around()
                            .gap(gap)
                            .px(px(16.0))
                            .children((0..label_count).map(|label_idx| {
                                div().flex().items_end().h_full().gap(px(2.0)).children(
                                    self.series.iter().enumerate().map(|(series_idx, series)| {
                                        let value =
                                            series.data.get(label_idx).copied().unwrap_or(0.0);
                                        let height_percent = if max_value > 0.0 {
                                            (value / max_value) as f32
                                        } else {
                                            0.0
                                        };
                                        let bar_color = series
                                            .color
                                            .unwrap_or_else(|| get_chart_color(series_idx));
                                        let bar_height = chart_height * height_percent;

                                        div()
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .justify_end()
                                            .h_full()
                                            .gap(px(2.0))
                                            .when(show_values, |this| {
                                                this.child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme.tokens.muted_foreground)
                                                        .child(format!("{:.0}", value)),
                                                )
                                            })
                                            .child(
                                                div()
                                                    .w(single_bar_width)
                                                    .h(bar_height)
                                                    .bg(bar_color)
                                                    .rounded_t(theme.radius),
                                            )
                                    }),
                                )
                            })),
                    ),
            )
            .when(show_axis_labels, |this| {
                this.child(
                    div()
                        .flex()
                        .justify_around()
                        .px(px(16.0))
                        .children(labels.iter().map(|label| {
                            div()
                                .text_xs()
                                .text_color(theme.tokens.muted_foreground)
                                .text_center()
                                .child(label.clone())
                        })),
                )
            })
            .when(show_legend, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(16.0))
                        .justify_center()
                        .children(series_for_legend.iter().enumerate().map(|(i, s)| {
                            let color = s.color.unwrap_or_else(|| get_chart_color(i));
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(div().size(px(12.0)).rounded(px(2.0)).bg(color))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.tokens.muted_foreground)
                                        .child(s.name.clone()),
                                )
                        })),
                )
            })
    }

    /// 渲染多系列纵向堆叠柱状图。
    fn render_multi_vertical_stacked(self, theme: &Theme) -> Div {
        let label_count = self.labels.len();
        let stacked_totals: Vec<f64> = (0..label_count)
            .map(|i| {
                self.series
                    .iter()
                    .map(|s| s.data.get(i).copied().unwrap_or(0.0))
                    .sum()
            })
            .collect();

        let max_total = stacked_totals.iter().fold(0.0_f64, |a, &b| a.max(b));

        let chart_height = self.height;
        let bar_width = self.bar_width.unwrap_or(px(40.0));
        let gap = self.gap;
        let show_values = self.show_values;
        let show_grid = self.show_grid;
        let show_axis_labels = self.show_axis_labels;
        let show_legend = self.show_legend;

        let grid_lines = if show_grid {
            Some(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .children((0..5).map(|_| {
                        div()
                            .w_full()
                            .h(px(1.0))
                            .bg(theme.tokens.border.opacity(0.3))
                    })),
            )
        } else {
            None
        };

        let labels = self.labels.clone();
        let series_for_legend = self.series.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .relative()
                    .h(chart_height)
                    .w_full()
                    .when_some(grid_lines, |this, grid| this.child(grid))
                    .child(
                        div()
                            .h_full()
                            .w_full()
                            .flex()
                            .items_end()
                            .justify_around()
                            .gap(gap)
                            .px(px(16.0))
                            .children((0..label_count).map(|label_idx| {
                                let total_height_percent = if max_total > 0.0 {
                                    (stacked_totals[label_idx] / max_total) as f32
                                } else {
                                    0.0
                                };
                                let total_value = stacked_totals[label_idx];
                                let bar_height = chart_height * total_height_percent;

                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .justify_end()
                                    .h_full()
                                    .gap(px(4.0))
                                    .when(show_values, |this| {
                                        this.child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.tokens.muted_foreground)
                                                .child(format!("{:.0}", total_value)),
                                        )
                                    })
                                    .child(
                                        div()
                                            .w(bar_width)
                                            .h(bar_height)
                                            .flex()
                                            .flex_col_reverse()
                                            .overflow_hidden()
                                            .rounded_t(theme.radius)
                                            .children(self.series.iter().enumerate().map(
                                                |(series_idx, series)| {
                                                    let value = series
                                                        .data
                                                        .get(label_idx)
                                                        .copied()
                                                        .unwrap_or(0.0);
                                                    let segment_percent =
                                                        if stacked_totals[label_idx] > 0.0 {
                                                            (value / stacked_totals[label_idx])
                                                                as f32
                                                        } else {
                                                            0.0
                                                        };
                                                    let bar_color =
                                                        series.color.unwrap_or_else(|| {
                                                            get_chart_color(series_idx)
                                                        });

                                                    div()
                                                        .w_full()
                                                        .h(relative(segment_percent))
                                                        .bg(bar_color)
                                                },
                                            )),
                                    )
                            })),
                    ),
            )
            .when(show_axis_labels, |this| {
                this.child(
                    div()
                        .flex()
                        .justify_around()
                        .px(px(16.0))
                        .children(labels.iter().map(|label| {
                            div()
                                .text_xs()
                                .text_color(theme.tokens.muted_foreground)
                                .text_center()
                                .child(label.clone())
                        })),
                )
            })
            .when(show_legend, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(16.0))
                        .justify_center()
                        .children(series_for_legend.iter().enumerate().map(|(i, s)| {
                            let color = s.color.unwrap_or_else(|| get_chart_color(i));
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(div().size(px(12.0)).rounded(px(2.0)).bg(color))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.tokens.muted_foreground)
                                        .child(s.name.clone()),
                                )
                        })),
                )
            })
    }

    /// 渲染多系列横向分组柱状图。
    fn render_multi_horizontal_grouped(self, theme: &Theme) -> Div {
        let max_value = self
            .series
            .iter()
            .flat_map(|s| s.data.iter())
            .fold(0.0_f64, |a, &b| a.max(b));

        let series_count = self.series.len();
        let bar_height = self.bar_width.unwrap_or(px(16.0));
        let single_bar_height = px(pixels_to_f32(bar_height) / series_count.max(1) as f32);
        let gap = self.gap;
        let show_values = self.show_values;
        let show_grid = self.show_grid;
        let show_axis_labels = self.show_axis_labels;
        let show_legend = self.show_legend;
        let grid_color = theme.tokens.border.opacity(0.3);

        let labels = self.labels.clone();
        let series_for_legend = self.series.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(gap)
                    .children(labels.iter().enumerate().map(|(label_idx, label)| {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .when(show_axis_labels, |this| {
                                this.child(
                                    div()
                                        .w(px(80.0))
                                        .text_xs()
                                        .text_color(theme.tokens.muted_foreground)
                                        .text_align(TextAlign::Right)
                                        .overflow_hidden()
                                        .child(label.clone()),
                                )
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .relative()
                                    .when(show_grid, |this| {
                                        this.child(
                                            div()
                                                .absolute()
                                                .inset_0()
                                                .flex()
                                                .justify_between()
                                                .children((0..5).map(|_| {
                                                    div().h_full().w(px(1.0)).bg(grid_color)
                                                })),
                                        )
                                    })
                                    .children(self.series.iter().enumerate().map(
                                        |(series_idx, series)| {
                                            let value =
                                                series.data.get(label_idx).copied().unwrap_or(0.0);
                                            let width_percent = if max_value > 0.0 {
                                                (value / max_value) as f32
                                            } else {
                                                0.0
                                            };
                                            let bar_color = series
                                                .color
                                                .unwrap_or_else(|| get_chart_color(series_idx));

                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(4.0))
                                                .child(
                                                    div()
                                                        .h(single_bar_height)
                                                        .w(relative(width_percent))
                                                        .bg(bar_color)
                                                        .rounded_r(theme.radius),
                                                )
                                                .when(show_values, |this| {
                                                    this.child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(
                                                                theme.tokens.muted_foreground,
                                                            )
                                                            .child(format!("{:.0}", value)),
                                                    )
                                                })
                                        },
                                    )),
                            )
                    })),
            )
            .when(show_legend, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(16.0))
                        .justify_center()
                        .children(series_for_legend.iter().enumerate().map(|(i, s)| {
                            let color = s.color.unwrap_or_else(|| get_chart_color(i));
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(div().size(px(12.0)).rounded(px(2.0)).bg(color))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.tokens.muted_foreground)
                                        .child(s.name.clone()),
                                )
                        })),
                )
            })
    }

    /// 渲染多系列横向堆叠柱状图。
    fn render_multi_horizontal_stacked(self, theme: &Theme) -> Div {
        let label_count = self.labels.len();
        let stacked_totals: Vec<f64> = (0..label_count)
            .map(|i| {
                self.series
                    .iter()
                    .map(|s| s.data.get(i).copied().unwrap_or(0.0))
                    .sum()
            })
            .collect();

        let max_total = stacked_totals.iter().fold(0.0_f64, |a, &b| a.max(b));

        let bar_height = self.bar_width.unwrap_or(px(24.0));
        let gap = self.gap;
        let show_values = self.show_values;
        let show_grid = self.show_grid;
        let show_axis_labels = self.show_axis_labels;
        let show_legend = self.show_legend;
        let grid_color = theme.tokens.border.opacity(0.3);

        let labels = self.labels.clone();
        let series_for_legend = self.series.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(gap)
                    .children(labels.iter().enumerate().map(|(label_idx, label)| {
                        let total_width_percent = if max_total > 0.0 {
                            (stacked_totals[label_idx] / max_total) as f32
                        } else {
                            0.0
                        };
                        let total_value = stacked_totals[label_idx];

                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .when(show_axis_labels, |this| {
                                this.child(
                                    div()
                                        .w(px(80.0))
                                        .text_xs()
                                        .text_color(theme.tokens.muted_foreground)
                                        .text_align(TextAlign::Right)
                                        .overflow_hidden()
                                        .child(label.clone()),
                                )
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .relative()
                                    .h(bar_height)
                                    .when(show_grid, |this| {
                                        this.child(
                                            div()
                                                .absolute()
                                                .inset_0()
                                                .flex()
                                                .justify_between()
                                                .children((0..5).map(|_| {
                                                    div().h_full().w(px(1.0)).bg(grid_color)
                                                })),
                                        )
                                    })
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative(total_width_percent))
                                            .flex()
                                            .overflow_hidden()
                                            .rounded_r(theme.radius)
                                            .children(self.series.iter().enumerate().map(
                                                |(series_idx, series)| {
                                                    let value = series
                                                        .data
                                                        .get(label_idx)
                                                        .copied()
                                                        .unwrap_or(0.0);
                                                    let segment_percent =
                                                        if stacked_totals[label_idx] > 0.0 {
                                                            (value / stacked_totals[label_idx])
                                                                as f32
                                                        } else {
                                                            0.0
                                                        };
                                                    let bar_color =
                                                        series.color.unwrap_or_else(|| {
                                                            get_chart_color(series_idx)
                                                        });

                                                    div()
                                                        .h_full()
                                                        .w(relative(segment_percent))
                                                        .bg(bar_color)
                                                },
                                            )),
                                    ),
                            )
                            .when(show_values, |this| {
                                this.child(
                                    div()
                                        .w(px(50.0))
                                        .text_xs()
                                        .text_color(theme.tokens.muted_foreground)
                                        .child(format!("{:.0}", total_value)),
                                )
                            })
                    })),
            )
            .when(show_legend, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(16.0))
                        .justify_center()
                        .children(series_for_legend.iter().enumerate().map(|(i, s)| {
                            let color = s.color.unwrap_or_else(|| get_chart_color(i));
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(div().size(px(12.0)).rounded(px(2.0)).bg(color))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.tokens.muted_foreground)
                                        .child(s.name.clone()),
                                )
                        })),
                )
            })
    }
}

impl Styled for BarChart {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for BarChart {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style.clone();

        let is_single = self.series.is_empty();
        let is_vertical = self.orientation == BarChartOrientation::Vertical;
        let is_stacked = self.mode == BarChartMode::Stacked;

        let content = match (is_single, is_vertical, is_stacked) {
            (true, true, _) => self.render_single_vertical(&theme),
            (true, false, _) => self.render_single_horizontal(&theme),
            (false, true, false) => self.render_multi_vertical_grouped(&theme),
            (false, true, true) => self.render_multi_vertical_stacked(&theme),
            (false, false, false) => self.render_multi_horizontal_grouped(&theme),
            (false, false, true) => self.render_multi_horizontal_stacked(&theme),
        };

        content.map(|this| {
            let mut div = this;
            div.style().refine(&user_style);
            div
        })
    }
}
