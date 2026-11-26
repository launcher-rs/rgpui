mod auto_launch;
mod dispatcher;
mod focused_window;
mod headless;
mod keyboard;
mod notifications;
mod permissions;
mod platform;
// 电源能力走 login1（ashpd/zbus），而 ashpd 只在带窗口后端的 feature 下才启用
#[cfg(any(feature = "wayland", feature = "x11"))]
mod power;
mod system_info;
#[cfg(any(feature = "wayland", feature = "x11"))]
mod text_system;
#[cfg(any(feature = "wayland", feature = "x11"))]
mod tray;
#[cfg(any(feature = "wayland", feature = "x11"))]
mod tray_sni;
mod url_scheme;
#[cfg(feature = "wayland")]
mod wayland;
#[cfg(feature = "x11")]
mod x11;

#[cfg(any(feature = "wayland", feature = "x11"))]
mod xdg_desktop_portal;

pub use dispatcher::*;
pub(crate) use headless::*;
pub(crate) use keyboard::*;
pub(crate) use notifications::*;
pub(crate) use permissions::*;
pub use platform::*;
#[cfg(any(feature = "wayland", feature = "x11"))]
pub(crate) use text_system::*;
#[cfg(any(feature = "wayland", feature = "x11"))]
pub(crate) use tray::*;
#[cfg(feature = "wayland")]
pub(crate) use wayland::*;
#[cfg(feature = "x11")]
pub(crate) use x11::*;

use std::rc::Rc;

/// 组装一个平台实例。`LinuxPlatform` 对后端类型是泛型的，三个后端在这里统一擦除成
/// `dyn Platform`，避免每处分支重复写 notifications / permissions 三个字段。
fn make_platform<P: LinuxClient + 'static>(inner: P) -> Rc<dyn rgpui::Platform> {
    Rc::new(LinuxPlatform {
        inner,
        notifications: LinuxNotifications::new(),
        permissions: LinuxPermissions::new(),
    })
}

/// Returns the default platform implementation for the current OS.
///
/// 会话声明的后端可能没有编译进来（feature 在 `rgpui` 与 `rgpui-linux` 两侧不一致），
/// 或初始化失败。这两种情况都逐级回退到可用后端，最后落到无头后端并给出明确日志，
/// 不再用 `unreachable!` / `unwrap` 把编译期配置问题变成启动期崩溃。
pub fn current_platform(headless: bool) -> Rc<dyn rgpui::Platform> {
    if headless {
        return make_platform(HeadlessClient::new());
    }

    let compositor = rgpui::guess_compositor();

    #[cfg(feature = "wayland")]
    if compositor == "Wayland" {
        match WaylandClient::new() {
            Ok(client) => return make_platform(client),
            Err(error) => log::warn!("Wayland 后端初始化失败，尝试其它后端：{error:#}"),
        }
    }

    // X11 是首选（会话就是 X11），也是 Wayland 后端不可用时的兜底——多数
    // Wayland 会话都带 XWayland，能拿它把窗口显示出来，而不是直接崩溃。
    #[cfg(feature = "x11")]
    if compositor != "Headless" {
        match X11Client::new() {
            Ok(client) => {
                if compositor != "X11" {
                    log::warn!("会话 {compositor:?} 的图形后端不可用，已回退到 X11");
                }
                return make_platform(client);
            }
            Err(error) => log::warn!("X11 后端初始化失败：{error:#}"),
        }
    }

    if compositor != "Headless" {
        log::warn!("没有可用的图形后端（会话={compositor:?}），使用无头后端；窗口将不会显示");
    }
    make_platform(HeadlessClient::new())
}
