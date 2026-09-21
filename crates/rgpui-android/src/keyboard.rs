//! Android 键盘：NDK 键码/`meta_state` → [`rgpui::Keystroke`]。
//!
//! 常量取值与 `android.view.KeyEvent.KEYCODE_*` / `<android/input.h>` 的
//! `AMETA_*` 一致；映射思路同 `rgpui-linux/src/linux/keyboard.rs`。

use rgpui::collections::HashMap;
use rgpui::{
    KeybindingKeystroke, Keystroke, Modifiers, PlatformKeyboardLayout, PlatformKeyboardMapper,
};

// ── 修饰位（`<android/input.h>`） ─────────────────────────────────────────────

/// 无修饰。
pub const AMETA_NONE: i32 = 0;
/// Shift 按下。
pub const AMETA_SHIFT_ON: i32 = 0x01;
/// Alt 按下。
pub const AMETA_ALT_ON: i32 = 0x02;
/// Ctrl 按下。
pub const AMETA_CTRL_ON: i32 = 0x1000;
/// Meta（Win/Command）按下。
pub const AMETA_META_ON: i32 = 0x10000;
/// 大写锁定。
pub const AMETA_CAPS_LOCK_ON: i32 = 0x100000;
/// Fn 按下。
pub const AMETA_FUNCTION_ON: i32 = 0x08;

// ── 键码（`<android/keycodes.h>` 子集） ────────────────────────────────────────

