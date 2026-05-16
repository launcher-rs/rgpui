// Windows 托盘图标实现

use crate::WM_GPUI_TRAY_ICON;
use gpui::*;
use std::collections::HashMap;
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::*,
        UI::{
            Shell::{
                Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD,
                NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
            },
            WindowsAndMessaging::*,
        },
    },
};

/// 托盘图标唯一标识
const TRAY_ICON_ID: u32 = 1;

/// Windows 托盘结构体
pub(crate) struct WindowsTray {
    /// 是否已添加图标到系统托盘
    icon_added: bool,
    /// 托盘窗口句柄
    hwnd: HWND,
    /// 当前显示的图标
    current_icon: Option<HICON>,
    /// 菜单项列表
    pub(crate) menu_items: Vec<TrayMenuItem>,
    /// 命令 ID 到菜单项 ID 的映射
    pub(crate) command_id_map: HashMap<u32, SharedString>,
}

impl WindowsTray {
    /// 创建新的托盘实例
    pub fn new(hwnd: HWND) -> Self {
        let mut tray = Self {
            icon_added: false,
            hwnd,
            current_icon: None,
            menu_items: Vec::new(),
            command_id_map: HashMap::new(),
        };
        // 添加托盘图标到系统，使用默认图标
        tray.ensure_icon_with_default(hwnd);
        tray
    }

