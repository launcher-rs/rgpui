//! macOS 系统信息查询：操作系统版本、空闲时长、网络状态
//!
//! - 版本号复用 `MacPlatform::os_version()`（`NSProcessInfo`），与 URL scheme
//!   注册的版本判断保持同一来源；
//! - 空闲时长走 CoreGraphics 的 `CGEventSourceSecondsSinceLastEventType`，
//!   统计的是 HID 系统状态，覆盖键盘、鼠标、触控板；
//! - 网络状态用 `getifaddrs` 找「已启用且有 IP 地址」的非回环接口。

use rgpui::{NetworkStatus, OsInfo};
use std::time::Duration;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
}

/// `kCGEventSourceStateHIDSystemState`
const CG_EVENT_SOURCE_STATE_HID_SYSTEM_STATE: i32 = 1;
/// `kCGAnyInputEventType`
const CG_ANY_INPUT_EVENT_TYPE: u32 = 0xFFFF_FFFF;

/// 返回操作系统名称与版本号。
pub(crate) fn os_info() -> OsInfo {
    OsInfo {
        name: "macOS".to_string(),
        version: crate::MacPlatform::os_version().to_string(),
    }
}

/// 返回系统空闲时长（自上次 HID 输入以来）。
pub(crate) fn system_idle_time() -> Option<Duration> {
    let seconds = unsafe {
        CGEventSourceSecondsSinceLastEventType(
            CG_EVENT_SOURCE_STATE_HID_SYSTEM_STATE,
            CG_ANY_INPUT_EVENT_TYPE,
        )
    };
    // 调用失败时返回 0，负数/NaN 视为不可用，避免把失败误报成「刚刚有输入」
    (seconds.is_finite() && seconds >= 0.0).then(|| Duration::from_secs_f64(seconds))
}

/// 返回当前网络状态。
///
/// `getifaddrs` 只能回答「有没有启用中的非回环接口拿到地址」，没有强制门户
/// 之类的探测能力，因此只映射 `Connected` / `Disconnected`。
pub(crate) fn network_status() -> NetworkStatus {
    if any_interface_up() {
        NetworkStatus::Connected
    } else {
        NetworkStatus::Disconnected
    }
}

/// 是否存在已启用、非回环、带 IPv4/IPv6 地址的网络接口。
fn any_interface_up() -> bool {
    unsafe {
        let mut interfaces: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut interfaces) != 0 {
            return false;
        }

        let mut up = false;
        let mut current = interfaces;
        while !current.is_null() {
            let interface = &*current;
            let flags = interface.ifa_flags;
            let wanted = (libc::IFF_UP | libc::IFF_LOOPBACK) as u32;
            if !interface.ifa_addr.is_null() && flags & wanted == libc::IFF_UP as u32 {
                let family = (*interface.ifa_addr).sa_family as i32;
                if family == libc::AF_INET || family == libc::AF_INET6 {
                    up = true;
                    break;
                }
            }
            current = interface.ifa_next;
        }

        libc::freeifaddrs(interfaces);
        up
    }
}
