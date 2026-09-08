//! 代码编辑器子系统（`editor` feature 门控）：`EditorState` + `Editor` 组件
//! 及行操作/多光标/自动闭合/括号匹配/当前行等编辑器功能。
//!
//! 与表单 `input/` 共享 `super::core::TextCore`，互不复制文本逻辑。

/// 子模块仅限 `input_ui` 内部经 `editor::` 路径引用（`pub(super)`），公开 API 走下方重导出。
pub(super) mod auto_close;
pub(super) mod bracket_match;
pub(super) mod current_line;
pub(super) mod editor_ui;
pub(super) mod line_ops;
pub(super) mod multicursor;
pub(super) mod state;

pub use editor_ui::Editor;
pub use state::EditorState;

// /// `input/` 动作与选择类型在 `editor/` 内的短路径（`pub(super)`，不进公开 API）。
pub(super) use super::{
    Selection,
    input::{
        AddCursorAbove, AddCursorBelow, CopyLine, DeleteLine, JoinLines, MoveLineDown, MoveLineUp,
        ToggleLineComment,
    },
};
