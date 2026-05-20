mod block;
mod config;
mod mappings;
mod middleware;
mod terminal;
mod terminal_element;
mod terminal_view;
mod theme_manager;

pub use block::{Block, BlockDetection};
pub use config::{TerminalConfig, TerminalTheme, TerminalThemeConfig, hsla_from_rgb};
pub use middleware::{InputOrigin, TerminalMiddleware};
pub use terminal::{
    Event, IndexedCell, Terminal, TerminalBounds, TerminalBuilder, TerminalContent, ZedListener,
};
pub use terminal_element::{TerminalElement, TextStyle, convert_color};
pub use terminal_view::{
    ChangeTheme, Clear, Copy, Paste, ScrollLineDown, ScrollLineUp, ScrollPageDown, ScrollPageUp,
    SelectAll, TerminalView,
};
pub use theme_manager::{ThemeDefinition, ThemeManager};
