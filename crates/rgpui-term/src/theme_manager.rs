//! Theme management system for runtime theme switching.
//!
//! This module provides the ThemeManager which manages a collection of terminal themes
//! and enables dynamic theme switching at runtime without requiring application restart.
//!
//! # Architecture
//!
//! - `ThemeManager` - Central manager for all themes, implemented as a rgpui Global
//! - `ThemeDefinition` - Named theme with metadata
//! - Preset themes: One Dark, One Light, Solarized Dark, Solarized Light, Dracula, Nord, Gruvbox Dark, Gruvbox Light, GitHub Light
//! - Support for custom themes loaded from config
//!
//! # Usage
//!
//! ```ignore
//! // Initialize theme manager
//! let manager = ThemeManager::new(config);
//! cx.set_global(manager);
//!
//! // Get current theme
//! let theme = ThemeManager::global(cx).current_theme();
//!
//! // Switch theme
//! ThemeManager::global_mut(cx).set_theme("Dracula");
//! ```

use crate::{TerminalTheme, TerminalThemeConfig, hsla_from_rgb};
use rgpui::{App, Global};
use std::collections::HashMap;

/// A named theme definition with metadata.
#[derive(Clone, Debug)]
pub struct ThemeDefinition {
    pub name: String,
    pub theme: TerminalTheme,
}

/// Central theme manager for runtime theme switching.
///
/// The ThemeManager maintains a collection of available themes and tracks
/// the currently active theme. It's designed as a rgpui Global to be accessible
/// throughout the application.
pub struct ThemeManager {
    themes: HashMap<String, ThemeDefinition>,
    current_theme_name: String,
}

impl Global for ThemeManager {}

impl ThemeManager {
    /// Creates a new ThemeManager with all preset themes and optionally a custom theme from config.
    pub fn new(config_theme: Option<TerminalThemeConfig>) -> Self {
        let mut themes = HashMap::new();

        // Register all preset themes
        for definition in Self::preset_themes() {
            themes.insert(definition.name.clone(), definition);
        }

        // Add custom theme from config if provided
        if let Some(theme_config) = config_theme {
            let custom = ThemeDefinition {
                name: "Custom".to_string(),
                theme: theme_config.to_theme(),
            };
            themes.insert(custom.name.clone(), custom);
        }

        Self {
            themes,
            current_theme_name: "One Dark".to_string(),
        }
    }

    /// Returns the currently active theme.
    pub fn current_theme(&self) -> &TerminalTheme {
        &self.themes[&self.current_theme_name].theme
    }

    /// Returns the name of the currently active theme.
    pub fn current_theme_name(&self) -> &str {
        &self.current_theme_name
    }

    /// Switches to a different theme by name.
    ///
    /// Returns true if the theme was found and switched, false otherwise.
    pub fn set_theme(&mut self, name: &str) -> bool {
        if self.themes.contains_key(name) {
            self.current_theme_name = name.to_string();
            true
        } else {
            false
        }
    }

    /// Returns a list of all available theme names.
    pub fn available_themes(&self) -> Vec<String> {
        let mut names: Vec<String> = self.themes.keys().cloned().collect();
        names.sort();
        names
    }

    /// Returns a theme by name, if it exists.
    pub fn get_theme(&self, name: &str) -> Option<&TerminalTheme> {
        self.themes.get(name).map(|def| &def.theme)
    }

    /// Adds or updates a theme in the manager.
    pub fn add_theme(&mut self, definition: ThemeDefinition) {
        self.themes.insert(definition.name.clone(), definition);
    }

    /// Accesses the global ThemeManager.
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// Mutably accesses the global ThemeManager.
    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    /// Defines all preset themes.
    fn preset_themes() -> Vec<ThemeDefinition> {
        vec![
            // Dark themes
            Self::theme_one_dark(),
            Self::theme_solarized_dark(),
            Self::theme_dracula(),
            Self::theme_nord(),
            Self::theme_gruvbox_dark(),
            // Light themes
            Self::theme_one_light(),
            Self::theme_solarized_light(),
            Self::theme_github_light(),
            Self::theme_gruvbox_light(),
        ]
    }

