//! Windows 全局快捷键实现
//!
//! 使用 Win32 `RegisterHotKey` / `UnregisterHotKey` API 实现系统级热键注册

use std::collections::HashMap;

use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

use rgpui::{Keystroke, Modifiers, Result};

/// Windows 修饰键标志
const MOD_CONTROL: u32 = 0x0002;
const MOD_ALT: u32 = 0x0001;
const MOD_SHIFT: u32 = 0x0004;
const MOD_WIN: u32 = 0x0008;

/// Windows 全局快捷键管理器
pub struct WindowsGlobalHotkey {
    /// 已注册的快捷键映射：ID -> (HWND, 修饰键, 虚拟键)
    registrations: HashMap<i32, (HWND, HOT_KEY_MODIFIERS, VIRTUAL_KEY)>,
}

impl WindowsGlobalHotkey {
    /// 创建新的全局快捷键管理器
    pub fn new() -> Self {
        Self {
            registrations: HashMap::new(),
        }
    }

    /// 注册全局快捷键
    ///
    /// # 参数
    /// * `hwnd` - 接收热键消息的窗口句柄
    /// * `id` - 快捷键的唯一标识符
    /// * `keystroke` - 快捷键组合
    ///
    /// # 返回
    /// 成功时返回 `Ok(())`，失败时返回错误
    pub fn register(&mut self, hwnd: HWND, id: i32, keystroke: &Keystroke) -> Result<()> {
        let modifiers = self.modifiers_to_win(keystroke.modifiers);
        let vk = self.key_to_virtual_key(keystroke);

        unsafe {
            RegisterHotKey(Some(hwnd), id, modifiers, vk.0 as u32).map_err(|e| {
                anyhow::anyhow!("无法注册全局快捷键: {:?} (错误: {:?})", keystroke, e)
            })?;
        }

        self.registrations.insert(id, (hwnd, modifiers, vk));
        Ok(())
    }

    /// 注销全局快捷键
    ///
    /// # 参数
    /// * `id` - 要注销的快捷键 ID
    pub fn unregister(&mut self, id: i32) {
        if let Some((hwnd, _, _)) = self.registrations.remove(&id) {
            unsafe {
                let _ = UnregisterHotKey(Some(hwnd), id);
            }
        }
    }

    /// 将 GPUI 修饰键转换为 Windows HOT_KEY_MODIFIERS
    fn modifiers_to_win(&self, modifiers: Modifiers) -> HOT_KEY_MODIFIERS {
        let mut mod_flags: u32 = 0;

        if modifiers.control {
            mod_flags |= MOD_CONTROL;
        }
        if modifiers.alt {
            mod_flags |= MOD_ALT;
        }
        if modifiers.shift {
            mod_flags |= MOD_SHIFT;
        }
        if modifiers.platform {
            mod_flags |= MOD_WIN;
        }

        HOT_KEY_MODIFIERS(mod_flags)
    }

    /// 将按键转换为 Windows 虚拟键码
    fn key_to_virtual_key(&self, keystroke: &Keystroke) -> VIRTUAL_KEY {
        let key = keystroke.key.as_str();
        match key {
            "f1" => VK_F1,
            "f2" => VK_F2,
            "f3" => VK_F3,
            "f4" => VK_F4,
            "f5" => VK_F5,
            "f6" => VK_F6,
            "f7" => VK_F7,
            "f8" => VK_F8,
            "f9" => VK_F9,
            "f10" => VK_F10,
            "f11" => VK_F11,
            "f12" => VK_F12,
            "space" => VK_SPACE,
            "escape" => VK_ESCAPE,
            "enter" => VK_RETURN,
            "tab" => VK_TAB,
            "backspace" => VK_BACK,
            "delete" => VK_DELETE,
            "insert" => VK_INSERT,
            "home" => VK_HOME,
            "end" => VK_END,
            "pageup" => VK_PRIOR,
            "pagedown" => VK_NEXT,
            "up" => VK_UP,
            "down" => VK_DOWN,
            "left" => VK_LEFT,
            "right" => VK_RIGHT,
            c if c.len() == 1 => {
                let ch = c.chars().next().unwrap();
                if ch.is_ascii_alphabetic() {
                    VIRTUAL_KEY(ch.to_ascii_uppercase() as u16)
                } else if ch.is_ascii_digit() {
                    VIRTUAL_KEY(ch as u16)
                } else {
                    VIRTUAL_KEY(ch as u16)
                }
            }
            _ => VIRTUAL_KEY(0),
        }
    }
}
