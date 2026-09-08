//! 多行文本域（表单侧）：固定行数高度的多行输入框。
//!
//! 与 [`Input`] 的区别：高度按 `rows × 行高` 固定（多行 `Input` 默认撑满父容器，
//! `rows` 只对自动增长模式有效），且不带行号/折叠/缩进引导线等编辑器铬。
//! 绑定的 [`InputState`] 须为多行（`multi_line(true)`），回车即换行。

use crate::styled_ext::StyledExt as _;
use crate::{App, Disableable, Entity, IntoElement, RenderOnce, StyleRefinement, Styled, Window};

use super::{Input, InputState};

/// 多行文本域：固定行高的表单多行输入。
#[derive(IntoElement)]
pub struct TextArea {
    state: Entity<InputState>,
    rows: usize,
    read_only: bool,
    disabled: bool,
    style: StyleRefinement,
}

impl TextArea {
    /// 创建一个绑定到 [`InputState`] 的 [`TextArea`] 元素（默认 3 行）。
    pub fn new(state: &Entity<InputState>) -> Self {
        Self {
            state: state.clone(),
            rows: 3,
            read_only: false,
            disabled: false,
            style: StyleRefinement::default(),
        }
    }

    /// 设置可见行数（至少 1 行），高度按当前行高折算。
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows;
        self
    }

    /// 设置只读（可选可复制，不可编辑）。
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }
}

impl Disableable for TextArea {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Styled for TextArea {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TextArea {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let rows = self.rows.max(1);
        Input::new(&self.state)
            .read_only(self.read_only)
            .disabled(self.disabled)
            .h(rows as f32 * window.line_height())
            .refine_style(&self.style)
    }
}
