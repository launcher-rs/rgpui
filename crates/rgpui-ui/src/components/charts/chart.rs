//! 图表基础组件：坐标轴、图例、提示框与通用绘制逻辑。

use rgpui::{prelude::FluentBuilder as _, *};
use std::rc::Rc;

/// 默认系列配色。
const CHART_COLORS: [u32; 8] = [
    0x3b82f6, 0x22c55e, 0xf59e0b, 0xef4444, 0x8b5cf6, 0x06b6d4, 0xf97316, 0xec4899,
];

/// 获取默认系列颜色。
fn default_color(index: usize) -> Hsla {
    rgb(CHART_COLORS[index % CHART_COLORS.len()]).into()
}

/// 数据点。
#[derive(Clone, Debug)]
pub struct DataPoint {
    /// x 坐标。
    pub x: f64,
    /// y 坐标。
    pub y: f64,
    /// 可选标签。
    pub label: Option<SharedString>,
}

impl DataPoint {
    /// 创建数据点。
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y, label: None }
    }

    /// 快捷构造（与 new 等价）。
    pub fn xy(x: f64, y: f64) -> Self {
        Self::new(x, y)
    }

    /// 创建带标签的数据点。
    pub fn labeled(x: f64, y: f64, label: impl Into<SharedString>) -> Self {
        Self {
            x,
            y,
            label: Some(label.into()),
        }
    }
}

/// 数据范围。
#[derive(Clone, Debug, Default)]
pub struct DataRange {
    /// x 最小值。
    pub x_min: f64,
    /// x 最大值。
    pub x_max: f64,
    /// y 最小值。
    pub y_min: f64,
    /// y 最大值。
    pub y_max: f64,
}

impl DataRange {
    /// 创建数据范围。
    pub fn new(x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> Self {
        Self {
            x_min,
            x_max,
            y_min,
            y_max,
        }
    }

    /// 从点集推导数据范围。
    pub fn from_points(points: &[DataPoint]) -> Self {
        if points.is_empty() {
            return Self::new(0.0, 1.0, 0.0, 1.0);
        }

        let mut x_min = f64::MAX;
        let mut x_max = f64::MIN;
        let mut y_min = f64::MAX;
        let mut y_max = f64::MIN;

        for p in points {
            x_min = x_min.min(p.x);
            x_max = x_max.max(p.x);
            y_min = y_min.min(p.y);
            y_max = y_max.max(p.y);
        }

        if (x_max - x_min).abs() < f64::EPSILON {
            x_max = x_min + 1.0;
        }
        if (y_max - y_min).abs() < f64::EPSILON {
            y_max = y_min + 1.0;
        }

        Self::new(x_min, x_max, y_min, y_max)
    }

    /// 合并另一数据范围。
    pub fn extend(&mut self, other: &DataRange) {
        self.x_min = self.x_min.min(other.x_min);
        self.x_max = self.x_max.max(other.x_max);
        self.y_min = self.y_min.min(other.y_min);
        self.y_max = self.y_max.max(other.y_max);
    }

    /// 将 x 归一化到 [0, 1]。
    pub fn normalize_x(&self, x: f64) -> f32 {
        ((x - self.x_min) / (self.x_max - self.x_min)) as f32
    }

    /// 将 y 归一化到 [0, 1]。
    pub fn normalize_y(&self, y: f64) -> f32 {
        ((y - self.y_min) / (self.y_max - self.y_min)) as f32
    }

    /// 按比例扩边生成新范围。
    pub fn with_padding(&self, padding: f64) -> Self {
        let x_range = self.x_max - self.x_min;
        let y_range = self.y_max - self.y_min;
        Self::new(
            self.x_min - x_range * padding,
            self.x_max + x_range * padding,
            self.y_min - y_range * padding,
            self.y_max + y_range * padding,
        )
    }
}

/// 图表内边距。
#[derive(Clone, Default)]
pub struct ChartPadding {
    /// 左内边距。
    pub left: f32,
    /// 右内边距。
    pub right: f32,
    /// 上内边距。
    pub top: f32,
    /// 下内边距。
    pub bottom: f32,
}

impl ChartPadding {
    /// 创建内边距。
    pub fn new(left: f32, right: f32, top: f32, bottom: f32) -> Self {
        Self {
            left,
            right,
            top,
            bottom,
        }
    }

