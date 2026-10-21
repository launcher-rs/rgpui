//! 雷达图组件。

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

/// 雷达图数据集。
#[derive(Clone)]
pub struct RadarDataset {
    /// 数据集标签。
    pub label: SharedString,
    /// 各轴数值（0.0 - 1.0）。
    pub values: Vec<f64>,
    /// 颜色（None 为自动分配）。
    pub color: Option<Hsla>,
}

impl RadarDataset {
    /// 创建数据集。
    pub fn new(label: impl Into<SharedString>, values: Vec<f64>) -> Self {
        Self {
            label: label.into(),
            values,
            color: None,
        }
    }

    /// 设置颜色。
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

/// 雷达图尺寸。
#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub enum RadarChartSize {
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

impl RadarChartSize {
    /// 转换为像素。
    fn to_pixels(self) -> Pixels {
        match self {
            RadarChartSize::Sm => px(200.0),
            RadarChartSize::Md => px(300.0),
            RadarChartSize::Lg => px(400.0),
            RadarChartSize::Custom(s) => px(s as f32),
        }
    }
}

/// 绘制状态。
struct PaintData {
    axes: Vec<SharedString>,
    datasets: Vec<RadarDataset>,
    show_grid: bool,
    grid_levels: usize,
    fill_opacity: f32,
    grid_color: Hsla,
    label_padding: f32,
}

/// 雷达图组件。
#[derive(IntoElement)]
pub struct RadarChart {
    axes: Vec<SharedString>,
    datasets: Vec<RadarDataset>,
    size: RadarChartSize,
    show_grid: bool,
    show_legend: bool,
    grid_levels: usize,
    fill_opacity: f32,
    /// 无障碍配置：元素 id + 画布名称（开启后发布一个整体摘要节点）。
    a11y: Option<(ElementId, SharedString)>,
    style: StyleRefinement,
}

/// 雷达图整体语义：一个零尺寸摘要节点（发布时由画布填充整块区域）。
///
/// 摘要按轴序列出各数据集数值（超过 6 轴 / 3 个数据集截断）。
fn radar_semantics(
    axes: &[SharedString],
    datasets: &[RadarDataset],
    label: &SharedString,
) -> CanvasSemantics {
    let mut semantics = CanvasSemantics::new();
    if axes.is_empty() || datasets.is_empty() {
        semantics.push(
            CanvasNode::new(1, Role::GraphicsObject, label.clone(), Bounds::default())
                .with_value("暂无数据"),
        );
        return semantics;
    }

    let summary = format!("{} 轴 · {} 个数据集", axes.len(), datasets.len());
    let mut parts = Vec::new();
    for dataset in datasets.iter().take(3) {
        let values: Vec<String> = (0..axes.len().min(6))
            .map(|i| {
                let axis = axes
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("轴{}", i + 1).into());
                let value = dataset
                    .values
                    .get(i)
                    .copied()
                    .unwrap_or(0.0)
                    .clamp(0.0, 1.0);
                format!("{axis} {value:.2}")
            })
            .collect();
        let mut detail = format!("{}：{}", dataset.label, values.join("、"));
        if axes.len() > 6 {
            detail.push_str(&format!("、等 {} 轴", axes.len()));
        }
        parts.push(detail);
    }
    if datasets.len() > 3 {
        parts.push(format!("等 {} 个数据集", datasets.len()));
    }

    semantics.push(
        CanvasNode::new(1, Role::GraphicsObject, label.clone(), Bounds::default())
            .with_value(summary)
            .with_description(parts.join("；")),
    );
    semantics
}

