//! 迷你走势图组件（Sparkline）：以折线、柱状或面积图展示一维数据序列。

use crate::{prelude::FluentBuilder as _, *};

/// 默认折线颜色（蓝色）。
const DEFAULT_LINE_COLOR: u32 = 0x3b82f6;
/// 上升趋势颜色（绿色）。
const TREND_UP_COLOR: u32 = 0x22c55e;
/// 下降趋势颜色（红色）。
const TREND_DOWN_COLOR: u32 = 0xef4444;

/// 走势图绘制样式。
#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub enum SparklineVariant {
    /// 折线。
    #[default]
    Line,
    /// 柱状。
    Bar,
    /// 面积。
    Area,
}

/// 走势图尺寸档位。
#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub enum SparklineSize {
    /// 小尺寸（60x20）。
    Sm,
    /// 中等尺寸（100x32）。
    #[default]
    Md,
    /// 大尺寸（150x48）。
    Lg,
}

impl SparklineSize {
    /// 返回默认宽高（像素）。
    fn dimensions(self) -> (Pixels, Pixels) {
        match self {
            SparklineSize::Sm => (px(60.0), px(20.0)),
            SparklineSize::Md => (px(100.0), px(32.0)),
            SparklineSize::Lg => (px(150.0), px(48.0)),
        }
    }

    /// 返回线条宽度（像素）。
    fn line_width(self) -> Pixels {
        match self {
            SparklineSize::Sm => px(1.0),
            SparklineSize::Md => px(1.5),
            SparklineSize::Lg => px(2.0),
        }
    }

    /// 返回端点圆点半径（像素）。
    fn point_radius(self) -> Pixels {
        match self {
            SparklineSize::Sm => px(2.0),
            SparklineSize::Md => px(3.0),
            SparklineSize::Lg => px(4.0),
        }
    }

    /// 返回柱状图的间隙（像素）。
    fn bar_gap(self) -> f32 {
        match self {
            SparklineSize::Sm => 1.0,
            SparklineSize::Md => 2.0,
            SparklineSize::Lg => 3.0,
        }
    }
}

/// 走势趋势。
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum SparklineTrend {
    /// 上升。
    Up,
    /// 下降。
    Down,
    /// 持平。
    Neutral,
}

/// 数据取值范围。
struct DataRange {
    /// 最小值。
    min: f64,
    /// 最大值。
    max: f64,
    /// 最小值所在索引。
    min_index: usize,
    /// 最大值所在索引。
    max_index: usize,
}

impl DataRange {
    /// 从数据计算取值范围。
    fn from_data(data: &[f64]) -> Self {
        if data.is_empty() {
            return Self {
                min: 0.0,
                max: 1.0,
                min_index: 0,
                max_index: 0,
            };
        }

        let mut min = f64::MAX;
        let mut max = f64::MIN;
        let mut min_index = 0;
        let mut max_index = 0;

        for (i, &value) in data.iter().enumerate() {
            if value < min {
                min = value;
                min_index = i;
            }
            if value > max {
                max = value;
                max_index = i;
            }
        }

        if (max - min).abs() < f64::EPSILON {
            max = min + 1.0;
        }

        Self {
            min,
            max,
            min_index,
            max_index,
        }
    }

    /// 将数值归一化到 [0, 1] 区间。
    fn normalize(&self, value: f64) -> f32 {
        ((value - self.min) / (self.max - self.min)) as f32
    }
}

/// 根据首尾数据点计算走势趋势。
fn compute_trend(data: &[f64]) -> SparklineTrend {
    if data.len() < 2 {
        return SparklineTrend::Neutral;
    }

    let first = data[0];
    let last = data[data.len() - 1];

    if (last - first).abs() < f64::EPSILON * 10.0 {
        SparklineTrend::Neutral
    } else if last > first {
        SparklineTrend::Up
    } else {
        SparklineTrend::Down
    }
}

