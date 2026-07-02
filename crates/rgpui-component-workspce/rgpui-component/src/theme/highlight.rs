use std::{ops::Deref, sync::Arc};

use rgpui::{App, FontWeight, HighlightStyle, Hsla};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::{ActiveTheme, DEFAULT_THEME_COLORS, ThemeMode};

const HIGHLIGHT_NAMES: [&str; 41] = [
    "attribute",
    "boolean",
    "comment",
    "comment.doc",
    "constant",
    "constructor",
    "embedded",
    "emphasis",
    "emphasis.strong",
    "enum",
    "function",
    "hint",
    "keyword",
    "label",
    "link_text",
    "link_uri",
    "number",
    "operator",
    "predictive",
    "preproc",
    "primary",
    "property",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "punctuation.list_marker",
    "punctuation.special",
    "string",
    "string.escape",
    "string.regex",
    "string.special",
    "string.special.symbol",
    "tag",
    "tag.doctype",
    "text.code.span",
    "text.literal",
    "title",
    "type",
    "variable",
    "variable.special",
    "variant",
];

/// 语法高亮主题中的语法颜色集合。
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, JsonSchema, Serialize, Deserialize)]
pub struct SyntaxColors {
    pub attribute: Option<ThemeStyle>,
    pub boolean: Option<ThemeStyle>,
    pub comment: Option<ThemeStyle>,
    pub comment_doc: Option<ThemeStyle>,
    pub constant: Option<ThemeStyle>,
    pub constructor: Option<ThemeStyle>,
    pub embedded: Option<ThemeStyle>,
    pub emphasis: Option<ThemeStyle>,
    #[serde(rename = "emphasis.strong")]
    pub emphasis_strong: Option<ThemeStyle>,
    #[serde(rename = "enum")]
    pub enum_: Option<ThemeStyle>,
    pub function: Option<ThemeStyle>,
    pub hint: Option<ThemeStyle>,
    pub keyword: Option<ThemeStyle>,
    pub label: Option<ThemeStyle>,
    #[serde(rename = "link_text")]
    pub link_text: Option<ThemeStyle>,
    #[serde(rename = "link_uri")]
    pub link_uri: Option<ThemeStyle>,
    pub number: Option<ThemeStyle>,
    pub operator: Option<ThemeStyle>,
    pub predictive: Option<ThemeStyle>,
    pub preproc: Option<ThemeStyle>,
    pub primary: Option<ThemeStyle>,
    pub property: Option<ThemeStyle>,
    pub punctuation: Option<ThemeStyle>,
    #[serde(rename = "punctuation.bracket")]
    pub punctuation_bracket: Option<ThemeStyle>,
    #[serde(rename = "punctuation.delimiter")]
    pub punctuation_delimiter: Option<ThemeStyle>,
    #[serde(rename = "punctuation.list_marker")]
    pub punctuation_list_marker: Option<ThemeStyle>,
    #[serde(rename = "punctuation.special")]
    pub punctuation_special: Option<ThemeStyle>,
    pub string: Option<ThemeStyle>,
    #[serde(rename = "string.escape")]
    pub string_escape: Option<ThemeStyle>,
    #[serde(rename = "string.regex")]
    pub string_regex: Option<ThemeStyle>,
    #[serde(rename = "string.special")]
    pub string_special: Option<ThemeStyle>,
    #[serde(rename = "string.special.symbol")]
    pub string_special_symbol: Option<ThemeStyle>,
    pub tag: Option<ThemeStyle>,
    #[serde(rename = "tag.doctype")]
    pub tag_doctype: Option<ThemeStyle>,
    #[serde(rename = "text.code.span")]
    pub text_code_span: Option<ThemeStyle>,
    #[serde(rename = "text.literal")]
    pub text_literal: Option<ThemeStyle>,
    pub title: Option<ThemeStyle>,
    #[serde(rename = "type")]
    pub type_: Option<ThemeStyle>,
    pub variable: Option<ThemeStyle>,
    #[serde(rename = "variable.special")]
    pub variable_special: Option<ThemeStyle>,
    pub variant: Option<ThemeStyle>,
}

/// 主题文件中的字体样式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontStyle {
    Normal,
    Italic,
    Underline,
}

impl From<FontStyle> for rgpui::FontStyle {
    fn from(style: FontStyle) -> Self {
        match style {
            FontStyle::Normal => rgpui::FontStyle::Normal,
            FontStyle::Italic => rgpui::FontStyle::Italic,
            FontStyle::Underline => rgpui::FontStyle::Normal,
        }
    }
}

/// 主题文件中的字体粗细。
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Serialize_repr, Deserialize_repr, JsonSchema)]
#[repr(u16)]
pub enum FontWeightContent {
    Thin = 100,
    ExtraLight = 200,
    Light = 300,
    Normal = 400,
    Medium = 500,
    Semibold = 600,
    Bold = 700,
    ExtraBold = 800,
    Black = 900,
}

