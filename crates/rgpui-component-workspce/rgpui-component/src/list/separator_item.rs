use rgpui::{AnyElement, ParentElement, RenderOnce, StyleRefinement};
use smallvec::SmallVec;

use crate::{Selectable, StyledExt, list::ListItem};

pub struct ListSeparatorItem {
    style: StyleRefinement,
    children: SmallVec<[AnyElement; 2]>,
}

impl ListSeparatorItem {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: SmallVec::new(),
        }
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
    fn render(self, _: &mut rgpui::Window, _: &mut rgpui::App) -> impl rgpui::IntoElement {
        ListItem::new("separator")
            .refine_style(&self.style)
            .children(self.children)
            .disabled(true)
    }
}
