//! 标尺刻度与对齐参考线/吸附 —— 通用轻量工具库。
//!
//! 两部分：
//! - **标尺刻度**：[`ruler_ticks`] 按可见区间与缩放挑选「好看」的步长并生成刻度，
//!   供水平/垂直标尺绘制。
//! - **对齐吸附**：[`align`] 把移动矩形按左/中/右与上/中/下六个锚点吸附到其它矩形
//!   （或用户从标尺拖出的参考线），并返回需要绘制的 [`Guide`]。
//!
//! 本模块只做纯计算（无状态、无渲染），页面/网格/画布控件共用同一套吸附规则。

use core::ops::Range;

use crate::{Bounds, Pixels, Point, px};

/// 标尺刻度结果。
#[derive(Clone, Debug, PartialEq)]
pub struct RulerTicks {
    /// 次刻度步长（文档坐标）。
    pub step: f32,
    /// 刻度位置（文档坐标，已按区间裁剪）。
    pub ticks: Vec<f32>,
}

/// 选择一个「好看」的步长（1/2/5 × 10ⁿ），使其不小于 `min`。
pub fn nice_step(min: f32) -> f32 {
    if min <= 0.0 || !min.is_finite() {
        return 1.0;
    }
    let base = 10f32.powf(min.log10().floor());
    for multiplier in [1.0, 2.0, 5.0, 10.0] {
        let step = multiplier * base;
        if step >= min - f32::EPSILON {
            return step;
        }
    }
    base * 10.0
}

/// 按可见区间 `range`（文档坐标）与 `zoom` 生成标尺刻度。
///
/// `min_spacing_px` 是相邻次刻度在屏幕上的最小像素间距。
pub fn ruler_ticks(range: Range<f32>, zoom: f32, min_spacing_px: f32) -> RulerTicks {
    let zoom = if zoom > f32::EPSILON { zoom } else { 1.0 };
    let step = nice_step(min_spacing_px / zoom);
    let mut ticks = Vec::new();
    let mut tick = (range.start / step).floor() * step;
    while tick <= range.end && ticks.len() < 10_000 {
        if tick >= range.start - f32::EPSILON {
            ticks.push(tick);
        }
        tick += step;
    }
    RulerTicks { step, ticks }
}

/// 参考线轴：垂直参考线固定 x，水平参考线固定 y。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GuideAxis {
    /// 垂直参考线（沿 x 位置）。
    Vertical,
    /// 水平参考线（沿 y 位置）。
    Horizontal,
}

/// 触发吸附的移动矩形锚点。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Anchor {
    /// 起始边（左/上）。
    Start,
    /// 居中（水平/垂直中心）。
    Center,
    /// 结束边（右/下）。
    End,
}

/// 一条参考线。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    /// 参考线轴。
    pub axis: GuideAxis,
    /// 参考线位置（窗口/屏幕坐标）。
    pub position: Pixels,
    /// 触发吸附的移动矩形锚点。
    pub anchor: Anchor,
}

/// 吸附结果。
#[derive(Clone, Debug, PartialEq)]
pub struct SnapResult {
    /// 对移动矩形的位移修正量。
    pub offset: Point<Pixels>,
    /// 需要绘制的参考线（每个轴最多一条）。
    pub guides: Vec<Guide>,
}

/// 把 `moving` 按六个锚点吸附到 `others`（其它矩形）与额外参考线位置。
///
/// `extra_x` / `extra_y` 是用户从标尺拖出的自由参考线位置（窗口坐标）。
/// 每个轴取「偏移量最小且不超过 `threshold`」的候选；无候选时偏移为 0。
pub fn align(
    moving: Bounds<Pixels>,
    others: &[Bounds<Pixels>],
    extra_x: &[Pixels],
    extra_y: &[Pixels],
    threshold: Pixels,
) -> SnapResult {
    let threshold = threshold.as_f32();

    let left = moving.origin.x;
    let top = moving.origin.y;
    let right = moving.origin.x + moving.size.width;
    let bottom = moving.origin.y + moving.size.height;
    let center_x = moving.origin.x + moving.size.width * 0.5;
    let center_y = moving.origin.y + moving.size.height * 0.5;

    // 收集两轴的可吸附目标：其它矩形的边/中心 + 额外参考线。
    let mut targets_x: Vec<Pixels> = extra_x.to_vec();
    let mut targets_y: Vec<Pixels> = extra_y.to_vec();
    for other in others {
        let (o_left, o_top) = (other.origin.x, other.origin.y);
        let o_right = other.origin.x + other.size.width;
        let o_bottom = other.origin.y + other.size.height;
        targets_x.extend([o_left, o_left + other.size.width * 0.5, o_right]);
        targets_y.extend([o_top, o_top + other.size.height * 0.5, o_bottom]);
    }

    let mut offset = Point::new(px(0.0), px(0.0));
    let mut guides = Vec::new();

    if let Some((delta, target, anchor)) = best_snap(
        &[
            (left, Anchor::Start),
            (center_x, Anchor::Center),
            (right, Anchor::End),
        ],
        &targets_x,
        threshold,
    ) {
        offset.x = delta;
        guides.push(Guide {
            axis: GuideAxis::Vertical,
            position: target,
            anchor,
        });
    }

    if let Some((delta, target, anchor)) = best_snap(
        &[
            (top, Anchor::Start),
            (center_y, Anchor::Center),
            (bottom, Anchor::End),
        ],
        &targets_y,
        threshold,
    ) {
        offset.y = delta;
        guides.push(Guide {
            axis: GuideAxis::Horizontal,
            position: target,
            anchor,
        });
    }

    SnapResult { offset, guides }
}

