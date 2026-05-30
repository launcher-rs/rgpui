//! Theme adapter for integrating gpui-component themes with gpui-term.
//!
//! This module provides utilities to convert gpui-component's ThemeColor
//! into TerminalTheme, allowing terminals to share the application's theme system.
//!
//! # Architecture
//!
//! ```text
//! rgpui-component Theme (Global)
//!     ↓
//! ThemeColor (80+ colors including base colors)
//!     ↓
//! ThemeAdapter::to_terminal_theme()
//!     ↓
//! TerminalTheme (ANSI colors + terminal colors)
//! ```
//!
//! # Color Mapping
//!
//! - background → terminal background
//! - foreground → terminal foreground
//! - caret → cursor
//! - selection → selection background
//! - base.{red,green,blue,...} → ANSI colors
//!
//! # Usage
//!
//! ```ignore
//! // Convert from gpui-component theme
//! let terminal_theme = ThemeAdapter::to_terminal_theme(component_theme);
//!
//! // Apply to terminal view
//! terminal_view.update(cx, |view, cx| {
//!     view.apply_component_theme(cx);
//! });
//! ```

use rgpui::{App, Hsla};
use rgpui_term::TerminalTheme;

/// Adapter for converting gpui-component themes to terminal themes.
pub struct ThemeAdapter;

impl ThemeAdapter {
    /// Converts a gpui-component ThemeColor to a TerminalTheme.
    ///
    /// This function maps the component's color scheme to terminal-specific colors,
    /// ensuring consistent theming across the application.
    ///
    /// # Color Mapping Strategy
    ///
    /// 1. **Basic colors**: Direct mapping from component theme
    /// 2. **ANSI colors**: Use component's base colors (red, green, blue, etc.)
    /// 3. **Bright colors**: Lighten base colors by 20%
    /// 4. **Dim colors**: Darken base colors by 20%
    ///
    /// # Parameters
    ///
    /// - `component_theme`: Reference to gpui-component's ThemeColor
    ///
    /// # Returns
    ///
    /// A fully configured TerminalTheme with colors derived from the component theme.
    pub fn to_terminal_theme(component_theme: &gpui_component::ThemeColor) -> TerminalTheme {
        use gpui_component::Colorize;

        // Extract base colors with fallbacks
        let fg = component_theme.foreground;
        let bg = component_theme.background;
        let cursor = component_theme.caret;
        let selection = component_theme.selection;

        // Extract or derive ANSI base colors
        // Use component's base colors if available, otherwise derive from theme
        let ansi = [
            component_theme
                .base_black()
                .unwrap_or_else(|| bg.lighten(0.1)), // Black
            component_theme.base_red().unwrap_or_else(Self::default_red), // Red
            component_theme
                .base_green()
                .unwrap_or_else(Self::default_green), // Green
            component_theme
                .base_yellow()
                .unwrap_or_else(Self::default_yellow), // Yellow
            component_theme
                .base_blue()
                .unwrap_or_else(Self::default_blue), // Blue
            component_theme
                .base_magenta()
                .unwrap_or_else(Self::default_magenta), // Magenta
            component_theme
                .base_cyan()
                .unwrap_or_else(Self::default_cyan), // Cyan
            component_theme.base_white().unwrap_or(fg),                   // White
        ];

        // Bright colors: lighten base colors
        let bright = [
            ansi[0].lighten(0.2),
            component_theme
                .base_red_light()
                .unwrap_or_else(|| ansi[1].lighten(0.2)),
            component_theme
                .base_green_light()
                .unwrap_or_else(|| ansi[2].lighten(0.2)),
            component_theme
                .base_yellow_light()
                .unwrap_or_else(|| ansi[3].lighten(0.2)),
            component_theme
                .base_blue_light()
                .unwrap_or_else(|| ansi[4].lighten(0.2)),
            component_theme
                .base_magenta_light()
                .unwrap_or_else(|| ansi[5].lighten(0.2)),
            component_theme
                .base_cyan_light()
                .unwrap_or_else(|| ansi[6].lighten(0.2)),
            fg.lighten(0.2),
        ];

        // Dim colors: darken base colors
        let dim = [
            ansi[0].darken(0.2),
            ansi[1].darken(0.2),
            ansi[2].darken(0.2),
            ansi[3].darken(0.2),
            ansi[4].darken(0.2),
            ansi[5].darken(0.2),
            ansi[6].darken(0.2),
            ansi[7].darken(0.2),
        ];

        TerminalTheme {
            foreground: fg,
            background: bg,
            cursor,
            selection,
            ansi,
            bright,
            dim,
            bright_foreground: bright[7],
            dim_foreground: dim[7],
        }
    }

