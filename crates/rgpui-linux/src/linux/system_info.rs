//! Linux 系统信息、会话空闲时长与网络状态查询

use std::collections::HashMap;
use std::ffi::CStr;
use std::time::Duration;

use rgpui::{NetworkStatus, OsInfo};

/// `org.freedesktop.ScreenSaver` 的服务名与对象路径
#[cfg(any(feature = "x11", feature = "wayland"))]
const SCREENSAVER_BUS: &str = "org.freedesktop.ScreenSaver";
#[cfg(any(feature = "x11", feature = "wayland"))]
const SCREENSAVER_PATH: &str = "/org/freedesktop/ScreenSaver";

/// 门户网络监控接口的总线名、对象路径与接口名
#[cfg(any(feature = "x11", feature = "wayland"))]
const PORTAL_BUS: &str = "org.freedesktop.portal.Desktop";
#[cfg(any(feature = "x11", feature = "wayland"))]
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
#[cfg(any(feature = "x11", feature = "wayland"))]
const NETWORK_MONITOR: &str = "org.freedesktop.portal.NetworkMonitor";

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

/// 调用一个无参数、返回单个 `bool` 的 D-Bus 方法
#[cfg(any(feature = "x11", feature = "wayland"))]
async fn call_bool(
    conn: &ashpd::zbus::Connection,
    bus: &str,
    path: &str,
    interface: &str,
    method: &str,
) -> Option<bool> {
    let reply = conn
        .call_method(Some(bus), path, Some(interface), method, &())
        .await
        .ok()?;

    reply.body().deserialize::<(bool,)>().ok().map(|v| v.0)
}

/// 查询网络连接状态
///
/// 先问桌面门户：连通性档位由 NetworkManager 之类后端给出，比本地接口状态准。
/// 门户不在（无会话总线、非桌面环境、容器里）时退到 `/sys/class/net` 兜底。
pub(crate) fn network_status() -> NetworkStatus {
    #[cfg(any(feature = "x11", feature = "wayland"))]
    if let Some(status) = pollster::block_on(portal_network_status()) {
        return status;
    }

    sysfs_network_status()
}

/// 门户 `NetworkMonitor` 给出的状态，问不到时返回 `None` 交给兜底路径
#[cfg(any(feature = "x11", feature = "wayland"))]
async fn portal_network_status() -> Option<NetworkStatus> {
    let conn = ashpd::zbus::Connection::session().await.ok()?;

    if !call_bool(
        &conn,
        PORTAL_BUS,
        PORTAL_PATH,
        NETWORK_MONITOR,
        "GetAvailable",
    )
    .await?
    {
        return Some(NetworkStatus::Disconnected);
    }

    // 档位取自 NetworkManager：0 未知、1 无到互联网的线路由、2 强制门户、3 有限连通、4 完整连通
    let connectivity = call_u32(
        &conn,
        PORTAL_BUS,
        PORTAL_PATH,
        NETWORK_MONITOR,
        "GetConnectivity",
    )
    .await?;

    Some(match connectivity {
        // 链路可用但后端没做过连通性探测，没有反证就按已连通处理
        0 | 4 => NetworkStatus::Connected,
        1..=3 => NetworkStatus::ConnectedBelowRequired,
        _ => NetworkStatus::Connected,
    })
}

/// 读 `/sys/class/net` 判断有没有可用的非回环链路
///
/// 只能回答「有没有 UP 的网卡」，够不着连通性，因此拿不到 `ConnectedBelowRequired`。
fn sysfs_network_status() -> NetworkStatus {
    let Ok(interfaces) = std::fs::read_dir("/sys/class/net") else {
        return NetworkStatus::Disconnected;
    };

    for interface in interfaces.flatten() {
        if interface.file_name() == "lo" {
            continue;
        }
        let Ok(operstate) = std::fs::read_to_string(interface.path().join("operstate")) else {
            continue;
        };
        if operstate.trim() == "up" {
            return NetworkStatus::Connected;
        }
    }

    NetworkStatus::Disconnected
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

    /// 在真实会话里验证网络状态的两条判定路径
    ///
    /// 需要 D-Bus 会话总线与真实网卡，CI 上必然失败，因此默认忽略。
    /// 手工验证：`cargo test -p rgpui-linux -- --ignored --nocapture network`
    /// 门户不可用时（把 `DBUS_SESSION_BUS_ADDRESS` 指向坏地址）会走 `/sys/class/net` 兜底。
    #[cfg(any(feature = "x11", feature = "wayland"))]
    #[test]
    #[ignore = "需要真实桌面会话的 D-Bus 总线与网络"]
    fn network_status_uses_portal_when_available() {
        let status = network_status();
        println!("network_status() => {status:?}");
        println!("sysfs_network_status() => {:?}", sysfs_network_status());

        if let Some(from_portal) = pollster::block_on(portal_network_status()) {
            assert_eq!(from_portal, status, "门户可用时结果应来自门户");
        }
        assert_ne!(
            sysfs_network_status(),
            NetworkStatus::ConnectedBelowRequired,
            "兜底路径只看链路状态，给不出连通性档位"
        );
    }
}
