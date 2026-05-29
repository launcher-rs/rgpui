use std::borrow::Cow;

use crate::{Event, TerminalContent};

/// Describes where input bytes originated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputOrigin {
    Keystroke,
    Text,
    Paste,
    Mouse,
    Scroll,
    Focus,
    Clipboard,
    System,
    Programmatic,
}

/// Middleware for observing or transforming terminal input/output.
pub trait TerminalMiddleware: Send + Sync {
    /// Inspect or transform input bytes before they reach the PTY.
    /// Returning `None` drops the input.
    fn on_input(
        &self,
        input: Cow<'static, [u8]>,
        _origin: InputOrigin,
    ) -> Option<Cow<'static, [u8]>> {
        Some(input)
    }

    /// Observe terminal events emitted to the view layer.
    fn on_event(&self, _event: &Event) {}

    /// Observe rendered output snapshots.
    fn on_output(&self, _content: &TerminalContent) {}

    /// Observe raw PTY output bytes (for future OSC 133 processing).
    fn on_raw_pty_output(&self, _bytes: &[u8]) {}
}
