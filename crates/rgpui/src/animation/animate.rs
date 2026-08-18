//! 上层动画便捷构造器（基于核心 `crate::Animation`）。
//!
//! 提供 `AnimationPreset`（时长/缓动/延迟）、`KeyframeAnimation`（关键帧插值）、
//! `StaggerConfig`（子元素交错延迟）等便捷 DSL，便于动画组件复用常见配置。
//!
//! 注意：核心 `rgpui` 已有 `transition::Transition`（元素过渡），本模块不复用/不重名导出，
//! 元素级过渡请直接用核心的 `Transition`。

use std::time::Duration;

use crate::Animation;

use crate::animation::easing::{ease_in_elastic, ease_out_back};

/// 动画预设配置（时长 + 缓动 + 延迟）。
#[derive(Clone)]
pub struct AnimationPreset {
    /// 动画时长。
    duration: Duration,
    /// 缓动函数。
    easing: fn(f32) -> f32,
    /// 延迟时间。
    delay: Duration,
}

impl AnimationPreset {
    /// 以指定时长和缓动函数创建预设。
    pub fn new(duration: Duration, easing: fn(f32) -> f32) -> Self {
        Self {
            duration,
            easing,
            delay: Duration::ZERO,
        }
    }

    /// 设置时长。
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// 设置缓动函数。
    pub fn easing(mut self, easing: fn(f32) -> f32) -> Self {
        self.easing = easing;
        self
    }

    /// 设置延迟时间。
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// 设置时长（与 `duration` 等价的别名，兼容链式调用习惯）。
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// 设置缓动函数（与 `easing` 等价的别名）。
    pub fn with_easing(mut self, easing: fn(f32) -> f32) -> Self {
        self.easing = easing;
        self
    }

    /// 设置延迟时间（与 `delay` 等价的别名）。
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// 返回动画时长。
    pub fn get_duration(&self) -> Duration {
        self.duration
    }

    /// 返回缓动函数。
    pub fn get_easing(&self) -> fn(f32) -> f32 {
        self.easing
    }

    /// 返回延迟时间。
    pub fn get_delay(&self) -> Duration {
        self.delay
    }

    /// 转换为核心 `Animation`。
    ///
    /// 注意：核心 `Animation` 无延迟概念，此处转换会丢弃 `delay`（延迟请用
    /// `StaggerConfig` 或外部定时逻辑处理）。
    pub fn to_animation(&self) -> Animation {
        Animation::new(self.duration).with_easing(self.easing)
    }
}

/// 淡入预设。
pub fn fade_in() -> AnimationPreset {
    AnimationPreset::new(Duration::from_millis(200), crate::ease_out_cubic)
}

/// 淡出预设。
pub fn fade_out() -> AnimationPreset {
    AnimationPreset::new(Duration::from_millis(200), crate::ease_in_cubic)
}

/// 上滑预设。
pub fn slide_up() -> AnimationPreset {
    AnimationPreset::new(Duration::from_millis(250), crate::ease_out_cubic)
}

/// 下滑预设。
pub fn slide_down() -> AnimationPreset {
    AnimationPreset::new(Duration::from_millis(250), crate::ease_out_cubic)
}

/// 缩放进入预设（带回弹过冲）。
pub fn scale_in() -> AnimationPreset {
    AnimationPreset::new(Duration::from_millis(200), ease_out_back)
}

/// 弹跳进入预设（弹性振荡）。
pub fn bounce_in() -> AnimationPreset {
    AnimationPreset::new(Duration::from_millis(400), ease_in_elastic)
}

/// 从左滑入预设。
pub fn slide_in_left() -> AnimationPreset {
    AnimationPreset::new(Duration::from_millis(250), crate::ease_out_cubic)
}

/// 从右滑入预设。
pub fn slide_in_right() -> AnimationPreset {
    AnimationPreset::new(Duration::from_millis(250), crate::ease_out_cubic)
}

/// 动画重复方式。
#[derive(Clone, Debug, PartialEq)]
pub enum AnimationRepeat {
    /// 仅播放一次。
    Once,
    /// 播放指定次数。
    Count(u32),
    /// 无限循环。
    Infinite,
}

/// 关键帧动画：在多个时间点（百分比, 值）之间做插值。
#[derive(Clone, Debug)]
pub struct KeyframeAnimation {
    /// 动画标识。
    id: String,
    /// 关键帧列表（百分比 0~1, 值）。
    keyframes: Vec<(f32, f32)>,
    /// 动画时长。
    duration: Duration,
    /// 重复方式。
    repeat: AnimationRepeat,
    /// 缓动函数（作用于整体进度）。
    easing: fn(f32) -> f32,
}

