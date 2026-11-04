//! Windows 系统信息查询：操作系统版本、空闲时长、网络状态
//!
//! 这三项能力此前都落在 `Platform` 的默认实现上（返回空版本号、`None`、
//! 无条件「已连接」），应用层拿不到真实结果又无从察觉。这里统一走 Win32：
//! `RtlGetVersion`（避免 `GetVersionEx` 的清单依赖）、`GetLastInputInfo`、
//! `InternetGetConnectedState`。

use rgpui::{NetworkStatus, OsInfo};
use std::time::Duration;
use windows::Win32::{
    Networking::WinInet::{INTERNET_CONNECTION, InternetGetConnectedState},
    System::SystemInformation::{GetTickCount64, OSVERSIONINFOW},
    UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
};

/// 返回操作系统名称与版本号。
///
/// 名称按构建号区分 Windows 10 / 11（22000 是 11 的分界）；版本号保留
/// `major.minor.build`，排查问题时比营销名称更有用。
pub(crate) fn os_info() -> OsInfo {
    let mut version = OSVERSIONINFOW {
        dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };

    let status = unsafe { windows::Wdk::System::SystemServices::RtlGetVersion(&mut version) };
    // NTSTATUS 以最高位表示失败，0 是 STATUS_SUCCESS
    if status.0 < 0 {
        log::warn!("RtlGetVersion 失败: {:?}", status);
        return OsInfo {
            name: "Windows".to_string(),
            version: String::new(),
        };
    }

    let name = if version.dwBuildNumber >= 22000 {
        "Windows 11"
    } else {
        "Windows 10"
    };

    OsInfo {
        name: name.to_string(),
        version: format!(
            "{}.{}.{}",
            version.dwMajorVersion, version.dwMinorVersion, version.dwBuildNumber
        ),
    }
}

/// 返回系统空闲时长（自上次键盘/鼠标输入以来）。
///
/// `GetLastInputInfo` 的时间戳是 32 位 tick，会和 `GetTickCount64` 一起回绕，
/// 所以用低位相减而不是直接换算绝对时间。
pub(crate) fn system_idle_time() -> Option<Duration> {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };

    if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        return None;
    }

    let elapsed = (unsafe { GetTickCount64() } as u32).wrapping_sub(info.dwTime);
    Some(Duration::from_millis(u64::from(elapsed)))
}

/// 返回当前网络状态。
///
/// WinINet 只回答「有没有可用的拨号/LAN/VPN 连接」，不区分强制门户或后端探测
/// 结果，因此只映射到 `Connected` / `Disconnected`；拿不到时按断开处理。
pub(crate) fn network_status() -> NetworkStatus {
    let mut flags = INTERNET_CONNECTION::default();
    match unsafe { InternetGetConnectedState(&mut flags, None) } {
        Ok(()) => NetworkStatus::Connected,
        Err(_) => NetworkStatus::Disconnected,
    }
}