    /// 四周统一内边距。
    pub fn all(value: f32) -> Self {
        Self::new(value, value, value, value)
    }
}

/// 图表绘制区域。
#[derive(Clone)]
pub struct ChartArea {
    /// 整体边界。
    pub bounds: Bounds<Pixels>,
    /// 数据范围。
    pub range: DataRange,
    /// 内边距。
    pub padding: ChartPadding,
}

impl ChartArea {
    /// 绘制区左边界。
    pub fn chart_left(&self) -> Pixels {
        self.bounds.left() + px(self.padding.left)
    }

    /// 绘制区右边界。
    pub fn chart_right(&self) -> Pixels {
        self.bounds.right() - px(self.padding.right)
    }

    /// 绘制区上边界。
    pub fn chart_top(&self) -> Pixels {
        self.bounds.top() + px(self.padding.top)
    }

    /// 绘制区下边界。
    pub fn chart_bottom(&self) -> Pixels {
        self.bounds.bottom() - px(self.padding.bottom)
    }

    /// 绘制区宽度。
    pub fn chart_width(&self) -> Pixels {
        self.chart_right() - self.chart_left()
    }

    /// 绘制区高度。
    pub fn chart_height(&self) -> Pixels {
        self.chart_bottom() - self.chart_top()
    }

    /// 数据坐标 → 屏幕坐标。
    pub fn data_to_screen(&self, data_point: &DataPoint) -> Point<Pixels> {
        let norm_x = self.range.normalize_x(data_point.x);
        let norm_y = self.range.normalize_y(data_point.y);
        let screen_x = self.chart_left() + self.chart_width() * norm_x;
        let screen_y = self.chart_bottom() - self.chart_height() * norm_y;
        point(screen_x, screen_y)
    }

    /// 屏幕坐标 → 数据坐标。
    pub fn screen_to_data(&self, screen_point: Point<Pixels>) -> DataPoint {
        let chart_width = self.chart_width();
        let chart_height = self.chart_height();

        let norm_x = (screen_point.x - self.chart_left()) / chart_width;
        let norm_y = 1.0 - (screen_point.y - self.chart_top()) / chart_height;

        let x = self.range.x_min + (self.range.x_max - self.range.x_min) * norm_x as f64;
        let y = self.range.y_min + (self.range.y_max - self.range.y_min) * norm_y as f64;

        DataPoint::new(x, y)
    }
}

/// 坐标轴位置。
#[derive(Clone, Default)]
pub enum AxisPosition {
    /// 左侧。
    #[default]
    Left,
    /// 右侧。
    Right,
    /// 顶部。
    Top,
    /// 底部。
    Bottom,
}

/// 坐标轴配置。
#[derive(Clone)]
pub struct Axis {
    /// 轴标签。
    pub label: Option<SharedString>,
    /// 轴位置。
    pub position: AxisPosition,
    /// 最小值（None 为自动）。
    pub min: Option<f64>,
    /// 最大值（None 为自动）。
    pub max: Option<f64>,
    /// 刻度数量。
    pub tick_count: usize,
    /// 刻度格式化函数。
    pub tick_format: Option<Rc<dyn Fn(f64) -> String>>,
    /// 是否显示网格线。
    pub show_grid: bool,
    /// 是否显示刻度线。
    pub show_ticks: bool,
    /// 是否显示刻度标签。
    pub show_labels: bool,
}

impl Default for Axis {
    fn default() -> Self {
        Self {
            label: None,
            position: AxisPosition::default(),
            min: None,
            max: None,
            tick_count: 5,
            tick_format: None,
            show_grid: true,
            show_ticks: true,
            show_labels: true,
        }
    }
}

impl Axis {
    /// 创建坐标轴。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置轴标签。
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 设置轴位置。
    pub fn position(mut self, position: AxisPosition) -> Self {
        self.position = position;
        self
    }

    /// 设置左侧轴。
    pub fn left(mut self) -> Self {
        self.position = AxisPosition::Left;
        self
    }

