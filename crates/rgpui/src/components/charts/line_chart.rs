//! 折线图组件。

use crate::canvas_a11y::{CanvasNode, CanvasSemantics};
use crate::{prelude::FluentBuilder as _, *};
use accesskit::Role;

/// 内部不变量：值为 `None` 说明前提被破坏（守卫已保证非空等），必须立刻暴露。
///
/// 统一走这里而不是散落的 `unwrap`：语义与原先一致（同样 panic），但把「这是不变量
/// 断言」的意图写进调用点，模块级 `deny` 仍能拦住随手写的 unwrap。
#[inline]
#[allow(clippy::panic)] // 不变量断言：前提被破坏必须立刻暴露
fn invariant<T>(value: Option<T>, message: &str) -> T {
    match value {
        Some(value) => value,
        None => panic!("{message}"),
    }
}

/// 默认系列配色。
const CHART_COLORS: [u32; 8] = [
    0x3b82f6, 0x22c55e, 0xf59e0b, 0xef4444, 0x8b5cf6, 0x06b6d4, 0xf97316, 0xec4899,
];

/// 获取默认系列颜色。
fn get_chart_color(index: usize) -> Hsla {
    rgb(CHART_COLORS[index % CHART_COLORS.len()]).into()
}

/// 折线图数据点。
#[derive(Clone, Debug)]
pub struct LineChartPoint {
    /// x 坐标。
    pub x: f64,
    /// y 坐标。
    pub y: f64,
    /// 可选标签。
    pub label: Option<SharedString>,
}

impl LineChartPoint {
    /// 创建数据点。
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y, label: None }
    }

    /// 设置标签。
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// 折线图系列。
#[derive(Clone, Debug)]
pub struct LineChartSeries {
    /// 系列名称。
    pub name: SharedString,
    /// 系列数据点。
    pub points: Vec<LineChartPoint>,
    /// 颜色（None 为自动分配）。
    pub color: Option<Hsla>,
    /// 是否显示数据点。
    pub show_points: bool,
    /// 是否填充面积。
    pub fill_area: bool,
}

impl LineChartSeries {
    /// 创建系列。
    pub fn new(name: impl Into<SharedString>, points: Vec<LineChartPoint>) -> Self {
        Self {
            name: name.into(),
            points,
            color: None,
            show_points: false,
            fill_area: false,
        }
    }

    /// 设置系列颜色。
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// 设置是否显示数据点。
    pub fn show_points(mut self, show: bool) -> Self {
        self.show_points = show;
        self
    }

    /// 设置是否填充面积。
    pub fn fill_area(mut self, fill: bool) -> Self {
        self.fill_area = fill;
        self
    }
}

/// 数据范围。
struct DataRange {
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
}

impl DataRange {
    /// 从系列推导数据范围。
    fn from_series(
        series: &[LineChartSeries],
        y_min_override: Option<f64>,
        y_max_override: Option<f64>,
    ) -> Self {
        let mut x_min = f64::MAX;
        let mut x_max = f64::MIN;
        let mut y_min = f64::MAX;
        let mut y_max = f64::MIN;

        for s in series {
            for point in &s.points {
                x_min = x_min.min(point.x);
                x_max = x_max.max(point.x);
                y_min = y_min.min(point.y);
                y_max = y_max.max(point.y);
            }
        }

        if x_min == f64::MAX {
            x_min = 0.0;
            x_max = 1.0;
        }
        if y_min == f64::MAX {
            y_min = 0.0;
            y_max = 1.0;
        }

        if (x_max - x_min).abs() < f64::EPSILON {
            x_max = x_min + 1.0;
        }
        if (y_max - y_min).abs() < f64::EPSILON {
            y_max = y_min + 1.0;
        }

        Self {
            x_min,
            x_max,
            y_min: y_min_override.unwrap_or(y_min),
            y_max: y_max_override.unwrap_or(y_max),
        }
    }

