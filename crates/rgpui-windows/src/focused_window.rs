//! Windows 聚焦窗口信息实现
//!
//! 使用 Win32 API 获取当前活动窗口的信息

use windows::{
    Win32::Foundation::*, Win32::System::Threading::*, Win32::UI::WindowsAndMessaging::*,
};

use rgpui::FocusedWindowInfo;

/// 获取当前系统聚焦窗口信息
///
/// # 返回
/// 返回聚焦窗口信息，如果无法获取则返回 `None`
pub fn get_focused_window_info() -> Option<FocusedWindowInfo> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }

        let window_title = get_window_title(hwnd).unwrap_or_default();

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        let app_name = get_process_name(pid).unwrap_or_default();

        Some(FocusedWindowInfo {
            app_name,
            window_title,
            bundle_id: None,
            pid: Some(pid),
        })
    }
}

unsafe fn get_window_title(hwnd: HWND) -> Option<String> {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length == 0 {
        return Some(String::new());
    }

    let mut buffer = vec![0u16; (length + 1) as usize];
    let chars_read = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    if chars_read == 0 {
        return None;
    }

    String::from_utf16(&buffer[..chars_read as usize]).ok()
}

unsafe fn get_process_name(pid: u32) -> Option<String> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;

    let mut buffer = [0u16; 1024];
    let mut length = buffer.len() as u32;

    unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    }
    .ok()?;

    let _ = unsafe { CloseHandle(handle) };

    let path = String::from_utf16(&buffer[..length as usize]).ok()?;
    path.rsplit('\\').next().map(|s| s.to_string())
}
