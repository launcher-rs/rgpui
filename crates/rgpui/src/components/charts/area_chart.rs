//! 面积图组件（叠加/堆叠两种模式）。

use crate::canvas_a11y::{CanvasNode, CanvasSemantics};
use crate::{prelude::FluentBuilder as _, *};
use accesskit::Role;

/// 默认系列配色。
const CHART_COLORS: [u32; 8] = [
    0x3b82f6, 0x22c55e, 0xf59e0b, 0xef4444, 0x8b5cf6, 0x06b6d4, 0xf97316, 0xec4899,
];

/// 获取默认系列颜色。
fn default_color(index: usize) -> Hsla {
    rgb(CHART_COLORS[index % CHART_COLORS.len()]).into()
}

/// 面积图系列。
#[derive(Clone, Debug)]
pub struct AreaChartSeries {
    /// 系列标签。
    pub label: SharedString,
    /// 数据点（x, y）。
    pub points: Vec<(f64, f64)>,
    /// 颜色（None 为自动分配）。
    pub color: Option<Hsla>,
}

impl AreaChartSeries {
    /// 创建系列。
    pub fn new(label: impl Into<SharedString>, points: Vec<(f64, f64)>) -> Self {
        Self {
            label: label.into(),
            points,
            color: None,
        }
    }

    /// 设置颜色。
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

/// 面积图尺寸。
#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub enum AreaChartSize {
    /// 小尺寸。
    Sm,
    /// 中等尺寸。
    #[default]
    Md,
    /// 大尺寸。
    Lg,
    /// 自定义尺寸。
    Custom(u32, u32),
}

impl AreaChartSize {
    /// 获取尺寸对应的宽高。
    fn dimensions(self) -> (Pixels, Pixels) {
        match self {
            AreaChartSize::Sm => (px(300.0), px(180.0)),
            AreaChartSize::Md => (px(500.0), px(280.0)),
            AreaChartSize::Lg => (px(700.0), px(400.0)),
            AreaChartSize::Custom(w, h) => (px(w as f32), px(h as f32)),
        }
    }
}

/// 面积图模式。
#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub enum AreaChartMode {
    /// 叠加（共享 y 轴）。
    #[default]
    Overlaid,
    /// 堆叠。
    Stacked,
}

/// 面积图数据范围。
struct AreaChartRange {
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
}