/// 未知键。
pub const AKEYCODE_UNKNOWN: i32 = 0;
/// 返回键（映射为 escape）。
pub const AKEYCODE_BACK: i32 = 4;
/// 数字行。
pub const AKEYCODE_0: i32 = 7;
/// 数字行。
pub const AKEYCODE_1: i32 = 8;
/// 数字行。
pub const AKEYCODE_2: i32 = 9;
/// 数字行。
pub const AKEYCODE_3: i32 = 10;
/// 数字行。
pub const AKEYCODE_4: i32 = 11;
/// 数字行。
pub const AKEYCODE_5: i32 = 12;
/// 数字行。
pub const AKEYCODE_6: i32 = 13;
/// 数字行。
pub const AKEYCODE_7: i32 = 14;
/// 数字行。
pub const AKEYCODE_8: i32 = 15;
/// 数字行。
pub const AKEYCODE_9: i32 = 16;
/// 小星星。
pub const AKEYCODE_STAR: i32 = 17;
/// 井号。
pub const AKEYCODE_POUND: i32 = 18;
/// 方向上。
pub const AKEYCODE_DPAD_UP: i32 = 19;
/// 方向下。
pub const AKEYCODE_DPAD_DOWN: i32 = 20;
/// 方向左。
pub const AKEYCODE_DPAD_LEFT: i32 = 21;
/// 方向右。
pub const AKEYCODE_DPAD_RIGHT: i32 = 22;
/// 方向中（映射为 enter）。
pub const AKEYCODE_DPAD_CENTER: i32 = 23;
/// 音量加（忽略）。
pub const AKEYCODE_VOLUME_UP: i32 = 24;
/// 音量减（忽略）。
pub const AKEYCODE_VOLUME_DOWN: i32 = 25;
/// 字母区（A～Z = 29～54）。
pub const AKEYCODE_A: i32 = 29;
/// 字母区。
pub const AKEYCODE_B: i32 = 30;
/// 字母区。
pub const AKEYCODE_C: i32 = 31;
/// 字母区。
pub const AKEYCODE_D: i32 = 32;
/// 字母区。
pub const AKEYCODE_E: i32 = 33;
/// 字母区。
pub const AKEYCODE_F: i32 = 34;
/// 字母区。
pub const AKEYCODE_G: i32 = 35;
/// 字母区。
pub const AKEYCODE_H: i32 = 36;
/// 字母区。
pub const AKEYCODE_I: i32 = 37;
/// 字母区。
pub const AKEYCODE_J: i32 = 38;
/// 字母区。
pub const AKEYCODE_K: i32 = 39;
/// 字母区。
pub const AKEYCODE_L: i32 = 40;
/// 字母区。
pub const AKEYCODE_M: i32 = 41;
/// 字母区。
pub const AKEYCODE_N: i32 = 42;
/// 字母区。
pub const AKEYCODE_O: i32 = 43;
/// 字母区。
pub const AKEYCODE_P: i32 = 44;
/// 字母区。
pub const AKEYCODE_Q: i32 = 45;
/// 字母区。
pub const AKEYCODE_R: i32 = 46;
/// 字母区。
pub const AKEYCODE_S: i32 = 47;
/// 字母区。
pub const AKEYCODE_T: i32 = 48;
/// 字母区。
pub const AKEYCODE_U: i32 = 49;
/// 字母区。
pub const AKEYCODE_V: i32 = 50;
/// 字母区。
pub const AKEYCODE_W: i32 = 51;
/// 字母区。
pub const AKEYCODE_X: i32 = 52;
/// 字母区。
pub const AKEYCODE_Y: i32 = 53;
/// 字母区。
pub const AKEYCODE_Z: i32 = 54;
/// 逗号。
pub const AKEYCODE_COMMA: i32 = 55;
/// 句号。
pub const AKEYCODE_PERIOD: i32 = 56;
/// 左 Alt（纯修饰，忽略）。
pub const AKEYCODE_ALT_LEFT: i32 = 57;
/// 右 Alt（纯修饰，忽略）。
pub const AKEYCODE_ALT_RIGHT: i32 = 58;
/// 左 Shift（纯修饰，忽略）。
pub const AKEYCODE_SHIFT_LEFT: i32 = 59;
/// 右 Shift（纯修饰，忽略）。
pub const AKEYCODE_SHIFT_RIGHT: i32 = 60;
/// 制表。
pub const AKEYCODE_TAB: i32 = 61;
/// 空格。
pub const AKEYCODE_SPACE: i32 = 62;
/// 回车。
pub const AKEYCODE_ENTER: i32 = 66;
/// 退格。
pub const AKEYCODE_DEL: i32 = 67;
/// 反引号。
pub const AKEYCODE_GRAVE: i32 = 68;
/// 减号。
pub const AKEYCODE_MINUS: i32 = 69;
/// 等号。
pub const AKEYCODE_EQUALS: i32 = 70;
/// 左方括号。
pub const AKEYCODE_LEFT_BRACKET: i32 = 71;
/// 右方括号。
pub const AKEYCODE_RIGHT_BRACKET: i32 = 72;
/// 反斜杠。
pub const AKEYCODE_BACKSLASH: i32 = 73;
/// 分号。
pub const AKEYCODE_SEMICOLON: i32 = 74;
/// 单引号。
pub const AKEYCODE_APOSTROPHE: i32 = 75;
/// 斜杠。
pub const AKEYCODE_SLASH: i32 = 76;
/// @。
pub const AKEYCODE_AT: i32 = 77;
/// 菜单键。
pub const AKEYCODE_MENU: i32 = 82;
/// 上页。
pub const AKEYCODE_PAGE_UP: i32 = 92;
/// 下页。
pub const AKEYCODE_PAGE_DOWN: i32 = 93;
/// Esc。
pub const AKEYCODE_ESCAPE: i32 = 111;
/// 前向删除。
pub const AKEYCODE_FORWARD_DEL: i32 = 112;
/// 左 Ctrl（纯修饰，忽略）。
pub const AKEYCODE_CTRL_LEFT: i32 = 113;
/// 右 Ctrl（纯修饰，忽略）。
pub const AKEYCODE_CTRL_RIGHT: i32 = 114;
/// 大写锁定（纯修饰，忽略）。
pub const AKEYCODE_CAPS_LOCK: i32 = 115;
/// 滚动锁定（忽略）。
pub const AKEYCODE_SCROLL_LOCK: i32 = 116;
/// 左 Meta（纯修饰，忽略）。
pub const AKEYCODE_META_LEFT: i32 = 117;
/// 右 Meta（纯修饰，忽略）。
pub const AKEYCODE_META_RIGHT: i32 = 118;
/// Fn（纯修饰，忽略）。
pub const AKEYCODE_FUNCTION: i32 = 119;
/// 截屏键（忽略）。
pub const AKEYCODE_SYSRQ: i32 = 120;
/// 中断键（忽略）。
pub const AKEYCODE_BREAK: i32 = 121;
/// Home。
pub const AKEYCODE_MOVE_HOME: i32 = 122;
/// End。
pub const AKEYCODE_MOVE_END: i32 = 123;
/// Insert。
pub const AKEYCODE_INSERT: i32 = 124;
/// F1～F12（131～142）。
pub const AKEYCODE_F1: i32 = 131;
/// 功能键。
pub const AKEYCODE_F2: i32 = 132;
/// 功能键。
pub const AKEYCODE_F3: i32 = 133;
/// 功能键。
pub const AKEYCODE_F4: i32 = 134;
/// 功能键。
pub const AKEYCODE_F5: i32 = 135;
/// 功能键。
pub const AKEYCODE_F6: i32 = 136;
/// 功能键。
pub const AKEYCODE_F7: i32 = 137;
/// 功能键。
pub const AKEYCODE_F8: i32 = 138;
/// 功能键。
pub const AKEYCODE_F9: i32 = 139;
/// 功能键。
pub const AKEYCODE_F10: i32 = 140;
/// 功能键。
pub const AKEYCODE_F11: i32 = 141;
/// 功能键。
pub const AKEYCODE_F12: i32 = 142;
/// 数字锁定（忽略）。
pub const AKEYCODE_NUM_LOCK: i32 = 143;
/// 小键盘 0～9（144～153）。
pub const AKEYCODE_NUMPAD_0: i32 = 144;
/// 小键盘。
pub const AKEYCODE_NUMPAD_1: i32 = 145;
/// 小键盘。
pub const AKEYCODE_NUMPAD_2: i32 = 146;
/// 小键盘。
pub const AKEYCODE_NUMPAD_3: i32 = 147;
/// 小键盘。
pub const AKEYCODE_NUMPAD_4: i32 = 148;
/// 小键盘。
pub const AKEYCODE_NUMPAD_5: i32 = 149;
/// 小键盘。
pub const AKEYCODE_NUMPAD_6: i32 = 150;
/// 小键盘。
pub const AKEYCODE_NUMPAD_7: i32 = 151;
/// 小键盘。
pub const AKEYCODE_NUMPAD_8: i32 = 152;
/// 小键盘。
pub const AKEYCODE_NUMPAD_9: i32 = 153;
/// 小键盘除。
pub const AKEYCODE_NUMPAD_DIVIDE: i32 = 154;
/// 小键盘乘。
pub const AKEYCODE_NUMPAD_MULTIPLY: i32 = 155;
/// 小键盘减。
pub const AKEYCODE_NUMPAD_SUBTRACT: i32 = 156;
/// 小键盘加。
pub const AKEYCODE_NUMPAD_ADD: i32 = 157;
/// 小键盘点。
pub const AKEYCODE_NUMPAD_DOT: i32 = 158;
/// 小键盘回车。
pub const AKEYCODE_NUMPAD_ENTER: i32 = 160;
/// 静音（忽略）。
pub const AKEYCODE_VOLUME_MUTE: i32 = 164;

