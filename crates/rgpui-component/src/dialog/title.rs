use rgpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StyleRefinement, Styled, Window, div, relative,
};

use crate::StyledExt as _;

/// 对话框标题组件
///
/// 用于显示对话框的标题文字，通常放置在 DialogHeader 内部。
#[derive(IntoElement)]
pub struct DialogTitle {
    /// 样式 refinement
    style: StyleRefinement,
    /// 子元素列表
    children: Vec<AnyElement>,
}

impl DialogTitle {
    /// 创建新的对话框标题组件
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: vec![],
        }
    }
}

impl ParentElement for DialogTitle {
    /// 向标题组件中添加子元素
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for DialogTitle {
    /// 获取可修改的样式引用
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DialogTitle {
    /// 渲染对话框标题
    ///
    /// 使用基础字号、半粗体、行高为 1 的样式。
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
