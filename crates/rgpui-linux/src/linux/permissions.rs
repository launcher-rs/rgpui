//! Linux 权限查询实现
//!
//! Linux 没有 macOS 那样按应用记账的统一权限库（TCC），三类权限的判定口径各不相同，
//! 这里一律回答「本机此刻能不能真的做成这件事」，而不是返回固定值骗过调用方。
//!
//! | 权限 | X11 会话 | Wayland 会话 |
//! |------|----------|--------------|
//! | Accessibility | AT-SPI 栈在跑就算可用（不按应用授权） | 同左 |
//! | ScreenCapture | 协议无访问控制，任何客户端都能读根窗口像素 | 必须经门户 ScreenCast 授权 |
//! | InputMonitoring | 根窗口 `GrabKey` 无需授权 | 需要门户 GlobalShortcuts 接口 |

use rgpui::{PermissionStatus, PermissionType};

/// 会话总线上的 AT-SPI 启动器接口名（同时也是它的总线名）
#[cfg(any(feature = "x11", feature = "wayland"))]
const A11Y_BUS: &str = "org.a11y.Bus";
/// AT-SPI 启动器的对象路径
#[cfg(any(feature = "x11", feature = "wayland"))]
const A11Y_BUS_PATH: &str = "/org/a11y/bus";
/// 辅助功能总线上的注册表接口名
#[cfg(any(feature = "x11", feature = "wayland"))]
const AT_SPI_REGISTRY: &str = "org.a11y.atspi.Registry";
/// 注册表的对象路径
#[cfg(any(feature = "x11", feature = "wayland"))]
const AT_SPI_REGISTRY_PATH: &str = "/org/a11y/atspi/registry";
/// 桌面门户的总线名
#[cfg(any(feature = "x11", feature = "wayland"))]
const PORTAL_BUS: &str = "org.freedesktop.portal.Desktop";
/// 桌面门户的根对象，各门户接口都实现在这一个对象上
#[cfg(any(feature = "x11", feature = "wayland"))]
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

/// Linux 权限查询管理器
pub struct LinuxPermissions;

impl LinuxPermissions {
    /// 创建新的权限查询管理器
    pub fn new() -> Self {
        Self
    }

    /// 查询指定权限的状态
    ///
    /// 每次调用都会走一到两次 D-Bus 往返（毫秒级），适合在应用启动或用户打开
    /// 设置面板时调用，不要放进渲染循环。
    ///
    /// # 参数
    /// * `permission` - 权限类型
    ///
    /// # 返回
    /// 返回权限状态
    pub fn query_permission(&self, permission: PermissionType) -> PermissionStatus {
        match permission {
            PermissionType::Accessibility => check_accessibility(),
            PermissionType::ScreenCapture => check_screen_capture(),
            PermissionType::InputMonitoring => check_input_monitoring(),
        }
    }

    /// 请求权限
    ///
    /// Linux 上没有「应用弹窗向系统要权限」的通用入口：辅助功能与输入监控取决于桌面环境
    /// 的实现，屏幕捕获的授权发生在真正创建门户会话的那一刻。
    /// 所以这里只报告当前状态并给出用户能照着做的指引，不假装已经拿到授权。
    pub fn request_permission(permission: PermissionType) {
        let status = Self.query_permission(permission);
        log::info!("Linux 没有应用侧权限弹窗，{permission:?} 当前状态: {status:?}");
        match permission {
            PermissionType::Accessibility => log::info!(
                "辅助功能由 AT-SPI 栈提供：确认进程环境里没有 NO_AT_BRIDGE=1，\
                 GNOME 下可用 `gsettings set org.gnome.desktop.interface toolkit-accessibility true` 打开"
            ),
            PermissionType::ScreenCapture => {
                log::info!("Wayland 会话需在门户弹窗中授权屏幕捕获，X11 会话没有这一限制")
            }
            PermissionType::InputMonitoring => log::info!(
                "X11 会话的全局按键监听无需授权；Wayland 会话要求合成器提供 global-shortcuts 门户"
            ),
        }
    }
}

/// 当前是否运行在 Wayland 会话里
#[cfg(any(feature = "x11", feature = "wayland"))]
fn is_wayland_session() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// 辅助功能：Linux 不按应用授权，真正的前置条件是 AT-SPI 栈可用
#[cfg(any(feature = "x11", feature = "wayland"))]
fn check_accessibility() -> PermissionStatus {
    // 应用侧显式关掉桥接时，本机栈再可用本进程也接不上去
    if std::env::var("NO_AT_BRIDGE").is_ok_and(|value| value == "1") {
        return PermissionStatus::Denied;
    }
    match at_spi_registry_alive() {
        Ok(true) => PermissionStatus::Granted,
        Ok(false) => PermissionStatus::Unavailable,
        Err(err) => {
            log::debug!("查询 AT-SPI 栈失败: {err:#}");
            PermissionStatus::Unavailable
        }
    }
}

/// 屏幕捕获：X11 无访问控制，Wayland 必须经门户
#[cfg(any(feature = "x11", feature = "wayland"))]
fn check_screen_capture() -> PermissionStatus {
    if is_wayland_session() {
        // 合成器隔离各客户端，屏幕内容只能由门户授权后交出来
        if !portal_interface_available("org.freedesktop.portal.ScreenCast") {
            return PermissionStatus::Unavailable;
        }
        // 真正的授权发生在创建会话时，rgpui 尚未发起 → 如实回答未确定
        return PermissionStatus::NotDetermined;
    }
    if std::env::var_os("DISPLAY").is_some() {
        // X11 协议里没有任何按客户端的屏幕访问控制
        return PermissionStatus::Granted;
    }
    PermissionStatus::Unavailable
}

