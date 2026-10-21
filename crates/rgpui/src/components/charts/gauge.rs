//! 仪表盘组件（半圆弧进度）。

use crate::canvas_a11y::{CanvasNode, CanvasSemantics};
use crate::{prelude::FluentBuilder as _, *};
use accesskit::Role;
use std::rc::Rc;

/// 仪表盘尺寸。
#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub enum GaugeSize {
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

impl GaugeSize {
    /// 获取尺寸对应的宽高。
    fn dimensions(self) -> (f32, f32) {
        match self {
            GaugeSize::Sm => (120.0, 80.0),
            GaugeSize::Md => (200.0, 130.0),
            GaugeSize::Lg => (300.0, 190.0),
            GaugeSize::Custom(w) => (w as f32, w as f32 * 0.65),
        }
    }

    /// 获取描边宽度。
    fn stroke_width(self) -> f32 {
        match self {
            GaugeSize::Sm => 10.0,
            GaugeSize::Md => 16.0,
            GaugeSize::Lg => 22.0,
            GaugeSize::Custom(w) => (w as f32 * 0.08).max(6.0),
        }
    }
}

/// 绘制状态。
struct PaintData {
    value: f32,
    color: Hsla,
    track_color: Hsla,
    stroke_width: f32,
}

/// 仪表盘组件。
#[derive(IntoElement)]
pub struct Gauge {
    /// 组件 id（保留占位）。
    _id: SharedString,
    value: f32,
    label: Option<SharedString>,
    format_fn: Option<Rc<dyn Fn(f32) -> String>>,
    size: GaugeSize,
    color: Option<Hsla>,
    track_color: Option<Hsla>,
    /// 无障碍配置：元素 id + 画布名称（开启后发布一个整体摘要节点）。
    a11y: Option<(ElementId, SharedString)>,
    style: StyleRefinement,
}

/// 仪表盘整体语义：一个零尺寸摘要节点（发布时由画布填充整块区域）。
fn gauge_semantics(value: f32, formatted_value: &str, label: &SharedString) -> CanvasSemantics {
    let mut semantics = CanvasSemantics::new();
    semantics.push(
        CanvasNode::new(1, Role::GraphicsObject, label.clone(), Bounds::default())
            .with_value(formatted_value.to_string())
            .with_description(format!(
                "半圆弧进度 · 量程 0–100%，当前 {:.0}%",
                (value.clamp(0.0, 1.0) * 100.0).round()
            )),
    );
    semantics
}

impl Gauge {
    /// 创建仪表盘。
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            _id: id.into(),
            value: 0.0,
            label: None,
            format_fn: None,
            size: GaugeSize::default(),
            color: None,
            track_color: None,
            a11y: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置进度值（0.0 - 1.0）。
    pub fn value(mut self, value: f32) -> Self {
        self.value = value.clamp(0.0, 1.0);
        self
    }

    /// 设置标签。
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 设置数值格式化函数。
    pub fn format(mut self, f: impl Fn(f32) -> String + 'static) -> Self {
        self.format_fn = Some(Rc::new(f));
        self
    }

    /// 设置尺寸。
    pub fn size(mut self, size: GaugeSize) -> Self {
        self.size = size;
        self
    }

    /// 设置进度颜色。
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// 设置轨道颜色。
    pub fn track_color(mut self, color: Hsla) -> Self {
        self.track_color = Some(color);
        self
    }

    /// 把进度值暴露给辅助技术：发布一个整体摘要节点（格式化数值 + 当前百分比），
    /// 节点矩形在发布时按画布大小填充。
    ///
    /// `id` 是画布元素自身的标识，同一仪表盘跨帧应使用同一 id。
    pub fn a11y(mut self, id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        self.a11y = Some((id.into(), label.into()));
        self
    }
}

impl Styled for Gauge {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Gauge {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;
        let (width, height) = self.size.dimensions();
        let stroke = self.size.stroke_width();

        let gauge_color = self.color.unwrap_or_else(|| rgb(0x3b82f6).into());
        let track = self.track_color.unwrap_or(theme.tokens.muted.color);

        let formatted_value = if let Some(ref fmt) = self.format_fn {
            fmt(self.value)
        } else {
            format!("{}%", (self.value * 100.0) as i32)
        };

        let text_color = theme.tokens.foreground;
        let label_color = theme.tokens.muted_foreground;

        let a11y = self.a11y;
        let semantics = a11y
            .as_ref()
            .map(|(_, label)| gauge_semantics(self.value, &formatted_value, label));

