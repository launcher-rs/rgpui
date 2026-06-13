//! macOS 聚焦窗口信息实现
//!
//! 使用 Cocoa `NSWorkspace` 和 `AXUIElement` API 获取当前活动窗口信息

use cocoa::base::{id, nil};
use objc::{msg_send, rc::autoreleasepool, runtime::Class, sel, sel_impl};
use rgpui::FocusedWindowInfo;

/// 获取当前系统聚焦窗口信息
///
/// # 返回
/// 返回聚焦窗口信息，如果无法获取则返回 `None`
pub fn get_focused_window_info() -> Option<FocusedWindowInfo> {
    unsafe {
        autoreleasepool(|| {
            // 获取当前活动应用
            let workspace_class = Class::get("NSWorkspace").unwrap();
            let workspace: id = msg_send![workspace_class, sharedWorkspace];
            let app: id = msg_send![workspace, frontmostApplication];

            if app == nil {
                return None;
            }

            // 获取应用名称
            let app_name_obj: id = msg_send![app, localizedName];
            let app_name_cstr: *const i8 = msg_send![app_name_obj, UTF8String];
            let app_name = if app_name_cstr.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(app_name_cstr)
                    .to_string_lossy()
                    .to_string()
            };

            // 获取 Bundle ID
            let bundle_id_obj: id = msg_send![app, bundleIdentifier];
            let bundle_id_cstr: *const i8 = msg_send![bundle_id_obj, UTF8String];
            let bundle_id = if bundle_id_cstr.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr(bundle_id_cstr)
                        .to_string_lossy()
                        .to_string(),
                )
            };

            // 获取进程 ID
            let pid: i32 = msg_send![app, processIdentifier];

            // 尝试获取窗口标题（需要辅助功能权限）
            let window_title = get_frontmost_window_title();

            Some(FocusedWindowInfo {
                app_name,
                window_title,
                bundle_id,
                pid: Some(pid as u32),
            })
        })
    }
}

/// 获取最前台窗口标题
///
/// # 返回
/// 返回窗口标题，如果无法获取则返回空字符串
fn get_frontmost_window_title() -> String {
    unsafe {
        // 使用 AXUIElement 获取窗口标题（需要辅助功能权限）
        let workspace_class = Class::get("NSWorkspace").unwrap();
        let workspace: id = msg_send![workspace_class, sharedWorkspace];
        let app: id = msg_send![workspace, frontmostApplication];
        if app == nil {
            return String::new();
        }

        // 简化实现：返回空字符串
        // 完整实现需要使用 AXUIElementCreateApplication 和 AXUIElementCopyAttributeValue
        String::new()
    }
}