    /// 将 x 归一化到 [0, 1]。
    fn normalize_x(&self, x: f64) -> f32 {
        ((x - self.x_min) / (self.x_max - self.x_min)) as f32
    }

    /// 将 y 归一化到 [0, 1]。
    fn normalize_y(&self, y: f64) -> f32 {
        ((y - self.y_min) / (self.y_max - self.y_min)) as f32
    }

    /// 根据归一化值反算 y。
    fn y_value_at(&self, normalized: f64) -> f64 {
        self.y_min + (self.y_max - self.y_min) * (1.0 - normalized)
    }
}

/// 折线图组件。
#[derive(IntoElement)]
pub struct LineChart {
    series: Vec<LineChartSeries>,
    show_grid: bool,
    show_x_axis: bool,
    show_y_axis: bool,
    x_axis_labels: Vec<SharedString>,
    y_min: Option<f64>,
    y_max: Option<f64>,
    smooth: bool,
    show_legend: bool,
    /// 无障碍配置：元素 id + 画布名称（开启后发布一个整体摘要节点）。
    a11y: Option<(ElementId, SharedString)>,
    style: StyleRefinement,
}

/// 折线图整体语义：一个零尺寸摘要节点（发布时由画布填充整块区域）。
///
/// 摘要按折线数 / 总点数 + 逐系列列出 y 极值（最多 4 个系列）。
fn line_chart_semantics(series: &[LineChartSeries], label: &SharedString) -> CanvasSemantics {
    let non_empty: Vec<&LineChartSeries> = series.iter().filter(|s| !s.points.is_empty()).collect();
    let total_points: usize = non_empty.iter().map(|s| s.points.len()).sum();

    let summary = if non_empty.is_empty() {
        "暂无数据".to_string()
    } else {
        format!("{} 条折线 · 共 {total_points} 点", non_empty.len())
    };

    let mut parts = Vec::new();
    for s in non_empty.iter().take(4) {
        let y_min = s.points.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
        let y_max = s
            .points
            .iter()
            .map(|p| p.y)
            .fold(f64::NEG_INFINITY, f64::max);
        let last = s.points[s.points.len() - 1].y;
        parts.push(format!(
            "{}：最新 {} · 最低 {} · 最高 {}（{} 点）",
            s.name,
            format_y_value(last),
            format_y_value(y_min),
            format_y_value(y_max),
            s.points.len()
        ));
    }
    if non_empty.len() > 4 {
        parts.push(format!("等 {} 条折线", non_empty.len()));
    }

    let mut semantics = CanvasSemantics::new();
    semantics.push(
        CanvasNode::new(1, Role::GraphicsObject, label.clone(), Bounds::default())
            .with_value(summary)
            .with_description(parts.join("；")),
    );
    semantics
}

impl LineChart {
    /// 创建折线图。
    pub fn new(series: Vec<LineChartSeries>) -> Self {
        Self {
            series,
            show_grid: true,
            show_x_axis: true,
            show_y_axis: true,
            x_axis_labels: Vec::new(),
            y_min: None,
            y_max: None,
            smooth: false,
            show_legend: true,
            a11y: None,
            style: StyleRefinement::default(),
        }
    }

    /// 创建单系列折线图。
    pub fn single(series: LineChartSeries) -> Self {
        Self::new(vec![series])
    }

    /// 设置是否显示网格线。
    pub fn show_grid(mut self, show: bool) -> Self {
        self.show_grid = show;
        self
    }

    /// 设置是否显示 x 轴标签。
    pub fn show_x_axis(mut self, show: bool) -> Self {
        self.show_x_axis = show;
        self
    }

    /// 设置是否显示 y 轴标签。
    pub fn show_y_axis(mut self, show: bool) -> Self {
        self.show_y_axis = show;
        self
    }

    /// 设置是否平滑曲线。
    pub fn smooth(mut self, smooth: bool) -> Self {
        self.smooth = smooth;
        self
    }

