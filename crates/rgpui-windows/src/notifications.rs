//! Windows 原生通知实现
//!
//! 使用 Win32 `Shell_NotifyIconW` API 实现系统托盘气泡通知

use windows::{Win32::Foundation::*, Win32::UI::Shell::*};

use rgpui::Result;

/// 显示 Windows 系统托盘气泡通知
///
/// # 参数
/// * `hwnd` - 窗口句柄
/// * `title` - 通知标题
/// * `body` - 通知内容
///
/// # 返回
/// 成功时返回 `Ok(())`，失败时返回错误
pub fn show_balloon_notification(hwnd: HWND, title: &str, body: &str) -> Result<()> {
    let title_utf16: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    let body_utf16: Vec<u16> = body.encode_utf16().chain(Some(0)).collect();

    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uFlags: NIF_INFO,
        dwInfoFlags: NIIF_INFO,
        ..Default::default()
    };

    let title_len = std::cmp::min(title_utf16.len() - 1, 63);
    nid.szInfoTitle[..title_len].copy_from_slice(&title_utf16[..title_len]);

    let body_len = std::cmp::min(body_utf16.len() - 1, 255);
    nid.szInfo[..body_len].copy_from_slice(&body_utf16[..body_len]);

    unsafe {
        let result = Shell_NotifyIconW(NIM_MODIFY, &nid as *const _);
        if !result.as_bool() {
            return Err(anyhow::anyhow!("显示通知失败"));
        }
    }

    Ok(())
}
