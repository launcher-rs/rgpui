//! 扩展组件库（核心没有的组件 + 动画组件/特效）。
//!
//! 原独立 `rgpui-ui` crate 并入核心后的子模块（2026-08-19），
//! 与 `menu`、`dialog` 等子系统并列。部分组件按 feature 门控：
//! - `charts`：图表组件（11 个，自成体系）
//! - `effects`：纯装饰特效（aurora/confetti/particle_emitter/ripple/shimmer/marquee/pulse_indicator）
//! - `qr-code`：二维码组件（引入 `qrcode` 依赖）
//! 其余约 30 个功能性组件默认启用。

use crate::App;

pub mod animated_collapsible;
pub mod animated_counter;
pub mod animated_list;
pub mod animated_presence;
pub mod animated_progress;
pub mod animated_switch;
pub mod animated_text;
pub mod app_menu;
#[cfg(feature = "effects")]
pub mod aurora;
pub mod bottom_sheet;
pub mod canvas_component;
pub mod command_palette;
#[cfg(feature = "effects")]
pub mod confetti;
pub mod countdown;
pub mod drag_drop;
pub mod drawer_navigation;
pub mod empty_state;
pub mod expandable_card;
pub mod hotkey_input;
pub mod image_viewer;
pub mod infinite_scroll;
pub mod inline_edit;
pub mod layout_transition;
#[cfg(feature = "effects")]
pub mod marquee;
pub mod navigation_menu;
pub mod notification_center;
pub mod number_ticker;
pub mod otp_input;
#[cfg(feature = "effects")]
pub mod particle_emitter;
#[cfg(feature = "effects")]
pub mod pulse_indicator;
#[cfg(feature = "qr-code")]
pub mod qr_code;
pub mod resizable;
#[cfg(feature = "effects")]
pub mod ripple;
pub mod segmented_nav;
#[cfg(feature = "effects")]
pub mod shimmer;
pub mod sortable_list;
pub mod sparkline;
pub mod split_pane;
pub mod spotlight;
pub mod svg_renderer;
pub mod tag_input;
pub mod text_reveal;
pub mod type_writer;
pub mod view_router;
pub mod waveform;

#[cfg(feature = "charts")]
pub mod charts;

pub use animated_collapsible::*;
pub use animated_counter::*;
pub use animated_list::*;
pub use animated_presence::*;
pub use animated_progress::*;
pub use animated_switch::*;
pub use animated_text::*;
pub use app_menu::*;
#[cfg(feature = "effects")]
pub use aurora::*;
pub use bottom_sheet::*;
pub use canvas_component::*;
pub use command_palette::*;
#[cfg(feature = "effects")]
pub use confetti::*;
pub use countdown::*;
pub use drag_drop::*;
pub use drawer_navigation::*;
pub use empty_state::*;
pub use expandable_card::*;
pub use hotkey_input::*;
pub use image_viewer::*;
pub use infinite_scroll::*;
pub use inline_edit::{
    Cancel, InlineEdit, InlineEditBlurBehavior, InlineEditState, InlineEditTrigger, Save,
};
pub use layout_transition::*;
#[cfg(feature = "effects")]
pub use marquee::*;
pub use navigation_menu::*;
pub use notification_center::*;
pub use number_ticker::*;
pub use otp_input::{
    OTPBackspace, OTPDelete, OTPEnd, OTPEscape, OTPHome, OTPInput, OTPInputEvent, OTPInputSize,
    OTPInputState, OTPLeft, OTPPaste, OTPRight, OTPState,
};
#[cfg(feature = "effects")]
pub use particle_emitter::*;
#[cfg(feature = "effects")]
pub use pulse_indicator::*;
#[cfg(feature = "qr-code")]
pub use qr_code::*;
pub use resizable::*;
#[cfg(feature = "effects")]
pub use ripple::*;
pub use segmented_nav::*;
#[cfg(feature = "effects")]
pub use shimmer::*;
pub use sortable_list::*;
pub use sparkline::*;
pub use split_pane::*;
pub use spotlight::*;
pub use svg_renderer::*;
pub use tag_input::*;
pub use text_reveal::*;
pub use type_writer::*;
pub use view_router::*;
pub use waveform::*;

#[cfg(feature = "charts")]
pub use charts::*;

/// 初始化需要注册快捷键绑定的组件（行内编辑、OTP 输入、图片查看器）。
pub fn init(cx: &mut App) {
    inline_edit::init(cx);
    otp_input::init(cx);
    image_viewer::init_image_viewer(cx);
}