/// 走势图绘制所需的数据。
struct SparklinePaintData {
    /// 原始数据序列。
    data: Vec<f64>,
    /// 绘制样式。
    variant: SparklineVariant,
    /// 线条颜色。
    line_color: Hsla,
    /// 填充颜色。
    fill_color: Hsla,
    /// 是否高亮最小值与最大值。
    show_min_max: bool,
    /// 最小/最大值高亮颜色。
    min_max_color: Hsla,
    /// 尺寸档位。
    size: SparklineSize,
}

/// 迷你走势图组件。
#[derive(IntoElement)]
pub struct Sparkline {
    /// 数据序列。
    data: Vec<f64>,
    /// 绘制样式。
    variant: SparklineVariant,
    /// 尺寸档位。
    size: SparklineSize,
    /// 线条颜色（默认蓝色）。
    line_color: Option<Hsla>,
    /// 填充颜色（默认线条色的 20% 透明度）。
    fill_color: Option<Hsla>,
    /// 是否高亮最小/最大值。
    show_min_max: bool,
    /// 最小/最大值高亮颜色（默认主题前景色）。
    min_max_color: Option<Hsla>,
    /// 是否显示趋势箭头。
    show_trend: bool,
    /// 自定义宽度。
    custom_width: Option<Pixels>,
    /// 自定义高度。
    custom_height: Option<Pixels>,
    /// 用户样式。
    style: StyleRefinement,
}

impl Sparkline {
    /// 创建走势图组件。
    pub fn new(data: Vec<f64>) -> Self {
        Self {
            data,
            variant: SparklineVariant::Line,
            size: SparklineSize::Md,
            line_color: None,
            fill_color: None,
            show_min_max: false,
            min_max_color: None,
            show_trend: false,
            custom_width: None,
            custom_height: None,
            style: StyleRefinement::default(),
        }
    }

    /// 创建折线走势图。
    pub fn line(data: Vec<f64>) -> Self {
        Self::new(data).variant(SparklineVariant::Line)
    }

    /// 创建柱状走势图。
    pub fn bar(data: Vec<f64>) -> Self {
        Self::new(data).variant(SparklineVariant::Bar)
    }

    /// 创建面积走势图。
    pub fn area(data: Vec<f64>) -> Self {
        Self::new(data).variant(SparklineVariant::Area)
    }

    /// 设置绘制样式。
    pub fn variant(mut self, variant: SparklineVariant) -> Self {
        self.variant = variant;
        self
    }

    /// 设置尺寸档位。
    pub fn size(mut self, size: SparklineSize) -> Self {
        self.size = size;
        self
    }

    /// 设置自定义宽度。
    pub fn width(mut self, width: Pixels) -> Self {
        self.custom_width = Some(width);
        self
    }

    /// 设置自定义高度。
    pub fn height(mut self, height: Pixels) -> Self {
        self.custom_height = Some(height);
        self
    }

    /// 设置线条颜色。
    pub fn line_color(mut self, color: Hsla) -> Self {
        self.line_color = Some(color);
        self
    }

    /// 设置填充颜色。
    pub fn fill_color(mut self, color: Hsla) -> Self {
        self.fill_color = Some(color);
        self
    }

    /// 设置是否高亮最小/最大值。
    pub fn show_min_max(mut self, show: bool) -> Self {
        self.show_min_max = show;
        self
    }

    /// 设置最小/最大值高亮颜色。
    pub fn min_max_color(mut self, color: Hsla) -> Self {
        self.min_max_color = Some(color);
        self
    }

    /// 设置是否显示趋势箭头。
    pub fn show_trend(mut self, show: bool) -> Self {
        self.show_trend = show;
        self
    }

    /// 返回实际宽高（自定义值优先，否则用尺寸档位默认值）。
    fn get_dimensions(&self) -> (Pixels, Pixels) {
        let (default_width, default_height) = self.size.dimensions();
        (
            self.custom_width.unwrap_or(default_width),
            self.custom_height.unwrap_or(default_height),
        )
    }

    /// 根据趋势返回对应的颜色。
    fn get_trend_color(&self) -> Hsla {
        match compute_trend(&self.data) {
            SparklineTrend::Up => rgb(TREND_UP_COLOR).into(),
            SparklineTrend::Down => rgb(TREND_DOWN_COLOR).into(),
            SparklineTrend::Neutral => self
                .line_color
                .unwrap_or_else(|| rgb(DEFAULT_LINE_COLOR).into()),
        }
    }
}