// ── 按键动作 ─────────────────────────────────────────────────────────────────

/// 按下。
pub const AKEY_EVENT_ACTION_DOWN: i32 = 0;
/// 抬起。
pub const AKEY_EVENT_ACTION_UP: i32 = 1;
/// 连击复述。
pub const AKEY_EVENT_ACTION_MULTIPLE: i32 = 2;

// ── 转换 ─────────────────────────────────────────────────────────────────────

/// `meta_state` 位图转 [`Modifiers`]。
pub fn android_meta_to_modifiers(meta_state: i32) -> Modifiers {
    Modifiers {
        control: meta_state & AMETA_CTRL_ON != 0,
        alt: meta_state & AMETA_ALT_ON != 0,
        shift: meta_state & AMETA_SHIFT_ON != 0,
        platform: meta_state & AMETA_META_ON != 0,
        function: meta_state & AMETA_FUNCTION_ON != 0,
    }
}

/// `meta_state` 是否含大写锁定。
pub fn android_meta_caps_lock(meta_state: i32) -> bool {
    meta_state & AMETA_CAPS_LOCK_ON != 0
}

/// Android 键码转 GPUI 键名（纯修饰/音量等返回空，由调用方丢弃）。
pub fn android_keycode_to_key(key_code: i32) -> Option<String> {
    let key = match key_code {
        AKEYCODE_A => "a",
        AKEYCODE_B => "b",
        AKEYCODE_C => "c",
        AKEYCODE_D => "d",
        AKEYCODE_E => "e",
        AKEYCODE_F => "f",
        AKEYCODE_G => "g",
        AKEYCODE_H => "h",
        AKEYCODE_I => "i",
        AKEYCODE_J => "j",
        AKEYCODE_K => "k",
        AKEYCODE_L => "l",
        AKEYCODE_M => "m",
        AKEYCODE_N => "n",
        AKEYCODE_O => "o",
        AKEYCODE_P => "p",
        AKEYCODE_Q => "q",
        AKEYCODE_R => "r",
        AKEYCODE_S => "s",
        AKEYCODE_T => "t",
        AKEYCODE_U => "u",
        AKEYCODE_V => "v",
        AKEYCODE_W => "w",
        AKEYCODE_X => "x",
        AKEYCODE_Y => "y",
        AKEYCODE_Z => "z",
        AKEYCODE_0 => "0",
        AKEYCODE_1 => "1",
        AKEYCODE_2 => "2",
        AKEYCODE_3 => "3",
        AKEYCODE_4 => "4",
        AKEYCODE_5 => "5",
        AKEYCODE_6 => "6",
        AKEYCODE_7 => "7",
        AKEYCODE_8 => "8",
        AKEYCODE_9 => "9",
        AKEYCODE_COMMA => ",",
        AKEYCODE_PERIOD => ".",
        AKEYCODE_SPACE => " ",
        AKEYCODE_GRAVE => "`",
        AKEYCODE_MINUS => "-",
        AKEYCODE_EQUALS => "=",
        AKEYCODE_LEFT_BRACKET => "[",
        AKEYCODE_RIGHT_BRACKET => "]",
        AKEYCODE_BACKSLASH => "\\",
        AKEYCODE_SEMICOLON => ";",
        AKEYCODE_APOSTROPHE => "'",
        AKEYCODE_SLASH => "/",
        AKEYCODE_AT => "@",
        AKEYCODE_STAR => "*",
        AKEYCODE_POUND => "#",
        AKEYCODE_ENTER => "enter",
        AKEYCODE_ESCAPE => "escape",
        AKEYCODE_DEL => "backspace",
        AKEYCODE_FORWARD_DEL => "delete",
        AKEYCODE_TAB => "tab",
        AKEYCODE_DPAD_UP => "up",
        AKEYCODE_DPAD_DOWN => "down",
        AKEYCODE_DPAD_LEFT => "left",
        AKEYCODE_DPAD_RIGHT => "right",
        AKEYCODE_DPAD_CENTER => "enter",
        AKEYCODE_MOVE_HOME => "home",
        AKEYCODE_MOVE_END => "end",
        AKEYCODE_INSERT => "insert",
        AKEYCODE_PAGE_UP => "pageup",
        AKEYCODE_PAGE_DOWN => "pagedown",
        AKEYCODE_F1 => "f1",
        AKEYCODE_F2 => "f2",
        AKEYCODE_F3 => "f3",
        AKEYCODE_F4 => "f4",
        AKEYCODE_F5 => "f5",
        AKEYCODE_F6 => "f6",
        AKEYCODE_F7 => "f7",
        AKEYCODE_F8 => "f8",
        AKEYCODE_F9 => "f9",
        AKEYCODE_F10 => "f10",
        AKEYCODE_F11 => "f11",
        AKEYCODE_F12 => "f12",
        AKEYCODE_NUMPAD_0 => "0",
        AKEYCODE_NUMPAD_1 => "1",
        AKEYCODE_NUMPAD_2 => "2",
        AKEYCODE_NUMPAD_3 => "3",
        AKEYCODE_NUMPAD_4 => "4",
        AKEYCODE_NUMPAD_5 => "5",
        AKEYCODE_NUMPAD_6 => "6",
        AKEYCODE_NUMPAD_7 => "7",
        AKEYCODE_NUMPAD_8 => "8",
        AKEYCODE_NUMPAD_9 => "9",
        AKEYCODE_NUMPAD_DIVIDE => "/",
        AKEYCODE_NUMPAD_MULTIPLY => "*",
        AKEYCODE_NUMPAD_SUBTRACT => "-",
        AKEYCODE_NUMPAD_ADD => "+",
        AKEYCODE_NUMPAD_DOT => ".",
        AKEYCODE_NUMPAD_ENTER => "enter",
        AKEYCODE_MENU => "menu",
        // 返回键按 escape 处理。
        AKEYCODE_BACK => "escape",
        // 纯修饰/音量/Home/截屏等忽略。
        AKEYCODE_SHIFT_LEFT | AKEYCODE_SHIFT_RIGHT | AKEYCODE_CTRL_LEFT | AKEYCODE_CTRL_RIGHT
        | AKEYCODE_ALT_LEFT | AKEYCODE_ALT_RIGHT | AKEYCODE_META_LEFT | AKEYCODE_META_RIGHT
        | AKEYCODE_FUNCTION | AKEYCODE_CAPS_LOCK | AKEYCODE_NUM_LOCK | AKEYCODE_SCROLL_LOCK
        | AKEYCODE_VOLUME_UP | AKEYCODE_VOLUME_DOWN | AKEYCODE_VOLUME_MUTE | AKEYCODE_SYSRQ
        | AKEYCODE_BREAK => return None,
        _ => return None,
    };
    Some(key.to_string())
}

