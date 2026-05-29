use std::env;
use std::fs;
use std::path::PathBuf;

use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor, Rgb};
use anyhow::{Context, Result};
use rgpui::{Hsla, Rgba};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct TerminalTheme {
    pub foreground: Hsla,
    pub background: Hsla,
    pub cursor: Hsla,
    pub selection: Hsla,
    pub ansi: [Hsla; 8],
    pub bright: [Hsla; 8],
    pub dim: [Hsla; 8],
    pub bright_foreground: Hsla,
    pub dim_foreground: Hsla,
}

impl TerminalTheme {
    pub fn resolve_color(&self, color: &AnsiColor) -> Hsla {
        match color {
            AnsiColor::Named(named) => self.named_color(*named),
            AnsiColor::Spec(rgb) => rgba_from_rgb(*rgb).into(),
            AnsiColor::Indexed(idx) => self.indexed_color(*idx),
        }
    }

    fn named_color(&self, named: NamedColor) -> Hsla {
        match named {
            NamedColor::Black => self.ansi[0],
            NamedColor::Red => self.ansi[1],
            NamedColor::Green => self.ansi[2],
            NamedColor::Yellow => self.ansi[3],
            NamedColor::Blue => self.ansi[4],
            NamedColor::Magenta => self.ansi[5],
            NamedColor::Cyan => self.ansi[6],
            NamedColor::White => self.ansi[7],
            NamedColor::BrightBlack => self.bright[0],
            NamedColor::BrightRed => self.bright[1],
            NamedColor::BrightGreen => self.bright[2],
            NamedColor::BrightYellow => self.bright[3],
            NamedColor::BrightBlue => self.bright[4],
            NamedColor::BrightMagenta => self.bright[5],
            NamedColor::BrightCyan => self.bright[6],
            NamedColor::BrightWhite => self.bright[7],
            NamedColor::Foreground => self.foreground,
            NamedColor::Background => self.background,
            NamedColor::Cursor => self.cursor,
            NamedColor::DimBlack => self.dim[0],
            NamedColor::DimRed => self.dim[1],
            NamedColor::DimGreen => self.dim[2],
            NamedColor::DimYellow => self.dim[3],
            NamedColor::DimBlue => self.dim[4],
            NamedColor::DimMagenta => self.dim[5],
            NamedColor::DimCyan => self.dim[6],
            NamedColor::DimWhite => self.dim[7],
            NamedColor::BrightForeground => self.bright_foreground,
            NamedColor::DimForeground => self.dim_foreground,
        }
    }

    fn indexed_color(&self, idx: u8) -> Hsla {
        match idx {
            0..=15 => {
                let named = match idx {
                    0 => NamedColor::Black,
                    1 => NamedColor::Red,
                    2 => NamedColor::Green,
                    3 => NamedColor::Yellow,
                    4 => NamedColor::Blue,
                    5 => NamedColor::Magenta,
                    6 => NamedColor::Cyan,
                    7 => NamedColor::White,
                    8 => NamedColor::BrightBlack,
                    9 => NamedColor::BrightRed,
                    10 => NamedColor::BrightGreen,
                    11 => NamedColor::BrightYellow,
                    12 => NamedColor::BrightBlue,
                    13 => NamedColor::BrightMagenta,
                    14 => NamedColor::BrightCyan,
                    15 => NamedColor::BrightWhite,
                    _ => NamedColor::Foreground,
                };
                self.named_color(named)
            }
            16..=231 => {
                let idx = idx - 16;
                let r = (idx / 36) % 6;
                let g = (idx / 6) % 6;
                let b = idx % 6;

                let r = if r > 0 { r * 40 + 55 } else { 0 };
                let g = if g > 0 { g * 40 + 55 } else { 0 };
                let b = if b > 0 { b * 40 + 55 } else { 0 };

                hsla_from_rgb(r, g, b)
            }
            232..=255 => {
                let gray = (idx - 232) * 10 + 8;
                hsla_from_rgb(gray, gray, gray)
            }
        }
    }
}

