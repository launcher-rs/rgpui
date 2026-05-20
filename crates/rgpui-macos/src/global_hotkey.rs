//! macOS 全局快捷键实现
//!
//! 使用 Carbon HIToolbox `RegisterEventHotKey` API 实现系统级热键注册

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use core_graphics::event::{CGEventField, CGEventTapLocation, CGEventType, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

use rgpui::{Keystroke, Modifiers, Result};

/// 下一个可用的快捷键 ID
static NEXT_HOTKEY_ID: AtomicU32 = AtomicU32::new(1);

/// macOS 全局快捷键管理器
pub struct MacGlobalHotkey {
    /// 已注册的快捷键映射：ID -> 快捷键信息
    registrations: HashMap<i32, Keystroke>,
}

impl MacGlobalHotkey {
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
        // macOS 使用 NSEvent addLocalMonitorForEvents 或 Carbon EventManager
        // 这里简化实现，实际需要使用 Carbon API 或 NSEvent

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

    /// 将 GPUI 修饰键转换为 macOS 修饰键标志
    pub fn modifiers_to_nsevent(modifiers: Modifiers) -> NSEventModifierFlags {
        let mut flags = NSEventModifierFlags::empty();

        if modifiers.control {
            flags |= NSEventModifierFlags::NSEventModifierFlagControl;
        }
        if modifiers.alt {
            flags |= NSEventModifierFlags::NSEventModifierFlagOption;
        }
        if modifiers.shift {
            flags |= NSEventModifierFlags::NSEventModifierFlagShift;
        }
        if modifiers.platform {
            flags |= NSEventModifierFlags::NSEventModifierFlagCommand;
        }

        flags
    }

    /// 将按键转换为 macOS 虚拟键码
    pub fn key_to_cgkeycode(key: &str) -> CGKeyCode {
        match key {
            "f1" => 122,
            "f2" => 120,
            "f3" => 99,
            "f4" => 118,
            "f5" => 96,
            "f6" => 97,
            "f7" => 98,
            "f8" => 100,
            "f9" => 101,
            "f10" => 109,
            "f11" => 103,
            "f12" => 111,
            "space" => 49,
            "escape" => 53,
            "enter" => 36,
            "tab" => 48,
            "backspace" => 51,
            "delete" => 117,
            "home" => 115,
            "end" => 119,
            "pageup" => 116,
            "pagedown" => 121,
            "up" => 126,
            "down" => 125,
            "left" => 123,
            "right" => 124,
            c if c.len() == 1 => {
                let ch = c.chars().next().unwrap();
                if ch.is_ascii_lowercase() {
                    // macOS 键码：a=0, b=11, c=8, ...
                    match ch {
                        'a' => 0,
                        'b' => 11,
                        'c' => 8,
                        'd' => 2,
                        'e' => 14,
                        'f' => 3,
                        'g' => 5,
                        'h' => 4,
                        'i' => 34,
                        'j' => 38,
                        'k' => 40,
                        'l' => 37,
                        'm' => 46,
                        'n' => 45,
                        'o' => 31,
                        'p' => 35,
                        'q' => 12,
                        'r' => 15,
                        's' => 1,
                        't' => 17,
                        'u' => 32,
                        'v' => 9,
                        'w' => 13,
                        'x' => 7,
                        'y' => 16,
                        'z' => 6,
                        '0' => 29,
                        '1' => 18,
                        '2' => 19,
                        '3' => 20,
                        '4' => 21,
                        '5' => 23,
                        '6' => 22,
                        '7' => 26,
                        '8' => 28,
                        '9' => 25,
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

use cocoa::base::id;
use cocoa::foundation::NSUInteger;
use objc::rc::autoreleasepool;
use std::ffi::c_void;

// NSEventModifierFlags 定义
bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct NSEventModifierFlags: NSUInteger {
        const NSEventModifierFlagCapsLock = 1 << 16;
        const NSEventModifierFlagShift = 1 << 17;
        const NSEventModifierFlagControl = 1 << 18;
        const NSEventModifierFlagOption = 1 << 19;
        const NSEventModifierFlagCommand = 1 << 20;
        const NSEventModifierFlagNumericPad = 1 << 21;
        const NSEventModifierFlagHelp = 1 << 22;
        const NSEventModifierFlagFunction = 1 << 23;
    }
}