    /// 设置 y 轴显式范围。
    pub fn y_range(mut self, min: f64, max: f64) -> Self {
        self.y_min = Some(min);
        self.y_max = Some(max);
        self
    }

    /// 设置 x 轴标签。
    pub fn x_labels(mut self, labels: Vec<impl Into<SharedString>>) -> Self {
        self.x_axis_labels = labels.into_iter().map(|l| l.into()).collect();
        self
    }

    /// 设置是否显示图例。
    pub fn show_legend(mut self, show: bool) -> Self {
        self.show_legend = show;
        self
    }

    /// 把系列数据暴露给辅助技术：发布一个整体摘要节点（折线数 / 总点数 +
    /// 逐系列 y 极值），节点矩形在发布时按画布大小填充。
    ///
    /// `id` 是画布元素自身的标识，同一图表跨帧应使用同一 id。
    pub fn a11y(mut self, id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        self.a11y = Some((id.into(), label.into()));
        self
    }
}

impl Styled for LineChart {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// 格式化 y 轴刻度值。
fn format_y_value(value: f64) -> String {
    if value.abs() >= 1000.0 {
        format!("{:.0}k", value / 1000.0)
    } else if value.abs() >= 1.0 {
        format!("{:.0}", value)
    } else {
        format!("{:.2}", value)
    }
}

/// 绘制状态。
struct PaintData {
    series: Vec<LineChartSeries>,
    show_grid: bool,
    smooth: bool,
    y_min: Option<f64>,
    y_max: Option<f64>,
    grid_color: Hsla,
    padding_left: f32,
    padding_right: f32,
    padding_top: f32,
    padding_bottom: f32,
}

impl RenderOnce for LineChart {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;

        let series = self.series.clone();
        let show_grid = self.show_grid;
        let show_x_axis = self.show_x_axis;
        let show_y_axis = self.show_y_axis;
        let smooth = self.smooth;
        let y_min = self.y_min;
        let y_max = self.y_max;
        let x_axis_labels = self.x_axis_labels.clone();

        let a11y = self.a11y;
        let semantics = a11y
            .as_ref()
            .map(|(_, label)| line_chart_semantics(&self.series, label));

        let grid_color: Hsla = theme.tokens.border.color;
        let text_color = theme.tokens.muted_foreground;

        let padding_left: f32 = if show_y_axis { 50.0 } else { 10.0 };
        let padding_right: f32 = 20.0;
        let padding_top: f32 = 20.0;
        let padding_bottom: f32 = if show_x_axis { 40.0 } else { 10.0 };

        let series_for_legend = series.clone();

        let data_range = DataRange::from_series(&series, y_min, y_max);

        let y_labels: Vec<String> = if show_y_axis {
            (0..=5)
                .map(|i| {
                    let normalized = i as f64 / 5.0;
                    let value = data_range.y_value_at(normalized);
                    format_y_value(value)
                })
                .collect()
        } else {
            Vec::new()
        };

