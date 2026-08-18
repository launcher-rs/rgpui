//! 饼图/环形图组件。

use crate::{prelude::FluentBuilder as _, *};

/// 默认系列配色。
const CHART_COLORS: [u32; 8] = [
    0x3b82f6, 0x22c55e, 0xf59e0b, 0xef4444, 0x8b5cf6, 0x06b6d4, 0xf97316, 0xec4899,
];

/// 获取默认系列颜色。
fn default_color(index: usize) -> Hsla {
    rgb(CHART_COLORS[index % CHART_COLORS.len()]).into()
}

/// 像素转 f32。
fn pixels_to_f32(p: Pixels) -> f32 {
    p / px(1.0)
}

/// 饼图扇区。
#[derive(Clone)]
pub struct PieChartSegment {
    /// 扇区标签。
    pub label: SharedString,
    /// 扇区数值。
    pub value: f64,
    /// 颜色（None 为自动分配）。
    pub color: Option<Hsla>,
}

impl PieChartSegment {
    /// 创建扇区。
    pub fn new(label: impl Into<SharedString>, value: f64) -> Self {
        Self {
            label: label.into(),
            value: value.max(0.0),
            color: None,
        }
    }

    /// 设置颜色。
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

/// 饼图变体。
#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub enum PieChartVariant {
    /// 实心饼图。
    #[default]
    Pie,
    /// 环形图。
    Donut,
}

/// 饼图标签位置。
#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub enum PieChartLabelPosition {
    /// 不显示标签。
    #[default]
    None,
    /// 图例形式。
    Legend,
}

/// 饼图尺寸。
#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub enum PieChartSize {
    /// 小尺寸。
    Sm,
    /// 中等尺寸。
    #[default]
    Md,
    /// 大尺寸。
    Lg,
    /// 自定义尺寸。
    Custom(u32),
}

impl PieChartSize {
    /// 转换为像素。
    fn to_pixels(self) -> Pixels {
        match self {
            PieChartSize::Sm => px(120.0),
            PieChartSize::Md => px(200.0),
            PieChartSize::Lg => px(280.0),
            PieChartSize::Custom(size) => px(size as f32),
        }
    }
}

/// 饼图组件。
#[derive(IntoElement)]
pub struct PieChart {
    segments: Vec<PieChartSegment>,
    variant: PieChartVariant,
    label_position: PieChartLabelPosition,
    show_percentages: bool,
    center_label: Option<SharedString>,
    size: PieChartSize,
    donut_thickness: f32,
    style: StyleRefinement,
}

impl PieChart {
    /// 创建饼图。
    pub fn new(segments: Vec<PieChartSegment>) -> Self {
        Self {
            segments,
            variant: PieChartVariant::Pie,
            label_position: PieChartLabelPosition::None,
            show_percentages: false,
            center_label: None,
            size: PieChartSize::Md,
            donut_thickness: 0.35,
            style: StyleRefinement::default(),
        }
    }

    /// 创建实心饼图。
    pub fn pie(segments: Vec<PieChartSegment>) -> Self {
        Self::new(segments).variant(PieChartVariant::Pie)
    }

    /// 创建环形图。
    pub fn donut(segments: Vec<PieChartSegment>) -> Self {
        Self::new(segments).variant(PieChartVariant::Donut)
    }

    /// 设置变体。
    pub fn variant(mut self, variant: PieChartVariant) -> Self {
        self.variant = variant;
        self
    }

    /// 设置尺寸。
    pub fn size(mut self, size: PieChartSize) -> Self {
        self.size = size;
        self
    }

    /// 设置像素尺寸。
    pub fn size_px(mut self, size_val: u32) -> Self {
        self.size = PieChartSize::Custom(size_val);
        self
    }

    /// 设置是否在图例中显示百分比。
    pub fn show_percentages(mut self, show: bool) -> Self {
        self.show_percentages = show;
        self
    }

    /// 设置中心标签（仅环形图生效）。
    pub fn center_label(mut self, label: impl Into<SharedString>) -> Self {
        self.center_label = Some(label.into());
        self
    }

    /// 设置环形厚度比例。
    pub fn donut_thickness(mut self, thickness: f32) -> Self {
        self.donut_thickness = thickness.clamp(0.1, 0.9);
        self
    }

    /// 设置标签位置。
    pub fn label_position(mut self, position: PieChartLabelPosition) -> Self {
        self.label_position = position;
        self
    }
}

impl Styled for PieChart {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for PieChart {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let chart_size = self.size.to_pixels();
        let show_legend = self.label_position == PieChartLabelPosition::Legend;
        let show_percentages = self.show_percentages;
        let user_style = self.style;

        let total: f64 = self.segments.iter().map(|s| s.value).sum();

        let chart = if total == 0.0 || self.segments.is_empty() {
            render_empty_chart(chart_size, theme)
        } else {
            render_pie_chart(
                chart_size,
                &self.segments,
                total,
                self.variant,
                self.donut_thickness,
                self.center_label.clone(),
                theme,
            )
        };

        let legend = if show_legend {
            Some(render_legend(
                &self.segments,
                total,
                show_percentages,
                theme,
            ))
        } else {
            None
        };

        div()
            .flex()
            .gap(px(24.0))
            .items_center()
            .child(chart)
            .when_some(legend, |this, legend| this.child(legend))
            .map(|this| {
                let mut d = this;
                d.style().refine(&user_style);
                d
            })
    }
}