    /// 设置右侧轴。
    pub fn right(mut self) -> Self {
        self.position = AxisPosition::Right;
        self
    }

    /// 设置顶部轴。
    pub fn top(mut self) -> Self {
        self.position = AxisPosition::Top;
        self
    }

    /// 设置底部轴。
    pub fn bottom(mut self) -> Self {
        self.position = AxisPosition::Bottom;
        self
    }

    /// 设置显式范围。
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    /// 设置刻度数量。
    pub fn tick_count(mut self, count: usize) -> Self {
        self.tick_count = count;
        self
    }

    /// 设置刻度格式化函数。
    pub fn tick_format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.tick_format = Some(Rc::new(format));
        self
    }

    /// 设置是否显示网格线。
    pub fn show_grid(mut self, show: bool) -> Self {
        self.show_grid = show;
        self
    }

    /// 设置是否显示刻度线。
    pub fn show_ticks(mut self, show: bool) -> Self {
        self.show_ticks = show;
        self
    }

    /// 设置是否显示刻度标签。
    pub fn show_labels(mut self, show: bool) -> Self {
        self.show_labels = show;
        self
    }

    /// 格式化刻度值。
    fn format_value(&self, value: f64) -> String {
        if let Some(ref format) = self.tick_format {
            format(value)
        } else if value.abs() >= 1000.0 {
            format!("{:.0}k", value / 1000.0)
        } else if value.abs() >= 1.0 {
            format!("{:.0}", value)
        } else {
            format!("{:.2}", value)
        }
    }
}

/// 图例位置。
#[derive(Clone, Default)]
pub enum LegendPosition {
    /// 顶部。
    Top,
    /// 底部。
    #[default]
    Bottom,
    /// 左侧。
    Left,
    /// 右侧。
    Right,
}

/// 图例配置。
#[derive(Clone)]
pub struct Legend {
    /// 图例位置。
    pub position: LegendPosition,
    /// 是否显示图例。
    pub show: bool,
}

impl Default for Legend {
    fn default() -> Self {
        Self {
            position: LegendPosition::default(),
            show: true,
        }
    }
}

impl Legend {
    /// 创建图例。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置图例位置。
    pub fn position(mut self, position: LegendPosition) -> Self {
        self.position = position;
        self
    }

    /// 设置图例在顶部。
    pub fn top(mut self) -> Self {
        self.position = LegendPosition::Top;
        self
    }

    /// 设置图例在底部。
    pub fn bottom(mut self) -> Self {
        self.position = LegendPosition::Bottom;
        self
    }

    /// 设置图例在左侧。
    pub fn left(mut self) -> Self {
        self.position = LegendPosition::Left;
        self
    }

    /// 设置图例在右侧。
    pub fn right(mut self) -> Self {
        self.position = LegendPosition::Right;
        self
    }

    /// 设置是否显示图例。
    pub fn show(mut self, show: bool) -> Self {
        self.show = show;
        self
    }

    /// 隐藏图例。
    pub fn hide(mut self) -> Self {
        self.show = false;
        self
    }
}

/// 提示框配置。
#[derive(Clone, Default)]
pub struct TooltipConfig {
    /// 是否显示提示框。
    pub show: bool,
    /// 提示框格式化函数。
    pub format: Option<Rc<dyn Fn(&DataPoint, &str) -> String>>,
}

impl TooltipConfig {
    /// 创建提示框配置。
    pub fn new() -> Self {
        Self {
            show: true,
            format: None,
        }
    }

    /// 设置提示框格式化函数。
    pub fn format(mut self, format: impl Fn(&DataPoint, &str) -> String + 'static) -> Self {
        self.format = Some(Rc::new(format));
        self
    }

    /// 设置是否显示提示框。
    pub fn show(mut self, show: bool) -> Self {
        self.show = show;
        self
    }

    /// 格式化提示框内容。
    fn format_tooltip(&self, point: &DataPoint, series_name: &str) -> String {
        if let Some(ref format) = self.format {
            format(point, series_name)
        } else {
            let label = point
                .label
                .clone()
                .unwrap_or_else(|| format!("x: {:.2}", point.x).into());
            format!("{}: {} = {:.2}", series_name, label, point.y)
        }
    }
}