        let paint_data = PaintData {
            value: self.value,
            color: gauge_color,
            track_color: track,
            stroke_width: stroke,
        };

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(4.0))
            .map(|this| {
                let mut d = this;
                d.style().refine(&user_style);
                d
            })
            .child(
                div()
                    .w(px(width))
                    .h(px(height))
                    .relative()
                    .child({
                        let mut chart = canvas(
                            move |_bounds, _window, _cx| paint_data,
                            move |bounds, data, window, _cx| {
                                if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
                                    return;
                                }

                                let w = bounds.size.width;
                                let center_x = bounds.left() + w * 0.5;
                                let arc_radius = w * 0.5 - px(data.stroke_width * 0.5);

                                if arc_radius <= px(0.0) {
                                    return;
                                }

                                let center_y = bounds.bottom() - px(data.stroke_width * 0.5);

                                let segments = 60_usize;
                                let start_angle: f32 = std::f32::consts::PI;
                                let end_angle: f32 = 0.0;

                                {
                                    let mut builder = PathBuilder::stroke(px(data.stroke_width));
                                    for i in 0..=segments {
                                        let t = i as f32 / segments as f32;
                                        let angle = start_angle + (end_angle - start_angle) * t;
                                        let pt = point(
                                            center_x + arc_radius * angle.cos(),
                                            center_y + arc_radius * angle.sin(),
                                        );
                                        if i == 0 {
                                            builder.move_to(pt);
                                        } else {
                                            builder.line_to(pt);
                                        }
                                    }
                                    if let Ok(path) = builder.build() {
                                        window.paint_path(path, data.track_color);
                                    }
                                }

                                if data.value > 0.0 {
                                    let value_end =
                                        start_angle + (end_angle - start_angle) * data.value;
                                    let value_segments =
                                        ((segments as f32 * data.value) as usize).max(2);
                                    let mut builder = PathBuilder::stroke(px(data.stroke_width));
                                    for i in 0..=value_segments {
                                        let t = i as f32 / value_segments as f32;
                                        let angle = start_angle + (value_end - start_angle) * t;
                                        let pt = point(
                                            center_x + arc_radius * angle.cos(),
                                            center_y + arc_radius * angle.sin(),
                                        );
                                        if i == 0 {
                                            builder.move_to(pt);
                                        } else {
                                            builder.line_to(pt);
                                        }
                                    }
                                    if let Ok(path) = builder.build() {
                                        window.paint_path(path, data.color);
                                    }
                                }
                            },
                        )
                        .size_full();

                        // 无障碍：整块仪表一个摘要节点（零尺寸，由画布按实际大小填充）。
                        if let (Some((id, label)), Some(semantics)) = (a11y, semantics) {
                            chart = chart
                                .id(id)
                                .a11y(Role::GraphicsObject, semantics)
                                .a11y_label(label);
                        }
                        chart
                    })
                    .child(
                        div()
                            .absolute()
                            .bottom(px(4.0))
                            .left_0()
                            .right_0()
                            .flex()
                            .justify_center()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(text_color)
                                    .child(formatted_value),
                            ),
                    ),
            )
            .when_some(self.label, |this, lbl| {
                this.child(div().text_sm().text_color(label_color).child(lbl))
            })
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::gauge_semantics;
    use crate::{Bounds, SharedString};

    /// 无障碍摘要：格式化数值 + 当前百分比描述（钳制到 0–100%）。
    #[test]
    fn gauge_a11y_summary() {
        let label: SharedString = "CPU 占用".into();
        let semantics = gauge_semantics(0.754, "75%", &label);
        assert_eq!(semantics.len(), 1);
        let node = semantics.get(1).expect("摘要节点");
        assert_eq!(node.label().to_string(), "CPU 占用");
        assert_eq!(
            node.value().map(|value| value.to_string()),
            Some("75%".to_string())
        );
        assert_eq!(
            node.description().map(|value| value.to_string()),
            Some("半圆弧进度 · 量程 0–100%，当前 75%".to_string())
        );
        // 零尺寸：发布时由画布按实际大小填充。
        assert_eq!(node.bounds(), Bounds::default());

        // 超界数值在描述里钳到量程。
        let over = gauge_semantics(2.0, "200%", &label);
        let node = over.get(1).expect("摘要节点");
        assert_eq!(
            node.description().map(|value| value.to_string()),
            Some("半圆弧进度 · 量程 0–100%，当前 100%".to_string())
        );
    }
}
