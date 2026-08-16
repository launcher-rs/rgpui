use std::fmt::Debug;

use super::Selection;
use super::history::HistoryItem;

/// 一次文本变更，用于撤销/重做。
#[derive(Debug, PartialEq, Clone)]
pub struct Change {
    /// 旧文本的范围。
    pub(crate) old_range: Selection,
    /// 旧文本内容。
    pub(crate) old_text: String,
    /// 新文本的范围。
    pub(crate) new_range: Selection,
    /// 新文本内容。
    pub(crate) new_text: String,
    version: usize,
}

impl Change {
    /// 创建新的变更。
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
