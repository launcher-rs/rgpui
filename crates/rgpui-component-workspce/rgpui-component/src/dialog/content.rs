use rgpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, StyleRefinement, Styled, Window,
};

use crate::{ActiveTheme as _, StyledExt as _, v_flex};

/// 对话框内容容器组件
///
/// 用于包裹对话框的主体内容区域，支持滚动和自定义样式。
#[derive(IntoElement)]
pub struct DialogContent {
    /// 样式 refinement
    style: StyleRefinement,
    /// 子元素列表
    children: Vec<AnyElement>,
}

impl DialogContent {
    /// 创建新的对话框内容容器
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
}

impl ParentElement for DialogContent {
    /// 向内容容器中添加子元素
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for DialogContent {
    /// 获取可修改的样式引用
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DialogContent {
    /// 渲染对话框内容容器
    ///
    /// 使用垂直弹性布局，宽度撑满，高度自适应，带圆角。
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        v_flex()
            .w_full()
            .flex_1()
            .rounded(cx.theme().radius_lg)
            .refine_style(&self.style)
            .children(self.children)
    }
}
