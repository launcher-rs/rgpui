//! 缓动函数（核心缓动 + 补充缺失的回弹/弹性类）。
//!
//! 核心 `rgpui` 提供 `linear` / `quadratic` / `ease_in_out` / `ease_out_quint` /
//! `bounce` / `pulsating_between` / `cubic_bezier` / `ease_out_cubic` /
//! `ease_in_cubic` / `ease_in_out_cubic`，本模块在 `easings` 命名空间下再导出，
//! 并补充核心没有的 back（回弹过冲）与 elastic（弹性振荡）系列。

use std::f32::consts::PI;

/// 缓动命名空间：核心缓动 + 本模块补充的回弹/弹性系列。
pub mod easings {
    // 再导出核心缓动函数
    pub use rgpui::{
        bounce, cubic_bezier, ease_in_cubic, ease_in_out, ease_in_out_cubic, ease_out_cubic,
        ease_out_quint, linear, pulsating_between, quadratic,
    };

    pub use super::{ease_in_elastic, ease_in_out_back, ease_out_back, ease_out_elastic, elastic};
}

/// Back 缓出——快速开始，接近终点时略微过冲再回落到 1.0。
///
/// 适合缩放、位移动画的强调效果（轻微回弹）。
pub fn ease_out_back(t: f32) -> f32 {
    if t >= 1.0 {
        return 1.0;
    }
    // 使用较小的常数以减少过冲幅度，确保结果保持在 [0, 1]。
    let c1 = 1.2;
    let c3 = c1 + 1.0;
    let t_adj = t - 1.0;
    let result = 1.0 + c3 * t_adj * t_adj * t_adj + c1 * t_adj * t_adj;
    result.clamp(0.0, 1.0)
}

/// Back 缓入缓出——两端均有过冲。
pub fn ease_in_out_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c2 = c1 * 1.525;
    if t < 0.5 {
        (((2.0 * t).powi(2) * ((c2 + 1.0) * 2.0 * t - c2)) / 2.0).clamp(0.0, 1.0)
    } else {
        let result = ((2.0 * t - 2.0).powi(2) * ((c2 + 1.0) * (t * 2.0 - 2.0) + c2) + 2.0) / 2.0;
        result.clamp(0.0, 1.0)
    }
}

/// Elastic——指数衰减的正弦振荡，模拟弹簧抖动效果。
pub fn elastic(t: f32) -> f32 {
    if t == 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let p = 0.3;
    let s = p / 4.0;
    let t_adj = t - 1.0;
    let result = 1.0 + 2.0_f32.powf(-10.0 * t_adj) * ((t_adj * 2.0 - s) * (2.0 * PI / p)).sin();
    result.clamp(0.0, 1.0)
}

/// Elastic 缓入——从静止弹入。
pub fn ease_in_elastic(t: f32) -> f32 {
    if t == 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let c4 = (2.0 * PI) / 3.0;
    let result = -(2_f32.powf(10.0 * t - 10.0) * ((t * 10.0 - 10.75) * c4).sin());
    result.clamp(0.0, 1.0)
}

/// Elastic 缓出——末尾带弹性振荡。
pub fn ease_out_elastic(t: f32) -> f32 {
    if t == 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let c4 = (2.0 * PI) / 3.0;
    let result = 2_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0;
    result.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证所有缓动函数在定义域内输出稳定。
    #[test]
    fn test_easings_clamped() {
        for easing in [
            ease_out_back,
            ease_in_out_back,
            elastic,
            ease_in_elastic,
            ease_out_elastic,
        ] {
            assert_eq!(easing(0.0), 0.0);
            assert_eq!(easing(1.0), 1.0);
            for i in 1..=100 {
                let t = i as f32 / 100.0;
                let v = easing(t);
                assert!((0.0..=1.0).contains(&v), "easing({t}) = {v} out of [0,1]");
            }
        }
    }
}
