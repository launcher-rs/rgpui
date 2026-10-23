//! 表单输入子系统：单行/多行输入框及配套组件。
//!
//! 对外保持 `input_ui::` 扁平路径（经父模块重导出），内部组织见各文件。

/// 子模块仅限 `input_ui` 内部经 `input::` 路径引用（`pub(super)`），公开 API 走下方重导出。
pub(super) mod clear_button;
pub(super) mod content_type;
pub(super) mod element;
pub(super) mod mask_pattern;
pub(super) mod mode;
pub(super) mod number_input;
pub(super) mod state;
pub(super) mod textarea;
pub(super) mod widget;

pub(crate) use clear_button::clear_button;
pub use content_type::InputContentType;
/// 折叠/行号布局常量，仅 `editor/` 覆盖层使用（`editor` feature 门控）。
#[cfg(feature = "editor")]
pub(crate) use element::{FOLD_ICON_HITBOX_WIDTH, LINE_NUMBER_RIGHT_MARGIN};
pub use mask_pattern::MaskPattern;
pub use number_input::{NumberInput, NumberInputEvent, NumberStep};
pub use state::*;
pub use textarea::TextArea;
pub use widget::Input;
pub(crate) use widget::input_style;

pub(crate) use state::init as init_input_state;
