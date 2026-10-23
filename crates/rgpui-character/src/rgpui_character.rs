//! rgpui 桌宠与 UI 角色运行时系统。

// 生产代码禁用 `unwrap` / `expect` / `panic!`（与 rgpui 核心同一口径）；
// 测试代码由根目录 `clippy.toml` 的 `allow-*-in-tests` 放行。
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod animation;
pub mod asset;
pub mod behavior;
pub mod core;
pub mod physics;
pub mod render;
pub mod runtime;

// 动画模块
pub use animation::{AnimationClip, AnimationPlayer, AnimationState};

// 资源模块
pub use asset::{AssetError, AssetManager};

// 行为模块
pub use behavior::{
    Behavior, BehaviorAction, BehaviorContext, BehaviorState, ConstantMoveBehavior, IdleBehavior,
};

// 核心类型
pub use core::{Character, CharacterState, Rect, TextureId, Vec2};

// 物理模块
pub use physics::{PhysicsConfig, update_physics};

// 渲染模块
pub use render::{RenderBackend, RenderCommand};

// 运行时模块
pub use runtime::{CharacterEvent, CharacterRuntime};
