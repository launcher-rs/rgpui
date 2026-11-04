//! macOS 全局快捷键实现
//!
//! 使用 Carbon HIToolbox `RegisterEventHotKey` API 注册系统级热键：
//! 安装一个键盘事件处理器，从 `kEventHotKeyPressed` 事件里取出热键 ID，
//! 再转交给应用注册的回调。`RegisterEventHotKey` 至今仍是 macOS 上唯一
//! 不需要辅助功能授权的全局热键通道。

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;

use rgpui::{Keystroke, Modifiers, Result};

/// Carbon 事件类型描述
#[repr(C)]
#[derive(Clone, Copy)]
struct EventTypeSpec {
    event_class: u32,
    event_kind: u32,
}

/// Carbon 热键标识：签名区分应用，ID 区分同应用内的热键
#[repr(C)]
#[derive(Clone, Copy)]
struct EventHotKeyID {
    signature: u32,
    id: u32,
}

type EventRef = *mut c_void;
type EventHandlerCallRef = *mut c_void;
type EventHandlerRef = *mut c_void;
type EventHotKeyRef = *mut c_void;

/// fourCC 常量：`keyb`（键盘事件类）
const K_EVENT_CLASS_KEYBOARD: u32 = 0x6B65_7962;
/// `kEventHotKeyPressed`
const K_EVENT_HOT_KEY_PRESSED: u32 = 5;
/// fourCC 常量：`----`（事件直接对象参数）
const K_EVENT_PARAM_DIRECT_OBJECT: u32 = 0x2D2D_2D2D;
/// fourCC 常量：`hkid`（热键 ID 参数类型）
const TYPE_EVENT_HOT_KEY_ID: u32 = 0x686B_6964;
/// `eventNotHandledErr`：把事件交还给系统
const EVENT_NOT_HANDLED_ERR: i32 = -9874;

/// 热键签名：任意唯一 fourCC，用于与其它应用的热键区分
const HOTKEY_SIGNATURE: u32 = 0x5247_5031; // "RGP1"

// Carbon 修饰键掩码（与 NSEventModifierFlag 不同）
const CMD_KEY: u32 = 0x0100;
const SHIFT_KEY: u32 = 0x0200;
const OPTION_KEY: u32 = 0x0800;
const CONTROL_KEY: u32 = 0x1000;

thread_local! {
    /// 热键触发回调；Carbon 处理器不携带 user_data，只能走线程局部存储
    static HOTKEY_CALLBACK: RefCell<Option<Box<dyn FnMut(u32)>>> = RefCell::new(None);
}

/// 设置热键触发回调（替换已有回调）。
pub(crate) fn set_callback(callback: Box<dyn FnMut(u32)>) {
    HOTKEY_CALLBACK.with(|slot| *slot.borrow_mut() = Some(callback));
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn GetApplicationEventTarget() -> *mut c_void;
    fn InstallEventHandler(
        target: *mut c_void,
        handler: unsafe extern "C" fn(EventHandlerCallRef, EventRef, *mut c_void) -> i32,
        num_types: usize,
        event_types: *const EventTypeSpec,
        user_data: *mut c_void,
        out_ref: *mut EventHandlerRef,
    ) -> i32;
    fn RegisterEventHotKey(
        hot_key_code: u32,
        hot_key_modifiers: u32,
        hot_key_id: EventHotKeyID,
        target: *mut c_void,
        options: u32,
        out_ref: *mut EventHotKeyRef,
    ) -> i32;
    fn UnregisterEventHotKey(hot_key: EventHotKeyRef) -> i32;
    fn GetEventClass(event: EventRef) -> u32;
    fn GetEventKind(event: EventRef) -> u32;
    fn GetEventParameter(
        event: EventRef,
        name: u32,
        desired_type: u32,
        actual_type: *mut u32,
        buffer_size: usize,
        actual_size: *mut usize,
        data: *mut c_void,
    ) -> i32;
}

