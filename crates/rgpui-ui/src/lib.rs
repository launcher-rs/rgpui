//! # rgpui-ui
//!
//! rgpui 扩展 UI 组件库，只收核心没有的组件，基于 rgpui 核心构建。
//! 定位（见 `docs/ui-crate-plan.md`）：
//! - 动画组件/特效（核心提供时间驱动动画原语，本库提供值驱动弹簧与动画组件）
//! - 高级输入、布局、通知/命令等核心没有的组件
//! - 手势识别、滚动惯性等辅助能力
//!
//! 核心已有的基础组件（elements/、form/、input_ui/、menu/、dialog/、list/、
//! table/、tabs/、title_bar/）一律不在此重复实现，直接使用核心。

pub mod animation;
pub mod components;
pub mod gestures;
pub mod prelude;
pub mod scroll_physics;

pub use animation::*;
pub use components::*;
pub use gestures::*;
pub use prelude::*;
pub use scroll_physics::*;
