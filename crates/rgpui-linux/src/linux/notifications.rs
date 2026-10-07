//! Linux 原生通知实现
//!
//! 走 XDG 桌面门户的 `org.freedesktop.portal.Notification` 接口。
//! 门户由 `xdg-desktop-portal` 提供，内部再代理给桌面的通知服务，
//! 因此 X11 与 Wayland 会话用的是同一条路径。

use rgpui::Result;

/// 通知实例计数，用于生成门户要求的 application-provided id
#[cfg(any(feature = "x11", feature = "wayland"))]
static NOTIFICATION_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Linux 原生通知管理器
pub struct LinuxNotifications;

impl LinuxNotifications {
    /// 创建新的通知管理器
    pub fn new() -> Self {
        Self
    }
}

#[cfg(any(feature = "x11", feature = "wayland"))]
impl LinuxNotifications {
    /// 发送原生通知
    ///
    /// # 参数
    /// * `title` - 通知标题
    /// * `body` - 通知内容
    /// * `icon` - 可选的图标名称或路径
    ///
    /// # 返回
    /// 成功时返回 `Ok(())`，失败时返回错误
    pub fn show_notification(&self, title: &str, body: &str, icon: Option<&str>) -> Result<()> {
        use ashpd::desktop::notification::{Notification, NotificationProxy};

        let mut notification = Notification::new(title).body(body);
        if let Some(icon) = icon {
            notification = notification.icon(ashpd::desktop::Icon::with_names([icon]));
        }

        // 门户要求应用自己给一个稳定 id：同一 id 的后续通知会替换前者，
        // 所以每条都给新 id，避免相互覆盖。
        let id = NOTIFICATION_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // 这是一次本地 D-Bus 往返（毫秒级），直接在调用线程等结果，
        // 好让失败如实返回给调用方，而不是先回 Ok 再到后台把错误丢掉。
        pollster::block_on(async move {
            let proxy = NotificationProxy::new().await?;
            proxy.add_notification(&id.to_string(), notification).await
        })?;

        Ok(())
    }
}

#[cfg(not(any(feature = "x11", feature = "wayland")))]
impl LinuxNotifications {
    /// 发送原生通知（无头模式不支持）
    ///
    /// 无头构建没有 DBus 会话总线依赖，明确报错而不是静默返回成功。
    pub fn show_notification(&self, _title: &str, _body: &str, _icon: Option<&str>) -> Result<()> {
        anyhow::bail!("无头模式不支持系统通知")
    }
}
