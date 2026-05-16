//! Windows 键盘布局映射器
//!
//! 本模块处理 Windows 键盘布局和按键映射，包括：
//! - 键盘布局检测和名称获取
//! - 虚拟键码到字符的转换
//! - 快捷键组合映射
//! - 支持不同键盘布局的按键等价物

use anyhow::Result;
use gpui::collections::HashMap;
use windows::Win32::UI::{
    Input::KeyboardAndMouse::{
        GetKeyboardLayoutNameW, MapVirtualKeyW, ToUnicode, MAPVK_VK_TO_CHAR, MAPVK_VK_TO_VSC,
        VIRTUAL_KEY, VK_0, VK_1, VK_2, VK_3, VK_4, VK_5, VK_6, VK_7, VK_8, VK_9, VK_ABNT_C1,
        VK_CONTROL, VK_MENU, VK_OEM_1, VK_OEM_102, VK_OEM_2, VK_OEM_3, VK_OEM_4, VK_OEM_5,
        VK_OEM_6, VK_OEM_7, VK_OEM_8, VK_OEM_COMMA, VK_OEM_MINUS, VK_OEM_PERIOD, VK_OEM_PLUS,
        VK_SHIFT,
    },
    WindowsAndMessaging::KL_NAMELENGTH,
};

use gpui::{
    KeybindingKeystroke, Keystroke, Modifiers, PlatformKeyboardLayout, PlatformKeyboardMapper,
};

/// Windows 键盘布局信息
///
/// 包含当前系统键盘布局的 ID 和显示名称
pub(crate) struct WindowsKeyboardLayout {
    id: String,
    name: String,
}

/// Windows 键盘映射器
///
/// 负责将按键映射到对应的字符，支持普通键和 Shift 组合键的映射
pub(crate) struct WindowsKeyboardMapper {
    key_to_vkey: HashMap<String, (u16, bool)>,
    vkey_to_key: HashMap<u16, String>,
    vkey_to_shifted: HashMap<u16, String>,
}

impl PlatformKeyboardLayout for WindowsKeyboardLayout {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl PlatformKeyboardMapper for WindowsKeyboardMapper {
    fn map_key_equivalent(
        &self,
        mut keystroke: Keystroke,
        use_key_equivalents: bool,
    ) -> KeybindingKeystroke {
        let Some((vkey, shifted_key)) = self.get_vkey_from_key(&keystroke.key, use_key_equivalents)
        else {
            return KeybindingKeystroke::from_keystroke(keystroke);
        };
        if shifted_key && keystroke.modifiers.shift {
            log::warn!(
                "Keystroke '{}' has both shift and a shifted key, this is likely a bug",
                keystroke.key
            );
        }

        let shift = shifted_key || keystroke.modifiers.shift;
        keystroke.modifiers.shift = false;

        let Some(key) = self.vkey_to_key.get(&vkey).cloned() else {
            log::error!(
                "Failed to map key equivalent '{:?}' to a valid key",
                keystroke
            );
            return KeybindingKeystroke::from_keystroke(keystroke);
        };

        keystroke.key = if shift {
            let Some(shifted_key) = self.vkey_to_shifted.get(&vkey).cloned() else {
                log::error!(
                    "Failed to map keystroke {:?} with virtual key '{:?}' to a shifted key",
                    keystroke,
                    vkey
                );
                return KeybindingKeystroke::from_keystroke(keystroke);
            };
            shifted_key
        } else {
            key.clone()
        };

        let modifiers = Modifiers {
            shift,
            ..keystroke.modifiers
        };

        KeybindingKeystroke::new(keystroke, modifiers, key)
    }

    fn get_key_equivalents(&self) -> Option<&HashMap<char, char>> {
        None
    }
}

impl WindowsKeyboardLayout {
    pub(crate) fn new() -> Result<Self> {
        let mut buffer = [0u16; KL_NAMELENGTH as usize]; // KL_NAMELENGTH includes the null terminator
        unsafe { GetKeyboardLayoutNameW(&mut buffer)? };
        let id = String::from_utf16_lossy(&buffer[..buffer.len() - 1]); // Remove the null terminator
        let entry = windows_registry::LOCAL_MACHINE.open(format!(
            "System\\CurrentControlSet\\Control\\Keyboard Layouts\\{id}"
        ))?;
        let name = entry.get_string("Layout Text")?;
        Ok(Self { id, name })
    }

