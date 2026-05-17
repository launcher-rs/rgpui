// Windows 托盘图标实现

use crate::WM_GPUI_TRAY_ICON;
use rgpui::*;
use std::collections::HashMap;
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        UI::{
            Shell::{
                Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD,
                NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
            },
            WindowsAndMessaging::*,
        },
    },
};

/// 托盘图标唯一标识符
const TRAY_ICON_ID: u32 = 1;

/// Windows 平台托盘结构体
/// 负责管理系统托盘图标的显示、菜单和交互事件
pub(crate) struct WindowsTray {
    /// 是否已将图标添加到系统托盘
    icon_added: bool,
    /// 托盘所属窗口的句柄
    hwnd: HWND,
    /// 当前显示的图标句柄
    current_icon: Option<HICON>,
    /// 菜单项列表
    pub(crate) menu_items: Vec<TrayMenuItem>,
    /// Windows 命令 ID 到菜单项 ID 的映射表
    pub(crate) command_id_map: HashMap<u32, SharedString>,
}

impl WindowsTray {
    /// 创建新的托盘实例
    /// 参数:
    ///   hwnd - 托盘所属窗口的句柄
    /// 返回: 带有默认图标的托盘实例
    pub fn new(hwnd: HWND) -> Self {
        let mut tray = Self {
            icon_added: false,
            hwnd,
            current_icon: None,
            menu_items: Vec::new(),
            command_id_map: HashMap::new(),
        };
        // 添加默认图标到系统托盘
        tray.ensure_icon_with_default(hwnd);
        tray
    }

    /// 确保托盘图标已添加到系统，并使用默认图标
    /// 如果图标已存在则直接返回
    fn ensure_icon_with_default(&mut self, hwnd: HWND) {
        if self.icon_added {
            return;
        }
        // 加载系统默认应用程序图标
        let default_icon = unsafe { LoadIconW(None, IDI_APPLICATION).ok() };
        self.current_icon = default_icon;

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
            let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        }
        self.icon_added = true;
    }

    /// 确保托盘图标已添加到系统（不设置默认图标）
    /// 如果图标已存在则直接返回
    fn ensure_icon(&mut self, hwnd: HWND) {
        if self.icon_added {
            return;
        }
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
            let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        }
        self.icon_added = true;
    }

    /// 设置托盘图标
    /// 参数:
    ///   icon_data - 图标字节数据（支持 PNG/ICO 格式）
    ///   hwnd - 托盘所属窗口的句柄
    pub fn set_icon(&mut self, icon_data: Option<&[u8]>, hwnd: HWND) {
        self.ensure_icon(hwnd);
        // 销毁旧图标释放资源
        if let Some(old_icon) = self.current_icon.take() {
            unsafe {
                let _ = DestroyIcon(old_icon);
            }
        }
        // 从字节数据创建新图标
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

    /// 设置托盘工具提示文本
    /// 参数:
    ///   tooltip - 提示文本内容
    ///   hwnd - 托盘所属窗口的句柄
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

    /// 显示气球提示通知
    /// 参数:
    ///   title - 提示标题
    ///   body - 提示内容
    ///   hwnd - 托盘所属窗口的句柄
    /// 返回: 操作结果
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
    /// 在鼠标当前位置弹出托盘菜单
    /// 参数:
    ///   hwnd - 托盘所属窗口的句柄
    pub fn show_context_menu(&mut self, hwnd: HWND) {
        if self.menu_items.is_empty() {
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
                let mut point = POINT::default();
                let _ = GetCursorPos(&mut point);
                let _ = SetForegroundWindow(hwnd);
                let _ = TrackPopupMenu(
                    hmenu,
                    TPM_LEFTALIGN | TPM_BOTTOMALIGN,
                    point.x,
                    point.y,
                    None,
                    hwnd,
                    None,
                );
                let _ = DestroyMenu(hmenu);
            }
        }
    }

    /// 递归构建 Windows 原生菜单
    /// 参数:
    ///   hmenu - 父菜单句柄
    ///   items - 菜单项列表
    ///   counter - 命令 ID 计数器（用于生成唯一 ID）
    ///   id_map - 命令 ID 到菜单项 ID 的映射表
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
    /// 销毁托盘实例时清理资源
    fn drop(&mut self) {
        // 从系统托盘删除图标
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
        // 销毁图标句柄释放 GDI 资源
        if let Some(icon) = self.current_icon.take() {
            unsafe {
                let _ = DestroyIcon(icon);
            }
        }
    }
}

/// 从字节数据创建 HICON 图标句柄
/// 支持 ICO 和 PNG 格式
/// 参数:
///   data - 图标字节数据
/// 返回: 成功时返回 HICON 句柄，失败时返回 None
fn create_hicon_from_bytes(data: &[u8]) -> Option<HICON> {
    unsafe {
        // 首先尝试作为 ICO 格式解析
        let offset = LookupIconIdFromDirectoryEx(data.as_ptr(), true, 0, 0, LR_DEFAULTCOLOR);
        if offset > 0 && (offset as usize) < data.len() {
            let icon_data = &data[offset as usize..];
            if let Ok(hicon) = CreateIconFromResourceEx(icon_data, true, 0x00030000, 0, 0, LR_DEFAULTCOLOR) {
                return Some(hicon);
            }
        }

        // ICO 解析失败时，尝试使用 image crate 解码 PNG 格式
        let img = match image::load_from_memory(data) {
            Ok(img) => img,
            Err(_) => return None,
        };

        let rgba = img.to_rgba8();
        let width = rgba.width();
        let height = rgba.height();

        // 创建 DIB 位图
        let hdc = GetDC(None);
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0, // BI_RGB
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [RGBQUAD::default()],
        };

        let pixels = rgba.into_raw();
        let mut pbits: *mut core::ffi::c_void = std::ptr::null_mut();
        let hbitmap = CreateDIBSection(
            Some(hdc),
            &bmi,
            DIB_RGB_COLORS,
            &mut pbits,
            None,
            0,
        );
        ReleaseDC(None, hdc);

        if let Ok(hbitmap) = hbitmap {
            // 将像素数据复制到 DIB 位图
            if !pbits.is_null() {
                std::ptr::copy_nonoverlapping(
                    pixels.as_ptr(),
                    pbits as *mut u8,
                    pixels.len(),
                );
            }

            // 通过 ICONINFO 创建 HICON
            let mut icon_info = ICONINFO::default();
            icon_info.fIcon = true.into();
            icon_info.xHotspot = 0;
            icon_info.yHotspot = 0;
            icon_info.hbmMask = hbitmap;
            icon_info.hbmColor = hbitmap;
            let hicon = CreateIconIndirect(&icon_info);
            if let Ok(hicon) = hicon {
                return Some(hicon);
            }
        }

        None
    }
}
