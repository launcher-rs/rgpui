//! 音频波形可视化：用垂直条形柱绘制波形。

use crate::canvas_a11y::{CanvasNode, CanvasSemantics};
use crate::{prelude::FluentBuilder as _, *};
use accesskit::Role;

/// 绘制用的波形快照数据。
#[derive(Clone)]
struct WaveformPaintData {
    data: Vec<f32>,
    bar_width: f32,
    gap: f32,
    color: Hsla,
    active_color: Hsla,
    playback_position: f32,
}

/// 音频波形组件。
#[derive(IntoElement)]
pub struct Waveform {
    /// 幅度数据（0~1）。
    data: Vec<f32>,
    /// 条形宽度。
    bar_width: Pixels,
    /// 条形间隙。
    gap: Pixels,
    /// 未播放颜色。
    color: Option<Hsla>,
    /// 已播放颜色。
    active_color: Option<Hsla>,
    /// 播放进度（0~1）。
    playback_position: f32,
    /// 无障碍配置：元素 id + 画布名称（开启后发布一个整体摘要节点）。
    a11y: Option<(ElementId, SharedString)>,
    /// 用户样式。
    style: StyleRefinement,
}

impl Waveform {
    /// 创建波形组件，默认条宽 3px、间隙 2px、高度 48px。
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            bar_width: px(3.0),
            gap: px(2.0),
            color: None,
            active_color: None,
            playback_position: 0.0,
            a11y: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置幅度数据。
    pub fn data(mut self, data: &[f32]) -> Self {
        self.data = data.to_vec();
        self
    }

    /// 设置条形宽度。
    pub fn bar_width(mut self, width: Pixels) -> Self {
        self.bar_width = width;
        self
    }

    /// 设置条形间隙。
    pub fn gap(mut self, gap: Pixels) -> Self {
        self.gap = gap;
        self
    }

    /// 设置未播放颜色。
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// 设置已播放颜色。
    pub fn active_color(mut self, color: Hsla) -> Self {
        self.active_color = Some(color);
        self
    }

    /// 设置播放进度（自动收敛到 0~1）。
    pub fn playback_position(mut self, position: f32) -> Self {
        self.playback_position = position.clamp(0.0, 1.0);
        self
    }

    /// 把波形暴露给辅助技术：整块波形发布一个摘要节点（采样点数 / 峰值 / 播放进度），
    /// 节点矩形在发布时按画布大小填充。
    ///
    /// `id` 是画布元素自身的标识，同一波形跨帧应使用同一 id。
    pub fn a11y(mut self, id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        self.a11y = Some((id.into(), label.into()));
        self
    }
}

impl Default for Waveform {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for Waveform {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Waveform {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;

        let default_color = theme.tokens.muted_foreground.opacity(0.4);
        let default_active = *theme.tokens.primary;

        let a11y = self.a11y;
        let semantics = a11y
            .as_ref()
            .map(|(_, label)| waveform_semantics(&self.data, self.playback_position, label));

        let paint_data = WaveformPaintData {
            data: self.data,
            bar_width: self.bar_width / px(1.0),
            gap: self.gap / px(1.0),
            color: self.color.unwrap_or(default_color),
            active_color: self.active_color.unwrap_or(default_active),
            playback_position: self.playback_position,
        };

        // 纯 DOM 模式下 canvas 隐藏，需为 canvas 元素附加等价 SVG 的 DOM 节点。
        #[cfg(feature = "dom-backend")]
        let dom_data = paint_data.clone();

        let mut chart = canvas(
            move |_bounds, _window, _cx| paint_data,
            move |bounds, data, window, _cx| {
                paint_waveform(bounds, &data, window);
            },
        )
        .absolute()
        .inset_0()
        .size_full();

        // 纯 DOM 模式下用 data URI 的 `<img>` 呈现波形。
        #[cfg(feature = "dom-backend")]
        {
            chart = chart.with_dom(move |bounds, _window, _cx| {
                let svg = waveform_svg(bounds, &dom_data);
                crate::components::dom_svg::svg_img_node(bounds, svg)
            });
        }

        // 无障碍：整块波形一个摘要节点（零尺寸，由画布按实际大小填充）。
        if let (Some((id, label)), Some(semantics)) = (a11y, semantics) {
            chart = chart
                .id(id)
                .a11y(Role::GraphicsObject, semantics)
                .a11y_label(label);
        }

        let mut root = div()
            .relative()
            .when(user_style.size.width.is_none(), |this| this.w_full())
            .when(user_style.size.height.is_none(), |this| this.h(px(48.0)))
            .child(chart);
        root.style().refine(&user_style);
        root
    }
}

/// 波形的整体语义：一个零尺寸摘要节点（发布时由画布填充整块区域）。
fn waveform_semantics(
    data: &[f32],
    playback_position: f32,
    label: &SharedString,
) -> CanvasSemantics {
    let (value, description) = if data.is_empty() {
        ("暂无采样点".to_string(), None)
    } else {
        let peak = data
            .iter()
            .copied()
            .filter(|value| value.is_finite())
            .map(f32::abs)
            .fold(0.0, f32::max);
        let progress = if playback_position.is_finite() {
            playback_position.clamp(0.0, 1.0)
        } else {
            0.0
        };
        (
            format!("{} 个采样点 · 峰值 {peak:.2}", data.len()),
            Some(format!("播放进度 {:.0}%", progress * 100.0)),
        )
    };
    let mut semantics = CanvasSemantics::new();
    let mut node = CanvasNode::new(1, Role::GraphicsObject, label.clone(), Bounds::default())
        .with_value(value);
    if let Some(description) = description {
        node = node.with_description(description);
    }
    semantics.push(node);
    semantics
}

/// 在窗口上绘制波形条形柱。
fn paint_waveform(bounds: Bounds<Pixels>, data: &WaveformPaintData, window: &mut Window) {
    if data.data.is_empty() || bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
        return;
    }

