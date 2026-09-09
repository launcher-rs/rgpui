/// 输入组件子系统 - 从 rgpui-component 移植。
///
/// 包含输入框、文本框、数字输入、掩码输入等组件。
/// 裁剪了 LSP 集成、搜索面板、弹窗等非核心功能。
///
/// 组织：`input/` 表单输入，`editor/` 代码编辑器（`editor` feature 门控），
/// 其余为两者共享（文本核/装饰/历史/映射等）。对外保持 `input_ui::` 扁平路径。
use crate::App;

/// 掩码字符，用于密码输入字段。
pub(super) const MASK_CHAR: char = '*';

pub mod async_file_loader;
mod auto_scroll;
mod blink_cursor;
mod change;
/// 输入框右键菜单（默认菜单 + 用户自定义），见 [`InputContextMenuBuilder`]。
pub mod context_menu;
/// 文本核心：表单与编辑器共享的文本地基（P1 抽核，见设计文档）。
mod core;
mod cursor;
/// 文本装饰集合（Monaco 式多区间高亮），见 [`TextDecorationCollection`]。
pub mod decorations;
mod display_map;
/// 代码编辑器子系统（`editor` feature 门控）。
#[cfg(feature = "editor")]
mod editor;
mod history;
mod indent;
/// 表单输入子系统。
mod input;
mod layout;
mod movement;
mod rope_ext;
mod selection;
mod word_selection;

pub use async_file_loader::{AsyncFileLoader, FileLoadResult, LargeFileConfig, LoadProgress};
pub use context_menu::InputContextMenuBuilder;
pub use cursor::*;
pub use decorations::{TextDecoration, TextDecorationCollection};
pub use display_map::{BufferPoint, DisplayMap, DisplayPoint, FoldRange, WrappingIndent};
/// 代码编辑器状态与组件（`editor` feature 门控）。
#[cfg(feature = "editor")]
pub use editor::{
    CodeLens, CodeLensOverlay, CodeLensProvider, EditEvent, EditHandler, Editor, EditorExtension,
    EditorExtensionFactory, EditorState, InlayHint, InlayProvider, ResolvedCodeLens, VimKey,
    VimMode, editor_extension, register_editor_extension,
};
pub use history::*;
pub use indent::TabSize;
/// 编辑器动作重导出（`editor` feature 门控）。
#[cfg(feature = "editor")]
pub use input::{
    AddCursorAbove, AddCursorBelow, CopyLine, DeleteLine, JoinLines, MoveLineDown, MoveLineUp,
    ToggleLineComment,
};
pub use input::{
    Backspace, Copy, Cut, Delete, DeleteToBeginningOfLine, DeleteToEndOfLine, DeleteToNextWordEnd,
    DeleteToPreviousWordStart, Enter, Escape, Indent, IndentInline, Input, InputEvent, InputState,
    MoveDown, MoveEnd, MoveHome, MoveLeft, MovePageDown, MovePageUp, MoveRight, MoveToEnd,
    MoveToEndOfLine, MoveToNextWord, MoveToPreviousWord, MoveToStart, MoveToStartOfLine, MoveUp,
    Outdent, OutdentInline, Paste, Redo, SelectAll, SelectToEnd, SelectToEndOfLine,
    SelectToNextWordEnd, SelectToPreviousWordStart, SelectToStart, SelectToStartOfLine,
    ShowCharacterPalette, Undo,
};
pub use input::{
    InputContentType, MaskPattern, NumberInput, NumberInputEvent, NumberStep, TextArea,
};
pub(crate) use layout::{LastLayout, WhitespaceIndicators};
pub use rope_ext::{InputEdit, Point, Position, RopeExt, RopeLines};
pub use ropey::Rope;

/// 初始化输入子系统，注册全局按键绑定。
pub fn init(cx: &mut App) {
    input::init_input_state(cx);
    // vim 键位后注册（同节点 tie 靠后优先，覆盖回车/退格/删除等默认行为）。
    #[cfg(feature = "editor")]
    editor::vim::init(cx);
}