    /// One Dark theme (default) - Based on Atom's One Dark
    fn theme_one_dark() -> ThemeDefinition {
        ThemeDefinition {
            name: "One Dark".to_string(),
            theme: TerminalTheme {
                foreground: hsla_from_rgb(0xAB, 0xB2, 0xBF),
                background: hsla_from_rgb(0x28, 0x2C, 0x34),
                cursor: hsla_from_rgb(0x52, 0x8B, 0xFF),
                selection: rgpui::Hsla {
                    h: 0.58,
                    s: 0.36,
                    l: 0.48,
                    a: 0.55,
                },
                ansi: [
                    hsla_from_rgb(0x3F, 0x44, 0x51), // Black
                    hsla_from_rgb(0xE0, 0x6C, 0x75), // Red
                    hsla_from_rgb(0x98, 0xC3, 0x79), // Green
                    hsla_from_rgb(0xE5, 0xC0, 0x7B), // Yellow
                    hsla_from_rgb(0x61, 0xAF, 0xEF), // Blue
                    hsla_from_rgb(0xC6, 0x78, 0xDD), // Magenta
                    hsla_from_rgb(0x56, 0xB6, 0xC2), // Cyan
                    hsla_from_rgb(0xAB, 0xB2, 0xBF), // White
                ],
                bright: [
                    hsla_from_rgb(0x5C, 0x63, 0x70), // Bright Black
                    hsla_from_rgb(0xE0, 0x6C, 0x75), // Bright Red
                    hsla_from_rgb(0x98, 0xC3, 0x79), // Bright Green
                    hsla_from_rgb(0xE5, 0xC0, 0x7B), // Bright Yellow
                    hsla_from_rgb(0x61, 0xAF, 0xEF), // Bright Blue
                    hsla_from_rgb(0xC6, 0x78, 0xDD), // Bright Magenta
                    hsla_from_rgb(0x56, 0xB6, 0xC2), // Bright Cyan
                    hsla_from_rgb(0xFF, 0xFF, 0xFF), // Bright White
                ],
                dim: [
                    hsla_from_rgb(0x2C, 0x31, 0x3A),
                    hsla_from_rgb(0xBE, 0x5B, 0x65),
                    hsla_from_rgb(0x7A, 0x9F, 0x60),
                    hsla_from_rgb(0xD1, 0x9A, 0x66),
                    hsla_from_rgb(0x4E, 0x88, 0xB8),
                    hsla_from_rgb(0xA0, 0x61, 0xB0),
                    hsla_from_rgb(0x44, 0x91, 0x9B),
                    hsla_from_rgb(0x8A, 0x8F, 0x98),
                ],
                bright_foreground: hsla_from_rgb(0xFF, 0xFF, 0xFF),
                dim_foreground: hsla_from_rgb(0x8A, 0x8F, 0x98),
            },
        }
    }

    /// Solarized Dark theme - Popular color scheme by Ethan Schoonover
    fn theme_solarized_dark() -> ThemeDefinition {
        ThemeDefinition {
            name: "Solarized Dark".to_string(),
            theme: TerminalTheme {
                foreground: hsla_from_rgb(0x83, 0x94, 0x96), // base0
                background: hsla_from_rgb(0x00, 0x2B, 0x36), // base03
                cursor: hsla_from_rgb(0x83, 0x94, 0x96),
                selection: rgpui::Hsla {
                    h: 0.48,
                    s: 0.25,
                    l: 0.35,
                    a: 0.50,
                },
                ansi: [
                    hsla_from_rgb(0x07, 0x36, 0x42), // base02
                    hsla_from_rgb(0xDC, 0x32, 0x2F), // red
                    hsla_from_rgb(0x85, 0x99, 0x00), // green
                    hsla_from_rgb(0xB5, 0x89, 0x00), // yellow
                    hsla_from_rgb(0x26, 0x8B, 0xD2), // blue
                    hsla_from_rgb(0xD3, 0x36, 0x82), // magenta
                    hsla_from_rgb(0x2A, 0xA1, 0x98), // cyan
                    hsla_from_rgb(0xEE, 0xE8, 0xD5), // base2
                ],
                bright: [
                    hsla_from_rgb(0x00, 0x2B, 0x36), // base03
                    hsla_from_rgb(0xCB, 0x4B, 0x16), // orange
                    hsla_from_rgb(0x58, 0x6E, 0x75), // base01
                    hsla_from_rgb(0x65, 0x7B, 0x83), // base00
                    hsla_from_rgb(0x83, 0x94, 0x96), // base0
                    hsla_from_rgb(0x6C, 0x71, 0xC4), // violet
                    hsla_from_rgb(0x93, 0xA1, 0xA1), // base1
                    hsla_from_rgb(0xFD, 0xF6, 0xE3), // base3
                ],
                dim: [
                    hsla_from_rgb(0x05, 0x28, 0x32),
                    hsla_from_rgb(0xB0, 0x28, 0x25),
                    hsla_from_rgb(0x6A, 0x7A, 0x00),
                    hsla_from_rgb(0x90, 0x6D, 0x00),
                    hsla_from_rgb(0x1E, 0x6D, 0xA8),
                    hsla_from_rgb(0xA8, 0x2B, 0x68),
                    hsla_from_rgb(0x22, 0x81, 0x79),
                    hsla_from_rgb(0xC0, 0xBA, 0xAA),
                ],
                bright_foreground: hsla_from_rgb(0xFD, 0xF6, 0xE3),
                dim_foreground: hsla_from_rgb(0x58, 0x6E, 0x75),
            },
        }
    }

