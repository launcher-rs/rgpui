use crate::{AnyElement, ParentElement, RenderOnce, StyleRefinement};
use smallvec::SmallVec;

use crate::{Selectable, StyledExt, list::ListItem};

/// 列表分隔条目。
pub struct ListSeparatorItem {
    style: StyleRefinement,
    children: SmallVec<[AnyElement; 2]>,
}

impl ListSeparatorItem {
    /// 创建新的列表分隔条目。
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: SmallVec::new(),
        }
    }
}

impl Default for ListSeparatorItem {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for ListSeparatorItem {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Selectable for ListSeparatorItem {
    fn selected(self, _: bool) -> Self {
        self
    }

    fn is_selected(&self) -> bool {
        false
    }
}

impl RenderOnce for ListSeparatorItem {
    fn render(self, _: &mut crate::Window, _: &mut crate::App) -> impl crate::IntoElement {
        ListItem::new("separator")
            .refine_style(&self.style)
            .children(self.children)
            .disabled(true)
    }
}
