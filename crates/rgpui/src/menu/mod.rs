use crate::App;

mod actions;
mod app_menu_bar;
mod context_menu;
mod dropdown_menu;
mod global_state;
mod hover_card;
mod menu_item;
mod notification;
mod popover;
mod popup_menu;

pub use actions::*;
pub use app_menu_bar::AppMenuBar;
pub use context_menu::{ContextMenu, ContextMenuExt, ContextMenuState};
pub use dropdown_menu::DropdownMenu;
pub use global_state::GlobalState;
pub use hover_card::{HoverCard, HoverCardState};
pub use notification::{Notification, NotificationId, NotificationList, NotificationType};
pub use popover::Popover;
pub use popup_menu::{PopupMenu, PopupMenuItem};

/// 初始化菜单相关的全局状态与快捷键绑定
pub fn init(cx: &mut App) {
    cx.set_global(GlobalState::new());
    popup_menu::init(cx);
    app_menu_bar::init(cx);
    popover::init(cx);
}
