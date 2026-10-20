//! macOS 全局快捷键实现
//!
//! 使用 Carbon HIToolbox `RegisterEventHotKey` API 实现系统级热键注册

use std::collections::HashMap;

use rgpui::{Keystroke, Result};

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
    ///
    /// # 注意
    /// 当前实现仅为存根（stub），仅记录到 HashMap 而未调用 Carbon
    /// `RegisterEventHotKey` API。全局热键实际不会生效。
    /// 待实现 Carbon EventManager 集成后可正常工作。
    pub fn register(&mut self, id: i32, keystroke: &Keystroke) -> Result<()> {
        log::warn!(
            "macOS 全局热键注册暂未实现（存根），快捷键 {:?} 不会生效",
            keystroke
        );
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
}
