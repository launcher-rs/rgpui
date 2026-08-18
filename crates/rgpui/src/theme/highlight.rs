use crate::{ActiveTheme, App, FontWeight, HighlightStyle, Hsla, theme::ThemeMode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::{ops::Deref, sync::Arc};

/// 语法高亮名称列表，与 tree-sitter highlight 查询名称对应。
pub(super) const HIGHLIGHT_NAMES: [&str; 41] = [
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

/// 语法高亮颜色表，包含各语法令牌的颜色样式。
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, JsonSchema, Serialize, Deserialize)]
pub struct SyntaxColors {
    /// 属性名样式。
    pub attribute: Option<ThemeStyle>,
    /// 布尔字面量样式。
    pub boolean: Option<ThemeStyle>,
    /// 注释样式。
    pub comment: Option<ThemeStyle>,
    /// 文档注释样式。
    pub comment_doc: Option<ThemeStyle>,
    /// 常量样式。
    pub constant: Option<ThemeStyle>,
    /// 构造函数样式。
    pub constructor: Option<ThemeStyle>,
    /// 内嵌代码样式。
    pub embedded: Option<ThemeStyle>,
    /// 强调（斜体）样式。
    pub emphasis: Option<ThemeStyle>,
    #[serde(rename = "emphasis.strong")]
    /// 强调加粗样式。
    pub emphasis_strong: Option<ThemeStyle>,
    #[serde(rename = "enum")]
    /// 枚举类型样式。
    pub enum_: Option<ThemeStyle>,
    /// 函数名样式。
    pub function: Option<ThemeStyle>,
    /// 提示样式。
    pub hint: Option<ThemeStyle>,
    /// 关键字样式。
    pub keyword: Option<ThemeStyle>,
    /// 标签样式。
    pub label: Option<ThemeStyle>,
    #[serde(rename = "link_text")]
    /// 链接文本样式。
    pub link_text: Option<ThemeStyle>,
    #[serde(rename = "link_uri")]
    /// 链接地址样式。
    pub link_uri: Option<ThemeStyle>,
    /// 数字字面量样式。
    pub number: Option<ThemeStyle>,
    /// 运算符样式。
    pub operator: Option<ThemeStyle>,
    /// 预测性补全样式。
    pub predictive: Option<ThemeStyle>,
    /// 预处理指令样式。
    pub preproc: Option<ThemeStyle>,
    /// 主要标识符样式。
    pub primary: Option<ThemeStyle>,
    /// 属性/字段访问样式。
    pub property: Option<ThemeStyle>,
    /// 标点符号样式。
    pub punctuation: Option<ThemeStyle>,
    #[serde(rename = "punctuation.bracket")]
    /// 括号标点样式。
    pub punctuation_bracket: Option<ThemeStyle>,
    #[serde(rename = "punctuation.delimiter")]
    /// 分隔符标点样式。
    pub punctuation_delimiter: Option<ThemeStyle>,
    #[serde(rename = "punctuation.list_marker")]
    /// 列表标记样式。
    pub punctuation_list_marker: Option<ThemeStyle>,
    #[serde(rename = "punctuation.special")]
    /// 特殊标点样式。
    pub punctuation_special: Option<ThemeStyle>,
    /// 字符串样式。
    pub string: Option<ThemeStyle>,
    #[serde(rename = "string.escape")]
    /// 字符串转义序列样式。
    pub string_escape: Option<ThemeStyle>,
    #[serde(rename = "string.regex")]
    /// 正则表达式样式。
    pub string_regex: Option<ThemeStyle>,
    #[serde(rename = "string.special")]
    /// 特殊字符串样式。
    pub string_special: Option<ThemeStyle>,
    #[serde(rename = "string.special.symbol")]
    /// 特殊符号字符串样式。
    pub string_special_symbol: Option<ThemeStyle>,
    /// 标签（tag）样式。
    pub tag: Option<ThemeStyle>,
    #[serde(rename = "tag.doctype")]
    /// 文档类型标签样式。
    pub tag_doctype: Option<ThemeStyle>,
    #[serde(rename = "text.code.span")]
    /// 行内代码段样式。
    pub text_code_span: Option<ThemeStyle>,
    #[serde(rename = "text.literal")]
    /// 字面文本样式。
    pub text_literal: Option<ThemeStyle>,
    /// 标题样式。
    pub title: Option<ThemeStyle>,
    #[serde(rename = "type")]
    /// 类型样式。
    pub type_: Option<ThemeStyle>,
    /// 变量样式。
    pub variable: Option<ThemeStyle>,
    #[serde(rename = "variable.special")]
    /// 特殊变量样式。
    pub variable_special: Option<ThemeStyle>,
    /// 变体样式。
    pub variant: Option<ThemeStyle>,
}