/// 系列类型。
#[derive(Clone, Copy, Default)]
pub enum SeriesType {
    /// 折线。
    #[default]
    Line,
    /// 柱状。
    Bar,
    /// 面积。
    Area,
    /// 散点。
    Scatter,
}

/// 数据系列。
#[derive(Clone)]
pub struct Series {
    /// 系列名称。
    pub name: SharedString,
    /// 系列数据。
    pub data: Vec<DataPoint>,
    /// 系列类型。
    pub series_type: SeriesType,
    /// 颜色（None 为自动分配）。
    pub color: Option<Hsla>,
    /// 线宽。
    pub stroke_width: f32,
    /// 是否显示数据点。
    pub show_points: bool,
    /// 数据点半径。
    pub point_radius: f32,
    /// 填充透明度。
    pub fill_opacity: f32,
    /// 是否平滑曲线。
    pub smooth: bool,
    /// 柱宽。
    pub bar_width: Option<f32>,
}

impl Series {
    /// 创建系列。
    pub fn new(name: impl Into<SharedString>, data: Vec<DataPoint>) -> Self {
        Self {
            name: name.into(),
            data,
            series_type: SeriesType::Line,
            color: None,
            stroke_width: 2.0,
            show_points: false,
            point_radius: 4.0,
            fill_opacity: 0.2,
            smooth: false,
            bar_width: None,
        }
    }

    /// 创建折线系列。
    pub fn line(name: impl Into<SharedString>, data: Vec<DataPoint>) -> Self {
        Self::new(name, data).series_type(SeriesType::Line)
    }

    /// 创建柱状系列。
    pub fn bar(name: impl Into<SharedString>, data: Vec<DataPoint>) -> Self {
        Self::new(name, data).series_type(SeriesType::Bar)
    }

    /// 创建面积系列。
    pub fn area(name: impl Into<SharedString>, data: Vec<DataPoint>) -> Self {
        Self::new(name, data)
            .series_type(SeriesType::Area)
            .fill_opacity(0.3)
    }

    /// 创建散点系列。
    pub fn scatter(name: impl Into<SharedString>, data: Vec<DataPoint>) -> Self {
        Self::new(name, data)
            .series_type(SeriesType::Scatter)
            .show_points(true)
    }

    /// 设置系列类型。
    pub fn series_type(mut self, series_type: SeriesType) -> Self {
        self.series_type = series_type;
        self
    }

    /// 设置系列颜色。
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// 设置线宽。
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = width;
        self
    }

    /// 设置是否显示数据点。
    pub fn show_points(mut self, show: bool) -> Self {
        self.show_points = show;
        self
    }

    /// 设置数据点半径。
    pub fn point_radius(mut self, radius: f32) -> Self {
        self.point_radius = radius;
        self
    }

    /// 设置填充透明度。
    pub fn fill_opacity(mut self, opacity: f32) -> Self {
        self.fill_opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// 设置是否平滑曲线。
    pub fn smooth(mut self, smooth: bool) -> Self {
        self.smooth = smooth;
        self
    }

    /// 设置柱宽。
    pub fn bar_width(mut self, width: f32) -> Self {
        self.bar_width = Some(width);
        self
    }

    /// 计算系列数据范围。
    pub fn data_range(&self) -> DataRange {
        DataRange::from_points(&self.data)
    }
}

/// 悬停数据点信息。
struct HoveredPoint {
    series_index: usize,
    screen_pos: Point<Pixels>,
    data_point: DataPoint,
}

/// 图表绘制状态。
struct ChartPaintState {
    series: Vec<Series>,
    x_axis: Axis,
    y_axis: Axis,
    tooltip: TooltipConfig,
    grid_color: Hsla,
    padding: ChartPadding,
}

/// 通用图表组件。
#[derive(IntoElement)]
pub struct Chart {
    series: Vec<Series>,
    x_axis: Axis,
    y_axis: Axis,
    legend: Legend,
    tooltip: TooltipConfig,
    style: StyleRefinement,
}