impl From<FontWeightContent> for FontWeight {
    fn from(value: FontWeightContent) -> Self {
        match value {
            FontWeightContent::Thin => FontWeight::THIN,
            FontWeightContent::ExtraLight => FontWeight::EXTRA_LIGHT,
            FontWeightContent::Light => FontWeight::LIGHT,
            FontWeightContent::Normal => FontWeight::NORMAL,
            FontWeightContent::Medium => FontWeight::MEDIUM,
            FontWeightContent::Semibold => FontWeight::SEMIBOLD,
            FontWeightContent::Bold => FontWeight::BOLD,
            FontWeightContent::ExtraBold => FontWeight::EXTRA_BOLD,
            FontWeightContent::Black => FontWeight::BLACK,
        }
    }
}

/// 单个语法作用域的显示样式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, JsonSchema, Serialize, Deserialize)]
pub struct ThemeStyle {
    color: Option<Hsla>,
    font_style: Option<FontStyle>,
    font_weight: Option<FontWeightContent>,
}

impl From<ThemeStyle> for HighlightStyle {
    fn from(style: ThemeStyle) -> Self {
        HighlightStyle {
            color: style.color,
            font_weight: style.font_weight.map(Into::into),
            font_style: style.font_style.map(Into::into),
            ..Default::default()
        }
    }
}

impl SyntaxColors {
    /// 按语法作用域名称查找高亮样式。
    pub fn style(&self, name: &str) -> Option<HighlightStyle> {
        if name.is_empty() {
            return None;
        }

        let style = match name {
            "attribute" => self.attribute,
            "boolean" => self.boolean,
            "comment" => self.comment,
            "comment.doc" => self.comment_doc,
            "constant" => self.constant,
            "constructor" => self.constructor,
            "embedded" => self.embedded,
            "emphasis" => self.emphasis,
            "emphasis.strong" => self.emphasis_strong,
            "enum" => self.enum_,
            "function" => self.function,
            "hint" => self.hint,
            "keyword" => self.keyword,
            "label" => self.label,
            "link_text" => self.link_text,
            "link_uri" => self.link_uri,
            "number" => self.number,
            "operator" => self.operator,
            "predictive" => self.predictive,
            "preproc" => self.preproc,
            "primary" => self.primary,
            "property" => self.property,
            "punctuation" => self.punctuation,
            "punctuation.bracket" => self.punctuation_bracket,
            "punctuation.delimiter" => self.punctuation_delimiter,
            "punctuation.list_marker" => self.punctuation_list_marker,
            "punctuation.special" => self.punctuation_special,
            "string" => self.string,
            "string.escape" => self.string_escape,
            "string.regex" => self.string_regex,
            "string.special" => self.string_special,
            "string.special.symbol" => self.string_special_symbol,
            "tag" => self.tag,
            "tag.doctype" => self.tag_doctype,
            "text.code.span" => self.text_code_span,
            "text.literal" => self.text_literal,
            "title" => self.title,
            "type" => self.type_,
            "variable" => self.variable,
            "variable.special" => self.variable_special,
            "variant" => self.variant,
            _ => None,
        }
        .map(|s| s.into());

        if style.is_some() {
            style
        } else if name.contains('.') {
            name.split('.').next().and_then(|prefix| self.style(prefix))
        } else {
            None
        }
    }

    /// 按查询捕获索引查找高亮样式。
    #[inline]
    pub fn style_for_index(&self, index: usize) -> Option<HighlightStyle> {
        HIGHLIGHT_NAMES.get(index).and_then(|name| self.style(name))
    }
}

/// 诊断和状态类颜色。
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, JsonSchema, Serialize, Deserialize)]
pub struct StatusColors {
    #[serde(rename = "error")]
    error: Option<Hsla>,
    #[serde(rename = "error.background")]
    error_background: Option<Hsla>,
    #[serde(rename = "error.border")]
    error_border: Option<Hsla>,
    #[serde(rename = "warning")]
    warning: Option<Hsla>,
    #[serde(rename = "warning.background")]
    warning_background: Option<Hsla>,
    #[serde(rename = "warning.border")]
    warning_border: Option<Hsla>,
    #[serde(rename = "info")]
    info: Option<Hsla>,
    #[serde(rename = "info.background")]
    info_background: Option<Hsla>,
    #[serde(rename = "info.border")]
    info_border: Option<Hsla>,
    #[serde(rename = "success")]
    success: Option<Hsla>,
    #[serde(rename = "success.background")]
    success_background: Option<Hsla>,
    #[serde(rename = "success.border")]
    success_border: Option<Hsla>,
    #[serde(rename = "hint")]
    hint: Option<Hsla>,
    #[serde(rename = "hint.background")]
    hint_background: Option<Hsla>,
    #[serde(rename = "hint.border")]
    hint_border: Option<Hsla>,
}

