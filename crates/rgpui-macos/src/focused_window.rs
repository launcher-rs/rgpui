//! macOS 聚焦窗口信息实现
//!
//! 使用 Cocoa `NSWorkspace` 和 `AXUIElement` API 获取当前活动窗口信息

use cocoa::base::{id, nil};
use core_foundation_sys::base::CFTypeRef;
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

/// 获取最前台窗口标题（通过 AXUIElement，需辅助功能权限）。
/// 取不到时返回空字符串 —— 调用方应把它当作「标题未知」而非「没有窗口」。
fn get_frontmost_window_title() -> String {
    use core_foundation::base::TCFType as _;

    unsafe {
        let workspace_class = Class::get("NSWorkspace").unwrap();
        let workspace: id = msg_send![workspace_class, sharedWorkspace];
        let app: id = msg_send![workspace, frontmostApplication];
        if app == nil {
            return String::new();
        }
        let pid: i32 = msg_send![app, processIdentifier];
        if pid <= 0 {
            return String::new();
        }

        let app_element = ax::AXUIElementCreateApplication(pid);
        if app_element.is_null() {
            return String::new();
        }

        // AX 属性名常量在部分 macOS SDK / deployment target 下不会作为可链接符号导出
        // （CI 上就报 _kAXFocusedWindowAttribute / _kAXTitleAttribute undefined）。
        // 这些常量本身就是固定的 CFString 字面量，直接构造即可，不依赖链接器符号。
        let mut focused_window: CFTypeRef = std::ptr::null();
        let focused_attr = core_foundation::string::CFString::new("AXFocusedWindow");
        let status = ax::AXUIElementCopyAttributeValue(
            app_element,
            focused_attr.as_concrete_TypeRef(),
            &mut focused_window,
        );
        if status != 0 || focused_window.is_null() {
            return String::new();
        }

        let mut title_value: CFTypeRef = std::ptr::null();
        let title_attr = core_foundation::string::CFString::new("AXTitle");
        let status = ax::AXUIElementCopyAttributeValue(
            focused_window.cast(),
            title_attr.as_concrete_TypeRef(),
            &mut title_value,
        );
        if status == 0 && !title_value.is_null() {
            // title_value 是 CFStringRef；用 core_foundation 的安全包装转成 Rust String
            let title: core_foundation::string::CFString =
                core_foundation::base::TCFType::wrap_under_get_rule(title_value.cast());
            return title.to_string();
        }
        String::new()
    }
}

/// AXUIElement API 绑定（ApplicationServices 框架）。
mod ax {
    use core_foundation_sys::base::CFTypeRef;
    use core_foundation_sys::string::CFStringRef;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        pub fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
        pub fn AXUIElementCopyAttributeValue(
            element: CFTypeRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> i32;
    }
}
