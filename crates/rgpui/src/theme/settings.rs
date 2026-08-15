use rgpui::{Anchor, Edges, Pixels, px};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 标题栏高度（用于避让通知等浮层与标题栏重叠）。
pub const TITLE_BAR_HEIGHT: Pixels = px(34.);

/// 滚动条显示模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Default, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ScrollbarShow {
    /// 滚动时显示，空闲后淡出。
    #[default]
    Scrolling,
    /// 悬停时显示。
    Hover,
    /// 始终显示。
    Always,
}

impl ScrollbarShow {
    pub fn is_hover(&self) -> bool {
        matches!(self, Self::Hover)
    }

    pub fn is_always(&self) -> bool {
        matches!(self, Self::Always)
    }
}

/// List 组件设置。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListSettings {
    /// 是否在 ListItem 上使用 active 高亮样式，默认开启。
    pub active_highlight: bool,
}

impl Default for ListSettings {
    fn default() -> Self {
        Self {
            active_highlight: true,
        }
    }
}

/// Sheet 组件设置。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SheetSettings {
    /// Sheet 顶部外边距，默认是 [`TITLE_BAR_HEIGHT`]。
    pub margin_top: Pixels,
}

impl Default for SheetSettings {
    fn default() -> Self {
        Self {
            margin_top: TITLE_BAR_HEIGHT,
        }
    }
}

/// 通知组件设置。
#[derive(Debug, Clone)]
pub struct NotificationSettings {
    /// 通知的放置位置，默认：右上角。
    pub placement: Anchor,
    /// 通知相对窗口边缘的边距。
    pub margins: Edges<Pixels>,
    /// 同时显示的最大通知数，默认 10。
    pub max_items: usize,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        let offset = px(16.);
        Self {
            placement: Anchor::TopRight,
            margins: Edges {
                top: TITLE_BAR_HEIGHT + offset,
                right: offset,
                bottom: offset,
                left: offset,
            },
            max_items: 10,
        }
    }
}