/// 渲染饼图主体（用点阵近似扇区）。
fn render_pie_chart(
    chart_size: Pixels,
    segments: &[PieChartSegment],
    total: f64,
    variant: PieChartVariant,
    donut_thickness: f32,
    center_label: Option<SharedString>,
    theme: &Theme,
) -> Div {
    let size_f32 = pixels_to_f32(chart_size);
    let center = size_f32 * 0.5;
    let outer_radius = size_f32 * 0.5;
    let inner_radius = if variant == PieChartVariant::Donut {
        outer_radius * (1.0 - donut_thickness)
    } else {
        0.0
    };

    let mut segment_data: Vec<(f32, f32, Hsla)> = Vec::new();
    let mut current_angle: f32 = -std::f32::consts::FRAC_PI_2;

    for (idx, segment) in segments.iter().enumerate() {
        if segment.value <= 0.0 {
            continue;
        }
        let fraction = (segment.value / total) as f32;
        let sweep_angle = fraction * std::f32::consts::TAU;
        let color = segment.color.unwrap_or_else(|| default_color(idx));
        segment_data.push((current_angle, sweep_angle, color));
        current_angle += sweep_angle;
    }

    if segment_data.len() == 1 {
        return render_single_segment(
            chart_size,
            segment_data[0].2,
            inner_radius,
            variant,
            center_label,
            theme,
        );
    }

    let ring_width = outer_radius - inner_radius;
    let ring_count = ((ring_width / 3.0).max(1.0) as usize).min(20);

    let mut container = div()
        .size(chart_size)
        .rounded(px(9999.0))
        .relative()
        .overflow_hidden();

    for ring_idx in 0..ring_count {
        let ring_radius = inner_radius + (ring_idx as f32 + 0.5) * (ring_width / ring_count as f32);
        let circumference = std::f32::consts::TAU * ring_radius;
        let dots_in_ring = (circumference / 4.0).max(16.0) as usize;

        for i in 0..dots_in_ring {
            let angle = -std::f32::consts::FRAC_PI_2
                + (i as f32 / dots_in_ring as f32) * std::f32::consts::TAU;
            let color = get_color_at_angle(angle, &segment_data);
            let x = center + ring_radius * angle.cos() - 2.0;
            let y = center + ring_radius * angle.sin() - 2.0;

            container = container.child(
                div()
                    .absolute()
                    .size(px(5.0))
                    .rounded(px(9999.0))
                    .bg(color)
                    .left(px(x))
                    .top(px(y)),
            );
        }
    }

    if variant == PieChartVariant::Donut {
        let inner_size = inner_radius * 2.0 - 4.0;
        let inner_offset = center - inner_radius + 2.0;

        container = container.child(
            div()
                .absolute()
                .size(px(inner_size))
                .rounded(px(9999.0))
                .bg(theme.tokens.background)
                .left(px(inner_offset))
                .top(px(inner_offset))
                .flex()
                .items_center()
                .justify_center()
                .when_some(center_label, |this, label| {
                    this.child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.tokens.foreground)
                            .child(label),
                    )
                }),
        );
    }

    container
}

/// 根据角度获取扇区颜色。
fn get_color_at_angle(angle: f32, segment_data: &[(f32, f32, Hsla)]) -> Hsla {
    let normalized_angle = if angle < -std::f32::consts::FRAC_PI_2 {
        angle + std::f32::consts::TAU
    } else {
        angle
    };

    for &(start_angle, sweep_angle, color) in segment_data {
        let end_angle = start_angle + sweep_angle;
        if normalized_angle >= start_angle && normalized_angle < end_angle {
            return color;
        }
    }

    segment_data
        .last()
        .map(|&(_, _, c)| c)
        .unwrap_or(hsla(0.0, 0.0, 0.5, 1.0))
}

/// 渲染单扇区（整圆）。
fn render_single_segment(
    chart_size: Pixels,
    color: Hsla,
    inner_radius: f32,
    variant: PieChartVariant,
    center_label: Option<SharedString>,
    theme: &Theme,
) -> Div {
    let size_f32 = pixels_to_f32(chart_size);
    let center = size_f32 * 0.5;

    let mut container = div()
        .size(chart_size)
        .rounded(px(9999.0))
        .relative()
        .bg(color);

    if variant == PieChartVariant::Donut {
        let inner_size = inner_radius * 2.0;
        let inner_offset = center - inner_radius;

        container = container.child(
            div()
                .absolute()
                .size(px(inner_size))
                .rounded(px(9999.0))
                .bg(theme.tokens.background)
                .left(px(inner_offset))
                .top(px(inner_offset))
                .flex()
                .items_center()
                .justify_center()
                .when_some(center_label, |this, label| {
                    this.child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.tokens.foreground)
                            .child(label),
                    )
                }),
        );
    }

    container
}

/// 渲染空数据占位。
fn render_empty_chart(chart_size: Pixels, theme: &Theme) -> Div {
    div()
        .size(chart_size)
        .rounded(px(9999.0))
        .bg(theme.tokens.muted)
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .text_sm()
                .text_color(theme.tokens.muted_foreground)
                .child("No data"),
        )
}

/// 渲染图例。
fn render_legend(
    segments: &[PieChartSegment],
    total: f64,
    show_percentages: bool,
    theme: &Theme,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .children(segments.iter().enumerate().filter_map(|(idx, segment)| {
            if segment.value <= 0.0 {
                return None;
            }

            let color = segment.color.unwrap_or_else(|| default_color(idx));
            let percentage = if total > 0.0 {
                (segment.value / total * 100.0) as u32
            } else {
                0
            };

            Some(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().size(px(12.0)).rounded(px(2.0)).bg(color))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .items_center()
                            .justify_between()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.tokens.foreground)
                                    .child(segment.label.clone()),
                            )
                            .when(show_percentages, |this| {
                                this.child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.tokens.muted_foreground)
                                        .child(format!("{}%", percentage)),
                                )
                            }),
                    ),
            )
        }))
}