    pub(crate) fn unknown() -> Self {
        Self {
            id: "unknown".to_string(),
            name: "unknown".to_string(),
        }
    }
}

impl WindowsKeyboardMapper {
    pub(crate) fn new() -> Self {
        let mut key_to_vkey = HashMap::default();
        let mut vkey_to_key = HashMap::default();
        let mut vkey_to_shifted = HashMap::default();
        for vkey in CANDIDATE_VKEYS {
            if let Some(key) = get_key_from_vkey(*vkey) {
                key_to_vkey.insert(key.clone(), (vkey.0, false));
                vkey_to_key.insert(vkey.0, key);
            }
            let scan_code = unsafe { MapVirtualKeyW(vkey.0 as u32, MAPVK_VK_TO_VSC) };
            if scan_code == 0 {
                continue;
            }
            if let Some(shifted_key) = get_shifted_key(*vkey, scan_code) {
                key_to_vkey.insert(shifted_key.clone(), (vkey.0, true));
                vkey_to_shifted.insert(vkey.0, shifted_key);
            }
        }
        Self {
            key_to_vkey,
            vkey_to_key,
            vkey_to_shifted,
        }
    }

    fn get_vkey_from_key(&self, key: &str, use_key_equivalents: bool) -> Option<(u16, bool)> {
        if use_key_equivalents {
            get_vkey_from_key_with_us_layout(key)
        } else {
            self.key_to_vkey.get(key).cloned()
        }
    }
}

/// 根据虚拟键码、扫描码和修饰键状态获取按键字符串
///
/// # 参数
/// * `vkey` - 虚拟键码
/// * `scan_code` - 扫描码
/// * `modifiers` - 修饰键状态（会被修改以反映实际的 Shift 状态）
///
/// # 返回
/// 返回按键对应的字符串，如果需要转换为 Shift 组合键则会自动处理
pub(crate) fn get_keystroke_key(
    vkey: VIRTUAL_KEY,
    scan_code: u32,
    modifiers: &mut Modifiers,
) -> Option<String> {
    if modifiers.shift && need_to_convert_to_shifted_key(vkey) {
        get_shifted_key(vkey, scan_code).inspect(|_| {
            modifiers.shift = false;
        })
    } else {
        get_key_from_vkey(vkey)
    }
}

fn get_key_from_vkey(vkey: VIRTUAL_KEY) -> Option<String> {
    let key_data = unsafe { MapVirtualKeyW(vkey.0 as u32, MAPVK_VK_TO_CHAR) };
    if key_data == 0 {
        return None;
    }

    // The high word contains dead key flag, the low word contains the character
    let key = char::from_u32(key_data & 0xFFFF)?;

    Some(key.to_ascii_lowercase().to_string())
}

#[inline]
fn need_to_convert_to_shifted_key(vkey: VIRTUAL_KEY) -> bool {
    matches!(
        vkey,
        VK_OEM_3
            | VK_OEM_MINUS
            | VK_OEM_PLUS
            | VK_OEM_4
            | VK_OEM_5
            | VK_OEM_6
            | VK_OEM_1
            | VK_OEM_7
            | VK_OEM_COMMA
            | VK_OEM_PERIOD
            | VK_OEM_2
            | VK_OEM_102
            | VK_OEM_8
            | VK_ABNT_C1
            | VK_0
            | VK_1
            | VK_2
            | VK_3
            | VK_4
            | VK_5
            | VK_6
            | VK_7
            | VK_8
            | VK_9
    )
}

fn get_shifted_key(vkey: VIRTUAL_KEY, scan_code: u32) -> Option<String> {
    generate_key_char(vkey, scan_code, false, true, false)
}

/// 生成按键字符
///
/// 使用 Windows ToUnicode API 将虚拟键码转换为对应的字符
///
/// # 参数
/// * `vkey` - 虚拟键码
/// * `scan_code` - 扫描码
/// * `control` - Control 键是否按下
/// * `shift` - Shift 键是否按下
/// * `alt` - Alt 键是否按下
///
/// # 返回
/// 返回转换后的字符字符串，如果无法转换则返回 None
pub(crate) fn generate_key_char(
    vkey: VIRTUAL_KEY,
    scan_code: u32,
    control: bool,
    shift: bool,
    alt: bool,
) -> Option<String> {
    let mut state = [0; 256];
    if control {
        state[VK_CONTROL.0 as usize] = 0x80;
    }
    if shift {
        state[VK_SHIFT.0 as usize] = 0x80;
    }
    if alt {
        state[VK_MENU.0 as usize] = 0x80;
    }

    let mut buffer = [0; 8];
    let len = unsafe { ToUnicode(vkey.0 as u32, scan_code, Some(&state), &mut buffer, 0x5) };

    match len {
        len if len > 0 => String::from_utf16(&buffer[..len as usize])
            .ok()
            .filter(|candidate| {
                !candidate.is_empty() && !candidate.chars().next().unwrap().is_control()
            }),
        len if len < 0 => String::from_utf16(&buffer[..(-len as usize)]).ok(),
        _ => None,
    }
}

fn get_vkey_from_key_with_us_layout(key: &str) -> Option<(u16, bool)> {
    match key {
        // ` => VK_OEM_3
        "`" => Some((VK_OEM_3.0, false)),
        "~" => Some((VK_OEM_3.0, true)),
        "1" => Some((VK_1.0, false)),
        "!" => Some((VK_1.0, true)),
        "2" => Some((VK_2.0, false)),
        "@" => Some((VK_2.0, true)),
        "3" => Some((VK_3.0, false)),
        "#" => Some((VK_3.0, true)),
        "4" => Some((VK_4.0, false)),
        "$" => Some((VK_4.0, true)),
        "5" => Some((VK_5.0, false)),
        "%" => Some((VK_5.0, true)),
        "6" => Some((VK_6.0, false)),
        "^" => Some((VK_6.0, true)),
        "7" => Some((VK_7.0, false)),
        "&" => Some((VK_7.0, true)),
        "8" => Some((VK_8.0, false)),
        "*" => Some((VK_8.0, true)),
        "9" => Some((VK_9.0, false)),
        "(" => Some((VK_9.0, true)),
        "0" => Some((VK_0.0, false)),
        ")" => Some((VK_0.0, true)),
        "-" => Some((VK_OEM_MINUS.0, false)),
        "_" => Some((VK_OEM_MINUS.0, true)),
        "=" => Some((VK_OEM_PLUS.0, false)),
        "+" => Some((VK_OEM_PLUS.0, true)),
        "[" => Some((VK_OEM_4.0, false)),
        "{" => Some((VK_OEM_4.0, true)),
        "]" => Some((VK_OEM_6.0, false)),
        "}" => Some((VK_OEM_6.0, true)),
        "\\" => Some((VK_OEM_5.0, false)),
        "|" => Some((VK_OEM_5.0, true)),
        ";" => Some((VK_OEM_1.0, false)),
        ":" => Some((VK_OEM_1.0, true)),
        "'" => Some((VK_OEM_7.0, false)),
        "\"" => Some((VK_OEM_7.0, true)),
        "," => Some((VK_OEM_COMMA.0, false)),
        "<" => Some((VK_OEM_COMMA.0, true)),
        "." => Some((VK_OEM_PERIOD.0, false)),
        ">" => Some((VK_OEM_PERIOD.0, true)),
        "/" => Some((VK_OEM_2.0, false)),
        "?" => Some((VK_OEM_2.0, true)),
        _ => None,
    }
}

const CANDIDATE_VKEYS: &[VIRTUAL_KEY] = &[
    VK_OEM_3,
    VK_OEM_MINUS,
    VK_OEM_PLUS,
    VK_OEM_4,
    VK_OEM_5,
    VK_OEM_6,
    VK_OEM_1,
    VK_OEM_7,
    VK_OEM_COMMA,
    VK_OEM_PERIOD,
    VK_OEM_2,
    VK_OEM_102,
    VK_OEM_8,
    VK_ABNT_C1,
    VK_0,
    VK_1,
    VK_2,
    VK_3,
    VK_4,
    VK_5,
    VK_6,
    VK_7,
    VK_8,
    VK_9,
];

#[cfg(test)]
mod tests {
    use crate::WindowsKeyboardMapper;
    use gpui::{Keystroke, Modifiers, PlatformKeyboardMapper};