impl AreaChartRange {
    /// 从系列推导数据范围。
    fn from_series(series: &[AreaChartSeries], mode: AreaChartMode) -> Self {
        if series.is_empty() {
            return Self {
                x_min: 0.0,
                x_max: 1.0,
                y_min: 0.0,
                y_max: 1.0,
            };
        }

        let mut x_min = f64::MAX;
        let mut x_max = f64::MIN;
        let y_min = 0.0_f64;
        let mut y_max = f64::MIN;

        for s in series {
            for &(x, y) in &s.points {
                x_min = x_min.min(x);
                x_max = x_max.max(x);
                if mode == AreaChartMode::Overlaid {
                    y_max = y_max.max(y);
                }
            }
        }

        if mode == AreaChartMode::Stacked && !series.is_empty() {
            let max_len = series.iter().map(|s| s.points.len()).max().unwrap_or(0);
            for i in 0..max_len {
                let stacked: f64 = series
                    .iter()
                    .map(|s| s.points.get(i).map(|p| p.1).unwrap_or(0.0))
                    .sum();
                y_max = y_max.max(stacked);
            }
        }

        if x_min == f64::MAX {
            x_min = 0.0;
            x_max = 1.0;
        }
        if y_max == f64::MIN {
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
            y_min,
            y_max,
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
}

/// 绘制状态。
struct PaintData {
    series: Vec<AreaChartSeries>,
    show_grid: bool,
    mode: AreaChartMode,
    y_label_count: usize,
    grid_color: Hsla,
    fill_opacity: f32,
}

/// 面积图组件。
#[derive(IntoElement)]
pub struct AreaChart {
    series: Vec<AreaChartSeries>,
    size: AreaChartSize,
    mode: AreaChartMode,
    show_grid: bool,
    show_x_axis: bool,
    show_y_axis: bool,
    show_legend: bool,
    x_labels: Vec<SharedString>,
    y_label_count: usize,
    fill_opacity: f32,
    /// 无障碍配置：元素 id + 画布名称（开启后发布一个整体摘要节点）。
    a11y: Option<(ElementId, SharedString)>,
    style: StyleRefinement,
}

/// 面积图整体语义：一个零尺寸摘要节点（发布时由画布填充整块区域）。
///
/// 摘要按系列列出点数与 y 极值（最多 4 个系列）；堆叠模式的数值为普通（非累计）值。
fn area_chart_semantics(series: &[AreaChartSeries], label: &SharedString) -> CanvasSemantics {
    let non_empty: Vec<&AreaChartSeries> = series.iter().filter(|s| !s.points.is_empty()).collect();
    let total_points: usize = non_empty.iter().map(|s| s.points.len()).sum();

    let summary = if non_empty.is_empty() {
        "暂无数据".to_string()
    } else {
        format!("{} 个面积系列 · 共 {total_points} 点", non_empty.len())
    };

    let mut parts = Vec::new();
    for s in non_empty.iter().take(4) {
        let y_min = s.points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let y_max = s
            .points
            .iter()
            .map(|p| p.1)
            .fold(f64::NEG_INFINITY, f64::max);
        let last = s.points[s.points.len() - 1].1;
        parts.push(format!(
            "{}：最新 {} · 最低 {} · 最高 {}（{} 点）",
            s.label,
            format_value(last),
            format_value(y_min),
            format_value(y_max),
            s.points.len()
        ));
    }
    if non_empty.len() > 4 {
        parts.push(format!("等 {} 个面积系列", non_empty.len()));
    }

    let mut semantics = CanvasSemantics::new();
    semantics.push(
        CanvasNode::new(1, Role::GraphicsObject, label.clone(), Bounds::default())
            .with_value(summary)
            .with_description(parts.join("；")),
    );
    semantics
}

impl AreaChart {
    /// 创建面积图。
    pub fn new() -> Self {
        Self {
            series: Vec::new(),
            size: AreaChartSize::default(),
            mode: AreaChartMode::default(),
            show_grid: true,
            show_x_axis: true,
            show_y_axis: true,
            show_legend: true,
            x_labels: Vec::new(),
            y_label_count: 5,
            fill_opacity: 0.25,
            a11y: None,
            style: StyleRefinement::default(),
        }
    }

    /// 添加系列。
    pub fn series(mut self, s: AreaChartSeries) -> Self {
        self.series.push(s);
        self
    }

    /// 添加系列（别名）。
    pub fn add_series(mut self, s: AreaChartSeries) -> Self {
        self.series.push(s);
        self
    }

    /// 设置尺寸。
    pub fn size(mut self, size: AreaChartSize) -> Self {
        self.size = size;
        self
    }

    /// 设置模式。
    pub fn mode(mut self, mode: AreaChartMode) -> Self {
        self.mode = mode;
        self
    }

    /// 设置堆叠模式。
    pub fn stacked(mut self) -> Self {
        self.mode = AreaChartMode::Stacked;
        self
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

    /// 设置是否显示图例。
    pub fn show_legend(mut self, show: bool) -> Self {
        self.show_legend = show;
        self
    }

    /// 设置 x 轴标签。
    pub fn x_labels(mut self, labels: Vec<impl Into<SharedString>>) -> Self {
        self.x_labels = labels.into_iter().map(|l| l.into()).collect();
        self
    }

    /// 设置 y 轴刻度数量。
    pub fn y_label_count(mut self, count: usize) -> Self {
        self.y_label_count = count.max(2);
        self
    }

    /// 设置填充透明度。
    pub fn fill_opacity(mut self, opacity: f32) -> Self {
        self.fill_opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// 把系列数据暴露给辅助技术：发布一个整体摘要节点（系列数 / 总点数 +
    /// 逐系列 y 极值），节点矩形在发布时按画布大小填充。
    ///
    /// `id` 是画布元素自身的标识，同一图表跨帧应使用同一 id。
    pub fn a11y(mut self, id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        self.a11y = Some((id.into(), label.into()));
        self
    }
}

impl Styled for AreaChart {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// 格式化刻度值。
fn format_value(value: f64) -> String {
    if value.abs() >= 1000.0 {
        format!("{:.0}k", value / 1000.0)
    } else if value.abs() >= 1.0 {
        format!("{:.0}", value)
    } else {
        format!("{:.2}", value)
    }
}

impl RenderOnce for AreaChart {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;
        let (chart_w, chart_h) = self.size.dimensions();

        let padding_left: f32 = if self.show_y_axis { 50.0 } else { 10.0 };
        let padding_right: f32 = 20.0;
        let padding_top: f32 = 20.0;
        let padding_bottom: f32 = if self.show_x_axis { 40.0 } else { 10.0 };

        let series_for_legend = self.series.clone();
        let show_legend = self.show_legend && self.series.len() > 1;

        let range = AreaChartRange::from_series(&self.series, self.mode);
        let y_labels: Vec<String> = if self.show_y_axis {
            (0..=self.y_label_count)
                .map(|i| {
                    let normalized = i as f64 / self.y_label_count as f64;
                    let value = range.y_min + (range.y_max - range.y_min) * (1.0 - normalized);
                    format_value(value)
                })
                .collect()
        } else {
            Vec::new()
        };

        let text_color = theme.tokens.muted_foreground;

        let paint_data = PaintData {
            series: self.series,
            show_grid: self.show_grid,
            mode: self.mode,
            y_label_count: self.y_label_count,
            grid_color: theme.tokens.border.color,
            fill_opacity: self.fill_opacity,
        };

        let a11y = self.a11y;
        let semantics = a11y
            .as_ref()
            .map(|(_, label)| area_chart_semantics(&series_for_legend, label));

        div()
            .flex()
            .flex_col()
            .w(chart_w)
            .map(|this| {
                let mut d = this;
                d.style().refine(&user_style);
                d
            })
            .child(
                div()
                    .h(chart_h)
                    .w_full()
                    .relative()
                    .child({
                        let mut chart = canvas(
                            move |_bounds, _window, _cx| paint_data,
                            move |bounds, data, window, _cx| {
                                if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
                                    return;
                                }

                                let range = AreaChartRange::from_series(&data.series, data.mode);

                                let chart_left = bounds.left() + px(padding_left);
                                let chart_right = bounds.right() - px(padding_right);
                                let chart_top = bounds.top() + px(padding_top);
                                let chart_bottom = bounds.bottom() - px(padding_bottom);
                                let chart_width = chart_right - chart_left;
                                let chart_height = chart_bottom - chart_top;

                                if chart_width <= px(0.0) || chart_height <= px(0.0) {
                                    return;
                                }

                                if data.show_grid {
                                    let grid_lines = data.y_label_count;
                                    for i in 0..=grid_lines {
                                        let y = chart_top
                                            + chart_height * (i as f32 / grid_lines as f32);
                                        let mut builder = PathBuilder::stroke(px(1.0));
                                        builder.move_to(point(chart_left, y));
                                        builder.line_to(point(chart_right, y));
                                        if let Ok(path) = builder.build() {
                                            window.paint_path(path, data.grid_color.opacity(0.2));
                                        }
                                    }

                                    for i in 0..=grid_lines {
                                        let x = chart_left
                                            + chart_width * (i as f32 / grid_lines as f32);
                                        let mut builder = PathBuilder::stroke(px(1.0));
                                        builder.move_to(point(x, chart_top));
                                        builder.line_to(point(x, chart_bottom));
                                        if let Ok(path) = builder.build() {
                                            window.paint_path(path, data.grid_color.opacity(0.2));
                                        }
                                    }
                                }

                                match data.mode {
                                    AreaChartMode::Overlaid => {
                                        for (idx, s) in data.series.iter().enumerate().rev() {
                                            if s.points.len() < 2 {
                                                continue;
                                            }
                                            let color =
                                                s.color.unwrap_or_else(|| default_color(idx));

                                            let screen_pts: Vec<Point<Pixels>> = s
                                                .points
                                                .iter()
                                                .map(|&(x, y)| {
                                                    let sx = chart_left
                                                        + chart_width * range.normalize_x(x);
                                                    let sy = chart_bottom
                                                        - chart_height * range.normalize_y(y);
                                                    point(sx, sy)
                                                })
                                                .collect();

                                            let mut fill_builder = PathBuilder::fill();
                                            fill_builder
                                                .move_to(point(screen_pts[0].x, chart_bottom));
                                            fill_builder.line_to(screen_pts[0]);
                                            for pt in screen_pts.iter().skip(1) {
                                                fill_builder.line_to(*pt);
                                            }
                                            fill_builder.line_to(point(
                                                screen_pts.last().unwrap().x,
                                                chart_bottom,
                                            ));
                                            fill_builder.close();
                                            if let Ok(path) = fill_builder.build() {
                                                window.paint_path(
                                                    path,
                                                    color.opacity(data.fill_opacity),
                                                );
                                            }

                                            let mut stroke_builder = PathBuilder::stroke(px(2.0));
                                            stroke_builder.move_to(screen_pts[0]);
                                            for pt in screen_pts.iter().skip(1) {
                                                stroke_builder.line_to(*pt);
                                            }
                                            if let Ok(path) = stroke_builder.build() {
                                                window.paint_path(path, color);
                                            }
                                        }
                                    }
                                    AreaChartMode::Stacked => {
                                        let max_len = data
                                            .series
                                            .iter()
                                            .map(|s| s.points.len())
                                            .max()
                                            .unwrap_or(0);
                                        if max_len < 2 {
                                            return;
                                        }

                                        let mut cumulative = vec![0.0_f64; max_len];
                                        let mut baselines: Vec<Vec<Point<Pixels>>> = Vec::new();

                                        let x_values: Vec<f64> = (0..max_len)
                                            .map(|i| {
                                                data.series
                                                    .iter()
                                                    .find_map(|s| s.points.get(i).map(|p| p.0))
                                                    .unwrap_or(i as f64)
                                            })
                                            .collect();

                                        let base_pts: Vec<Point<Pixels>> = x_values
                                            .iter()
                                            .map(|&x| {
                                                let sx =
                                                    chart_left + chart_width * range.normalize_x(x);
                                                point(sx, chart_bottom)
                                            })
                                            .collect();
                                        baselines.push(base_pts);

                                        for (idx, s) in data.series.iter().enumerate() {
                                            let color =
                                                s.color.unwrap_or_else(|| default_color(idx));

                                            for (i, pt) in s.points.iter().enumerate() {
                                                if i < max_len {
                                                    cumulative[i] += pt.1;
                                                }
                                            }

                                            let top_pts: Vec<Point<Pixels>> = (0..max_len)
                                                .map(|i| {
                                                    let sx = chart_left
                                                        + chart_width
                                                            * range.normalize_x(x_values[i]);
                                                    let sy = chart_bottom
                                                        - chart_height
                                                            * range.normalize_y(cumulative[i]);
                                                    point(sx, sy)
                                                })
                                                .collect();

                                            let bottom_pts = baselines.last().unwrap();

                                            let mut fill_builder = PathBuilder::fill();
                                            fill_builder.move_to(bottom_pts[0]);
                                            for pt in top_pts.iter() {
                                                fill_builder.line_to(*pt);
                                            }
                                            for pt in bottom_pts.iter().rev() {
                                                fill_builder.line_to(*pt);
                                            }
                                            fill_builder.close();
                                            if let Ok(path) = fill_builder.build() {
                                                window.paint_path(
                                                    path,
                                                    color.opacity(data.fill_opacity),
                                                );
                                            }

                                            let mut stroke_builder = PathBuilder::stroke(px(2.0));
                                            stroke_builder.move_to(top_pts[0]);
                                            for pt in top_pts.iter().skip(1) {
                                                stroke_builder.line_to(*pt);
                                            }
                                            if let Ok(path) = stroke_builder.build() {
                                                window.paint_path(path, color);
                                            }

                                            baselines.push(top_pts);
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
                    .when(self.show_y_axis, |this| {
                        this.children(y_labels.iter().enumerate().map(|(i, label)| {
                            let tick_count = y_labels.len().saturating_sub(1).max(1);
                            let top_percent = i as f32 / tick_count as f32;
                            div()
                                .absolute()
                                .left(px(4.0))
                                .top(relative(top_percent))
                                .mt(px(padding_top - 6.0))
                                .text_size(px(11.0))
                                .text_color(text_color)
                                .child(label.clone())
                        }))
                    })
                    .when(self.show_x_axis && !self.x_labels.is_empty(), |this| {
                        let num_labels = self.x_labels.len();
                        this.children(self.x_labels.iter().enumerate().map(|(i, label)| {
                            let left_percent = if num_labels == 1 {
                                0.5
                            } else {
                                i as f32 / (num_labels - 1) as f32
                            };
                            let chart_w_f32 = chart_w / px(1.0);
                            let chart_frac = 1.0 - (padding_left + padding_right) / chart_w_f32;
                            let adjusted_left =
                                padding_left / chart_w_f32 + left_percent * chart_frac;
                            div()
                                .absolute()
                                .bottom(px(8.0))
                                .left(relative(adjusted_left))
                                .ml(px(-15.0))
                                .text_size(px(11.0))
                                .text_color(text_color)
                                .child(label.clone())
                        }))
                    }),
            )
            .when(show_legend, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(16.0))
                        .px(px(padding_left))
                        .py(px(8.0))
                        .children(series_for_legend.iter().enumerate().map(|(i, s)| {
                            let color = s.color.unwrap_or_else(|| default_color(i));
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(div().size(px(12.0)).rounded(px(2.0)).bg(color))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(text_color)
                                        .child(s.label.clone()),
                                )
                        })),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{AreaChartSeries, area_chart_semantics};
    use crate::{Bounds, SharedString};

    /// 无障碍摘要：系列数 / 总点数 + 逐系列 y 极值（小数值走两位小数格式）。
    #[test]
    fn area_chart_a11y_summary() {
        let label: SharedString = "流量构成".into();
        let series = vec![
            AreaChartSeries::new("直接流量", vec![(0.0, 800.0), (1.0, 650.0), (2.0, 910.0)]),
            AreaChartSeries::new("搜索", vec![(0.0, 400.0), (1.0, 520.0)]),
            AreaChartSeries::new("空系列", vec![]),
        ];
        let semantics = area_chart_semantics(&series, &label);
        assert_eq!(semantics.len(), 1);
        let node = semantics.get(1).expect("摘要节点");
        assert_eq!(node.label().to_string(), "流量构成");
        assert_eq!(
            node.value().map(|value| value.to_string()),
            Some("2 个面积系列 · 共 5 点".to_string())
        );
        assert_eq!(
            node.description().map(|value| value.to_string()),
            Some("直接流量：最新 910 · 最低 650 · 最高 910（3 点）；搜索：最新 520 · 最低 400 · 最高 520（2 点）".to_string())
        );
        // 零尺寸：发布时由画布按实际大小填充。
        assert_eq!(node.bounds(), Bounds::default());

        // 小数值（< 1）用两位小数格式。
        let small = AreaChartSeries::new("比率", vec![(0.0, 0.5), (1.0, 0.25)]);
        let small = area_chart_semantics(&[small], &label);
        let node = small.get(1).expect("摘要节点");
        assert_eq!(
            node.value().map(|value| value.to_string()),
            Some("1 个面积系列 · 共 2 点".to_string())
        );
        assert_eq!(
            node.description().map(|value| value.to_string()),
            Some("比率：最新 0.25 · 最低 0.25 · 最高 0.50（2 点）".to_string())
        );
    }
}