impl StatusColors {
    #[inline]
    pub fn error(&self, cx: &App) -> Hsla {
        self.error.unwrap_or(cx.theme().red)
    }

    #[inline]
    pub fn error_background(&self, cx: &App) -> Hsla {
        let bg = cx.theme().background;
        self.error_background
            .unwrap_or(bg.blend(self.error(cx).alpha(0.2)))
    }

    #[inline]
    pub fn error_border(&self, cx: &App) -> Hsla {
        self.error_border.unwrap_or(self.error(cx))
    }

    #[inline]
    pub fn warning(&self, cx: &App) -> Hsla {
        self.warning.unwrap_or(cx.theme().yellow)
    }

    #[inline]
    pub fn warning_background(&self, cx: &App) -> Hsla {
        let bg = cx.theme().background;
        self.warning_background
            .unwrap_or(bg.blend(self.warning(cx).alpha(0.2)))
    }

    #[inline]
    pub fn warning_border(&self, cx: &App) -> Hsla {
        self.warning_border.unwrap_or(self.warning(cx))
    }

    #[inline]
    pub fn info(&self, cx: &App) -> Hsla {
        self.info.unwrap_or(cx.theme().blue)
    }

    #[inline]
    pub fn info_background(&self, cx: &App) -> Hsla {
        let bg = cx.theme().background;
        self.info_background
            .unwrap_or(bg.blend(self.info(cx).alpha(0.2)))
    }

    #[inline]
    pub fn info_border(&self, cx: &App) -> Hsla {
        self.info_border.unwrap_or(self.info(cx))
    }

    #[inline]
    pub fn success(&self, cx: &App) -> Hsla {
        self.success.unwrap_or(cx.theme().green)
    }

    #[inline]
    pub fn success_background(&self, cx: &App) -> Hsla {
        let bg = cx.theme().background;
        self.success_background
            .unwrap_or(bg.blend(self.success(cx).alpha(0.2)))
    }

    #[inline]
    pub fn success_border(&self, cx: &App) -> Hsla {
        self.success_border.unwrap_or(self.success(cx))
    }

    #[inline]
    pub fn hint(&self, cx: &App) -> Hsla {
        self.hint.unwrap_or(cx.theme().cyan)
    }

    #[inline]
    pub fn hint_background(&self, cx: &App) -> Hsla {
        let bg = cx.theme().background;
        self.hint_background
            .unwrap_or(bg.blend(self.hint(cx).alpha(0.2)))
    }

    #[inline]
    pub fn hint_border(&self, cx: &App) -> Hsla {
        self.hint_border.unwrap_or(self.hint(cx))
    }
}

/// 主题文件中的编辑器和语法高亮配置。
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, JsonSchema, Serialize, Deserialize)]
pub struct HighlightThemeStyle {
    #[serde(rename = "editor.background")]
    pub editor_background: Option<Hsla>,
    #[serde(rename = "editor.foreground")]
    pub editor_foreground: Option<Hsla>,
    #[serde(rename = "editor.active_line.background")]
    pub editor_active_line: Option<Hsla>,
    #[serde(rename = "editor.line_number")]
    pub editor_line_number: Option<Hsla>,
    #[serde(rename = "editor.active_line_number")]
    pub editor_active_line_number: Option<Hsla>,
    #[serde(rename = "editor.invisible")]
    pub editor_invisible: Option<Hsla>,
    #[serde(rename = "editor.gutter.background")]
    pub editor_gutter_background: Option<Hsla>,
    #[serde(flatten)]
    pub status: StatusColors,
    #[serde(rename = "syntax")]
    pub syntax: SyntaxColors,
}

/// 可被编辑器和 Markdown 代码块共用的高亮主题。
#[derive(Debug, Clone, PartialEq, Eq, Hash, JsonSchema, Serialize, Deserialize)]
pub struct HighlightTheme {
    pub name: String,
    #[serde(default)]
    pub appearance: ThemeMode,
    pub style: HighlightThemeStyle,
}

impl Deref for HighlightTheme {
    type Target = SyntaxColors;

    fn deref(&self) -> &Self::Target {
        &self.style.syntax
    }
}

impl HighlightTheme {
    /// 返回默认暗色高亮主题。
    pub fn default_dark() -> Arc<Self> {
        DEFAULT_THEME_COLORS[&ThemeMode::Dark].1.clone()
    }

    /// 返回默认亮色高亮主题。
    pub fn default_light() -> Arc<Self> {
        DEFAULT_THEME_COLORS[&ThemeMode::Light].1.clone()
    }
}
