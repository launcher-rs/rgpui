//! macOS 原生通知实现
//!
//! 使用 `NSUserNotificationCenter` API 发送原生通知

use cocoa::base::{id, nil};
use cocoa::foundation::NSString;
use objc::{msg_send, rc::autoreleasepool, runtime::Class, sel, sel_impl};
use rgpui::Result;

/// macOS 原生通知管理器
pub struct MacNotifications;

impl MacNotifications {
    /// 创建新的通知管理器
    pub fn new() -> Self {
        Self
    }

    /// 发送原生通知
    ///
    /// # 参数
    /// * `title` - 通知标题
    /// * `body` - 通知内容
    /// * `icon` - 可选的图标数据（PNG 格式）
    ///
    /// # 返回
    /// 成功时返回 `Ok(())`，失败时返回错误
    pub fn show_notification(&self, title: &str, body: &str, icon: Option<&[u8]>) -> Result<()> {
        unsafe {
            autoreleasepool(|| -> Result<()> {
                // 新版 macOS 可能移除已废弃的 NSUserNotification；缺失时返回错误，
                // 不再 panic，也不假装投递成功
                let Some(class) = Class::get("NSUserNotification") else {
                    anyhow::bail!("NSUserNotification 类不可用");
                };
                let Some(center_class) = Class::get("NSUserNotificationCenter") else {
                    anyhow::bail!("NSUserNotificationCenter 类不可用");
                };
                let notification: id = msg_send![class, new];

                let title_str = NSString::alloc(nil).init_str(title);
                let _: () = msg_send![notification, setTitle: title_str];

                let body_str = NSString::alloc(nil).init_str(body);
                let _: () = msg_send![notification, setInformativeText: body_str];

                if let Some(icon_data) = icon {
                    // 从字节数据创建 NSImage；缺失的类只跳过图标，不阻断通知
                    if let (Some(image_class), Some(data_class)) =
                        (Class::get("NSImage"), Class::get("NSData"))
                    {
                        let ns_data: id = msg_send![data_class, dataWithBytes:icon_data.as_ptr() length:icon_data.len()];
                        let image: id = msg_send![image_class, alloc];
                        let image: id = msg_send![image, initWithData: ns_data];
                        let _: () = msg_send![notification, setContentImage: image];
                    } else {
                        log::warn!("NSImage/NSData 类不可用，通知图标已跳过");
                    }
                }

                let center: id = msg_send![center_class, defaultUserNotificationCenter];
                let _: () = msg_send![center, deliverNotification: notification];
                let _: () = msg_send![notification, release];
                Ok(())
            })
        }
    }
}
