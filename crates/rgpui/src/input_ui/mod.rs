/// 输入组件子系统 - 从 rgpui-component 移植。
///
/// 包含输入框、文本框、数字输入、掩码输入等组件。
/// 裁剪了 LSP 集成、搜索面板、弹窗等非核心功能。
use crate::App;

/// 掩码字符，用于密码输入字段。
pub(super) const MASK_CHAR: char = '*';

mod auto_scroll;
mod blink_cursor;
mod change;
mod clear_button;
mod content_type;
mod cursor;
mod decorations;
mod display_map;
mod element;
mod history;
mod indent;
mod input;
mod layout;
mod mask_pattern;
mod mode;
mod movement;
mod number_input;
mod rope_ext;
mod selection;
mod state;
mod word_selection;

pub(crate) use clear_button::clear_button;
pub use content_type::InputContentType;
pub use cursor::*;
pub(crate) use decorations::TextDecoration;
pub use display_map::{BufferPoint, DisplayMap, DisplayPoint, FoldRange, WrappingIndent};
pub use history::*;
pub use indent::TabSize;
pub use input::Input;
pub(crate) use input::input_style;
pub(crate) use layout::{LastLayout, WhitespaceIndicators};
pub use mask_pattern::MaskPattern;
pub use number_input::{NumberInput, NumberInputEvent, NumberStep};
pub use rope_ext::{InputEdit, Point, Position, RopeExt, RopeLines};
pub use ropey::Rope;
pub use state::*;

/// 初始化输入子系统，注册全局按键绑定。
pub fn init(cx: &mut App) {
    state::init(cx);
}
