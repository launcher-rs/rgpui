/// 输入组件子系统 - 从 rgpui-component 移植。
///
/// 包含输入框、文本框、数字输入、掩码输入等组件。
/// 裁剪了 LSP 集成、搜索面板、弹窗等非核心功能。
use crate::App;

/// 掩码字符，用于密码输入字段。
pub(super) const MASK_CHAR: char = '*';

pub mod async_file_loader;
/// 括号自动闭合（键入拦截），见 [`InputState::auto_close_pairs`]。
/// `editor` feature 门控。
#[cfg(feature = "editor")]
mod auto_close;
mod auto_scroll;
mod blink_cursor;
/// 括号匹配高亮，见 [`InputState::bracket_match`]。
/// `editor` feature 门控。
#[cfg(feature = "editor")]
mod bracket_match;
mod change;
mod clear_button;
mod content_type;
/// 输入框右键菜单（默认菜单 + 用户自定义），见 [`InputContextMenuBuilder`]。
pub mod context_menu;
/// 当前行高亮，见 [`InputState::current_line_highlight`]。
/// `editor` feature 门控。
#[cfg(feature = "editor")]
mod current_line;
mod cursor;
/// 文本装饰集合（Monaco 式多区间高亮），见 [`TextDecorationCollection`]。
pub mod decorations;
mod display_map;
mod element;
mod history;
mod indent;
mod input;
mod layout;
/// 行操作命令，见 `InputState` 行方法（`editor` feature 门控）。
#[cfg(feature = "editor")]
mod line_ops;
mod mask_pattern;
mod mode;
mod movement;
/// 多光标多选区，见 [`InputState::has_multiple_cursors`]（`editor` feature 门控）。
#[cfg(feature = "editor")]
mod multicursor;
mod number_input;
mod rope_ext;
mod selection;
mod state;
mod word_selection;

pub use async_file_loader::{AsyncFileLoader, FileLoadResult, LargeFileConfig, LoadProgress};
pub(crate) use clear_button::clear_button;
pub use content_type::InputContentType;
pub use context_menu::InputContextMenuBuilder;
pub use cursor::*;
pub use decorations::{TextDecoration, TextDecorationCollection};
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
