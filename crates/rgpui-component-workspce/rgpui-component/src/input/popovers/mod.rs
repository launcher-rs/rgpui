mod context_menu;

pub(crate) use context_menu::*;

use rgpui::{App, Entity, IntoElement};

pub(crate) enum ContextMenu {
    RightClick(Entity<InputContextMenu>),
}

impl ContextMenu {
    pub(crate) fn is_open(&self, cx: &App) -> bool {
        match self {
            ContextMenu::RightClick(menu) => menu.read(cx).is_open(),
        }
    }

    pub(crate) fn render(&self) -> impl IntoElement {
        match self {
            ContextMenu::RightClick(menu) => menu.clone().into_any_element(),
        }
    }
}
