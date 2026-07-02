use std::fmt::Debug;

use crate::{history::HistoryItem, input::Selection};

/// 输入框内容变更记录
#[derive(Debug, PartialEq, Clone)]
pub struct Change {
    /// 变更前的选区范围
    pub(crate) old_range: Selection,
    /// 变更前的文本
    pub(crate) old_text: String,
    /// 变更后的选区范围
    pub(crate) new_range: Selection,
    /// 变更后的文本
    pub(crate) new_text: String,
    /// 版本号
    version: usize,
}

impl Change {
    /// 创建新的变更记录
    pub fn new(
        old_range: impl Into<Selection>,
        old_text: &str,
        new_range: impl Into<Selection>,
        new_text: &str,
    ) -> Self {
        Self {
            old_range: old_range.into(),
            old_text: old_text.to_string(),
            new_range: new_range.into(),
            new_text: new_text.to_string(),
            version: 0,
        }
    }
}

impl HistoryItem for Change {
    fn version(&self) -> usize {
        self.version
    }

    fn set_version(&mut self, version: usize) {
        self.version = version;
    }
}
