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
            autoreleasepool(|| {
                let class = Class::get("NSUserNotification").unwrap();
                let notification: id = msg_send![class, new];

                let title_str = NSString::alloc(nil).init_str(title);
                let _: () = msg_send![notification, setTitle: title_str];

                let body_str = NSString::alloc(nil).init_str(body);
                let _: () = msg_send![notification, setInformativeText: body_str];

                if let Some(icon_data) = icon {
                    // 从字节数据创建 NSImage
                    let class = Class::get("NSImage").unwrap();
                    let ns_data: id = msg_send![Class::get("NSData").unwrap(), dataWithBytes:icon_data.as_ptr() length:icon_data.len()];
                    let image: id = msg_send![class, alloc];
                    let image: id = msg_send![image, initWithData: ns_data];
                    let _: () = msg_send![notification, setContentImage: image];
                }

                let center_class = Class::get("NSUserNotificationCenter").unwrap();
                let center: id = msg_send![center_class, defaultUserNotificationCenter];
                let _: () = msg_send![center, deliverNotification: notification];

                let _: () = msg_send![notification, release];
            });
        }

        Ok(())
    }

    /// 请求通知权限
    ///
    /// # 返回
    /// 成功时返回 `Ok(())`，失败时返回错误
    pub fn request_permission() -> Result<()> {
        unsafe {
            let class = Class::get("NSUserNotificationCenter").unwrap();
            let center: id = msg_send![class, defaultUserNotificationCenter];
            // macOS 10.8+ 自动请求权限，无需显式调用
        }

        Ok(())
    }
}
