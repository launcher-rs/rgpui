use rgpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StyleRefinement, Styled, Window, div,
};

use crate::{ActiveTheme as _, StyledExt as _};

/// 对话框描述文本组件
///
/// 通常在 DialogHeader 中使用，为对话框标题提供补充说明信息。
///
/// # 示例
///
/// ```ignore
/// DialogDescription::new("此操作无法撤销。")
/// ```
#[derive(IntoElement)]
pub struct DialogDescription {
    /// 样式 refinement
    style: StyleRefinement,
    /// 子元素列表
    children: Vec<AnyElement>,
}

impl DialogDescription {
    /// 创建新的对话框描述组件
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: vec![],
        }
    }
}

impl ParentElement for DialogDescription {
    /// 向描述组件中添加子元素
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for DialogDescription {
    /// 获取可修改的样式引用
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DialogDescription {
    /// 渲染对话框描述文本
    ///
    /// 使用小号字体，颜色为主题中的 muted_foreground 色。
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .id("dialog-description")
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .refine_style(&self.style)
            .children(self.children)
    }
}
