mod auto_launch;
mod dispatcher;
mod focused_window;
mod global_hotkey;
mod headless;
mod keyboard;
mod notifications;
mod permissions;
mod platform;
#[cfg(any(feature = "wayland", feature = "x11"))]
mod text_system;
#[cfg(feature = "wayland")]
mod wayland;
#[cfg(feature = "x11")]
mod x11;

#[cfg(any(feature = "wayland", feature = "x11"))]
mod xdg_desktop_portal;

pub(crate) use auto_launch::*;
pub use dispatcher::*;
pub(crate) use focused_window::*;
pub(crate) use global_hotkey::*;
pub(crate) use headless::*;
pub(crate) use keyboard::*;
pub(crate) use notifications::*;
pub(crate) use permissions::*;
pub use platform::*;
#[cfg(any(feature = "wayland", feature = "x11"))]
pub(crate) use text_system::*;
#[cfg(feature = "wayland")]
pub(crate) use wayland::*;
#[cfg(feature = "x11")]
pub(crate) use x11::*;

use std::cell::RefCell;
use std::rc::Rc;

/// Returns the default platform implementation for the current OS.
pub fn current_platform(headless: bool) -> Rc<dyn rgpui::Platform> {
    #[cfg(feature = "x11")]
    use anyhow::Context as _;

    if headless {
        return Rc::new(LinuxPlatform {
            inner: HeadlessClient::new(),
            global_hotkey: RefCell::new(LinuxGlobalHotkey::new()),
            notifications: LinuxNotifications::new(),
            permissions: LinuxPermissions::new(),
        });
    }

    match rgpui::guess_compositor() {
        #[cfg(feature = "wayland")]
        "Wayland" => Rc::new(LinuxPlatform {
            inner: WaylandClient::new(),
            global_hotkey: RefCell::new(LinuxGlobalHotkey::new()),
            notifications: LinuxNotifications::new(),
            permissions: LinuxPermissions::new(),
        }),

        #[cfg(feature = "x11")]
        "X11" => Rc::new(LinuxPlatform {
            inner: X11Client::new()
                .context("Failed to initialize X11 client.")
                .unwrap(),
            global_hotkey: RefCell::new(LinuxGlobalHotkey::new()),
            notifications: LinuxNotifications::new(),
            permissions: LinuxPermissions::new(),
        }),

        "Headless" => Rc::new(LinuxPlatform {
            inner: HeadlessClient::new(),
            global_hotkey: RefCell::new(LinuxGlobalHotkey::new()),
            notifications: LinuxNotifications::new(),
            permissions: LinuxPermissions::new(),
        }),
        _ => unreachable!(
            r#"At least one of the "wayland" or "x11" features must be enabled on rgpui_linux or rgpui_platform."#
        ),
    }
}