impl KeyframeAnimation {
    /// 创建关键帧动画，默认起点 (0, 0) 与终点 (1, 1)。
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            keyframes: vec![(0.0, 0.0), (1.0, 1.0)],
            duration: Duration::from_millis(300),
            repeat: AnimationRepeat::Once,
            easing: crate::linear,
        }
    }

    /// 添加关键帧（百分比会被钳制到 [0, 1] 并按百分比排序）。
    pub fn at(mut self, pct: f32, value: f32) -> Self {
        let pct = pct.clamp(0.0, 1.0);
        if let Some(pos) = self
            .keyframes
            .iter()
            .position(|(p, _)| (*p - pct).abs() < f32::EPSILON)
        {
            self.keyframes[pos] = (pct, value);
        } else {
            self.keyframes.push((pct, value));
            self.keyframes
                .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        }
        self
    }

    /// 设置时长。
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// 设置重复方式。
    pub fn repeat(mut self, repeat: AnimationRepeat) -> Self {
        self.repeat = repeat;
        self
    }

    /// 设置缓动函数。
    pub fn easing(mut self, easing: fn(f32) -> f32) -> Self {
        self.easing = easing;
        self
    }

    /// 返回动画标识。
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 返回动画时长。
    pub fn get_duration(&self) -> Duration {
        self.duration
    }

    /// 返回重复方式。
    pub fn get_repeat(&self) -> &AnimationRepeat {
        &self.repeat
    }

    /// 按进度（0~1）插值出当前值，自动应用缓动。
    pub fn interpolate(&self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);

        if self.keyframes.len() < 2 {
            return if self.keyframes.is_empty() {
                0.0
            } else {
                self.keyframes[0].1
            };
        }

        let eased = (self.easing)(progress);

        let mut prev = &self.keyframes[0];
        for kf in &self.keyframes[1..] {
            if eased <= kf.0 {
                let range = kf.0 - prev.0;
                if range <= f32::EPSILON {
                    return kf.1;
                }
                let local_t = (eased - prev.0) / range;
                return prev.1 + (kf.1 - prev.1) * local_t;
            }
            prev = kf;
        }

        self.keyframes.last().map(|kf| kf.1).unwrap_or(1.0)
    }

    /// 转换为核心 `Animation`（循环类重复映射为 `repeat()`）。
    pub fn to_animation(&self) -> Animation {
        let anim = Animation::new(self.duration).with_easing(self.easing);
        match &self.repeat {
            AnimationRepeat::Once => anim,
            AnimationRepeat::Count(_) | AnimationRepeat::Infinite => anim.repeat(),
        }
    }
}

/// 子元素交错动画配置：为每个子元素生成递增的延迟。
#[derive(Clone)]
pub struct StaggerConfig {
    /// 每个子元素递增的延迟。
    delay_per_child: Duration,
    /// 基础动画预设。
    preset: AnimationPreset,
}

impl StaggerConfig {
    /// 创建默认交错配置（每个子元素递增 50ms，基础预设为淡入）。
    pub fn new() -> Self {
        Self {
            delay_per_child: Duration::from_millis(50),
            preset: fade_in(),
        }
    }

    /// 设置每个子元素递增的延迟。
    pub fn delay_per_child(mut self, delay: Duration) -> Self {
        self.delay_per_child = delay;
        self
    }

    /// 设置基础动画预设。
    pub fn animation(mut self, preset: AnimationPreset) -> Self {
        self.preset = preset;
        self
    }

    /// 计算第 `index` 个子元素的延迟（基础延迟 + 递增延迟）。
    pub fn delay_for_index(&self, index: usize) -> Duration {
        self.preset.delay + self.delay_per_child * index as u32
    }

    /// 计算第 `index` 个子元素使用的动画预设（已叠加延迟）。
    pub fn preset_for_index(&self, index: usize) -> AnimationPreset {
        let mut preset = self.preset.clone();
        preset.delay = self.delay_for_index(index);
        preset
    }

    /// 返回基础动画预设。
    pub fn get_preset(&self) -> &AnimationPreset {
        &self.preset
    }
}

impl Default for StaggerConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证交错延迟按索引递增（默认预设无基础延迟，故从 0 起）。
    #[test]
    fn test_stagger_delay() {
        let config = StaggerConfig::new().delay_per_child(Duration::from_millis(50));
        assert_eq!(config.delay_for_index(0), Duration::from_millis(0));
        assert_eq!(config.delay_for_index(1), Duration::from_millis(50));
        assert_eq!(config.delay_for_index(3), Duration::from_millis(150));
    }

    /// 验证关键帧插值正确。
    #[test]
    fn test_keyframe_interpolate() {
        let anim = KeyframeAnimation::new("kf")
            .at(0.0, 0.0)
            .at(0.5, 1.0)
            .at(1.0, 0.0)
            .easing(crate::linear);
        assert_eq!(anim.interpolate(0.0), 0.0);
        assert_eq!(anim.interpolate(0.5), 1.0);
        assert_eq!(anim.interpolate(1.0), 0.0);
        assert!((anim.interpolate(0.25) - 0.5).abs() < 1e-6);
    }

    /// 验证预设可转换为核心 Animation。
    #[test]
    fn test_preset_to_animation() {
        let preset = scale_in().with_duration(Duration::from_millis(150));
        let animation = preset.to_animation();
        assert_eq!(animation.duration, Duration::from_millis(150));
    }
}
