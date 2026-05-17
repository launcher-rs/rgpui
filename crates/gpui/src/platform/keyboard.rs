use crate::collections::HashMap;

use crate::{KeybindingKeystroke, Keystroke};

/// 平台特定键盘布局的 trait
pub trait PlatformKeyboardLayout {
    /// 获取键盘布局 ID，该 ID 对布局应该是唯一的
    fn id(&self) -> &str;
    /// 获取键盘布局显示名称
    fn name(&self) -> &str;
}

/// 平台特定键盘映射的 trait
pub trait PlatformKeyboardMapper {
    /// 将按键等效映射为平台特定的表示
    fn map_key_equivalent(
        &self,
        keystroke: Keystroke,
        use_key_equivalents: bool,
    ) -> KeybindingKeystroke;
    /// 获取当前键盘布局的按键等效映射，
    /// 仅在 macOS 上使用
    fn get_key_equivalents(&self) -> Option<&HashMap<char, char>>;
}

/// 平台键盘映射器的虚拟实现
pub struct DummyKeyboardMapper;

impl PlatformKeyboardMapper for DummyKeyboardMapper {
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