/// 字体样式枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontStyle {
    /// 常规字体。
    Normal,
    /// 斜体。
    Italic,
    /// 下划线。
    Underline,
}

impl From<FontStyle> for crate::FontStyle {
    fn from(style: FontStyle) -> Self {
        match style {
            FontStyle::Normal => crate::FontStyle::Normal,
            FontStyle::Italic => crate::FontStyle::Italic,
            FontStyle::Underline => crate::FontStyle::Normal,
        }
    }
}

/// 字体字重枚举（与 CSS font-weight 数值对应）。
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Serialize_repr, Deserialize_repr, JsonSchema)]
#[repr(u16)]
pub enum FontWeightContent {
    /// 细体（100）。
    Thin = 100,
    /// 超细体（200）。
    ExtraLight = 200,
    /// 细体（300）。
    Light = 300,
    /// 常规（400）。
    Normal = 400,
    /// 中等（500）。
    Medium = 500,
    /// 半粗（600）。
    Semibold = 600,
    /// 粗体（700）。
    Bold = 700,
    /// 超粗（800）。
    ExtraBold = 800,
    /// 黑色（900）。
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

/// 单个语法令牌的主题样式（颜色 + 字体样式 + 字重）。
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
    /// 根据名称返回对应的高亮样式。
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
        } else {
            // 回退 `keyword.modifier` 到 `keyword`
            if name.contains(".") {
                if let Some(prefix) = name.split(".").next() {
                    return self.style(prefix);
                }

                None
            } else {
                None
            }
        }
    }

    /// 根据索引返回对应的高亮样式（索引对应 [`HIGHLIGHT_NAMES`]）。
    #[inline]
    pub fn style_for_index(&self, index: usize) -> Option<HighlightStyle> {
        HIGHLIGHT_NAMES.get(index).and_then(|name| self.style(name))
    }
}

/// 状态颜色表（错误/警告/信息/成功/提示）。
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, JsonSchema, Serialize, Deserialize)]
pub struct StatusColors {
    #[serde(rename = "error")]
    /// 错误主色。
    error: Option<Hsla>,
    #[serde(rename = "error.background")]
    /// 错误背景色。
    error_background: Option<Hsla>,
    #[serde(rename = "error.border")]
    /// 错误边框色。
    error_border: Option<Hsla>,
    #[serde(rename = "warning")]
    /// 警告主色。
    warning: Option<Hsla>,
    #[serde(rename = "warning.background")]
    /// 警告背景色。
    warning_background: Option<Hsla>,
    #[serde(rename = "warning.border")]
    /// 警告边框色。
    warning_border: Option<Hsla>,
    #[serde(rename = "info")]
    /// 信息主色。
    info: Option<Hsla>,
    #[serde(rename = "info.background")]
    /// 信息背景色。
    info_background: Option<Hsla>,
    #[serde(rename = "info.border")]
    /// 信息边框色。
    info_border: Option<Hsla>,
    #[serde(rename = "success")]
    /// 成功主色。
    success: Option<Hsla>,
    #[serde(rename = "success.background")]
    /// 成功背景色。
    success_background: Option<Hsla>,
    #[serde(rename = "success.border")]
    /// 成功边框色。
    success_border: Option<Hsla>,
    #[serde(rename = "hint")]
    /// 提示主色。
    hint: Option<Hsla>,
    #[serde(rename = "hint.background")]
    /// 提示背景色。
    hint_background: Option<Hsla>,
    #[serde(rename = "hint.border")]
    /// 提示边框色。
    hint_border: Option<Hsla>,
}

impl StatusColors {
    /// 返回错误主色，未设置时回退到主题红色。
    #[inline]
    pub fn error(&self, cx: &App) -> Hsla {
        self.error.unwrap_or(cx.theme().red)
    }

    /// 返回错误背景色，未设置时由错误主色混合 20% 透明度生成。
    #[inline]
    pub fn error_background(&self, cx: &App) -> Hsla {
        let bg = cx.theme().background;
        self.error_background
            .unwrap_or(bg.blend(self.error(cx).alpha(0.2)))
    }

    /// 返回错误边框色，未设置时回退到错误主色。
    #[inline]
    pub fn error_border(&self, cx: &App) -> Hsla {
        self.error_border.unwrap_or(self.error(cx))
    }

    /// 返回警告主色，未设置时回退到主题黄色。
    #[inline]
    pub fn warning(&self, cx: &App) -> Hsla {
        self.warning.unwrap_or(cx.theme().yellow)
    }

