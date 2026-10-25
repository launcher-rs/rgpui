// 生产代码禁用 `unwrap` / `expect` / `panic!`（与 rgpui 核心同一口径）；
// 测试代码由根目录 `clippy.toml` 的 `allow-*-in-tests` 放行。
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod block;
mod config;
mod external;
mod mappings;
mod middleware;
mod terminal;
mod terminal_element;
mod terminal_view;
mod theme_manager;

pub use block::{Block, BlockDetection};
pub use config::{TerminalConfig, TerminalTheme, TerminalThemeConfig, hsla_from_rgb};
pub use external::{ExternalHandles, FeedHandle, PtyBoundMessage, PtyBoundReceiver};
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