    /// Dracula theme - Popular dark theme
    fn theme_dracula() -> ThemeDefinition {
        ThemeDefinition {
            name: "Dracula".to_string(),
            theme: TerminalTheme {
                foreground: hsla_from_rgb(0xF8, 0xF8, 0xF2),
                background: hsla_from_rgb(0x28, 0x2A, 0x36),
                cursor: hsla_from_rgb(0xF8, 0xF8, 0xF2),
                selection: rgpui::Hsla {
                    h: 0.76,
                    s: 0.25,
                    l: 0.40,
                    a: 0.50,
                },
                ansi: [
                    hsla_from_rgb(0x21, 0x22, 0x2C), // Black
                    hsla_from_rgb(0xFF, 0x55, 0x55), // Red
                    hsla_from_rgb(0x50, 0xFA, 0x7B), // Green
                    hsla_from_rgb(0xF1, 0xFA, 0x8C), // Yellow
                    hsla_from_rgb(0xBD, 0x93, 0xF9), // Blue (purple)
                    hsla_from_rgb(0xFF, 0x79, 0xC6), // Magenta (pink)
                    hsla_from_rgb(0x8B, 0xE9, 0xFD), // Cyan
                    hsla_from_rgb(0xF8, 0xF8, 0xF2), // White
                ],
                bright: [
                    hsla_from_rgb(0x62, 0x72, 0xA4), // Bright Black
                    hsla_from_rgb(0xFF, 0x6E, 0x6E), // Bright Red
                    hsla_from_rgb(0x69, 0xFF, 0x94), // Bright Green
                    hsla_from_rgb(0xFF, 0xFF, 0xA5), // Bright Yellow
                    hsla_from_rgb(0xD6, 0xAC, 0xFF), // Bright Blue
                    hsla_from_rgb(0xFF, 0x92, 0xDF), // Bright Magenta
                    hsla_from_rgb(0xA4, 0xFF, 0xFF), // Bright Cyan
                    hsla_from_rgb(0xFF, 0xFF, 0xFF), // Bright White
                ],
                dim: [
                    hsla_from_rgb(0x21, 0x22, 0x2C),
                    hsla_from_rgb(0xCC, 0x44, 0x44),
                    hsla_from_rgb(0x40, 0xC8, 0x62),
                    hsla_from_rgb(0xC1, 0xC8, 0x70),
                    hsla_from_rgb(0x97, 0x76, 0xC7),
                    hsla_from_rgb(0xCC, 0x61, 0x9E),
                    hsla_from_rgb(0x6F, 0xBA, 0xCA),
                    hsla_from_rgb(0xC0, 0xC0, 0xC0),
                ],
                bright_foreground: hsla_from_rgb(0xFF, 0xFF, 0xFF),
                dim_foreground: hsla_from_rgb(0xC0, 0xC0, 0xC0),
            },
        }
    }

