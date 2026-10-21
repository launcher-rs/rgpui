//! 饼图/环形图组件。

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
    /// 无障碍配置：元素 id + 画布名称（开启后发布一个整体摘要节点）。
    a11y: Option<(ElementId, SharedString)>,
    style: StyleRefinement,
}

/// 饼图整体语义：一个零尺寸摘要节点（发布时由画布填充整块区域）。
fn pie_semantics(segments: &[PieChartSegment], label: &SharedString) -> CanvasSemantics {
    // 占比按正扇区计算，零 / 负值不参与。
    let mut sorted: Vec<&PieChartSegment> = segments.iter().filter(|s| s.value > 0.0).collect();
    sorted.sort_by(|a, b| b.value.total_cmp(&a.value));
    let total: f64 = sorted.iter().map(|s| s.value).sum();

    let summary = if total <= 0.0 {
        "暂无数据".to_string()
    } else {
        format!("{} 个扇区 · 总计 {total:.2}", sorted.len())
    };
    let mut parts = Vec::new();
    for segment in sorted.iter().take(5) {
        parts.push(format!(
            "{} {:.1}%",
            segment.label,
            segment.value / total * 100.0
        ));
    }
    if sorted.len() > 5 {
        parts.push(format!("等 {} 项", sorted.len()));
    }

    let mut semantics = CanvasSemantics::new();
    semantics.push(
        CanvasNode::new(1, Role::GraphicsObject, label.clone(), Bounds::default())
            .with_value(summary)
            .with_description(parts.join("、")),
    );
    semantics
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
            a11y: None,
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

    /// 把图表数据暴露给辅助技术：发布一个整体摘要节点（扇区数 / 总计 + 占比最高项），
    /// 节点矩形在发布时按画布大小填充。
    ///
    /// `id` 是画布元素自身的标识，同一图表跨帧应使用同一 id。
    pub fn a11y(mut self, id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        self.a11y = Some((id.into(), label.into()));
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

        let a11y = if total == 0.0 || self.segments.is_empty() {
            None
        } else {
            self.a11y
        };
        let semantics = a11y
            .as_ref()
            .map(|(_, label)| pie_semantics(&self.segments, label));

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
                a11y,
                semantics,
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
    a11y: Option<(ElementId, SharedString)>,
    semantics: Option<CanvasSemantics>,
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

    let mut container = div()
        .size(chart_size)
        .rounded(px(9999.0))
        .relative()
        .overflow_hidden();

    // 用矢量填充路径绘制每个扇区（实心扇形 / 环形扇环），替代点阵圆点：
    // 点阵会在圆点之间留下缝隙（透出背景形成白点）与锯齿边缘。
    let paint_data = PiePaintData {
        segments: segment_data,
        outer_radius,
        inner_radius,
    };
    container = container.child({
        let mut chart = canvas(
            move |_bounds, _window, _cx| paint_data,
            move |bounds, data, window, _cx| {
                if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
                    return;
                }

                let center_x = pixels_to_f32(bounds.left() + bounds.size.width * 0.5);
                let center_y = pixels_to_f32(bounds.top() + bounds.size.height * 0.5);
                // 相邻扇区略微重叠，避免抗锯齿接缝处透出背景。
                let overlap = 0.0035;

                for &(start, sweep, color) in &data.segments {
                    let end = start + sweep + overlap;
                    // 按扫过弧长自适应分段，保证曲线平滑。
                    let steps = ((sweep * data.outer_radius / 2.0).ceil() as usize).clamp(2, 256);

                    let mut builder = PathBuilder::fill();
                    if data.inner_radius <= 0.0 {
                        builder.move_to(point(px(center_x), px(center_y)));
                    } else {
                        builder.move_to(point(
                            px(center_x + data.inner_radius * start.cos()),
                            px(center_y + data.inner_radius * start.sin()),
                        ));
                    }
                    for i in 0..=steps {
                        let t = i as f32 / steps as f32;
                        let angle = start + (end - start) * t;
                        builder.line_to(point(
                            px(center_x + data.outer_radius * angle.cos()),
                            px(center_y + data.outer_radius * angle.sin()),
                        ));
                    }
                    if data.inner_radius > 0.0 {
                        for i in (0..=steps).rev() {
                            let t = i as f32 / steps as f32;
                            let angle = start + (end - start) * t;
                            builder.line_to(point(
                                px(center_x + data.inner_radius * angle.cos()),
                                px(center_y + data.inner_radius * angle.sin()),
                            ));
                        }
                    }
                    builder.close();
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, color);
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
    });

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

/// 饼图画布绘制数据：各扇区（起始角，扫过角，颜色）与内外半径。
#[derive(Clone)]
struct PiePaintData {
    segments: Vec<(f32, f32, Hsla)>,
    outer_radius: f32,
    inner_radius: f32,
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

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{PieChartSegment, pie_semantics};
    use crate::{Bounds, SharedString};

    /// 无障碍摘要：扇区数 / 总计 + 占比降序列举（零值剔除、最多 5 项）。
    #[test]
    fn pie_a11y_summary() {
        let label: SharedString = "磁盘分布".into();
        let semantics = pie_semantics(
            &[
                PieChartSegment::new("A", 40.0),
                PieChartSegment::new("B", 35.0),
                PieChartSegment::new("C", 25.0),
            ],
            &label,
        );
        assert_eq!(semantics.len(), 1);
        let node = semantics.get(1).expect("摘要节点");
        assert_eq!(node.label().to_string(), "磁盘分布");
        assert_eq!(
            node.value().map(|value| value.to_string()),
            Some("3 个扇区 · 总计 100.00".to_string())
        );
        assert_eq!(
            node.description().map(|value| value.to_string()),
            Some("A 40.0%、B 35.0%、C 25.0%".to_string())
        );
        // 零尺寸：发布时由画布按实际大小填充。
        assert_eq!(node.bounds(), Bounds::default());

        // 超过 5 项只列前 5，末尾补总数提示。
        let segments: Vec<PieChartSegment> = (0..7)
            .map(|i| PieChartSegment::new(format!("s{i}"), 1.0))
            .collect();
        let many = pie_semantics(&segments, &label);
        let node = many.get(1).expect("摘要节点");
        assert_eq!(
            node.description().map(|value| value.to_string()),
            Some("s0 14.3%、s1 14.3%、s2 14.3%、s3 14.3%、s4 14.3%、等 7 项".to_string())
        );
    }
}
