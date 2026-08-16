use crate::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StyleRefinement, Styled, Window, div, relative,
};

use crate::StyledExt as _;

/// 对话框标题元素。
#[derive(IntoElement)]
pub struct DialogTitle {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl DialogTitle {
    /// 创建新的对话框标题元素。
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: vec![],
        }
    }
}

impl ParentElement for DialogTitle {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for DialogTitle {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DialogTitle {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .id("dialog-title")
            .text_base()
            .font_semibold()
            .line_height(relative(1.))
            .refine_style(&self.style)
            .children(self.children)
    }
}
