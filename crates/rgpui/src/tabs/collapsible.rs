use crate::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, StyleRefinement, Styled, StyledExt,
    Window, v_flex,
};

/// 折叠面板的子元素，分为普通元素与内容元素两种。
enum CollapsibleChild {
    /// 普通子元素，始终渲染。
    Element(AnyElement),
    /// 内容子元素，仅在展开时渲染。
    Content(AnyElement),
}

impl CollapsibleChild {
    /// 是否为内容子元素。
    fn is_content(&self) -> bool {
        matches!(self, CollapsibleChild::Content(_))
    }
}

/// 一个可展开/折叠的交互式元素。
#[derive(IntoElement)]
pub struct Collapsible {
    style: StyleRefinement,
    children: Vec<CollapsibleChild>,
    open: bool,
}

impl Collapsible {
    /// 创建一个新的 `Collapsible` 实例。
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            open: false,
            children: vec![],
        }
    }

    /// 设置折叠面板是否展开，默认为 false。
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// 设置折叠面板的内容。
    ///
    /// 当 `open` 为 false 时内容会被隐藏。
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.children
            .push(CollapsibleChild::Content(content.into_any_element()));
        self
    }
}

impl Styled for Collapsible {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for Collapsible {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children
            .extend(elements.into_iter().map(|el| CollapsibleChild::Element(el)));
    }
}

impl RenderOnce for Collapsible {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        v_flex()
            .refine_style(&self.style)
            .children(self.children.into_iter().filter_map(|child| {
                if child.is_content() && !self.open {
                    None
                } else {
                    match child {
                        CollapsibleChild::Element(el) => Some(el),
                        CollapsibleChild::Content(el) => Some(el),
                    }
                }
            }))
    }
}