        let paint_data = PaintData {
            series,
            show_grid,
            smooth,
            y_min,
            y_max,
            grid_color,
            padding_left,
            padding_right,
            padding_top,
            padding_bottom,
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .map(|this| {
                let mut d = this;
                d.style().refine(&user_style);
                d
            })
            .child(
                div()
                    .flex_1()
                    .min_h(px(200.0))
                    .relative()
                    .child({
                        let mut chart = canvas(
                            move |_bounds, _window, _cx| paint_data,
                            move |bounds, paint_data, window, _cx| {
                                if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
                                    return;
                                }

                                let data_range = DataRange::from_series(
                                    &paint_data.series,
                                    paint_data.y_min,
                                    paint_data.y_max,
                                );

                                let chart_left = bounds.left() + px(paint_data.padding_left);
                                let chart_right = bounds.right() - px(paint_data.padding_right);
                                let chart_top = bounds.top() + px(paint_data.padding_top);
                                let chart_bottom = bounds.bottom() - px(paint_data.padding_bottom);
                                let chart_width = chart_right - chart_left;
                                let chart_height = chart_bottom - chart_top;

                                if chart_width <= px(0.0) || chart_height <= px(0.0) {
                                    return;
                                }

                                if paint_data.show_grid {
                                    let grid_lines = 5;
                                    for i in 0..=grid_lines {
                                        let y = chart_top
                                            + chart_height * (i as f32 / grid_lines as f32);
                                        let mut builder = PathBuilder::stroke(px(1.0));
                                        builder.move_to(point(chart_left, y));
                                        builder.line_to(point(chart_right, y));
                                        if let Ok(path) = builder.build() {
                                            window.paint_path(
                                                path,
                                                paint_data.grid_color.opacity(0.3),
                                            );
                                        }
                                    }

                                    for i in 0..=grid_lines {
                                        let x = chart_left
                                            + chart_width * (i as f32 / grid_lines as f32);
                                        let mut builder = PathBuilder::stroke(px(1.0));
                                        builder.move_to(point(x, chart_top));
                                        builder.line_to(point(x, chart_bottom));
                                        if let Ok(path) = builder.build() {
                                            window.paint_path(
                                                path,
                                                paint_data.grid_color.opacity(0.3),
                                            );
                                        }
                                    }
                                }

                                for (series_index, s) in paint_data.series.iter().enumerate() {
                                    if s.points.is_empty() {
                                        continue;
                                    }

                                    let color =
                                        s.color.unwrap_or_else(|| get_chart_color(series_index));

                                    let screen_points: Vec<Point<Pixels>> = s
                                        .points
                                        .iter()
                                        .map(|p| {
                                            let norm_x = data_range.normalize_x(p.x);
                                            let norm_y = data_range.normalize_y(p.y);
                                            let screen_x = chart_left + chart_width * norm_x;
                                            let screen_y = chart_bottom - chart_height * norm_y;
                                            point(screen_x, screen_y)
                                        })
                                        .collect();

                                    if s.fill_area && screen_points.len() >= 2 {
                                        let mut builder = PathBuilder::fill();
                                        builder.move_to(point(screen_points[0].x, chart_bottom));
                                        builder.line_to(screen_points[0]);

                                        for pt in screen_points.iter().skip(1) {
                                            builder.line_to(*pt);
                                        }

                                        builder.line_to(point(
                                            // 上方 screen_points.len() >= 2 守卫，点集必非空
                                            invariant(screen_points.last(), "折线图：点集非空").x,
                                            chart_bottom,
                                        ));
                                        builder.close();

                                        if let Ok(path) = builder.build() {
                                            window.paint_path(path, color.opacity(0.15));
                                        }
                                    }

                                    if screen_points.len() >= 2 {
                                        let mut builder = PathBuilder::stroke(px(2.0));
                                        builder.move_to(screen_points[0]);

                                        if paint_data.smooth && screen_points.len() >= 3 {
                                            for i in 0..screen_points.len() - 1 {
                                                let p0 = screen_points[i];
                                                let p1 = screen_points[i + 1];
                                                let ctrl_x = (p0.x + p1.x) * 0.5;
                                                builder.curve_to(p1, point(ctrl_x, p0.y));
                                            }
                                        } else {
                                            for pt in screen_points.iter().skip(1) {
                                                builder.line_to(*pt);
                                            }
                                        }

                                        if let Ok(path) = builder.build() {
                                            window.paint_path(path, color);
                                        }
                                    }

                                    if s.show_points {
                                        let point_radius = px(4.0);
                                        for pt in &screen_points {
                                            window.paint_quad(fill(
                                                Bounds::centered_at(
                                                    *pt,
                                                    size(point_radius * 2.0, point_radius * 2.0),
                                                ),
                                                color,
                                            ));
                                        }
                                    }
                                }
                            },
                        )
                        .size_full();

                        // 无障碍：整块图表一个摘要节点（零尺寸，由画布按实际大小填充）。
                        if let (Some((id, label)), Some(semantics)) = (a11y, semantics) {
                            chart = chart
                                .id(id)
                                .a11y(Role::GraphicsObject, semantics)
                                .a11y_label(label);
                        }
                        chart
                    })
                    .when(show_y_axis, |this| {
                        this.children(y_labels.iter().enumerate().map(|(i, label)| {
                            let top_percent = (i as f32 / 5.0) * 100.0;
                            div()
                                .absolute()
                                .left(px(4.0))
                                .top(relative(top_percent / 100.0))
                                .mt(px(padding_top - 6.0))
                                .text_size(px(11.0))
                                .text_color(text_color)
                                .child(label.clone())
                        }))
                    })
                    .when(show_x_axis && !x_axis_labels.is_empty(), |this| {
                        let num_labels = x_axis_labels.len();
                        this.children(x_axis_labels.iter().enumerate().map(|(i, label)| {
                            let left_percent = if num_labels == 1 {
                                50.0
                            } else {
                                (i as f32 / (num_labels - 1) as f32) * 100.0
                            };
                            let chart_width_percent =
                                (100.0 - padding_left / 4.0 - padding_right / 4.0) / 100.0;
                            let adjusted_left =
                                padding_left / 4.0 + left_percent * chart_width_percent;
                            div()
                                .absolute()
                                .bottom(px(8.0))
                                .left(relative(adjusted_left / 100.0))
                                .ml(px(-15.0))
                                .text_size(px(11.0))
                                .text_color(text_color)
                                .child(label.clone())
                        }))
                    }),
            )
            .when(self.show_legend && series_for_legend.len() > 1, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(16.0))
                        .px(px(padding_left))
                        .py(px(8.0))
                        .children(series_for_legend.iter().enumerate().map(|(i, s)| {
                            let color = s.color.unwrap_or_else(|| get_chart_color(i));
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(div().size(px(12.0)).rounded(px(2.0)).bg(color))
                                .child(div().text_sm().text_color(text_color).child(s.name.clone()))
                        })),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{LineChartPoint, LineChartSeries, line_chart_semantics};
    use crate::{Bounds, SharedString};