    /// 返回警告背景色，未设置时由警告主色混合 20% 透明度生成。
    #[inline]
    pub fn warning_background(&self, cx: &App) -> Hsla {
        let bg = cx.theme().background;
        self.warning_background
            .unwrap_or(bg.blend(self.warning(cx).alpha(0.2)))
    }

    /// 返回警告边框色，未设置时回退到警告主色。
    #[inline]
    pub fn warning_border(&self, cx: &App) -> Hsla {
        self.warning_border.unwrap_or(self.warning(cx))
    }

    /// 返回信息主色，未设置时回退到主题蓝色。
    #[inline]
    pub fn info(&self, cx: &App) -> Hsla {
        self.info.unwrap_or(cx.theme().blue)
    }

    /// 返回信息背景色，未设置时由信息主色混合 20% 透明度生成。
    #[inline]
    pub fn info_background(&self, cx: &App) -> Hsla {
        let bg = cx.theme().background;
        self.info_background
            .unwrap_or(bg.blend(self.info(cx).alpha(0.2)))
    }

    /// 返回信息边框色，未设置时回退到信息主色。
    #[inline]
    pub fn info_border(&self, cx: &App) -> Hsla {
        self.info_border.unwrap_or(self.info(cx))
    }

    /// 返回成功主色，未设置时回退到主题绿色。
    #[inline]
    pub fn success(&self, cx: &App) -> Hsla {
        self.success.unwrap_or(cx.theme().green)
    }

    /// 返回成功背景色，未设置时由成功主色混合 20% 透明度生成。
    #[inline]
    pub fn success_background(&self, cx: &App) -> Hsla {
        let bg = cx.theme().background;
        self.success_background
            .unwrap_or(bg.blend(self.success(cx).alpha(0.2)))
    }

    /// 返回成功边框色，未设置时回退到成功主色。
    #[inline]
    pub fn success_border(&self, cx: &App) -> Hsla {
        self.success_border.unwrap_or(self.success(cx))
    }

    /// 返回提示主色，未设置时回退到主题青色。
    #[inline]
    pub fn hint(&self, cx: &App) -> Hsla {
        self.hint.unwrap_or(cx.theme().cyan)
    }

    /// 返回提示背景色，未设置时由提示主色混合 20% 透明度生成。
    #[inline]
    pub fn hint_background(&self, cx: &App) -> Hsla {
        let bg = cx.theme().background;
        self.hint_background
            .unwrap_or(bg.blend(self.hint(cx).alpha(0.2)))
    }

    /// 返回提示边框色，未设置时回退到提示主色。
    #[inline]
    pub fn hint_border(&self, cx: &App) -> Hsla {
        self.hint_border.unwrap_or(self.hint(cx))
    }
}

/// 高亮主题样式，包含编辑器与状态颜色。
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, JsonSchema, Serialize, Deserialize)]
pub struct HighlightThemeStyle {
    #[serde(rename = "editor.background")]
    /// 编辑器背景色。
    pub editor_background: Option<Hsla>,
    #[serde(rename = "editor.foreground")]
    /// 编辑器前景色。
    pub editor_foreground: Option<Hsla>,
    #[serde(rename = "editor.active_line.background")]
    /// 当前活动行背景色。
    pub editor_active_line: Option<Hsla>,
    #[serde(rename = "editor.line_number")]
    /// 行号颜色。
    pub editor_line_number: Option<Hsla>,
    #[serde(rename = "editor.active_line_number")]
    /// 当前活动行行号颜色。
    pub editor_active_line_number: Option<Hsla>,
    #[serde(rename = "editor.invisible")]
    /// 不可见字符（空白）颜色。
    pub editor_invisible: Option<Hsla>,
    /// 行号栏（gutter）背景色，未设置时回退到 `editor_background`。
    #[serde(rename = "editor.gutter.background")]
    pub editor_gutter_background: Option<Hsla>,
    #[serde(flatten)]
    /// 状态颜色（错误/警告/信息/成功/提示）。
    pub status: StatusColors,
    #[serde(rename = "syntax")]
    /// 语法高亮颜色表。
    pub syntax: SyntaxColors,
}

/// 代码高亮主题，兼容 Zed 主题格式。
#[derive(Debug, Clone, PartialEq, Eq, Hash, JsonSchema, Serialize, Deserialize)]
pub struct HighlightTheme {
    /// 主题名称。
    pub name: String,
    #[serde(default)]
    /// 主题适用模式（亮/暗）。
    pub appearance: ThemeMode,
    /// 高亮主题样式。
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
        crate::DEFAULT_THEME_COLORS[&ThemeMode::Dark].1.clone()
    }

    /// 返回默认亮色高亮主题。
    pub fn default_light() -> Arc<Self> {
        crate::DEFAULT_THEME_COLORS[&ThemeMode::Light].1.clone()
    }
}