    /// Nord theme - Arctic, north-bluish color palette
    fn theme_nord() -> ThemeDefinition {
        ThemeDefinition {
            name: "Nord".to_string(),
            theme: TerminalTheme {
                foreground: hsla_from_rgb(0xD8, 0xDE, 0xE9), // Nord4
                background: hsla_from_rgb(0x2E, 0x34, 0x40), // Nord0
                cursor: hsla_from_rgb(0xD8, 0xDE, 0xE9),
                selection: rgpui::Hsla {
                    h: 0.62,
                    s: 0.25,
                    l: 0.40,
                    a: 0.50,
                },
                ansi: [
                    hsla_from_rgb(0x3B, 0x42, 0x52), // Nord1
                    hsla_from_rgb(0xBF, 0x61, 0x6A), // Nord11 (red)
                    hsla_from_rgb(0xA3, 0xBE, 0x8C), // Nord14 (green)
                    hsla_from_rgb(0xEB, 0xCB, 0x8B), // Nord13 (yellow)
                    hsla_from_rgb(0x81, 0xA1, 0xC1), // Nord9 (blue)
                    hsla_from_rgb(0xB4, 0x8E, 0xAD), // Nord15 (magenta)
                    hsla_from_rgb(0x88, 0xC0, 0xD0), // Nord8 (cyan)
                    hsla_from_rgb(0xE5, 0xE9, 0xF0), // Nord5
                ],
                bright: [
                    hsla_from_rgb(0x4C, 0x56, 0x6A), // Nord3
                    hsla_from_rgb(0xBF, 0x61, 0x6A), // Nord11
                    hsla_from_rgb(0xA3, 0xBE, 0x8C), // Nord14
                    hsla_from_rgb(0xEB, 0xCB, 0x8B), // Nord13
                    hsla_from_rgb(0x81, 0xA1, 0xC1), // Nord9
                    hsla_from_rgb(0xB4, 0x8E, 0xAD), // Nord15
                    hsla_from_rgb(0x8F, 0xBC, 0xBB), // Nord7
                    hsla_from_rgb(0xEC, 0xEF, 0xF4), // Nord6
                ],
                dim: [
                    hsla_from_rgb(0x2E, 0x34, 0x40),
                    hsla_from_rgb(0x99, 0x4E, 0x55),
                    hsla_from_rgb(0x82, 0x98, 0x70),
                    hsla_from_rgb(0xBC, 0xA2, 0x6F),
                    hsla_from_rgb(0x67, 0x81, 0x9A),
                    hsla_from_rgb(0x90, 0x71, 0x8A),
                    hsla_from_rgb(0x6D, 0x99, 0xA6),
                    hsla_from_rgb(0xB8, 0xBB, 0xC0),
                ],
                bright_foreground: hsla_from_rgb(0xEC, 0xEF, 0xF4),
                dim_foreground: hsla_from_rgb(0x4C, 0x56, 0x6A),
            },
        }
    }

