//! Password input component.
//!
//! A single-line password input component with masking.

mod actions;
mod component;
mod element;
mod state;

pub use actions::*;
pub use component::*;
pub use state::*;

use rgpui::App;
use rgpui::ElementId;

/// Creates a new password input element.
/// Requires an id to be set via `.id()` for internal state management.
pub fn password_input(id: impl Into<ElementId>) -> PasswordInput {
    PasswordInput::new().id(id)
}

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        rgpui::KeyBinding::new("backspace", Backspace, Some("UIPasswordInput")),
        rgpui::KeyBinding::new("delete", Delete, Some("UIPasswordInput")),
        rgpui::KeyBinding::new("left", Left, Some("UIPasswordInput")),
        rgpui::KeyBinding::new("right", Right, Some("UIPasswordInput")),
        rgpui::KeyBinding::new("shift-left", SelectLeft, Some("UIPasswordInput")),
        rgpui::KeyBinding::new("shift-right", SelectRight, Some("UIPasswordInput")),
        rgpui::KeyBinding::new("secondary-a", SelectAll, Some("UIPasswordInput")),
        rgpui::KeyBinding::new("secondary-v", Paste, Some("UIPasswordInput")),
        rgpui::KeyBinding::new("secondary-c", Copy, Some("UIPasswordInput")),
        rgpui::KeyBinding::new("secondary-x", Cut, Some("UIPasswordInput")),
        rgpui::KeyBinding::new("home", Home, Some("UIPasswordInput")),
        rgpui::KeyBinding::new("end", End, Some("UIPasswordInput")),
        rgpui::KeyBinding::new(
            "ctrl-secondary-space",
            ShowCharacterPalette,
            Some("UIPasswordInput"),
        ),
    ]);
}