impl RadarChart {
    /// 创建雷达图。
    pub fn new() -> Self {
        Self {
            axes: Vec::new(),
            datasets: Vec::new(),
            size: RadarChartSize::default(),
            show_grid: true,
            show_legend: true,
            grid_levels: 5,
            fill_opacity: 0.2,
            a11y: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置轴名称。
    pub fn axes(mut self, axes: Vec<impl Into<SharedString>>) -> Self {
        self.axes = axes.into_iter().map(|a| a.into()).collect();
        self
    }

    /// 添加数据集。
    pub fn dataset(mut self, dataset: RadarDataset) -> Self {
        self.datasets.push(dataset);
        self
    }

    /// 设置全部数据集。
    pub fn datasets(mut self, datasets: Vec<RadarDataset>) -> Self {
        self.datasets = datasets;
        self
    }

    /// 设置尺寸。
    pub fn size(mut self, size: RadarChartSize) -> Self {
        self.size = size;
        self
    }

    /// 设置是否显示网格。
    pub fn show_grid(mut self, show: bool) -> Self {
        self.show_grid = show;
        self
    }

    /// 设置是否显示图例。
    pub fn show_legend(mut self, show: bool) -> Self {
        self.show_legend = show;
        self
    }

    /// 设置网格层级数。
    pub fn grid_levels(mut self, levels: usize) -> Self {
        self.grid_levels = levels.max(2);
        self
    }

    /// 设置填充透明度。
    pub fn fill_opacity(mut self, opacity: f32) -> Self {
        self.fill_opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// 把各数据集数值暴露给辅助技术：发布一个整体摘要节点（轴数 / 数据集数 +
    /// 按轴序列出的数值），节点矩形在发布时按画布大小填充。
    ///
    /// `id` 是画布元素自身的标识，同一图表跨帧应使用同一 id。
    pub fn a11y(mut self, id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        self.a11y = Some((id.into(), label.into()));
        self
    }
}

impl Styled for RadarChart {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// 计算轴角度。
fn angle_for_axis(index: usize, total: usize) -> f32 {
    -std::f32::consts::FRAC_PI_2 + (index as f32 / total as f32) * std::f32::consts::TAU
}

impl RenderOnce for RadarChart {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;
        let chart_size = self.size.to_pixels();
        let show_legend = self.show_legend && self.datasets.len() > 1;
        let datasets_for_legend = self.datasets.clone();
        let text_color = theme.tokens.muted_foreground;
        let label_padding: f32 = 30.0;

        let n_axes = self.axes.len();

        if n_axes < 3 {
            return div()
                .size(chart_size)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.tokens.muted_foreground)
                        .child("Need at least 3 axes"),
                )
                .map(|this| {
                    let mut d = this;
                    d.style().refine(&user_style);
                    d
                })
                .into_any_element();
        }

        let a11y = self.a11y;
        let semantics = a11y
            .as_ref()
            .map(|(_, label)| radar_semantics(&self.axes, &self.datasets, label));

        let paint_data = PaintData {
            axes: self.axes.clone(),
            datasets: self.datasets,
            show_grid: self.show_grid,
            grid_levels: self.grid_levels,
            fill_opacity: self.fill_opacity,
            grid_color: theme.tokens.border.color,
            label_padding,
        };

        let axis_labels = self.axes;

        div()
            .flex()
            .flex_col()
            .items_center()
            .map(|this| {
                let mut d = this;
                d.style().refine(&user_style);
                d
            })
            .child(
                div()
                    .size(chart_size)
                    .relative()
                    .child({
                        let mut chart = canvas(
                            move |_bounds, _window, _cx| paint_data,
                            move |bounds, data, window, _cx| {
                                if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
                                    return;
                                }

                                let n = data.axes.len();
                                if n < 3 {
                                    return;
                                }

                                let cx_f = bounds.left() + bounds.size.width * 0.5;
                                let cy_f = bounds.top() + bounds.size.height * 0.5;
                                let max_radius = (bounds.size.width.min(bounds.size.height) * 0.5)
                                    - px(data.label_padding);

                                if max_radius <= px(0.0) {
                                    return;
                                }

                                if data.show_grid {
                                    for level in 1..=data.grid_levels {
                                        let radius =
                                            max_radius * (level as f32 / data.grid_levels as f32);
                                        let mut builder = PathBuilder::stroke(px(1.0));
                                        for i in 0..=n {
                                            let angle = angle_for_axis(i % n, n);
                                            let pt = point(
                                                cx_f + radius * angle.cos(),
                                                cy_f + radius * angle.sin(),
                                            );
                                            if i == 0 {
                                                builder.move_to(pt);
                                            } else {
                                                builder.line_to(pt);
                                            }
                                        }
                                        if let Ok(path) = builder.build() {
                                            window.paint_path(path, data.grid_color.opacity(0.2));
                                        }
                                    }

                                    for i in 0..n {
                                        let angle = angle_for_axis(i, n);
                                        let mut builder = PathBuilder::stroke(px(1.0));
                                        builder.move_to(point(cx_f, cy_f));
                                        builder.line_to(point(
                                            cx_f + max_radius * angle.cos(),
                                            cy_f + max_radius * angle.sin(),
                                        ));
                                        if let Ok(path) = builder.build() {
                                            window.paint_path(path, data.grid_color.opacity(0.3));
                                        }
                                    }
                                }

                                for (ds_idx, ds) in data.datasets.iter().enumerate() {
                                    if ds.values.is_empty() {
                                        continue;
                                    }
                                    let color = ds.color.unwrap_or_else(|| default_color(ds_idx));

                                    let pts: Vec<Point<Pixels>> = (0..n)
                                        .map(|i| {
                                            let val = ds
                                                .values
                                                .get(i)
                                                .copied()
                                                .unwrap_or(0.0)
                                                .clamp(0.0, 1.0);
                                            let angle = angle_for_axis(i, n);
                                            let radius = max_radius * val as f32;
                                            point(
                                                cx_f + radius * angle.cos(),
                                                cy_f + radius * angle.sin(),
                                            )
                                        })
                                        .collect();

                                    if pts.len() >= 3 {
                                        let mut fill_builder = PathBuilder::fill();
                                        fill_builder.move_to(pts[0]);
                                        for pt in pts.iter().skip(1) {
                                            fill_builder.line_to(*pt);
                                        }
                                        fill_builder.close();
                                        if let Ok(path) = fill_builder.build() {
                                            window
                                                .paint_path(path, color.opacity(data.fill_opacity));
                                        }

                                        let mut stroke_builder = PathBuilder::stroke(px(2.0));
                                        stroke_builder.move_to(pts[0]);
                                        for pt in pts.iter().skip(1) {
                                            stroke_builder.line_to(*pt);
                                        }
                                        stroke_builder.line_to(pts[0]);
                                        if let Ok(path) = stroke_builder.build() {
                                            window.paint_path(path, color);
                                        }

                                        let dot_size = px(6.0);
                                        for pt in &pts {
                                            window.paint_quad(fill(
                                                Bounds::centered_at(*pt, size(dot_size, dot_size)),
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
                    .children(axis_labels.iter().enumerate().map(|(i, label)| {
                        let angle = angle_for_axis(i, n_axes);
                        let label_dist = 0.5 + label_padding / (chart_size / px(1.0));
                        let left_frac = 0.5 + label_dist * angle.cos();
                        let top_frac = 0.5 + label_dist * angle.sin();
                        div()
                            .absolute()
                            .left(relative(left_frac))
                            .top(relative(top_frac))
                            .ml(px(-20.0))
                            .mt(px(-8.0))
                            .text_size(px(11.0))
                            .text_color(text_color)
                            .child(label.clone())
                    })),
            )
            .when(show_legend, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(16.0))
                        .py(px(8.0))
                        .justify_center()
                        .children(datasets_for_legend.iter().enumerate().map(|(i, ds)| {
                            let color = ds.color.unwrap_or_else(|| default_color(i));
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(div().size(px(12.0)).rounded(px(2.0)).bg(color))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(text_color)
                                        .child(ds.label.clone()),
                                )
                        })),
                )
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{RadarDataset, radar_semantics};
    use crate::{Bounds, SharedString};

    /// 无障碍摘要：轴数 / 数据集数 + 按轴序列出各数据集数值；空数据退化。
    #[test]
    fn radar_a11y_summary() {
        let label: SharedString = "角色属性".into();
        let axes: Vec<SharedString> = vec!["力量".into(), "敏捷".into(), "智力".into()];
        let datasets = vec![
            RadarDataset::new("战士", vec![0.8, 0.5, 0.3]),
            RadarDataset::new("法师", vec![0.3, 0.6, 0.9]),
        ];
        let semantics = radar_semantics(&axes, &datasets, &label);
        assert_eq!(semantics.len(), 1);
        let node = semantics.get(1).expect("摘要节点");
        assert_eq!(node.label().to_string(), "角色属性");
        assert_eq!(
            node.value().map(|value| value.to_string()),
            Some("3 轴 · 2 个数据集".to_string())
        );
        assert_eq!(
            node.description().map(|value| value.to_string()),
            Some(
                "战士：力量 0.80、敏捷 0.50、智力 0.30；法师：力量 0.30、敏捷 0.60、智力 0.90"
                    .to_string()
            )
        );
        // 零尺寸：发布时由画布按实际大小填充。
        assert_eq!(node.bounds(), Bounds::default());

        // 空数据退化为「暂无数据」摘要。
        let empty = radar_semantics(&axes, &[], &label);
        let node = empty.get(1).expect("摘要节点");
        assert_eq!(
            node.value().map(|value| value.to_string()),
            Some("暂无数据".to_string())
        );
    }
}