impl Styled for Sparkline {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Sparkline {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (width, height) = self.get_dimensions();
        let trend_color = self.get_trend_color();

        let effective_line_color = if self.show_trend {
            trend_color
        } else {
            self.line_color
                .unwrap_or_else(|| rgb(DEFAULT_LINE_COLOR).into())
        };

        let effective_fill_color = self
            .fill_color
            .unwrap_or_else(|| effective_line_color.opacity(0.2));

        let effective_min_max_color = self
            .min_max_color
            .unwrap_or_else(|| *theme.tokens.foreground);

        let trend = if self.show_trend {
            Some(compute_trend(&self.data))
        } else {
            None
        };

        let paint_data = SparklinePaintData {
            data: self.data,
            variant: self.variant,
            line_color: effective_line_color,
            fill_color: effective_fill_color,
            show_min_max: self.show_min_max,
            min_max_color: effective_min_max_color,
            size: self.size,
        };

        let user_style = self.style;

        let trend_icon = trend.map(|t| {
            let (icon_char, color) = match t {
                SparklineTrend::Up => ("↑", rgb(TREND_UP_COLOR).into()),
                SparklineTrend::Down => ("↓", rgb(TREND_DOWN_COLOR).into()),
                SparklineTrend::Neutral => ("→", *theme.tokens.muted_foreground),
            };
            (icon_char, color)
        });

        let mut root = div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .child(
                canvas(
                    move |_bounds, _window, _cx| paint_data,
                    move |bounds, paint_data, window, _cx| {
                        paint_sparkline(bounds, &paint_data, window);
                    },
                )
                .w(width)
                .h(height),
            )
            .when_some(trend_icon, |this, (icon, color)| {
                this.child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(color)
                        .child(icon),
                )
            });
        root.style().refine(&user_style);
        root
    }
}

/// 根据样式分派绘制函数。
fn paint_sparkline(bounds: Bounds<Pixels>, data: &SparklinePaintData, window: &mut Window) {
    if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
        return;
    }

    if data.data.is_empty() {
        return;
    }

    match data.variant {
        SparklineVariant::Line => paint_line_sparkline(bounds, data, window),
        SparklineVariant::Bar => paint_bar_sparkline(bounds, data, window),
        SparklineVariant::Area => paint_area_sparkline(bounds, data, window),
    }
}

/// 绘制折线走势图。
fn paint_line_sparkline(bounds: Bounds<Pixels>, data: &SparklinePaintData, window: &mut Window) {
    let range = DataRange::from_data(&data.data);
    let point_count = data.data.len();

    if point_count < 2 {
        return;
    }

    let padding = data.size.point_radius();
    let chart_left = bounds.left() + padding;
    let chart_right = bounds.right() - padding;
    let chart_top = bounds.top() + padding;
    let chart_bottom = bounds.bottom() - padding;
    let chart_width = chart_right - chart_left;
    let chart_height = chart_bottom - chart_top;

    if chart_width <= px(0.0) || chart_height <= px(0.0) {
        return;
    }

    let screen_points: Vec<Point<Pixels>> = data
        .data
        .iter()
        .enumerate()
        .map(|(i, &value)| {
            let x_ratio = i as f32 / (point_count - 1) as f32;
            let y_ratio = range.normalize(value);
            let screen_x = chart_left + chart_width * x_ratio;
            let screen_y = chart_bottom - chart_height * y_ratio;
            point(screen_x, screen_y)
        })
        .collect();

    let mut builder = PathBuilder::stroke(data.size.line_width());
    builder.move_to(screen_points[0]);
    for pt in screen_points.iter().skip(1) {
        builder.line_to(*pt);
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, data.line_color);
    }

    if data.show_min_max {
        let point_radius = data.size.point_radius();
        let min_pt = screen_points[range.min_index];
        let max_pt = screen_points[range.max_index];

        window.paint_quad(fill(
            Bounds::centered_at(min_pt, size(point_radius * 2.0, point_radius * 2.0)),
            data.min_max_color,
        ));
        window.paint_quad(fill(
            Bounds::centered_at(max_pt, size(point_radius * 2.0, point_radius * 2.0)),
            data.min_max_color,
        ));
    }
}

