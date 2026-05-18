use rgpui::{App, actions};

pub mod highlighter;
pub mod input;

pub use input::{Input as Editor, InputState as EditorState, Rope, RopeExt, RopeLines};

pub(crate) mod actions {
    pub use rgpui_component::actions::*;
}

/// 初始化编辑器部件的快捷键和内部状态。
pub fn init(cx: &mut App) {
    input::init(cx);
}
