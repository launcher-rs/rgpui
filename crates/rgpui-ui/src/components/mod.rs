//! rgpui-ui 组件库（核心没有的组件 + 动画组件/特效）。

use rgpui::App;

pub mod animated_collapsible;
pub mod animated_counter;
pub mod animated_list;
pub mod animated_presence;
pub mod animated_switch;
pub mod animated_text;
pub mod app_menu;
pub mod aurora;
pub mod bottom_sheet;
pub mod canvas_component;
pub mod command_palette;
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
pub mod marquee;
pub mod navigation_menu;
pub mod notification_center;
pub mod number_ticker;
pub mod otp_input;
pub mod particle_emitter;
pub mod pulse_indicator;
pub mod qr_code;
pub mod resizable;
pub mod ripple;
pub mod segmented_nav;
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
pub use animated_switch::*;
pub use animated_text::*;
pub use app_menu::*;
pub use aurora::*;
pub use bottom_sheet::*;
pub use canvas_component::*;
pub use command_palette::*;
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
pub use marquee::*;
pub use navigation_menu::*;
pub use notification_center::*;
pub use number_ticker::*;
pub use otp_input::{
    OTPBackspace, OTPDelete, OTPEnd, OTPEscape, OTPHome, OTPInput, OTPInputEvent, OTPInputSize,
    OTPInputState, OTPLeft, OTPPaste, OTPRight, OTPState,
};
pub use particle_emitter::*;
pub use pulse_indicator::*;
pub use qr_code::*;
pub use resizable::*;
pub use ripple::*;
pub use segmented_nav::*;
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