    /// 确保托盘图标已添加到系统，并使用默认图标
    fn ensure_icon_with_default(&mut self, hwnd: HWND) {
        if self.icon_added {
            return;
        }
        // 加载系统默认图标
        let default_icon = unsafe { LoadIconW(None, IDI_APPLICATION).ok() };
        self.current_icon = default_icon;
        eprintln!("[Tray] ensure_icon_with_default: loading default icon");

        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_MESSAGE | NIF_SHOWTIP | NIF_ICON,
            uCallbackMessage: WM_GPUI_TRAY_ICON,
            hIcon: self.current_icon.unwrap_or_default(),
            ..Default::default()
        };
        unsafe {
            let result = Shell_NotifyIconW(NIM_ADD, &nid);
            eprintln!("[Tray] Shell_NotifyIconW(NIM_ADD) with default icon result: {}", result.as_bool());
        }
        self.icon_added = true;
    }

    /// 确保托盘图标已添加到系统（不设置默认图标）
    fn ensure_icon(&mut self, hwnd: HWND) {
        if self.icon_added {
            eprintln!("[Tray] ensure_icon: icon already added");
            return;
        }
        eprintln!("[Tray] ensure_icon: adding icon to system tray");
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_MESSAGE | NIF_SHOWTIP | NIF_ICON,
            uCallbackMessage: WM_GPUI_TRAY_ICON,
            hIcon: self.current_icon.unwrap_or_default(),
            ..Default::default()
        };
        unsafe {
            let result = Shell_NotifyIconW(NIM_ADD, &nid);
            eprintln!("[Tray] Shell_NotifyIconW(NIM_ADD) result: {}", result.as_bool());
        }
        self.icon_added = true;
    }

    /// 设置托盘图标
    pub fn set_icon(&mut self, icon_data: Option<&[u8]>, hwnd: HWND) {
        self.ensure_icon(hwnd);
        // 销毁旧图标
        if let Some(old_icon) = self.current_icon.take() {
            unsafe {
                let _ = DestroyIcon(old_icon);
            }
        }
        // 创建新图标
        let hicon = match icon_data {
            Some(data) => create_hicon_from_bytes(data),
            None => None,
        };
        self.current_icon = hicon;
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_ICON,
            hIcon: hicon.unwrap_or_default(),
            ..Default::default()
        };
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    /// 设置托盘工具提示
    pub fn set_tooltip(&mut self, tooltip: &str, hwnd: HWND) {
        self.ensure_icon(hwnd);
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_TIP | NIF_ICON,
            hIcon: self.current_icon.unwrap_or_default(),
            ..Default::default()
        };
        let wide: Vec<u16> = tooltip.encode_utf16().collect();
        let len = wide.len().min(nid.szTip.len() - 1);
        nid.szTip[..len].copy_from_slice(&wide[..len]);
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    /// 显示气球提示
    #[allow(dead_code)]
    pub fn show_balloon(&self, title: &str, body: &str, hwnd: HWND) -> anyhow::Result<()> {
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_INFO,
            ..Default::default()
        };

        let title_wide: Vec<u16> = title.encode_utf16().collect();
        let title_len = title_wide.len().min(nid.szInfoTitle.len() - 1);
        nid.szInfoTitle[..title_len].copy_from_slice(&title_wide[..title_len]);

        let body_wide: Vec<u16> = body.encode_utf16().collect();
        let body_len = body_wide.len().min(nid.szInfo.len() - 1);
        nid.szInfo[..body_len].copy_from_slice(&body_wide[..body_len]);

        unsafe {
            Shell_NotifyIconW(NIM_MODIFY, &nid)
                .ok()
                .map_err(|e| anyhow::anyhow!("显示气球提示失败: {}", e))
        }
    }

    /// 显示上下文菜单
    pub fn show_context_menu(&mut self, hwnd: HWND) {
        eprintln!("[Tray] show_context_menu called, menu_items count: {}", self.menu_items.len());
        if self.menu_items.is_empty() {
            eprintln!("[Tray] No menu items, returning");
            return;
        }
        self.command_id_map.clear();
        unsafe {
            let hmenu = CreatePopupMenu();
            if let Ok(hmenu) = hmenu {
                let mut counter: u32 = 1;
                Self::build_menu(
                    hmenu,
                    &self.menu_items,
                    &mut counter,
                    &mut self.command_id_map,
                );
                eprintln!("[Tray] Built menu with {} items", self.command_id_map.len());
                let mut point = POINT::default();
                let _ = GetCursorPos(&mut point);
                eprintln!("[Tray] Cursor position: ({}, {})", point.x, point.y);
                let _ = SetForegroundWindow(hwnd);
                let result = TrackPopupMenu(
                    hmenu,
                    TPM_LEFTALIGN | TPM_BOTTOMALIGN,
                    point.x,
                    point.y,
                    None,
                    hwnd,
                    None,
                );
                eprintln!("[Tray] TrackPopupMenu result: {}", result.0);
                let _ = DestroyMenu(hmenu);
            }
        }
    }

    /// 构建 Windows 原生菜单
    pub(crate) unsafe fn build_menu(
        hmenu: HMENU,
        items: &[TrayMenuItem],
        counter: &mut u32,
        id_map: &mut HashMap<u32, SharedString>,
    ) {
        for item in items.iter() {
            match item {
                TrayMenuItem::Action { label, id } => {
                    let cmd_id = *counter;
                    *counter += 1;
                    id_map.insert(cmd_id, id.clone());
                    let wide: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
                    unsafe {
                        let _ =
                            AppendMenuW(hmenu, MF_STRING, cmd_id as usize, PCWSTR(wide.as_ptr()));
                    }
                }
                TrayMenuItem::Separator => unsafe {
                    let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, None);
                },
                TrayMenuItem::Submenu {
                    label,
                    items: sub_items,
                } => {
                    if let Ok(submenu) = unsafe { CreatePopupMenu() } {
                        unsafe { Self::build_menu(submenu, sub_items, counter, id_map) };
                        let wide: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
                        unsafe {
                            let _ = AppendMenuW(
                                hmenu,
                                MF_POPUP,
                                submenu.0 as usize,
                                PCWSTR(wide.as_ptr()),
                            );
                        }
                    }
                }
                TrayMenuItem::Toggle {
                    label, checked, id, ..
                } => {
                    let cmd_id = *counter;
                    *counter += 1;
                    id_map.insert(cmd_id, id.clone());
                    let flags = if *checked {
                        MF_STRING | MF_CHECKED
                    } else {
                        MF_STRING
                    };
                    let wide: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
                    unsafe {
                        let _ = AppendMenuW(hmenu, flags, cmd_id as usize, PCWSTR(wide.as_ptr()));
                    }
                }
            }
        }
    }
}

impl Drop for WindowsTray {
    fn drop(&mut self) {
        // 删除托盘图标
        if self.icon_added {
            let nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: TRAY_ICON_ID,
                ..Default::default()
            };
            unsafe {
                let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            }
        }
        // 销毁图标资源
        if let Some(icon) = self.current_icon.take() {
            unsafe {
                let _ = DestroyIcon(icon);
            }
        }
    }
}

/// 从字节数据创建 HICON 图标
fn create_hicon_from_bytes(data: &[u8]) -> Option<HICON> {
    unsafe {
        // 查找图标资源偏移量
        let offset = LookupIconIdFromDirectoryEx(data.as_ptr(), true, 0, 0, LR_DEFAULTCOLOR);
        if offset <= 0 {
            return None;
        }
        if (offset as usize) >= data.len() {
            return None;
        }
        // 从资源数据创建图标
        let icon_data = &data[offset as usize..];
        let hicon = CreateIconFromResourceEx(icon_data, true, 0x00030000, 0, 0, LR_DEFAULTCOLOR);
        hicon.ok()
    }
}
