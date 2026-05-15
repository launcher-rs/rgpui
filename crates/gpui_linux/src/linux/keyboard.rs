//! Linux 键盘布局模块。
//!
//! 提供基于 XKB 的键盘布局抽象，支持动态布局切换。

use gpui::{PlatformKeyboardLayout, SharedString};

/// Linux 键盘布局实现。
///
/// 封装键盘布局的名称信息，用于在布局切换时向 GPUI 报告当前布局。
#[derive(Clone)]
pub(crate) struct LinuxKeyboardLayout {
    name: SharedString,
}

impl PlatformKeyboardLayout for LinuxKeyboardLayout {
    fn id(&self) -> &str {
        &self.name
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl LinuxKeyboardLayout {
    pub(crate) fn new(name: SharedString) -> Self {
        Self { name }
    }
}