impl Default for Chart {
    fn default() -> Self {
        Self::new()
    }
}

impl Chart {
    /// 创建图表。
    pub fn new() -> Self {
        Self {
            series: Vec::new(),
            x_axis: Axis::new().bottom(),
            y_axis: Axis::new().left(),
            legend: Legend::new(),
            tooltip: TooltipConfig::new(),
            style: StyleRefinement::default(),
        }
    }

    /// 添加系列。
    pub fn series(mut self, series: Series) -> Self {
        self.series.push(series);
        self
    }

    /// 添加系列（别名）。
    pub fn add_series(mut self, series: Series) -> Self {
        self.series.push(series);
        self
    }

    /// 设置 x 轴。
    pub fn x_axis(mut self, axis: Axis) -> Self {
        self.x_axis = axis;
        self
    }

    /// 设置 y 轴。
    pub fn y_axis(mut self, axis: Axis) -> Self {
        self.y_axis = axis;
        self
    }

    /// 设置图例。
    pub fn legend(mut self, legend: Legend) -> Self {
        self.legend = legend;
        self
    }

    /// 设置是否显示图例。
    pub fn show_legend(mut self, show: bool) -> Self {
        self.legend.show = show;
        self
    }

    /// 设置提示框配置。
    pub fn tooltip(mut self, tooltip: TooltipConfig) -> Self {
        self.tooltip = tooltip;
        self
    }

    /// 设置是否显示提示框。
    pub fn show_tooltip(mut self, show: bool) -> Self {
        self.tooltip.show = show;
        self
    }

    /// 计算整体数据范围（合并系列范围并应用轴显式范围）。
    fn compute_data_range(&self) -> DataRange {
        let mut range = DataRange::new(f64::MAX, f64::MIN, f64::MAX, f64::MIN);

        for series in &self.series {
            let series_range = series.data_range();
            range.extend(&series_range);
        }

        if range.x_min == f64::MAX {
            range = DataRange::new(0.0, 1.0, 0.0, 1.0);
        }

        if let Some(min) = self.x_axis.min {
            range.x_min = min;
        }
        if let Some(max) = self.x_axis.max {
            range.x_max = max;
        }
        if let Some(min) = self.y_axis.min {
            range.y_min = min;
        }
        if let Some(max) = self.y_axis.max {
            range.y_max = max;
        }

        range
    }
}

impl Styled for Chart {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Chart {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        let show_y_axis = self.y_axis.show_labels;
        let show_x_axis = self.x_axis.show_labels;

        let padding = ChartPadding::new(
            if show_y_axis { 60.0 } else { 20.0 },
            20.0,
            20.0,
            if show_x_axis { 40.0 } else { 20.0 },
        );

        let data_range = self.compute_data_range();
        let user_style = self.style;
        let series_for_legend = self.series.clone();
        let legend = self.legend.clone();

        let y_axis_clone = self.y_axis.clone();
        let y_labels: Vec<String> = if show_y_axis {
            (0..=self.y_axis.tick_count)
                .map(|i| {
                    let normalized = i as f64 / self.y_axis.tick_count as f64;
                    let value = data_range.y_min
                        + (data_range.y_max - data_range.y_min) * (1.0 - normalized);
                    y_axis_clone.format_value(value)
                })
                .collect()
        } else {
            Vec::new()
        };

        let paint_state = ChartPaintState {
            series: self.series,
            x_axis: self.x_axis,
            y_axis: self.y_axis,
            tooltip: self.tooltip,
            grid_color: theme.tokens.border.color,
            padding: padding.clone(),
        };