impl Default for TerminalTheme {
    fn default() -> Self {
        let ansi = [
            hsla_from_rgb(0x1E, 0x1E, 0x1E),
            hsla_from_rgb(0xE0, 0x6C, 0x75),
            hsla_from_rgb(0x98, 0xC3, 0x79),
            hsla_from_rgb(0xE5, 0xC0, 0x7B),
            hsla_from_rgb(0x61, 0xAF, 0xEF),
            hsla_from_rgb(0xC6, 0x78, 0xDD),
            hsla_from_rgb(0x56, 0xB6, 0xC2),
            hsla_from_rgb(0xAB, 0xB2, 0xBF),
        ];
        let bright = [
            hsla_from_rgb(0x5C, 0x63, 0x70),
            hsla_from_rgb(0xE0, 0x6C, 0x75),
            hsla_from_rgb(0x98, 0xC3, 0x79),
            hsla_from_rgb(0xE5, 0xC0, 0x7B),
            hsla_from_rgb(0x61, 0xAF, 0xEF),
            hsla_from_rgb(0xC6, 0x78, 0xDD),
            hsla_from_rgb(0x56, 0xB6, 0xC2),
            hsla_from_rgb(0xDF, 0xDF, 0xDF),
        ];
        let dim = [
            hsla_from_rgb(0x1E, 0x1E, 0x1E),
            hsla_from_rgb(0xBE, 0x5B, 0x65),
            hsla_from_rgb(0x7A, 0x9F, 0x60),
            hsla_from_rgb(0xD1, 0x9A, 0x66),
            hsla_from_rgb(0x4E, 0x88, 0xB8),
            hsla_from_rgb(0xA0, 0x61, 0xB0),
            hsla_from_rgb(0x44, 0x91, 0x9B),
            hsla_from_rgb(0x8A, 0x8F, 0x98),
        ];

        let background = hsla_from_rgb(0x1E, 0x1E, 0x1E);

        TerminalTheme {
            foreground: hsla_from_rgb(0xD4, 0xD4, 0xD4),
            background,
            cursor: hsla_from_rgb(0xAE, 0xAF, 0xAD),
            selection: Hsla {
                h: 0.58,
                s: 0.36,
                l: 0.48,
                a: 0.55,
            },
            ansi,
            bright,
            dim,
            bright_foreground: hsla_from_rgb(0xDF, 0xDF, 0xDF),
            dim_foreground: hsla_from_rgb(0x8A, 0x8F, 0x98),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalThemeConfig {
    pub foreground: String,
    pub background: String,
    pub cursor: String,
    pub selection: String,
    pub ansi: [String; 8],
    pub bright: [String; 8],
    #[serde(default)]
    pub dim: Option<[String; 8]>,
    #[serde(default)]
    pub bright_foreground: Option<String>,
    #[serde(default)]
    pub dim_foreground: Option<String>,
}

impl Default for TerminalThemeConfig {
    fn default() -> Self {
        let theme = TerminalTheme::default();
        TerminalThemeConfig {
            foreground: color_to_hex(theme.foreground),
            background: color_to_hex(theme.background),
            cursor: color_to_hex(theme.cursor),
            selection: color_to_hex(theme.selection),
            ansi: theme.ansi.map(color_to_hex),
            bright: theme.bright.map(color_to_hex),
            dim: Some(theme.dim.map(color_to_hex)),
            bright_foreground: Some(color_to_hex(theme.bright_foreground)),
            dim_foreground: Some(color_to_hex(theme.dim_foreground)),
        }
    }
}

impl TerminalThemeConfig {
    pub fn to_theme(&self) -> TerminalTheme {
        let fallback = TerminalTheme::default();
        TerminalTheme {
            foreground: parse_color(&self.foreground, fallback.foreground),
            background: parse_color(&self.background, fallback.background),
            cursor: parse_color(&self.cursor, fallback.cursor),
            selection: parse_color(&self.selection, fallback.selection),
            ansi: parse_color_array(&self.ansi, fallback.ansi),
            bright: parse_color_array(&self.bright, fallback.bright),
            dim: self
                .dim
                .as_ref()
                .map(|dim| parse_color_array(dim, fallback.dim))
                .unwrap_or(fallback.dim),
            bright_foreground: self
                .bright_foreground
                .as_deref()
                .map(|value| parse_color(value, fallback.bright_foreground))
                .unwrap_or(fallback.bright_foreground),
            dim_foreground: self
                .dim_foreground
                .as_deref()
                .map(|value| parse_color(value, fallback.dim_foreground))
                .unwrap_or(fallback.dim_foreground),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalConfig {
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
    pub letter_spacing: f32,
    pub theme: TerminalThemeConfig,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        TerminalConfig {
            font_family: "FiraCode Nerd Font".to_string(),
            font_size: 14.0,
            line_height: 1.2,
            letter_spacing: 0.0,
            theme: TerminalThemeConfig::default(),
        }
    }
}

impl TerminalConfig {
    pub fn config_path() -> PathBuf {
        if let Ok(path) = env::var("GPUI_TERM_CONFIG") {
            return PathBuf::from(path);
        }

        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("../../../.."));
        path.push("gpui-term");
        path.push("config.toml");
        path
    }

    pub fn load_or_create() -> Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let data = fs::read_to_string(&path)
                .with_context(|| format!("failed to read config {}", path.display()))?;
            match toml::from_str(&data) {
                Ok(config) => Ok(config),
                Err(err) => {
                    log::warn!("failed to parse config {}: {err}", path.display());
                    Ok(Self::default())
                }
            }
        } else {
            let config = Self::default();
            if let Err(err) = config.save() {
                log::warn!("failed to write default config {}: {err}", path.display());
            }
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let data = toml::to_string_pretty(self).context("failed to serialize config")?;
        fs::write(&path, data).with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }
}

fn parse_color_array(values: &[String; 8], fallback: [Hsla; 8]) -> [Hsla; 8] {
    let mut out = fallback;
    for (idx, value) in values.iter().enumerate() {
        out[idx] = parse_color(value, fallback[idx]);
    }
    out
}

fn parse_color(value: &str, fallback: Hsla) -> Hsla {
    parse_hex_color(value).map(Hsla::from).unwrap_or(fallback)
}

fn parse_hex_color(value: &str) -> Option<Rgba> {
    let value = value.trim();
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 && hex.len() != 8 {
        return None;
    }

    let parse_byte = |chunk: &str| u8::from_str_radix(chunk, 16).ok();
    let r = parse_byte(&hex[0..2])?;
    let g = parse_byte(&hex[2..4])?;
    let b = parse_byte(&hex[4..6])?;
    let a = if hex.len() == 8 {
        parse_byte(&hex[6..8])?
    } else {
        0xFF
    };

    Some(Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: a as f32 / 255.0,
    })
}

fn color_to_hex(color: Hsla) -> String {
    let rgba: Rgba = color.into();
    let r = (rgba.r * 255.0).round() as u8;
    let g = (rgba.g * 255.0).round() as u8;
    let b = (rgba.b * 255.0).round() as u8;
    let a = (rgba.a * 255.0).round() as u8;
    format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
}

fn rgba_from_rgb(rgb: Rgb) -> Rgba {
    Rgba {
        r: rgb.r as f32 / 255.0,
        g: rgb.g as f32 / 255.0,
        b: rgb.b as f32 / 255.0,
        a: 1.0,
    }
}

pub fn hsla_from_rgb(r: u8, g: u8, b: u8) -> Hsla {
    let rgba = Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    };
    rgba.into()
}