/// 由 Android 按键事件建 [`Keystroke`]（可忽略键返回空）。
///
/// `unicode_char` 为 JNI 回填的 Unicode（无则传 0，单字符键按 shift 推导大小写）。
pub fn android_key_to_keystroke(
    key_code: i32,
    meta_state: i32,
    unicode_char: u32,
) -> Option<Keystroke> {
    let key = android_keycode_to_key(key_code)?;
    let modifiers = android_meta_to_modifiers(meta_state);
    let key_char = if unicode_char != 0 {
        char::from_u32(unicode_char).map(|symbol| symbol.to_string())
    } else if key.len() == 1 {
        let symbol = key.chars().next().unwrap_or_default();
        if modifiers.shift && symbol.is_ascii_alphabetic() {
            Some(symbol.to_ascii_uppercase().to_string())
        } else {
            Some(key.clone())
        }
    } else {
        None
    };
    Some(Keystroke {
        modifiers,
        key,
        key_char,
    })
}

/// Android 键盘布局（M2 固定 `en-US`，M3 经 `InputMethodManager` 回填）。
pub struct AndroidKeyboardLayout {
    /// 布局标识（如 `en-US`）。
    id: String,
}

impl AndroidKeyboardLayout {
    /// 构造指定标识的键盘布局。
    pub fn new(id: &str) -> Self {
        Self { id: id.to_string() }
    }
}

