//! 动画子模块：值驱动（弹簧）原语 + 上层动画 DSL + 核心动画桥接。
//!
//! 与核心动画的关系（见 `docs/ui-crate-plan.md` §4）：
//! - 核心 `rgpui::Animation` 是**时间驱动**（0~1 进度 delta），本模块不复用其内部，直接在其之上构建。
//! - `spring` 提供**值驱动**的弹簧物理（刚度/阻尼/质量），是核心没有的唯一新原语。
//! - `bridge` 提供弹簧与核心动画的桥接（`delta * duration` 换算为 dt）。
//! - `animate` 提供 `AnimationPreset` / `KeyframeAnimation` / `StaggerConfig` 等上层便捷构造器。

pub mod animate;
pub mod bridge;
pub mod durations;
pub mod easing;
pub mod helpers;
pub mod spring;

pub use animate::*;
pub use bridge::*;
pub use durations::*;
pub use easing::*;
pub use helpers::*;
pub use spring::*;