/// 键盘事件处理器：把 `kEventHotKeyPressed` 还原成应用层的热键 ID。
unsafe extern "C" fn hotkey_event_handler(
    _call_ref: EventHandlerCallRef,
    event: EventRef,
    _user_data: *mut c_void,
) -> i32 {
    unsafe {
        if GetEventClass(event) != K_EVENT_CLASS_KEYBOARD
            || GetEventKind(event) != K_EVENT_HOT_KEY_PRESSED
        {
            return EVENT_NOT_HANDLED_ERR;
        }

        let mut hotkey_id = EventHotKeyID {
            signature: 0,
            id: 0,
        };
        let status = GetEventParameter(
            event,
            K_EVENT_PARAM_DIRECT_OBJECT,
            TYPE_EVENT_HOT_KEY_ID,
            std::ptr::null_mut(),
            std::mem::size_of::<EventHotKeyID>(),
            std::ptr::null_mut(),
            &mut hotkey_id as *mut _ as *mut c_void,
        );
        if status != 0 {
            log::warn!("读取全局热键事件参数失败: OSStatus {status}");
            return EVENT_NOT_HANDLED_ERR;
        }
        if hotkey_id.signature != HOTKEY_SIGNATURE {
            return EVENT_NOT_HANDLED_ERR;
        }

        HOTKEY_CALLBACK.with(|slot| {
            // 先取出回调再调用：回调里可能重新注册回调，持借用会 panic
            let callback = slot.borrow_mut().take();
            if let Some(mut callback) = callback {
                callback(hotkey_id.id);
                *slot.borrow_mut() = Some(callback);
            }
        });
    }
    0
}

/// macOS 全局快捷键管理器
pub struct MacGlobalHotkey {
    /// 已注册的快捷键映射：ID -> (Carbon 热键引用, 快捷键信息)
    registrations: HashMap<i32, (EventHotKeyRef, Keystroke)>,
    /// 键盘事件处理器是否已安装（进程内只安装一次）
    handler_installed: bool,
}

impl MacGlobalHotkey {
    /// 创建新的全局快捷键管理器
    pub fn new() -> Self {
        Self {
            registrations: HashMap::new(),
            handler_installed: false,
        }
    }

    /// 注册全局快捷键
    ///
    /// # 参数
    /// * `id` - 快捷键的唯一标识符
    /// * `keystroke` - 快捷键组合
    ///
    /// # 返回
    /// 成功时返回 `Ok(())`；按键无法映射到 macOS 虚拟键码、或 Carbon 拒绝注册
    /// 时返回错误（系统里已存在的同组合热键会让 `RegisterEventHotKey` 失败）。
    pub fn register(&mut self, id: i32, keystroke: &Keystroke) -> Result<()> {
        let key_code = key_code_for(&keystroke.key).ok_or_else(|| {
            anyhow::anyhow!("macOS 无法把按键 {:?} 映射到虚拟键码", keystroke.key)
        })?;
        let modifiers = modifiers_to_carbon(keystroke.modifiers);

        self.unregister(id);
        self.install_handler_once()?;

        let mut hotkey_ref: EventHotKeyRef = std::ptr::null_mut();
        let hotkey_id = EventHotKeyID {
            signature: HOTKEY_SIGNATURE,
            id: id as u32,
        };
        let status = unsafe {
            RegisterEventHotKey(
                key_code,
                modifiers,
                hotkey_id,
                GetApplicationEventTarget(),
                0,
                &mut hotkey_ref,
            )
        };
        if status != 0 {
            return Err(anyhow::anyhow!(
                "注册全局快捷键失败: {:?} (OSStatus {status})",
                keystroke
            ));
        }

        self.registrations
            .insert(id, (hotkey_ref, keystroke.clone()));
        Ok(())
    }

    /// 注销全局快捷键
    ///
    /// # 参数
    /// * `id` - 要注销的快捷键 ID
    pub fn unregister(&mut self, id: i32) {
        if let Some((hotkey_ref, _)) = self.registrations.remove(&id) {
            if !hotkey_ref.is_null() {
                unsafe {
                    UnregisterEventHotKey(hotkey_ref);
                }
            }
        }
    }

    /// 安装键盘事件处理器；失败时返回错误，避免静默无效。
    fn install_handler_once(&mut self) -> Result<()> {
        if self.handler_installed {
            return Ok(());
        }

        let event_type = EventTypeSpec {
            event_class: K_EVENT_CLASS_KEYBOARD,
            event_kind: K_EVENT_HOT_KEY_PRESSED,
        };
        debug_assert_eq!(event_type.event_class, K_EVENT_CLASS_KEYBOARD);
        debug_assert_eq!(event_type.event_kind, K_EVENT_HOT_KEY_PRESSED);
        let mut handler_ref: EventHandlerRef = std::ptr::null_mut();
        let status = unsafe {
            InstallEventHandler(
                GetApplicationEventTarget(),
                hotkey_event_handler,
                1,
                &event_type,
                std::ptr::null_mut(),
                &mut handler_ref,
            )
        };
        if status != 0 {
            return Err(anyhow::anyhow!(
                "安装全局热键事件处理器失败: OSStatus {status}"
            ));
        }

        self.handler_installed = true;
        Ok(())
    }
}