impl PlatformKeyboardLayout for AndroidKeyboardLayout {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.id
    }
}

/// Android 键位映射（直通，M2 无平台相关映射）。
pub struct AndroidKeyboardMapper;

impl PlatformKeyboardMapper for AndroidKeyboardMapper {
    fn map_key_equivalent(
        &self,
        keystroke: Keystroke,
        _use_key_equivalents: bool,
    ) -> KeybindingKeystroke {
        KeybindingKeystroke::from_keystroke(keystroke)
    }

    fn get_key_equivalents(&self) -> Option<&HashMap<char, char>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 空修饰全假。
    #[test]
    fn meta_to_modifiers_none() {
        let modifiers = android_meta_to_modifiers(AMETA_NONE);
        assert!(!modifiers.control);
        assert!(!modifiers.alt);
        assert!(!modifiers.shift);
        assert!(!modifiers.platform);
        assert!(!modifiers.function);
    }

    /// ctrl+shift 组合。
    #[test]
    fn meta_to_modifiers_ctrl_shift() {
        let modifiers = android_meta_to_modifiers(AMETA_CTRL_ON | AMETA_SHIFT_ON);
        assert!(modifiers.control);
        assert!(modifiers.shift);
        assert!(!modifiers.alt);
        assert!(!modifiers.platform);
    }

