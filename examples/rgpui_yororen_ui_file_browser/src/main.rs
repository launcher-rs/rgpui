//! yororen-ui File Browser Demo
//!
//! Demonstrates rendering and interacting with a complex hierarchical data structure:
//! - Directory tree (Tree + TreeItem)
//! - Icons
//! - Empty state
//! - Right-click popover menu (copy/paste)
//!
//! ## Run
//! ```bash
//! cd demo/file_browser && cargo run
//! ```

mod actions;
mod clipboard;
mod components;
mod file_browser_app;
mod format;
mod fs_ops;
mod scan;
mod state;

use rgpui::{App, AppContext, WindowOptions, px, size};
use rgpui_platform::application;
use rgpui_yororen_ui::assets::UiAsset;
use rgpui_yororen_ui::component;
use rgpui_yororen_ui::i18n::{I18n, Locale};
use rgpui_yororen_ui::theme::GlobalTheme;

fn main() {
    let app = application().with_assets(UiAsset);

    app.run(|cx: &mut App| {
        component::init(cx);
        cx.set_global(GlobalTheme::new(cx.window_appearance()));
        cx.set_global(I18n::with_embedded(Locale::new("en").unwrap()));
        let file_browser_state = state::FileBrowserState::new(cx);
        cx.set_global(file_browser_state);

        let options = WindowOptions {
            window_bounds: Some(rgpui::WindowBounds::Windowed(rgpui::Bounds::centered(
                None,
                size(px(980.0), px(680.0)),
                cx,
            ))),
            ..Default::default()
        };

        cx.open_window(options, |_, cx| {
            cx.new(|cx| file_browser_app::FileBrowserApp::new(cx))
        })
        .unwrap();
    });
}
