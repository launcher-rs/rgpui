use rgpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, StyleRefinement, Styled, Window,
};

use crate::{StyledExt as _, v_flex};

/// 对话框头部组件
///
/// 通常包含 DialogTitle 和 DialogDescription，用于展示对话框的标题区域。
///
/// # 示例
///
/// ```ignore
/// DialogHeader::new()
///     .child(DialogTitle::new().child("删除账户"))
///     .child(DialogDescription::new().child("此操作无法撤销。"))
/// ```
#[derive(IntoElement)]
pub struct DialogHeader {
    /// 样式 refinement
    style: StyleRefinement,
    /// 子元素列表
    children: Vec<AnyElement>,
}

impl DialogHeader {
    /// 创建新的对话框头部组件
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
}

impl ParentElement for DialogHeader {
    /// 向头部组件中添加子元素
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for DialogHeader {
    /// 获取可修改的样式引用
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DialogHeader {
    /// 渲染对话框头部
    ///
    /// 使用垂直弹性布局，子元素之间有间距。
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        v_flex()
            .gap_2()
            .refine_style(&self.style)
            .children(self.children)
    }
}
