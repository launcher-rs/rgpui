//! Linux 系统信息与会话空闲时长查询

use std::collections::HashMap;
use std::ffi::CStr;
use std::time::Duration;

use rgpui::OsInfo;

/// `org.freedesktop.ScreenSaver` 的服务名与对象路径
#[cfg(any(feature = "x11", feature = "wayland"))]
const SCREENSAVER_BUS: &str = "org.freedesktop.ScreenSaver";
#[cfg(any(feature = "x11", feature = "wayland"))]
const SCREENSAVER_PATH: &str = "/org/freedesktop/ScreenSaver";

/// 读取操作系统信息
///
/// 发行版名称与版本取自 `/etc/os-release`；该文件缺失或缺字段时回退到
/// `uname(2)` 的内核版本，保证 `version` 不会是空串。
pub(crate) fn os_info() -> OsInfo {
    let release = read_os_release();

    let name = release
        .get("NAME")
        .cloned()
        .unwrap_or_else(|| std::env::consts::OS.to_owned());
    let version = release
        .get("VERSION")
        .or_else(|| release.get("VERSION_ID"))
        .cloned()
        .unwrap_or_else(kernel_release);

    OsInfo { name, version }
}

/// 查询用户会话的空闲时长，取不到（无 session bus、桌面环境未提供接口）时返回 `None`
#[cfg(any(feature = "x11", feature = "wayland"))]
pub(crate) fn system_idle_time() -> Option<Duration> {
    pollster::block_on(idle_time())
}

/// 无窗口后端（headless）时没有会话总线可查，直接返回 `None`
#[cfg(not(any(feature = "x11", feature = "wayland")))]
pub(crate) fn system_idle_time() -> Option<Duration> {
    None
}

/// 依次尝试各桌面环境提供的空闲时长接口
///
/// 三个接口的可用性因桌面而异，本机（Ubuntu 22.04 GNOME）实测：
/// `GetIdleTime` 是 `UnknownMethod`、`GetSessionIdleTime` 是 `NotSupported`，
/// 只有 Mutter 的 `GetIdletime` 能用；而 KDE 的 `kdesktop` 只提供前两者。
#[cfg(any(feature = "x11", feature = "wayland"))]
async fn idle_time() -> Option<Duration> {
    use ashpd::zbus::Connection;

    let conn = Connection::session().await.ok()?;

    if let Some(millis) = call_u32(
        &conn,
        SCREENSAVER_BUS,
        SCREENSAVER_PATH,
        SCREENSAVER_BUS,
        "GetIdleTime",
    )
    .await
    {
        return Some(Duration::from_millis(millis as u64));
    }
    if let Some(seconds) = call_u32(
        &conn,
        SCREENSAVER_BUS,
        SCREENSAVER_PATH,
        SCREENSAVER_BUS,
        "GetSessionIdleTime",
    )
    .await
    {
        return Some(Duration::from_secs(seconds as u64));
    }

    let reply = conn
        .call_method(
            Some("org.gnome.Mutter.IdleMonitor"),
            "/org/gnome/Mutter/IdleMonitor/Core",
            Some("org.gnome.Mutter.IdleMonitor"),
            "GetIdletime",
            &(),
        )
        .await
        .ok()?;
    let micros = reply.body().deserialize::<(u64,)>().ok()?;

    Some(Duration::from_micros(micros.0))
}

/// 调用一个无参数、返回单个 `u32` 的 D-Bus 方法
#[cfg(any(feature = "x11", feature = "wayland"))]
async fn call_u32(
    conn: &ashpd::zbus::Connection,
    bus: &str,
    path: &str,
    interface: &str,
    method: &str,
) -> Option<u32> {
    let reply = conn
        .call_method(Some(bus), path, Some(interface), method, &())
        .await
        .ok()?;

    reply.body().deserialize::<(u32,)>().ok().map(|v| v.0)
}

/// 读取 `/etc/os-release`
fn read_os_release() -> HashMap<String, String> {
    let Ok(content) = std::fs::read_to_string("/etc/os-release") else {
        return HashMap::new();
    };

    parse_os_release(&content)
}

/// 解析 os-release 文本，按 freedesktop 规则去掉值两侧的引号
fn parse_os_release(content: &str) -> HashMap<String, String> {
    content
        .lines()
        .filter_map(|line| {
            let (key, value) = line.trim_start().split_once('=')?;
            if key.is_empty() || key.starts_with('#') {
                return None;
            }
            let value = value.trim();
            let value = value
                .strip_prefix(['"', '\''])
                .unwrap_or(value)
                .strip_suffix(['"', '\''])
                .unwrap_or(value);
            Some((key.to_owned(), value.to_owned()))
        })
        .collect()
}

/// 取 `uname(2)` 报告的内核版本号，失败时返回空串
fn kernel_release() -> String {
    let mut uts: libc::utsname = unsafe { std::mem::zeroed() };
    if unsafe { libc::uname(&mut uts) } != 0 {
        return String::new();
    }

    let release = unsafe { CStr::from_ptr(uts.release.as_ptr()) };
    release.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 校验 os-release 文本解析：去引号、跳过注释与空键
    #[test]
    fn parses_os_release_entries() {
        let parsed = parse_os_release("# comment\nNAME=\"Ubuntu\"\nVERSION_ID='22.04'\nEMPTY=\n");
        assert_eq!(parsed.get("NAME").map(String::as_str), Some("Ubuntu"));
        assert_eq!(parsed.get("VERSION_ID").map(String::as_str), Some("22.04"));
        assert_eq!(parsed.get("EMPTY").map(String::as_str), Some(""));
        assert!(!parsed.contains_key("# comment"));
    }
}
