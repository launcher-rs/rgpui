//! Spring 与核心 `Animation` 的桥接。
//!
//! 核心 `rgpui::Animation` 是**进度驱动**（缓动后 delta 0~1），而 `Spring` 是
//! **dt 驱动**（`tick(dt)`）。本模块提供将核心动画进度换算为弹簧 dt 的适配器：
//! `dt = delta * duration`（delta 已含缓动）。动画组件可用它在
//! `AnimationExt::with_animation` 的 animator 里驱动弹簧。

use std::time::Duration;

use rgpui::Animation;

use super::spring::Spring;

/// Spring 与核心 `Animation` 的桥接适配器。
///
/// 持有一个弹簧与目标时长，把核心动画的进度（0~1，已缓动）换算成真实时间
/// （`delta * duration`）并推进弹簧。适用于"弹簧渲染进度由核心动画驱动"的场景。
pub struct SpringBridge {
    /// 被驱动的弹簧。
    spring: Spring,
    /// 核心动画时长（用于把进度换算为 dt）。
    duration: Duration,
    /// 上一帧的进度（用于计算帧间增量）。
    last_delta: f32,
    /// 累计推进的时间。
    elapsed: f32,
}

impl SpringBridge {
    /// 以弹簧与动画时长创建桥接器。
    pub fn new(spring: Spring, duration: Duration) -> Self {
        Self {
            spring,
            duration,
            last_delta: 0.0,
            elapsed: 0.0,
        }
    }

    /// 从核心动画便捷创建桥接器（时长取自动画）。
    pub fn from_animation(spring: Spring, animation: &Animation) -> Self {
        Self::new(spring, animation.duration)
    }

    /// 推进弹簧：`delta` 为核心动画当前已缓动进度（0~1，累计值）。
    ///
    /// 换算规则：`dt = (delta - 上一帧delta) * duration`，即按帧间进度增量换算真实时间，
    /// 使得整个动画周期推进的总时间恰为 `duration`。返回弹簧是否仍在运动中。
    pub fn advance(&mut self, delta: f32) -> bool {
        let delta = delta.clamp(0.0, 1.0);
        let dt = (delta - self.last_delta) * self.duration.as_secs_f32();
        self.last_delta = delta;
        self.elapsed += dt;
        self.spring.tick(dt)
    }

    /// 返回被驱动的弹簧（可变引用）。
    pub fn spring(&mut self) -> &mut Spring {
        &mut self.spring
    }

    /// 返回累计推进的真实时间。
    pub fn elapsed(&self) -> f32 {
        self.elapsed
    }

    /// 判断弹簧是否已静止。
    pub fn is_at_rest(&self) -> bool {
        self.spring.is_at_rest()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::spring::Spring;
    use rgpui::linear;

    /// 验证桥接：核心动画的进度驱动弹簧；动画结束后弹簧按真实帧间隔自由运行直至静止。
    ///
    /// 模拟固定 60fps（约 16.7ms/帧），线性缓动下 delta == 累计时间 / 时长。
    #[test]
    fn test_bridge_spring_reaches_target_with_animation() {
        let duration = Duration::from_millis(500);
        let animation = Animation::new(duration).with_easing(linear);
        let mut bridge =
            SpringBridge::from_animation(Spring::gentle().with_target(100.0), &animation);

        let frame_dt = 1.0 / 60.0;
        let mut elapsed = 0.0;
        // 动画期间：进度随累计时间线性增长，驱动弹簧
        while elapsed < duration.as_secs_f32() {
            let delta = elapsed / duration.as_secs_f32();
            bridge.advance(delta);
            elapsed += frame_dt;
        }
        // 最后一帧补到完成进度
        bridge.advance(1.0);

        // 总推进时间恰为一个动画时长
        assert!((bridge.elapsed() - duration.as_secs_f32()).abs() < 1e-3);

        // 动画结束后弹簧可能仍在回弹（弹簧可过冲），按真实帧间隔继续自由运行至静止
        let mut frames = 0;
        while !bridge.is_at_rest() && frames < 500 {
            bridge.spring().tick(frame_dt);
            frames += 1;
        }

        assert!(
            bridge.is_at_rest(),
            "spring not at rest after {frames} frames"
        );
        assert!((bridge.spring().position - 100.0).abs() < 0.01);
    }

    /// 验证进度换算：从 0 到 0.5 推进的真实时间应为时长的一半。
    #[test]
    fn test_bridge_delta_maps_to_dt() {
        let duration = Duration::from_millis(400);
        let mut bridge = SpringBridge::new(Spring::gentle().with_target(10.0), duration);

        bridge.advance(0.0);
        assert_eq!(bridge.elapsed(), 0.0);

        bridge.advance(0.5);
        assert!((bridge.elapsed() - 0.2).abs() < 1e-6);

        bridge.advance(1.0);
        assert!((bridge.elapsed() - 0.4).abs() < 1e-6);
    }
}
