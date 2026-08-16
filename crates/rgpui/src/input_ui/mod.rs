/// 输入组件子系统 - 从 rgpui-component 移植。
///
/// 包含输入框、文本框、数字输入、掩码输入等组件。
/// 裁剪了 LSP 集成、搜索面板、弹窗等非核心功能。

/// 掩码字符，用于密码输入字段。
pub(super) const MASK_CHAR: char = '?';

mod auto_scroll;
mod blink_cursor;
mod change;
mod content_type;
mod cursor;
mod display_map;
mod history;
mod indent;
mod layout;
mod mask_pattern;
mod mode;
mod rope_ext;
mod selection;
mod word_selection;

pub(crate) use blink_cursor::BlinkCursor;
pub(crate) use change::*;
pub use content_type::InputContentType;
pub use cursor::*;
pub use display_map::{BufferPoint, DisplayMap, DisplayPoint, FoldRange, WrappingIndent};
pub use history::*;
pub use indent::TabSize;
pub(crate) use layout::{LastLayout, WhitespaceIndicators};
pub use mask_pattern::MaskPattern;
pub(crate) use mode::InputMode;
pub use rope_ext::{InputEdit, Point, Position, RopeExt, RopeLines};
pub use ropey::Rope;