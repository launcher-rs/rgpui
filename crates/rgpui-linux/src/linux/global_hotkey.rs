//! Linux 全局快捷键实现
//!
//! 使用 X11 或 Wayland 协议实现系统级热键注册

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use rgpui::{Keystroke, Modifiers, Result};

/// 下一个可用的快捷键 ID
static NEXT_HOTKEY_ID: AtomicU32 = AtomicU32::new(1);

/// Linux 全局快捷键管理器
pub struct LinuxGlobalHotkey {
    /// 已注册的快捷键映射：ID -> 快捷键信息
    registrations: HashMap<i32, Keystroke>,
}

impl LinuxGlobalHotkey {
    /// 创建新的全局快捷键管理器
    pub fn new() -> Self {
        Self {
            registrations: HashMap::new(),
        }
    }

    /// 注册全局快捷键
    ///
    /// # 参数
    /// * `id` - 快捷键的唯一标识符
    /// * `keystroke` - 快捷键组合
    ///
    /// # 返回
    /// 成功时返回 `Ok(())`，失败时返回错误
    pub fn register(&mut self, id: i32, keystroke: &Keystroke) -> Result<()> {
        // Linux 上使用 X11 GrabKey 或 Wayland ext-global-shortcuts
        // 这里简化实现，实际需要根据显示服务器选择后端

        self.registrations.insert(id, keystroke.clone());
        Ok(())
    }

    /// 注销全局快捷键
    ///
    /// # 参数
    /// * `id` - 要注销的快捷键 ID
    pub fn unregister(&mut self, id: i32) {
        self.registrations.remove(&id);
    }

    /// 获取下一个可用的快捷键 ID
    pub fn next_id() -> i32 {
        NEXT_HOTKEY_ID.fetch_add(1, Ordering::Relaxed) as i32
    }

    /// 将 GPUI 修饰键转换为 X11 修饰键标志
    pub fn modifiers_to_x11(modifiers: Modifiers) -> u32 {
        let mut mod_flags: u32 = 0;

        if modifiers.control {
            mod_flags |= 0x4; // ControlMask
        }
        if modifiers.alt {
            mod_flags |= 0x8; // Mod1Mask
        }
        if modifiers.shift {
            mod_flags |= 0x1; // ShiftMask
        }
        if modifiers.platform {
            mod_flags |= 0x40; // Mod4Mask (Super/Win)
        }

        mod_flags
    }

    /// 将按键转换为 X11 键码
    pub fn key_to_x11_keycode(key: &str) -> u32 {
        match key {
            "f1" => 67,
            "f2" => 68,
            "f3" => 69,
            "f4" => 70,
            "f5" => 71,
            "f6" => 72,
            "f7" => 73,
            "f8" => 74,
            "f9" => 75,
            "f10" => 76,
            "f11" => 95,
            "f12" => 96,
            "space" => 65,
            "escape" => 9,
            "enter" => 36,
            "tab" => 23,
            "backspace" => 22,
            "delete" => 119,
            "home" => 110,
            "end" => 115,
            "pageup" => 112,
            "pagedown" => 117,
            "up" => 111,
            "down" => 116,
            "left" => 113,
            "right" => 114,
            c if c.len() == 1 => {
                let ch = c.chars().next().unwrap();
                if ch.is_ascii_lowercase() {
                    // X11 键码：a=38, b=56, c=54, ...
                    match ch {
                        'a' => 38,
                        'b' => 56,
                        'c' => 54,
                        'd' => 40,
                        'e' => 26,
                        'f' => 41,
                        'g' => 42,
                        'h' => 43,
                        'i' => 31,
                        'j' => 44,
                        'k' => 45,
                        'l' => 46,
                        'm' => 58,
                        'n' => 57,
                        'o' => 32,
                        'p' => 33,
                        'q' => 24,
                        'r' => 27,
                        's' => 39,
                        't' => 28,
                        'u' => 30,
                        'v' => 55,
                        'w' => 25,
                        'x' => 53,
                        'y' => 29,
                        'z' => 52,
                        '0' => 19,
                        '1' => 10,
                        '2' => 11,
                        '3' => 12,
                        '4' => 13,
                        '5' => 14,
                        '6' => 15,
                        '7' => 16,
                        '8' => 17,
                        '9' => 18,
                        _ => 0,
                    }
                } else {
                    0
                }
            }
            _ => 0,
        }
    }
}