    /// Gruvbox Dark theme - Retro groove color scheme
    fn theme_gruvbox_dark() -> ThemeDefinition {
        ThemeDefinition {
            name: "Gruvbox Dark".to_string(),
            theme: TerminalTheme {
                foreground: hsla_from_rgb(0xEB, 0xDB, 0xB2), // fg
                background: hsla_from_rgb(0x28, 0x28, 0x28), // bg0
                cursor: hsla_from_rgb(0xEB, 0xDB, 0xB2),
                selection: rgpui::Hsla {
                    h: 0.11,
                    s: 0.25,
                    l: 0.35,
                    a: 0.50,
                },
                ansi: [
                    hsla_from_rgb(0x28, 0x28, 0x28), // bg0
                    hsla_from_rgb(0xCC, 0x24, 0x1D), // red
                    hsla_from_rgb(0x98, 0x97, 0x1A), // green
                    hsla_from_rgb(0xD7, 0x99, 0x21), // yellow
                    hsla_from_rgb(0x45, 0x85, 0x88), // blue
                    hsla_from_rgb(0xB1, 0x62, 0x86), // purple
                    hsla_from_rgb(0x68, 0x9D, 0x6A), // aqua
                    hsla_from_rgb(0xA8, 0x99, 0x84), // fg4
                ],
                bright: [
                    hsla_from_rgb(0x92, 0x83, 0x74), // gray
                    hsla_from_rgb(0xFB, 0x49, 0x34), // bright red
                    hsla_from_rgb(0xB8, 0xBB, 0x26), // bright green
                    hsla_from_rgb(0xFA, 0xBD, 0x2F), // bright yellow
                    hsla_from_rgb(0x83, 0xA5, 0x98), // bright blue
                    hsla_from_rgb(0xD3, 0x86, 0x9B), // bright purple
                    hsla_from_rgb(0x8E, 0xC0, 0x7C), // bright aqua
                    hsla_from_rgb(0xEB, 0xDB, 0xB2), // fg0
                ],
                dim: [
                    hsla_from_rgb(0x1D, 0x20, 0x21),
                    hsla_from_rgb(0x9D, 0x00, 0x06),
                    hsla_from_rgb(0x79, 0x74, 0x0E),
                    hsla_from_rgb(0xB5, 0x76, 0x14),
                    hsla_from_rgb(0x07, 0x66, 0x78),
                    hsla_from_rgb(0x8F, 0x3F, 0x71),
                    hsla_from_rgb(0x42, 0x7B, 0x58),
                    hsla_from_rgb(0x66, 0x5C, 0x54),
                ],
                bright_foreground: hsla_from_rgb(0xEB, 0xDB, 0xB2),
                dim_foreground: hsla_from_rgb(0x66, 0x5C, 0x54),
            },
        }
    }

    /// One Light theme - Based on Atom's One Light
    fn theme_one_light() -> ThemeDefinition {
        ThemeDefinition {
            name: "One Light".to_string(),
            theme: TerminalTheme {
                foreground: hsla_from_rgb(0x38, 0x3A, 0x42), // fg
                background: hsla_from_rgb(0xFA, 0xFA, 0xFA), // bg
                cursor: hsla_from_rgb(0x52, 0x6F, 0xFF),
                selection: rgpui::Hsla {
                    h: 0.58,
                    s: 0.25,
                    l: 0.75,
                    a: 0.50,
                },
                ansi: [
                    hsla_from_rgb(0x38, 0x3A, 0x42), // Black
                    hsla_from_rgb(0xE4, 0x56, 0x49), // Red
                    hsla_from_rgb(0x50, 0xA1, 0x4F), // Green
                    hsla_from_rgb(0xC1, 0x84, 0x01), // Yellow
                    hsla_from_rgb(0x40, 0x78, 0xF2), // Blue
                    hsla_from_rgb(0xA6, 0x26, 0xA4), // Magenta
                    hsla_from_rgb(0x01, 0x84, 0xBC), // Cyan
                    hsla_from_rgb(0xA0, 0xA1, 0xA7), // White
                ],
                bright: [
                    hsla_from_rgb(0x69, 0x6C, 0x77), // Bright Black
                    hsla_from_rgb(0xE4, 0x56, 0x49), // Bright Red
                    hsla_from_rgb(0x50, 0xA1, 0x4F), // Bright Green
                    hsla_from_rgb(0xC1, 0x84, 0x01), // Bright Yellow
                    hsla_from_rgb(0x40, 0x78, 0xF2), // Bright Blue
                    hsla_from_rgb(0xA6, 0x26, 0xA4), // Bright Magenta
                    hsla_from_rgb(0x01, 0x84, 0xBC), // Bright Cyan
                    hsla_from_rgb(0x09, 0x0A, 0x0B), // Bright White
                ],
                dim: [
                    hsla_from_rgb(0x69, 0x6C, 0x77),
                    hsla_from_rgb(0xB8, 0x45, 0x3A),
                    hsla_from_rgb(0x40, 0x81, 0x3F),
                    hsla_from_rgb(0x9A, 0x6A, 0x01),
                    hsla_from_rgb(0x33, 0x60, 0xC2),
                    hsla_from_rgb(0x85, 0x1E, 0x83),
                    hsla_from_rgb(0x01, 0x6A, 0x96),
                    hsla_from_rgb(0x2C, 0x2D, 0x33),
                ],
                bright_foreground: hsla_from_rgb(0x09, 0x0A, 0x0B),
                dim_foreground: hsla_from_rgb(0xA0, 0xA1, 0xA7),
            },
        }
    }

