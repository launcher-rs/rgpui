pub(crate) mod cache;
mod delegate;
mod list;
mod list_item;
mod loading;
mod separator_item;

pub use delegate::*;
pub use list::*;
pub use list_item::*;
use schemars::JsonSchema;
pub use separator_item::*;
use serde::{Deserialize, Serialize};

/// 列表组件设置
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListSettings {
    /// 是否在 ListItem 上使用活动高亮样式，默认为 true
    pub active_highlight: bool,
}

impl Default for ListSettings {
    fn default() -> Self {
        Self {
            active_highlight: true,
        }
    }
}
