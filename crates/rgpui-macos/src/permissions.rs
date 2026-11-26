//! macOS 权限查询实现
//!
//! 使用 Accessibility API 和 EventTap 检查系统权限状态

use rgpui::{PermissionStatus, PermissionType};

/// macOS 权限查询管理器
pub struct MacPermissions;

impl MacPermissions {
    /// 创建新的权限查询管理器
    pub fn new() -> Self {
        Self
    }

    /// 查询指定权限的状态
    ///
    /// # 参数
    /// * `permission` - 权限类型
    ///
    /// # 返回
    /// 返回权限状态
    pub fn query_permission(&self, permission: PermissionType) -> PermissionStatus {
        match permission {
            PermissionType::Accessibility => self.check_accessibility_permission(),
            PermissionType::ScreenCapture => self.check_screen_capture_permission(),
            PermissionType::InputMonitoring => self.check_input_monitoring_permission(),
        }
    }

    /// 检查辅助功能权限
    fn check_accessibility_permission(&self) -> PermissionStatus {
        unsafe {
            // 使用 AXIsProcessTrusted 检查辅助功能权限
            let trusted = accessibility::AXIsProcessTrusted();
            if trusted {
                PermissionStatus::Granted
            } else {
                PermissionStatus::Denied
            }
        }
    }

    /// 检查屏幕录制权限
    fn check_screen_capture_permission(&self) -> PermissionStatus {
        // macOS 10.15+ 使用 CGPreflightScreenCaptureAccess
        #[cfg(target_os = "macos")]
        {
            if unsafe { screen_capture::CGPreflightScreenCaptureAccess() } {
                PermissionStatus::Granted
            } else {
                PermissionStatus::Denied
            }
        }

        #[cfg(not(target_os = "macos"))]
        PermissionStatus::Unavailable
    }

    /// 检查输入监控权限。
    /// macOS 的输入监控（监听其它应用的按键）是独立的 TCC 类别，
    /// 不能借辅助功能权限代替 —— 此前两者被混为一谈，UI 会误报已授权。
    fn check_input_monitoring_permission(&self) -> PermissionStatus {
        #[cfg(target_os = "macos")]
        {
            // SAFETY: CGPreflightListenEventAccess 只查询 TCC 授权状态，不访问用户数据
            if unsafe { input_monitoring::CGPreflightListenEventAccess() } {
                PermissionStatus::Granted
            } else {
                PermissionStatus::Denied
            }
        }
        #[cfg(not(target_os = "macos"))]
        PermissionStatus::Unavailable
    }

    /// 请求输入监控权限（弹出系统 TCC 授权框）
    pub fn request_input_monitoring_permission() {
        #[cfg(target_os = "macos")]
        unsafe {
            input_monitoring::CGRequestListenEventAccess();
        }
    }

    /// 请求辅助功能权限
    pub fn request_accessibility_permission() {
        unsafe {
            // 使用 AXIsProcessTrustedWithOptions 请求权限
            let key = accessibility::kAXTrustedCheckOptionPrompt as *const std::ffi::c_void;
            let value = core_foundation::boolean::kCFBooleanTrue as *const std::ffi::c_void;
            accessibility::AXIsProcessTrustedWithOptions(
                core_foundation::dictionary::CFDictionaryCreate(
                    std::ptr::null(),
                    &key,
                    &value,
                    1,
                    &core_foundation::dictionary::kCFTypeDictionaryKeyCallBacks,
                    &core_foundation::dictionary::kCFTypeDictionaryValueCallBacks,
                ),
            );
        }
    }
}

// 辅助功能 API 绑定
mod accessibility {
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::string::CFStringRef;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        pub fn AXIsProcessTrusted() -> bool;
        pub fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;

        pub static kAXTrustedCheckOptionPrompt: CFStringRef;
    }
}

// 屏幕录制权限 API 绑定
mod screen_capture {
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        pub fn CGPreflightScreenCaptureAccess() -> bool;
    }
}

// 输入监控权限 API 绑定（独立于辅助功能）
mod input_monitoring {
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        pub fn CGPreflightListenEventAccess() -> bool;
        pub fn CGRequestListenEventAccess() -> bool;
    }
}