    #[test]
    fn test_keyboard_mapper() {
        let mapper = WindowsKeyboardMapper::new();

        // Normal case
        let keystroke = Keystroke {
            modifiers: Modifiers::control(),
            key: "a".to_string(),
            key_char: None,
        };
        let mapped = mapper.map_key_equivalent(keystroke.clone(), true);
        assert_eq!(*mapped.inner(), keystroke);
        assert_eq!(mapped.key(), "a");
        assert_eq!(*mapped.modifiers(), Modifiers::control());

        // Shifted case, ctrl-$
        let keystroke = Keystroke {
            modifiers: Modifiers::control(),
            key: "$".to_string(),
            key_char: None,
        };
        let mapped = mapper.map_key_equivalent(keystroke.clone(), true);
        assert_eq!(*mapped.inner(), keystroke);
        assert_eq!(mapped.key(), "4");
        assert_eq!(*mapped.modifiers(), Modifiers::control_shift());

        // Shifted case, but shift is true
        let keystroke = Keystroke {
            modifiers: Modifiers::control_shift(),
            key: "$".to_string(),
            key_char: None,
        };
        let mapped = mapper.map_key_equivalent(keystroke, true);
        assert_eq!(mapped.inner().modifiers, Modifiers::control());
        assert_eq!(mapped.key(), "4");
        assert_eq!(*mapped.modifiers(), Modifiers::control_shift());

        // Windows style
        let keystroke = Keystroke {
            modifiers: Modifiers::control_shift(),
            key: "4".to_string(),
            key_char: None,
        };
        let mapped = mapper.map_key_equivalent(keystroke, true);
        assert_eq!(mapped.inner().modifiers, Modifiers::control());
        assert_eq!(mapped.inner().key, "$");
        assert_eq!(mapped.key(), "4");
        assert_eq!(*mapped.modifiers(), Modifiers::control_shift());
    }
}
