//! Text area component.
//!
//! A multi-line text input component with editing capabilities.

mod actions;
mod component;
mod element;
mod layout;
mod state;

pub use actions::*;
pub use component::*;
pub use state::*;

use rgpui::{App, ElementId};

/// Creates a new text area element.
/// Requires an id to be set via `.id()` for internal state management.
pub fn text_area(id: impl Into<ElementId>) -> TextArea {
    TextArea::new().id(id)
}

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        rgpui::KeyBinding::new("backspace", Backspace, Some("UITextArea")),
        rgpui::KeyBinding::new("delete", Delete, Some("UITextArea")),
        rgpui::KeyBinding::new("left", Left, Some("UITextArea")),
        rgpui::KeyBinding::new("right", Right, Some("UITextArea")),
        rgpui::KeyBinding::new("up", Up, Some("UITextArea")),
        rgpui::KeyBinding::new("down", Down, Some("UITextArea")),
        rgpui::KeyBinding::new("shift-left", SelectLeft, Some("UITextArea")),
        rgpui::KeyBinding::new("shift-right", SelectRight, Some("UITextArea")),
        rgpui::KeyBinding::new("shift-up", SelectUp, Some("UITextArea")),
        rgpui::KeyBinding::new("shift-down", SelectDown, Some("UITextArea")),
        rgpui::KeyBinding::new("secondary-a", SelectAll, Some("UITextArea")),
        rgpui::KeyBinding::new("secondary-v", Paste, Some("UITextArea")),
        rgpui::KeyBinding::new("secondary-c", Copy, Some("UITextArea")),
        rgpui::KeyBinding::new("secondary-x", Cut, Some("UITextArea")),
        rgpui::KeyBinding::new("home", Home, Some("UITextArea")),
        rgpui::KeyBinding::new("end", End, Some("UITextArea")),
        rgpui::KeyBinding::new("enter", Enter, Some("UITextArea")),
        rgpui::KeyBinding::new(
            "ctrl-secondary-space",
            ShowCharacterPalette,
            Some("UITextArea"),
        ),
    ]);
}