    /// Solarized Light theme - Light variant of Solarized
    fn theme_solarized_light() -> ThemeDefinition {
        ThemeDefinition {
            name: "Solarized Light".to_string(),
            theme: TerminalTheme {
                foreground: hsla_from_rgb(0x65, 0x7B, 0x83), // base00
                background: hsla_from_rgb(0xFD, 0xF6, 0xE3), // base3
                cursor: hsla_from_rgb(0x65, 0x7B, 0x83),
                selection: rgpui::Hsla {
                    h: 0.48,
                    s: 0.15,
                    l: 0.75,
                    a: 0.45,
                },
                ansi: [
                    hsla_from_rgb(0x07, 0x36, 0x42), // base02
                    hsla_from_rgb(0xDC, 0x32, 0x2F), // red
                    hsla_from_rgb(0x85, 0x99, 0x00), // green
                    hsla_from_rgb(0xB5, 0x89, 0x00), // yellow
                    hsla_from_rgb(0x26, 0x8B, 0xD2), // blue
                    hsla_from_rgb(0xD3, 0x36, 0x82), // magenta
                    hsla_from_rgb(0x2A, 0xA1, 0x98), // cyan
                    hsla_from_rgb(0xEE, 0xE8, 0xD5), // base2
                ],
                bright: [
                    hsla_from_rgb(0x00, 0x2B, 0x36), // base03
                    hsla_from_rgb(0xCB, 0x4B, 0x16), // orange
                    hsla_from_rgb(0x58, 0x6E, 0x75), // base01
                    hsla_from_rgb(0x65, 0x7B, 0x83), // base00
                    hsla_from_rgb(0x83, 0x94, 0x96), // base0
                    hsla_from_rgb(0x6C, 0x71, 0xC4), // violet
                    hsla_from_rgb(0x93, 0xA1, 0xA1), // base1
                    hsla_from_rgb(0x00, 0x2B, 0x36), // base03
                ],
                dim: [
                    hsla_from_rgb(0xC0, 0xBA, 0xAA),
                    hsla_from_rgb(0xB0, 0x28, 0x25),
                    hsla_from_rgb(0x6A, 0x7A, 0x00),
                    hsla_from_rgb(0x90, 0x6D, 0x00),
                    hsla_from_rgb(0x1E, 0x6D, 0xA8),
                    hsla_from_rgb(0xA8, 0x2B, 0x68),
                    hsla_from_rgb(0x22, 0x81, 0x79),
                    hsla_from_rgb(0x05, 0x28, 0x32),
                ],
                bright_foreground: hsla_from_rgb(0x00, 0x2B, 0x36),
                dim_foreground: hsla_from_rgb(0x93, 0xA1, 0xA1),
            },
        }
    }

    /// GitHub Light theme - Based on GitHub's light theme
    fn theme_github_light() -> ThemeDefinition {
        ThemeDefinition {
            name: "GitHub Light".to_string(),
            theme: TerminalTheme {
                foreground: hsla_from_rgb(0x24, 0x29, 0x2E), // fg.default
                background: hsla_from_rgb(0xFF, 0xFF, 0xFF), // canvas.default
                cursor: hsla_from_rgb(0x04, 0x4A, 0x89),
                selection: rgpui::Hsla {
                    h: 0.60,
                    s: 0.20,
                    l: 0.80,
                    a: 0.45,
                },
                ansi: [
                    hsla_from_rgb(0x24, 0x29, 0x2E), // Black
                    hsla_from_rgb(0xCF, 0x22, 0x2E), // Red
                    hsla_from_rgb(0x11, 0x6B, 0x29), // Green
                    hsla_from_rgb(0x4D, 0x2D, 0x00), // Yellow
                    hsla_from_rgb(0x03, 0x49, 0xB4), // Blue
                    hsla_from_rgb(0x58, 0x41, 0x93), // Magenta
                    hsla_from_rgb(0x0E, 0x60, 0x94), // Cyan
                    hsla_from_rgb(0x6E, 0x77, 0x81), // White
                ],
                bright: [
                    hsla_from_rgb(0x57, 0x60, 0x6A), // Bright Black
                    hsla_from_rgb(0xA4, 0x00, 0x00), // Bright Red
                    hsla_from_rgb(0x11, 0x6B, 0x29), // Bright Green
                    hsla_from_rgb(0x4D, 0x2D, 0x00), // Bright Yellow
                    hsla_from_rgb(0x03, 0x49, 0xB4), // Bright Blue
                    hsla_from_rgb(0x58, 0x41, 0x93), // Bright Magenta
                    hsla_from_rgb(0x0E, 0x60, 0x94), // Bright Cyan
                    hsla_from_rgb(0x24, 0x29, 0x2E), // Bright White
                ],
                dim: [
                    hsla_from_rgb(0x8C, 0x95, 0x9F),
                    hsla_from_rgb(0xA4, 0x1B, 0x24),
                    hsla_from_rgb(0x1B, 0x6B, 0x2E),
                    hsla_from_rgb(0xAF, 0x89, 0x07),
                    hsla_from_rgb(0x02, 0x52, 0xAB),
                    hsla_from_rgb(0x6A, 0x40, 0xBC),
                    hsla_from_rgb(0x12, 0x61, 0x95),
                    hsla_from_rgb(0x1C, 0x21, 0x25),
                ],
                bright_foreground: hsla_from_rgb(0x00, 0x00, 0x00),
                dim_foreground: hsla_from_rgb(0x6E, 0x77, 0x81),
            },
        }
    }