    /// 大写锁定检测。
    #[test]
    fn caps_lock_detection() {
        assert!(!android_meta_caps_lock(AMETA_NONE));
        assert!(android_meta_caps_lock(AMETA_CAPS_LOCK_ON));
    }

    /// 字母/数字/特殊键映射。
    #[test]
    fn basic_keycode_mapping() {
        assert_eq!(android_keycode_to_key(AKEYCODE_A), Some("a".to_string()));
        assert_eq!(android_keycode_to_key(AKEYCODE_Z), Some("z".to_string()));
        assert_eq!(android_keycode_to_key(AKEYCODE_0), Some("0".to_string()));
        assert_eq!(
            android_keycode_to_key(AKEYCODE_ENTER),
            Some("enter".to_string())
        );
        assert_eq!(
            android_keycode_to_key(AKEYCODE_DEL),
            Some("backspace".to_string())
        );
        assert_eq!(
            android_keycode_to_key(AKEYCODE_DPAD_LEFT),
            Some("left".to_string())
        );
        assert_eq!(
            android_keycode_to_key(AKEYCODE_F12),
            Some("f12".to_string())
        );
        assert_eq!(
            android_keycode_to_key(AKEYCODE_NUMPAD_ADD),
            Some("+".to_string())
        );
    }

    /// 修饰/音量/未知键忽略，返回键按 escape。
    #[test]
    fn ignored_keys_are_none() {
        assert_eq!(android_keycode_to_key(AKEYCODE_SHIFT_LEFT), None);
        assert_eq!(android_keycode_to_key(AKEYCODE_CTRL_LEFT), None);
        assert_eq!(android_keycode_to_key(AKEYCODE_VOLUME_UP), None);
        assert_eq!(android_keycode_to_key(AKEYCODE_UNKNOWN), None);
        assert_eq!(android_keycode_to_key(99999), None);
        assert_eq!(
            android_keycode_to_key(AKEYCODE_BACK),
            Some("escape".to_string())
        );
    }

    /// 按键事件组装 `Keystroke`（含 shift 大写与 unicode 回填）。
    #[test]
    fn keystroke_from_key_event() {
        let plain = android_key_to_keystroke(AKEYCODE_A, AMETA_NONE, 0).unwrap();
        assert_eq!(plain.key, "a");
        assert_eq!(plain.key_char, Some("a".to_string()));

        let shifted = android_key_to_keystroke(AKEYCODE_A, AMETA_SHIFT_ON, u32::from('A')).unwrap();
        assert_eq!(shifted.key_char, Some("A".to_string()));
        assert!(shifted.modifiers.shift);

        let enter = android_key_to_keystroke(AKEYCODE_ENTER, AMETA_NONE, 0).unwrap();
        assert_eq!(enter.key, "enter");
        assert_eq!(enter.key_char, None);

        assert!(android_key_to_keystroke(AKEYCODE_SHIFT_LEFT, AMETA_SHIFT_ON, 0).is_none());
    }
}