/// 绘制面积走势图。
fn paint_area_sparkline(bounds: Bounds<Pixels>, data: &SparklinePaintData, window: &mut Window) {
    let range = DataRange::from_data(&data.data);
    let point_count = data.data.len();

    if point_count < 2 {
        return;
    }

    let padding = data.size.point_radius();
    let chart_left = bounds.left() + padding;
    let chart_right = bounds.right() - padding;
    let chart_top = bounds.top() + padding;
    let chart_bottom = bounds.bottom() - padding;
    let chart_width = chart_right - chart_left;
    let chart_height = chart_bottom - chart_top;

    if chart_width <= px(0.0) || chart_height <= px(0.0) {
        return;
    }

    let screen_points: Vec<Point<Pixels>> = data
        .data
        .iter()
        .enumerate()
        .map(|(i, &value)| {
            let x_ratio = i as f32 / (point_count - 1) as f32;
            let y_ratio = range.normalize(value);
            let screen_x = chart_left + chart_width * x_ratio;
            let screen_y = chart_bottom - chart_height * y_ratio;
            point(screen_x, screen_y)
        })
        .collect();

    let mut fill_builder = PathBuilder::fill();
    fill_builder.move_to(point(screen_points[0].x, chart_bottom));
    fill_builder.line_to(screen_points[0]);
    for pt in screen_points.iter().skip(1) {
        fill_builder.line_to(*pt);
    }
    fill_builder.line_to(point(screen_points.last().unwrap().x, chart_bottom));
    fill_builder.close();

    if let Ok(path) = fill_builder.build() {
        window.paint_path(path, data.fill_color);
    }

    let mut line_builder = PathBuilder::stroke(data.size.line_width());
    line_builder.move_to(screen_points[0]);
    for pt in screen_points.iter().skip(1) {
        line_builder.line_to(*pt);
    }
    if let Ok(path) = line_builder.build() {
        window.paint_path(path, data.line_color);
    }

    if data.show_min_max {
        let point_radius = data.size.point_radius();
        let min_pt = screen_points[range.min_index];
        let max_pt = screen_points[range.max_index];

        window.paint_quad(fill(
            Bounds::centered_at(min_pt, size(point_radius * 2.0, point_radius * 2.0)),
            data.min_max_color,
        ));
        window.paint_quad(fill(
            Bounds::centered_at(max_pt, size(point_radius * 2.0, point_radius * 2.0)),
            data.min_max_color,
        ));
    }
}

/// 绘制柱状走势图。
fn paint_bar_sparkline(bounds: Bounds<Pixels>, data: &SparklinePaintData, window: &mut Window) {
    let range = DataRange::from_data(&data.data);
    let point_count = data.data.len();

    if point_count == 0 {
        return;
    }

    let padding_y = px(2.0);
    let chart_left = bounds.left();
    let chart_right = bounds.right();
    let chart_top = bounds.top() + padding_y;
    let chart_bottom = bounds.bottom() - padding_y;
    let chart_width = chart_right - chart_left;
    let chart_height = chart_bottom - chart_top;

    if chart_width <= px(0.0) || chart_height <= px(0.0) {
        return;
    }

    let gap = data.size.bar_gap();
    let total_gap = gap * (point_count.saturating_sub(1)) as f32;
    let bar_width_f32 = ((chart_width - px(total_gap)) / point_count as f32).max(px(1.0));
    let bar_width = bar_width_f32;

    for (i, &value) in data.data.iter().enumerate() {
        let x = chart_left + bar_width * i as f32 + px(gap * i as f32);
        let height_ratio = range.normalize(value);
        let bar_height = chart_height * height_ratio;
        let y = chart_bottom - bar_height;

        let bar_color = if data.show_min_max && (i == range.min_index || i == range.max_index) {
            data.min_max_color
        } else {
            data.line_color
        };

        window.paint_quad(fill(
            Bounds::new(point(x, y), size(bar_width, bar_height)),
            bar_color,
        ));
    }
}