    /// Gruvbox Light theme - Light variant of Gruvbox
    fn theme_gruvbox_light() -> ThemeDefinition {
        ThemeDefinition {
            name: "Gruvbox Light".to_string(),
            theme: TerminalTheme {
                foreground: hsla_from_rgb(0x3C, 0x38, 0x36), // fg (dark1)
                background: hsla_from_rgb(0xFB, 0xF1, 0xC7), // bg0_h
                cursor: hsla_from_rgb(0x3C, 0x38, 0x36),
                selection: rgpui::Hsla {
                    h: 0.11,
                    s: 0.20,
                    l: 0.80,
                    a: 0.45,
                },
                ansi: [
                    hsla_from_rgb(0x3C, 0x38, 0x36), // dark1
                    hsla_from_rgb(0xCC, 0x24, 0x1D), // red
                    hsla_from_rgb(0x98, 0x97, 0x1A), // green
                    hsla_from_rgb(0xD7, 0x99, 0x21), // yellow
                    hsla_from_rgb(0x45, 0x85, 0x88), // blue
                    hsla_from_rgb(0xB1, 0x62, 0x86), // purple
                    hsla_from_rgb(0x68, 0x9D, 0x6A), // aqua
                    hsla_from_rgb(0x7C, 0x6F, 0x64), // fg4
                ],
                bright: [
                    hsla_from_rgb(0x66, 0x5C, 0x54), // dark4
                    hsla_from_rgb(0x9D, 0x00, 0x06), // bright red
                    hsla_from_rgb(0x79, 0x74, 0x0E), // bright green
                    hsla_from_rgb(0xB5, 0x76, 0x14), // bright yellow
                    hsla_from_rgb(0x07, 0x66, 0x78), // bright blue
                    hsla_from_rgb(0x8F, 0x3F, 0x71), // bright purple
                    hsla_from_rgb(0x42, 0x7B, 0x58), // bright aqua
                    hsla_from_rgb(0x28, 0x28, 0x28), // fg0 (dark)
                ],
                dim: [
                    hsla_from_rgb(0xA8, 0x99, 0x84),
                    hsla_from_rgb(0xFB, 0x49, 0x34),
                    hsla_from_rgb(0xB8, 0xBB, 0x26),
                    hsla_from_rgb(0xFA, 0xBD, 0x2F),
                    hsla_from_rgb(0x83, 0xA5, 0x98),
                    hsla_from_rgb(0xD3, 0x86, 0x9B),
                    hsla_from_rgb(0x8E, 0xC0, 0x7C),
                    hsla_from_rgb(0x66, 0x5C, 0x54),
                ],
                bright_foreground: hsla_from_rgb(0x28, 0x28, 0x28),
                dim_foreground: hsla_from_rgb(0x92, 0x83, 0x74),
            },
        }
    }
}
