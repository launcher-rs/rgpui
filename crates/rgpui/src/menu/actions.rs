use crate::Action;
use serde::Deserialize;

/// 确认动作 - 用于菜单、弹窗等 UI 的确认操作
#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = ui, no_json)]
pub struct Confirm {
    /// 是否使用辅助确认（例如 Shift+Enter）
    pub secondary: bool,
}

actions!(
    ui,
    [
        Cancel,
        SelectUp,
        SelectDown,
        SelectLeft,
        SelectRight,
        SelectFirst,
        SelectLast,
        SelectPrevColumn,
        SelectNextColumn,
        SelectPageUp,
        SelectPageDown
    ]
);