    /// Creates a terminal theme from the global gpui-component theme.
    ///
    /// This is a convenience method that accesses the global Theme and converts it.
    pub fn from_global_theme(cx: &App) -> TerminalTheme {
        let theme = gpui_component::ActiveTheme::theme(cx);
        Self::to_terminal_theme(&theme.colors)
    }

    // Default ANSI colors (fallback when component theme doesn't define them)

    fn default_red() -> Hsla {
        gpui_term::hsla_from_rgb(0xE0, 0x6C, 0x75)
    }

    fn default_green() -> Hsla {
        gpui_term::hsla_from_rgb(0x98, 0xC3, 0x79)
    }

    fn default_yellow() -> Hsla {
        gpui_term::hsla_from_rgb(0xE5, 0xC0, 0x7B)
    }

    fn default_blue() -> Hsla {
        gpui_term::hsla_from_rgb(0x61, 0xAF, 0xEF)
    }

    fn default_magenta() -> Hsla {
        gpui_term::hsla_from_rgb(0xC6, 0x78, 0xDD)
    }

    fn default_cyan() -> Hsla {
        gpui_term::hsla_from_rgb(0x56, 0xB6, 0xC2)
    }
}

/// Extension trait for TerminalView to apply component themes.
pub trait ComponentThemeExt {
    /// Applies the current gpui-component theme to the terminal view.
    ///
    /// This updates the terminal's text_style with colors derived from
    /// the global component theme.
    fn apply_component_theme(&mut self, cx: &mut gpui::Context<Self>)
    where
        Self: Sized;

    /// Sets up a theme observer that automatically updates when the component theme changes.
    ///
    /// This ensures the terminal stays in sync with application-wide theme changes.
    fn observe_component_theme(&mut self, cx: &mut gpui::Context<Self>)
    where
        Self: Sized;
}

impl ComponentThemeExt for gpui_term::TerminalView {
    fn apply_component_theme(&mut self, cx: &mut gpui::Context<Self>) {
        let terminal_theme = ThemeAdapter::from_global_theme(cx);
        let text_style = self.text_style_mut();

        // Update colors while preserving font settings
        text_style.theme = terminal_theme.clone();
        text_style.foreground = terminal_theme.foreground;
        text_style.background = terminal_theme.background;

        cx.notify();
    }

    fn observe_component_theme(&mut self, cx: &mut gpui::Context<Self>) {
        cx.observe_global::<gpui_component::Theme>(|this, cx| {
            this.apply_component_theme(cx);
        })
        .detach();
    }
}

/// Helper to check if a gpui-component theme field exists.
///
/// This trait provides methods to safely extract optional base colors
/// from the component theme.
trait BaseColorExt {
    fn base_black(&self) -> Option<Hsla>;
    fn base_red(&self) -> Option<Hsla>;
    fn base_green(&self) -> Option<Hsla>;
    fn base_yellow(&self) -> Option<Hsla>;
    fn base_blue(&self) -> Option<Hsla>;
    fn base_magenta(&self) -> Option<Hsla>;
    fn base_cyan(&self) -> Option<Hsla>;
    fn base_white(&self) -> Option<Hsla>;
    fn base_red_light(&self) -> Option<Hsla>;
    fn base_green_light(&self) -> Option<Hsla>;
    fn base_yellow_light(&self) -> Option<Hsla>;
    fn base_blue_light(&self) -> Option<Hsla>;
    fn base_magenta_light(&self) -> Option<Hsla>;
    fn base_cyan_light(&self) -> Option<Hsla>;
}

impl BaseColorExt for gpui_component::ThemeColor {
    fn base_black(&self) -> Option<Hsla> {
        // gpui-component doesn't have explicit base_black, use background
        None
    }

    fn base_red(&self) -> Option<Hsla> {
        Some(self.red)
    }

    fn base_green(&self) -> Option<Hsla> {
        Some(self.green)
    }

    fn base_yellow(&self) -> Option<Hsla> {
        Some(self.yellow)
    }

    fn base_blue(&self) -> Option<Hsla> {
        Some(self.blue)
    }

    fn base_magenta(&self) -> Option<Hsla> {
        Some(self.magenta)
    }

    fn base_cyan(&self) -> Option<Hsla> {
        Some(self.cyan)
    }

    fn base_white(&self) -> Option<Hsla> {
        // Use foreground as white
        None
    }

    fn base_red_light(&self) -> Option<Hsla> {
        Some(self.red_light)
    }

    fn base_green_light(&self) -> Option<Hsla> {
        Some(self.green_light)
    }

    fn base_yellow_light(&self) -> Option<Hsla> {
        Some(self.yellow_light)
    }

    fn base_blue_light(&self) -> Option<Hsla> {
        Some(self.blue_light)
    }

    fn base_magenta_light(&self) -> Option<Hsla> {
        Some(self.magenta_light)
    }

    fn base_cyan_light(&self) -> Option<Hsla> {
        Some(self.cyan_light)
    }
}