        let text_color: Hsla = theme.tokens.muted_foreground.color;
        let tooltip_bg = theme.tokens.popover;
        let tooltip_border = theme.tokens.border;

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h(px(250.0))
            .map(|this| {
                let mut d = this;
                d.style().refine(&user_style);
                d
            })
            .child(
                div()
                    .flex_1()
                    .relative()
                    .child(
                        canvas(
                            move |bounds, window, _cx| {
                                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                                (paint_state, data_range, bounds, hitbox)
                            },
                            move |bounds, (state, range, _, hitbox), window, cx| {
                                let hitbox_for_event = hitbox.clone();
                                window.on_mouse_event(
                                    move |_event: &MouseMoveEvent, _phase, window, cx| {
                                        if hitbox_for_event.is_hovered(window) {
                                            cx.refresh_windows();
                                        }
                                    },
                                );

                                let mouse_pos = window.mouse_position();
                                if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
                                    return;
                                }

                                let area = ChartArea {
                                    bounds,
                                    range: range.clone(),
                                    padding: state.padding.clone(),
                                };

                                if area.chart_width() <= px(0.0) || area.chart_height() <= px(0.0) {
                                    return;
                                }

                                if state.y_axis.show_grid {
                                    for i in 0..=state.y_axis.tick_count {
                                        let y = area.chart_top()
                                            + area.chart_height()
                                                * (i as f32 / state.y_axis.tick_count as f32);
                                        let mut builder = PathBuilder::stroke(px(1.0));
                                        builder.move_to(point(area.chart_left(), y));
                                        builder.line_to(point(area.chart_right(), y));
                                        if let Ok(path) = builder.build() {
                                            window.paint_path(path, state.grid_color.opacity(0.2));
                                        }
                                    }
                                }

                                if state.x_axis.show_grid {
                                    for i in 0..=state.x_axis.tick_count {
                                        let x = area.chart_left()
                                            + area.chart_width()
                                                * (i as f32 / state.x_axis.tick_count as f32);
                                        let mut builder = PathBuilder::stroke(px(1.0));
                                        builder.move_to(point(x, area.chart_top()));
                                        builder.line_to(point(x, area.chart_bottom()));
                                        if let Ok(path) = builder.build() {
                                            window.paint_path(path, state.grid_color.opacity(0.2));
                                        }
                                    }
                                }

                                let mut hovered_point: Option<HoveredPoint> = None;
                                let hover_radius = px(15.0);

                                for (series_index, series) in state.series.iter().enumerate() {
                                    let color =
                                        series.color.unwrap_or_else(|| default_color(series_index));

                                    if series.data.is_empty() {
                                        continue;
                                    }

                                    let screen_points: Vec<Point<Pixels>> = series
                                        .data
                                        .iter()
                                        .map(|p| area.data_to_screen(p))
                                        .collect();

                                    match series.series_type {
                                        SeriesType::Line | SeriesType::Area => {
                                            if matches!(series.series_type, SeriesType::Area)
                                                && screen_points.len() >= 2
                                            {
                                                let mut builder = PathBuilder::fill();
                                                builder.move_to(point(
                                                    screen_points[0].x,
                                                    area.chart_bottom(),
                                                ));
                                                builder.line_to(screen_points[0]);

                                                for pt in screen_points.iter().skip(1) {
                                                    builder.line_to(*pt);
                                                }

                                                builder.line_to(point(
                                                    screen_points.last().unwrap().x,
                                                    area.chart_bottom(),
                                                ));
                                                builder.close();

                                                if let Ok(path) = builder.build() {
                                                    window.paint_path(
                                                        path,
                                                        color.opacity(series.fill_opacity),
                                                    );
                                                }
                                            }

                                            if screen_points.len() >= 2 {
                                                let mut builder =
                                                    PathBuilder::stroke(px(series.stroke_width));
                                                builder.move_to(screen_points[0]);

                                                if series.smooth && screen_points.len() >= 3 {
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
                                        }
                                        SeriesType::Bar => {
                                            let bar_width = series.bar_width.unwrap_or(20.0);
                                            for (data_pt, screen_pt) in
                                                series.data.iter().zip(screen_points.iter())
                                            {
                                                let bar_height = area.chart_bottom() - screen_pt.y;
                                                let bar_bounds = Bounds::new(
                                                    point(
                                                        screen_pt.x - px(bar_width / 2.0),
                                                        screen_pt.y,
                                                    ),
                                                    size(px(bar_width), bar_height),
                                                );

                                                window.paint_quad(fill(bar_bounds, color));

                                                if bar_bounds.contains(&mouse_pos) {
                                                    hovered_point = Some(HoveredPoint {
                                                        series_index,
                                                        screen_pos: *screen_pt,
                                                        data_point: data_pt.clone(),
                                                    });
                                                }
                                            }
                                        }
                                        SeriesType::Scatter => {}
                                    }

                                    if series.show_points
                                        || matches!(series.series_type, SeriesType::Scatter)
                                    {
                                        let radius = px(series.point_radius);
                                        for (data_pt, screen_pt) in
                                            series.data.iter().zip(screen_points.iter())
                                        {
                                            let is_hovered = (mouse_pos.x - screen_pt.x).abs()
                                                < hover_radius
                                                && (mouse_pos.y - screen_pt.y).abs() < hover_radius;

                                            let point_radius =
                                                if is_hovered { radius * 1.5 } else { radius };

                                            window.paint_quad(fill(
                                                Bounds::centered_at(
                                                    *screen_pt,
                                                    size(point_radius * 2.0, point_radius * 2.0),
                                                ),
                                                color,
                                            ));

                                            if is_hovered && hovered_point.is_none() {
                                                hovered_point = Some(HoveredPoint {
                                                    series_index,
                                                    screen_pos: *screen_pt,
                                                    data_point: data_pt.clone(),
                                                });
                                            }
                                        }
                                    }
                                }

                                if state.tooltip.show && hitbox.is_hovered(window) {
                                    if let Some(hp) = hovered_point {
                                        let series = &state.series[hp.series_index];
                                        let tooltip_text = state
                                            .tooltip
                                            .format_tooltip(&hp.data_point, &series.name);

                                        let text_style = window.text_style();
                                        let font = text_style.font();
                                        let font_size = px(12.0);
                                        let text_len = tooltip_text.len();

                                        let text_run = TextRun {
                                            len: text_len,
                                            font,
                                            color: text_color,
                                            background_color: None,
                                            underline: None,
                                            strikethrough: None,
                                        };

                                        let shaped_line = window.text_system().shape_line(
                                            tooltip_text.into(),
                                            font_size,
                                            &[text_run],
                                            None,
                                        );

                                        let text_width = shaped_line.width;
                                        let padding_h = px(12.0);
                                        let padding_v = px(8.0);
                                        let tooltip_width = text_width + padding_h * 2.0;
                                        let tooltip_height = font_size + padding_v * 2.0;

                                        let tooltip_x = (hp.screen_pos.x - tooltip_width / 2.0)
                                            .max(bounds.left())
                                            .min(bounds.right() - tooltip_width);
                                        let tooltip_y = hp.screen_pos.y - tooltip_height - px(10.0);

                                        let tooltip_bounds = Bounds::new(
                                            point(tooltip_x, tooltip_y),
                                            size(tooltip_width, tooltip_height),
                                        );

                                        window.paint_quad(quad(
                                            tooltip_bounds,
                                            px(6.0),
                                            tooltip_bg,
                                            px(1.0),
                                            tooltip_border,
                                            BorderStyle::default(),
                                        ));

                                        let text_origin =
                                            point(tooltip_x + padding_h, tooltip_y + padding_v);
                                        let _ = shaped_line.paint(
                                            text_origin,
                                            font_size,
                                            TextAlign::default(),
                                            None,
                                            window,
                                            cx,
                                        );
                                    }
                                }
                            },
                        )
                        .size_full(),
                    )
                    .when(show_y_axis, |this| {
                        this.children(y_labels.iter().enumerate().map(|(i, label)| {
                            let tick_count = y_labels.len() - 1;
                            let top_percent = (i as f32 / tick_count.max(1) as f32) * 100.0;
                            div()
                                .absolute()
                                .left(px(4.0))
                                .top(relative(top_percent / 100.0))
                                .mt(px(padding.top - 6.0))
                                .text_size(px(11.0))
                                .text_color(text_color)
                                .child(label.clone())
                        }))
                    }),
            )
            .when(legend.show && series_for_legend.len() > 1, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(16.0))
                        .px(px(padding.left))
                        .py(px(12.0))
                        .justify_center()
                        .children(series_for_legend.iter().enumerate().map(|(i, s)| {
                            let color = s.color.unwrap_or_else(|| default_color(i));
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