    /// 无障碍摘要：折线数 / 总点数 + 逐系列 y 极值（空系列不计、空数据退化）。
    #[test]
    fn line_chart_a11y_summary() {
        let label: SharedString = "周销量".into();
        let series = vec![
            LineChartSeries::new(
                "手机",
                vec![
                    LineChartPoint::new(0.0, 120.0),
                    LineChartPoint::new(1.0, 95.0),
                    LineChartPoint::new(2.0, 140.0),
                ],
            ),
            LineChartSeries::new(
                "耳机",
                vec![
                    LineChartPoint::new(0.0, 30.0),
                    LineChartPoint::new(1.0, 45.0),
                ],
            ),
            LineChartSeries::new("空系列", vec![]),
        ];
        let semantics = line_chart_semantics(&series, &label);
        assert_eq!(semantics.len(), 1);
        let node = semantics.get(1).expect("摘要节点");
        assert_eq!(node.label().to_string(), "周销量");
        assert_eq!(
            node.value().map(|value| value.to_string()),
            Some("2 条折线 · 共 5 点".to_string())
        );
        assert_eq!(
            node.description().map(|value| value.to_string()),
            Some("手机：最新 140 · 最低 95 · 最高 140（3 点）；耳机：最新 45 · 最低 30 · 最高 45（2 点）".to_string())
        );
        // 零尺寸：发布时由画布按实际大小填充。
        assert_eq!(node.bounds(), Bounds::default());

        let empty = line_chart_semantics(&[], &label);
        let node = empty.get(1).expect("摘要节点");
        assert_eq!(
            node.value().map(|value| value.to_string()),
            Some("暂无数据".to_string())
        );
    }
}
