//! GPUI 预导入集合：常用 trait 与类型。推荐在应用中使用
//! `use rgpui::prelude::*` 一次性导入，避免逐个手动引入。
//!
//! 说明：
//! - 前部为核心框架的基础 trait 与类型。
//! - 后部为扩展组件库（原 `rgpui-ui` 并入核心后的 `components` 子模块）
//!   及动画/手势/滚动物理（`animation`/`mouse_gestures`/`scroll_physics`），
//!   与基础组件的 `use rgpui::*` 使用体验对齐。
//! - `components` 采用 glob 重导出（与核心根对 `elements` 的处理一致），
//!   图表（`charts`）、特效（`effects`）、二维码（`qr-code`）组件受 feature 门控。

pub use crate::{
    ActiveTheme, AppContext as _, BorrowAppContext, Context, Element, ElementExt,
    InteractiveElement, InteractiveElementExt, IntoElement, ParentElement, Refineable, Render,
    RenderOnce, Selectable, Sizable, StatefulInteractiveElement, Styled, StyledExt, StyledImage,
    TaskExt as _, VisualContext, util::FluentBuilder,
};

// 扩展组件库（原 rgpui-ui 并入）。`charts`/`effects`/`qr-code` 受 feature 门控，
// 未开启对应 feature 时相关组件不会出现在此 glob 中。
pub use crate::components::*;

// 动画子模块：仅暴露类型与便捷构造器，不 glob 时长常量（如 `NORMAL`/`FAST`），
// 避免与使用者自身命名冲突；需时长常量请显式 `use rgpui::animation::durations::*`。
pub use crate::animation::{
    AnimationPreset, AnimationRepeat, KeyframeAnimation, Spring, SpringBridge, StaggerConfig,
    bounce_in, fade_in, fade_out, scale_in, slide_down, slide_in_left, slide_in_right, slide_up,
};

// 鼠标手势识别与滚动物理。
pub use crate::mouse_gestures::{
    GestureDetector, GestureEvent, LongPressGesture, PanGesture, SwipeDirection, SwipeGesture,
    TapGesture,
};
pub use crate::scroll_physics::ScrollPhysics;