/// 在候选锚点与目标位置间选偏移量最小且不超过阈值的匹配。
fn best_snap(
    anchors: &[(Pixels, Anchor)],
    targets: &[Pixels],
    threshold: f32,
) -> Option<(Pixels, Pixels, Anchor)> {
    let mut best: Option<(f32, Pixels, Pixels, Anchor)> = None;
    for &(anchor_pos, anchor) in anchors {
        for &target in targets {
            let delta = (target - anchor_pos).as_f32();
            let distance = delta.abs();
            if distance <= threshold
                && best
                    .as_ref()
                    .map(|(best_distance, ..)| distance < *best_distance)
                    .unwrap_or(true)
            {
                best = Some((distance, px(delta), target, anchor));
            }
        }
    }
    best.map(|(_, delta, target, anchor)| (delta, target, anchor))
}

#[cfg(test)]
mod tests {
    use super::{Anchor, GuideAxis, align, nice_step, ruler_ticks};
    use crate::{bounds, point, px, size};

    /// 好看步长取 1/2/5 × 10ⁿ。
    #[test]
    fn nice_step_rounds_up() {
        assert_eq!(nice_step(0.7), 1.0);
        assert_eq!(nice_step(1.0), 1.0);
        assert_eq!(nice_step(1.5), 2.0);
        assert_eq!(nice_step(3.0), 5.0);
        assert_eq!(nice_step(7.0), 10.0);
        assert_eq!(nice_step(23.0), 50.0);
    }

    /// 刻度按缩放选择步长，并落在区间内。
    #[test]
    fn ruler_ticks_respects_zoom_and_range() {
        // 1x、最小间距 50px → 步长 50。
        let ticks = ruler_ticks(0.0..200.0, 1.0, 50.0);
        assert_eq!(ticks.step, 50.0);
        assert_eq!(ticks.ticks, vec![0.0, 50.0, 100.0, 150.0, 200.0]);

        // 放大 3x 时所需文档步长 ≈16.7 → 取「好看」值 20。
        let zoomed = ruler_ticks(0.0..100.0, 3.0, 50.0);
        assert_eq!(zoomed.step, 20.0);
    }

    /// 非正缩放回退为 1x。
    #[test]
    fn ruler_ticks_handles_zero_zoom() {
        let ticks = ruler_ticks(0.0..100.0, 0.0, 50.0);
        assert_eq!(ticks.step, 50.0);
    }

    /// 左边缘吸附到其它矩形的左边缘。
    #[test]
    fn align_snaps_left_edge() {
        let moving = bounds(point(px(99.0), px(300.0)), size(px(20.0), px(20.0)));
        let other = bounds(point(px(100.0), px(0.0)), size(px(50.0), px(50.0)));
        let result = align(moving, &[other], &[], &[], px(4.0));
        assert_eq!(result.offset.x, px(1.0));
        assert_eq!(result.offset.y, px(0.0));
        let guide = result
            .guides
            .iter()
            .find(|g| g.axis == GuideAxis::Vertical)
            .expect("应有垂直参考线");
        assert_eq!(guide.position, px(100.0));
        assert_eq!(guide.anchor, Anchor::Start);
    }

    /// 中心吸附优先选择偏移量最小的目标。
    #[test]
    fn align_prefers_smallest_offset() {
        let moving = bounds(point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        // 左边缘可吸附到 1（距离 1）或 3（距离 3）→ 选 1。
        let result = align(moving, &[], &[px(3.0), px(1.0)], &[], px(5.0));
        assert_eq!(result.offset.x, px(1.0));
        assert_eq!(result.guides[0].anchor, Anchor::Start);
        assert_eq!(result.guides[0].position, px(1.0));
    }

    /// 超出阈值不吸附，且无参考线。
    #[test]
    fn align_ignores_far_targets() {
        let moving = bounds(point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        let other = bounds(point(px(100.0), px(100.0)), size(px(10.0), px(10.0)));
        let result = align(moving, &[other], &[], &[], px(4.0));
        assert_eq!(result.offset, point(px(0.0), px(0.0)));
        assert!(result.guides.is_empty());
    }

    /// 额外参考线可作为吸附目标。
    #[test]
    fn align_uses_extra_guides() {
        let moving = bounds(point(px(0.0), px(0.0)), size(px(10.0), px(10.0)));
        let result = align(moving, &[], &[px(2.0)], &[], px(4.0));
        assert_eq!(result.offset.x, px(2.0));
        assert_eq!(result.guides.len(), 1);
        assert_eq!(result.guides[0].position, px(2.0));
    }

    /// 两个轴可同时吸附。
    #[test]
    fn align_snaps_both_axes() {
        let moving = bounds(point(px(98.0), px(198.0)), size(px(20.0), px(20.0)));
        let other = bounds(point(px(100.0), px(200.0)), size(px(50.0), px(50.0)));
        let result = align(moving, &[other], &[], &[], px(4.0));
        assert_eq!(result.offset, point(px(2.0), px(2.0)));
        assert_eq!(result.guides.len(), 2);
    }
}