impl Drop for MacGlobalHotkey {
    fn drop(&mut self) {
        for (hotkey_ref, _) in self.registrations.drain().map(|(_, entry)| entry) {
            if !hotkey_ref.is_null() {
                unsafe {
                    UnregisterEventHotKey(hotkey_ref);
                }
            }
        }
    }
}

/// 把 GPUI 修饰键转换成 Carbon 掩码
fn modifiers_to_carbon(modifiers: Modifiers) -> u32 {
    let mut flags = 0;
    if modifiers.platform {
        flags |= CMD_KEY;
    }
    if modifiers.control {
        flags |= CONTROL_KEY;
    }
    if modifiers.alt {
        flags |= OPTION_KEY;
    }
    if modifiers.shift {
        flags |= SHIFT_KEY;
    }
    flags
}

/// 把按键名映射到 macOS 虚拟键码（ANSI 布局）。
///
/// Carbon 注册的是物理键位，字母/数字键在所有拉丁布局上位置一致；
/// 无法表示的按键返回 `None`，由调用方报错而不是静默注册一个无效热键。
fn key_code_for(key: &str) -> Option<u32> {
    let key = key.to_ascii_lowercase();
    let code = match key.as_str() {
        "a" => 0x00,
        "s" => 0x01,
        "d" => 0x02,
        "f" => 0x03,
        "h" => 0x04,
        "g" => 0x05,
        "z" => 0x06,
        "x" => 0x07,
        "c" => 0x08,
        "v" => 0x09,
        "b" => 0x0B,
        "q" => 0x0C,
        "w" => 0x0D,
        "e" => 0x0E,
        "r" => 0x0F,
        "y" => 0x10,
        "t" => 0x11,
        "1" => 0x12,
        "2" => 0x13,
        "3" => 0x14,
        "4" => 0x15,
        "6" => 0x16,
        "5" => 0x17,
        "=" | "equal" => 0x18,
        "9" => 0x19,
        "7" => 0x1A,
        "-" | "minus" => 0x1B,
        "8" => 0x1C,
        "0" => 0x1D,
        "]" | "bracketright" => 0x1E,
        "o" => 0x1F,
        "u" => 0x20,
        "[" | "bracketleft" => 0x21,
        "i" => 0x22,
        "p" => 0x23,
        "enter" | "return" => 0x24,
        "l" => 0x25,
        "j" => 0x26,
        "'" | "quote" => 0x27,
        "k" => 0x28,
        ";" | "semicolon" => 0x29,
        "\\" | "backslash" => 0x2A,
        "," | "comma" => 0x2B,
        "/" | "slash" => 0x2C,
        "n" => 0x2D,
        "m" => 0x2E,
        "." | "period" => 0x2F,
        "tab" => 0x30,
        "space" => 0x31,
        "`" | "backquote" => 0x32,
        "backspace" => 0x33,
        "escape" | "esc" => 0x35,
        "insert" => 0x72,
        "home" => 0x73,
        "pageup" => 0x74,
        "delete" => 0x75,
        "end" => 0x77,
        "pagedown" => 0x79,
        "f1" => 0x7A,
        "f2" => 0x78,
        "f3" => 0x63,
        "f4" => 0x76,
        "f5" => 0x60,
        "f6" => 0x61,
        "f7" => 0x62,
        "f8" => 0x64,
        "f9" => 0x65,
        "f10" => 0x6D,
        "f11" => 0x67,
        "f12" => 0x6F,
        "left" => 0x7B,
        "right" => 0x7C,
        "down" => 0x7D,
        "up" => 0x7E,
        _ => return None,
    };
    Some(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 字母、数字与常用编辑键都必须有键码；未知按键必须报错而不是静默注册。
    #[test]
    fn key_code_mapping_covers_common_keys() {
        for key in ["a", "Z", "0", "9", "space", "enter", "escape", "f12", "up"] {
            assert!(key_code_for(key).is_some(), "{key} 应该有键码");
        }
        assert_eq!(key_code_for("a"), Some(0x00));
        assert_eq!(key_code_for("A"), Some(0x00));
        assert_eq!(key_code_for("unknown-key"), None);
    }

    /// 修饰键按 Carbon 掩码映射：command 是 platform，option 是 alt。
    #[test]
    fn modifiers_map_to_carbon_masks() {
        let modifiers = Modifiers {
            control: true,
            alt: true,
            shift: true,
            platform: true,
            function: false,
        };
        assert_eq!(
            modifiers_to_carbon(modifiers),
            CMD_KEY | CONTROL_KEY | OPTION_KEY | SHIFT_KEY
        );
    }
}
