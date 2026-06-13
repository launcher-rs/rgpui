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
        PermissionStatus::Unknown
    }

    /// 检查输入监控权限
    fn check_input_monitoring_permission(&self) -> PermissionStatus {
        // 输入监控权限通常与辅助功能权限相同
        self.check_accessibility_permission()
    }

    /// 请求辅助功能权限
    pub fn request_accessibility_permission() {
        unsafe {
            // 使用 AXIsProcessTrustedWithOptions 请求权限
            accessibility::AXIsProcessTrustedWithOptions(
                core_foundation::dictionary::CFDictionaryCreate(
                    std::ptr::null(),
                    &[accessibility::kAXTrustedCheckOptionPrompt.take()],
                    &[core_foundation::boolean::kCFBooleanTrue],
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