    let bar_w = data.bar_width;
    let gap_w = data.gap;
    let step = bar_w + gap_w;

    if step <= 0.0 {
        return;
    }

    let available_width = bounds.size.width / px(1.0);
    let max_bars = (available_width / step).floor() as usize;

    if max_bars == 0 {
        return;
    }

    let bar_count = max_bars.min(data.data.len());
    let active_bar_boundary = (data.playback_position * bar_count as f32).floor() as usize;
    let height_f = bounds.size.height / px(1.0);

    for i in 0..bar_count {
        let sample_idx = if bar_count < data.data.len() {
            (i as f32 / bar_count as f32 * data.data.len() as f32) as usize
        } else {
            i
        };

        let amplitude = data
            .data
            .get(sample_idx)
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let bar_height = (amplitude * height_f).max(2.0);

        let x = bounds.left() + px(i as f32 * step);
        let y = bounds.top() + px((height_f - bar_height) * 0.5);

        let bar_color = if i < active_bar_boundary {
            data.active_color
        } else {
            data.color
        };

        window.paint_quad(PaintQuad {
            bounds: Bounds {
                origin: point(x, y),
                size: size(px(bar_w), px(bar_height)),
            },
            corner_radii: Corners::all(px(bar_w * 0.5)),
            background: bar_color.into(),
            border_widths: Edges::default(),
            border_color: transparent_black(),
            border_style: BorderStyle::default(),
        });
    }
}

/// 把波形数据转为等价的 SVG 字符串（纯 DOM 模式显示用）。
///
/// 与 [`paint_waveform`] 几何一致（本地坐标，去掉 bounds 原点），用圆角矩形
/// 条形柱复现波形，播放进度之前的部分用高亮色。
#[cfg(feature = "dom-backend")]
fn waveform_svg(bounds: Bounds<Pixels>, data: &WaveformPaintData) -> String {
    use crate::components::dom_svg::css_color;

    let width = bounds.size.width / px(1.0);
    let height = bounds.size.height / px(1.0);
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\">",
        width, height
    );

    let color = css_color(data.color);
    let active_color = css_color(data.active_color);

    if !data.data.is_empty() && width > 0.0 && height > 0.0 {
        let bar_w = data.bar_width;
        let gap_w = data.gap;
        let step = bar_w + gap_w;
        if step > 0.0 {
            let max_bars = (width / step).floor() as usize;
            let bar_count = max_bars.min(data.data.len());
            let active_bar_boundary = (data.playback_position * bar_count as f32).floor() as usize;
            for i in 0..bar_count {
                let sample_idx = if bar_count < data.data.len() {
                    (i as f32 / bar_count as f32 * data.data.len() as f32) as usize
                } else {
                    i
                };
                let amplitude = data
                    .data
                    .get(sample_idx)
                    .copied()
                    .unwrap_or(0.0)
                    .clamp(0.0, 1.0);
                let bar_height = (amplitude * height).max(2.0);
                let x = i as f32 * step;
                let y = (height - bar_height) * 0.5;
                let bar_color = if i < active_bar_boundary {
                    active_color.clone()
                } else {
                    color.clone()
                };
                svg.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\"/>",
                    x,
                    y,
                    bar_w,
                    bar_height,
                    bar_w * 0.5,
                    bar_color
                ));
            }
        }
    }

    svg.push_str("</svg>");
    svg
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::waveform_semantics;
    use crate::{Bounds, SharedString};

    /// 无障碍摘要：采样点数 / 峰值 / 播放进度；空数据退化。
    #[test]
    fn waveform_a11y_summary() {
        let label: SharedString = "音频".into();
        let semantics = waveform_semantics(&[0.2, -0.8, 0.4], 0.25, &label);
        assert_eq!(semantics.len(), 1);
        let node = semantics.get(1).expect("摘要节点");
        assert_eq!(node.label().to_string(), "音频");
        assert_eq!(
            node.value().map(|value| value.to_string()),
            Some("3 个采样点 · 峰值 0.80".to_string())
        );
        assert_eq!(
            node.description().map(|value| value.to_string()),
            Some("播放进度 25%".to_string())
        );
        assert_eq!(node.bounds(), Bounds::default());

        let semantics = waveform_semantics(&[], 0.0, &label);
        let node = semantics.get(1).expect("摘要节点");
        assert_eq!(
            node.value().map(|value| value.to_string()),
            Some("暂无采样点".to_string())
        );
        assert!(node.description().is_none());
    }
}