/// 输入监控：能不能监听其它应用/全局的按键
#[cfg(any(feature = "x11", feature = "wayland"))]
fn check_input_monitoring() -> PermissionStatus {
    if is_wayland_session() {
        // Wayland 上要拿到全局按键只有 global-shortcuts 门户这一条路
        // （rgpui 的全局热键目前只在 X11 上实现，见审计文档 §4.2-2）
        return if portal_interface_available("org.freedesktop.portal.GlobalShortcuts") {
            PermissionStatus::NotDetermined
        } else {
            PermissionStatus::Unavailable
        };
    }
    if std::env::var_os("DISPLAY").is_some() {
        // 根窗口 GrabKey 不需要授权，X11 全局热键正是建立在这之上
        return PermissionStatus::Granted;
    }
    PermissionStatus::Unavailable
}

/// 探测辅助功能总线上的注册表进程是否可达
///
/// `org.a11y.Bus.GetAddress` 只说明启动器在跑，地址指向的私有总线上
/// 还必须有 `org.a11y.atspi.Registry`（at-spi2-registryd）才算真的能用。
#[cfg(any(feature = "x11", feature = "wayland"))]
fn at_spi_registry_alive() -> anyhow::Result<bool> {
    use anyhow::Context as _;
    use ashpd::zbus::Connection;
    use ashpd::zbus::connection::Builder as ConnectionBuilder;

    pollster::block_on(async {
        let session = Connection::session().await.context("连接会话总线失败")?;
        let (address,): (String,) = session
            .call_method(
                Some(A11Y_BUS),
                A11Y_BUS_PATH,
                Some(A11Y_BUS),
                "GetAddress",
                &(),
            )
            .await
            .context("调用 org.a11y.Bus.GetAddress 失败")?
            .body()
            .deserialize()
            .context("解析 org.a11y.Bus.GetAddress 返回值失败")?;

        let a11y = ConnectionBuilder::address(address.as_str())?
            .build()
            .await
            .context("连接辅助功能总线失败")?;
        match a11y
            .call_method(
                Some(AT_SPI_REGISTRY),
                AT_SPI_REGISTRY_PATH,
                Some(AT_SPI_REGISTRY),
                "GetRegisteredEvents",
                &(),
            )
            .await
        {
            Ok(_) => Ok(true),
            Err(err) => {
                log::debug!("AT-SPI 注册表不可达: {err}");
                Ok(false)
            }
        }
    })
}

/// 检查门户根对象上是否实现了指定接口
///
/// 用一次 `Introspect` 往返来判断，这样不必为 `ashpd` 打开各接口的 feature
/// ——这里只需要「有没有这个接口」，不需要发起会话。
#[cfg(any(feature = "x11", feature = "wayland"))]
fn portal_interface_available(interface: &str) -> bool {
    use anyhow::Context as _;
    use ashpd::zbus::Connection;

    let introspection = pollster::block_on(async {
        let connection = Connection::session().await.context("连接会话总线失败")?;
        let (xml,): (String,) = connection
            .call_method(
                Some(PORTAL_BUS),
                PORTAL_PATH,
                Some("org.freedesktop.DBus.Introspectable"),
                "Introspect",
                &(),
            )
            .await
            .context("调用门户的 Introspect 失败")?
            .body()
            .deserialize()
            .context("解析 Introspect 返回值失败")?;
        anyhow::Ok(xml)
    });

    match introspection {
        Ok(xml) => xml.contains(&format!(r#"interface name="{interface}""#)),
        Err(err) => {
            log::debug!("查询门户接口 {interface} 失败: {err:#}");
            false
        }
    }
}

#[cfg(not(any(feature = "x11", feature = "wayland")))]
fn check_accessibility() -> PermissionStatus {
    PermissionStatus::Unavailable
}

#[cfg(not(any(feature = "x11", feature = "wayland")))]
fn check_screen_capture() -> PermissionStatus {
    PermissionStatus::Unavailable
}

#[cfg(not(any(feature = "x11", feature = "wayland")))]
fn check_input_monitoring() -> PermissionStatus {
    PermissionStatus::Unavailable
}

#[cfg(all(test, any(feature = "x11", feature = "wayland")))]
mod tests {
    use super::*;

    /// 在真实桌面会话里跑一遍三项权限查询
    ///
    /// 需要可用的 D-Bus 会话总线，CI 上必然失败，因此默认忽略。
    /// 手工验证：`cargo test -p rgpui-linux -- --ignored --nocapture permissions`
    #[test]
    #[ignore = "需要真实桌面会话的 D-Bus 总线"]
    fn query_permissions_in_live_session() {
        let permissions = LinuxPermissions::new();
        for permission in [
            PermissionType::Accessibility,
            PermissionType::ScreenCapture,
            PermissionType::InputMonitoring,
        ] {
            println!(
                "{permission:?} => {:?}",
                permissions.query_permission(permission)
            );
        }

        let screen_capture = permissions.query_permission(PermissionType::ScreenCapture);
        let input_monitoring = permissions.query_permission(PermissionType::InputMonitoring);
        if is_wayland_session() {
            // Wayland 上屏幕内容必须经门户，X11 那种「直接就算已授权」不成立
            assert!(
                matches!(
                    screen_capture,
                    PermissionStatus::NotDetermined | PermissionStatus::Unavailable
                ),
                "Wayland 会话下屏幕捕获不该是 {screen_capture:?}"
            );
        } else if std::env::var_os("DISPLAY").is_some() {
            assert_eq!(screen_capture, PermissionStatus::Granted);
            assert_eq!(input_monitoring, PermissionStatus::Granted);
        } else {
            assert_eq!(screen_capture, PermissionStatus::Unavailable);
            assert_eq!(input_monitoring, PermissionStatus::Unavailable);
        }
    }
}
