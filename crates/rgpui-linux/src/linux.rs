//! Linux 平台模块，整合 Wayland、X11 和无头（headless）后端。
//!
//! 本模块根据运行时检测的显示服务器环境或特性配置，自动选择
//! 合适的平台客户端实现。

use std::cell::RefCell;

use self::global_hotkey::LinuxGlobalHotkey;
use self::notifications::LinuxNotifications;
use self::permissions::LinuxPermissions;

mod dispatcher;
mod headless;
mod keyboard;
mod platform;
#[cfg(any(feature = "wayland", feature = "x11"))]
mod text_system;
#[cfg(feature = "wayland")]
mod wayland;
#[cfg(feature = "x11")]
mod x11;

#[cfg(any(feature = "wayland", feature = "x11"))]
mod xdg_desktop_portal;

// 新功能模块
mod auto_launch;
mod focused_window;
mod global_hotkey;
mod notifications;
mod permissions;

pub use dispatcher::*;
pub(crate) use headless::*;
pub(crate) use keyboard::*;
pub(crate) use platform::*;
#[cfg(any(feature = "wayland", feature = "x11"))]
pub(crate) use text_system::*;
#[cfg(feature = "wayland")]
pub(crate) use wayland::*;
#[cfg(feature = "x11")]
pub(crate) use x11::*;

use std::rc::Rc;

/// 返回当前操作系统的默认平台实现。
///
/// 根据 `headless` 参数和运行时检测的显示服务器类型，
/// 返回对应的平台客户端（Wayland、X11 或 Headless）。
///
/// # 参数
///
/// * `headless` - 若为 true，则使用无头模式（无图形界面）
pub fn current_platform(headless: bool) -> Rc<dyn rgpui::Platform> {
    #[cfg(feature = "x11")]
    use anyhow::Context as _;

    fn create_platform<P: LinuxClient + 'static>(inner: P) -> Rc<dyn rgpui::Platform> {
        Rc::new(LinuxPlatform {
            inner,
            global_hotkey: RefCell::new(LinuxGlobalHotkey::new()),
            notifications: LinuxNotifications::new(),
            permissions: LinuxPermissions::new(),
        })
    }

    if headless {
        return create_platform(HeadlessClient::new());
    }

    match rgpui::guess_compositor() {
        #[cfg(feature = "wayland")]
        "Wayland" => create_platform(WaylandClient::new()),

        #[cfg(feature = "x11")]
        "X11" => create_platform(
            X11Client::new()
                .context("Failed to initialize X11 client.")
                .unwrap(),
        ),

        "Headless" => create_platform(HeadlessClient::new()),
        _ => unreachable!(),
    }
}
