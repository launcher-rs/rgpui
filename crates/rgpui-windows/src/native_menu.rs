//! Windows 原生弹出菜单：`CreatePopupMenu` + `TrackPopupMenuEx`。
//!
//! 与托盘菜单（`tray.rs`）用的是同一套 Win32 菜单 API，区别在于这里不走 `WM_COMMAND`：
//! `TPM_RETURNCMD` 让跟踪循环把命中的命令 ID 直接作为返回值给出，菜单因此不必参与窗口的
//! 消息路由，选中结果只经由一个 oneshot 通道回传给应用。
//!
//! `TrackPopupMenuEx` 内部跑一个嵌套的 Win32 消息循环，所以只能在主线程调用；调用方
//! （`window.rs` 的 `show_native_menu`）把它放进前台执行器的任务里，等当前这次 `App`
//! 借用结束、消息循环重新转起来以后才真正弹出。

use std::collections::HashMap;

use ::rgpui::util::ResultExt as _;
use rgpui::{NativeMenu, NativeMenuItem, SharedString};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

/// 命令 ID 的起始值：0 是「用户没选任何东西」的返回值，不能占用。
const FIRST_COMMAND_ID: u32 = 1;

/// 在客户区坐标 `client_point`（物理像素）处弹出原生菜单，返回被选中项的 id。
///
/// 返回 `None` 涵盖三种情况：建菜单/坐标换算失败、用户直接关掉菜单、点中的是禁用项或
/// 子菜单标题。调用方据此判断「没有选择」，不必区分。
pub(crate) fn track_native_menu(
    hwnd: HWND,
    client_point: POINT,
    menu: NativeMenu,
) -> Option<SharedString> {
    let hmenu = unsafe { CreatePopupMenu() }.log_err()?;

    let mut id_map = HashMap::new();
    let mut next_command = FIRST_COMMAND_ID;
    append_items(hmenu, menu.items(), &mut next_command, &mut id_map);

    // 菜单位置要的是屏幕坐标，而框架给的是客户区坐标。
    let mut point = client_point;
    unsafe { ClientToScreen(hwnd, &mut point) }.ok().log_err()?;

    // 先把窗口置为前台，否则菜单不会在点击窗口外部时关闭。
    unsafe { SetForegroundWindow(hwnd) }.ok().log_err();
    let command = unsafe {
        TrackPopupMenuEx(
            hmenu,
            (TPM_RETURNCMD | TPM_NONOTIFY | TPM_LEFTALIGN | TPM_TOPALIGN).0,
            point.x,
            point.y,
            hwnd,
            None,
        )
    };
    // 菜单循环结束后前台状态没有还回窗口，补一条空消息让 Windows 完成这次交接；
    // 少了它，菜单关掉后的第一次窗口点击会被吞掉。
    unsafe { PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0)) }.log_err();
    unsafe { DestroyMenu(hmenu) }.log_err();

    id_map.get(&(command.0 as u32)).cloned()
}

/// 递归把菜单项装进 `HMENU`，同时记下「命令 ID → 菜单项 id」的映射。
///
/// 命令 ID 在整棵菜单树里连续分配（子菜单共用同一个计数器），所以回传的 ID 能唯一确定
/// 一项 —— 这就是 [`NativeMenuItem::Action`] 自带 `id` 而不靠索引定位的原因。
fn append_items(
    hmenu: HMENU,
    items: &[NativeMenuItem],
    next_command: &mut u32,
    id_map: &mut HashMap<u32, SharedString>,
) {
    for item in items {
        match item {
            NativeMenuItem::Action {
                label,
                id,
                disabled,
                checked,
            } => {
                let command = *next_command;
                *next_command += 1;
                id_map.insert(command, id.clone());

                let mut flags = MF_STRING;
                if *checked {
                    flags |= MF_CHECKED;
                }
                if *disabled {
                    // `MF_DISABLED` 只拦键盘不拦鼠标，置灰要用 `MF_GRAYED`。
                    flags |= MF_GRAYED;
                }
                let wide: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
                unsafe { AppendMenuW(hmenu, flags, command as usize, PCWSTR(wide.as_ptr())) }
                    .log_err();
            }
            NativeMenuItem::Separator => {
                unsafe { AppendMenuW(hmenu, MF_SEPARATOR, 0, None) }.log_err();
            }
            NativeMenuItem::Submenu {
                label,
                items: sub_items,
            } => {
                let Some(submenu) = (unsafe { CreatePopupMenu() }).log_err() else {
                    continue;
                };
                append_items(submenu, sub_items, next_command, id_map);
                let wide: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
                unsafe { AppendMenuW(hmenu, MF_POPUP, submenu.0 as usize, PCWSTR(wide.as_ptr())) }
                    .log_err();
            }
        }
    }
}
