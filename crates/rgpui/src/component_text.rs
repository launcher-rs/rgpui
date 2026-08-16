use std::sync::Arc;

use crate::{
    App, HighlightStyle, HighlightTheme, IntoElement, Pixels, Rems, RenderOnce, SharedString,
    StyleRefinement, Window, px, rems,
};

/// 用于自定义 [`ComponentText`] 渲染样式的配置。
#[derive(Clone)]
pub struct ComponentTextViewStyle {
    /// 每个段落的间距，默认为 1 rem。
    pub paragraph_gap: Rems,
    /// 标题的基础字体大小，默认为 14px。
    pub heading_base_font_size: Pixels,
    /// 根据标题级别（1-6）计算标题字体大小的函数。
    pub heading_font_size: Option<Arc<dyn Fn(u8, Pixels) -> Pixels + Send + Sync + 'static>>,
    /// 代码块的高亮主题。
    pub highlight_theme: Arc<HighlightTheme>,
    /// 代码块的样式精炼。
    pub code_block: StyleRefinement,
    /// 表格容器的样式精炼。
    pub table: StyleRefinement,
    /// 表格单元格的样式精炼。
    pub table_cell: StyleRefinement,
    /// 行内代码的高亮样式。
    pub inline_code: HighlightStyle,
    /// 是否为深色主题。
    pub is_dark: bool,
}

impl Default for ComponentTextViewStyle {
    fn default() -> Self {
        Self {
            paragraph_gap: rems(1.),
            heading_base_font_size: px(14.),
            heading_font_size: None,
            highlight_theme: HighlightTheme::default_light().clone(),
            code_block: StyleRefinement::default(),
            table: StyleRefinement::default(),
            table_cell: StyleRefinement::default(),
            inline_code: HighlightStyle::default(),
            is_dark: false,
        }
    }
}

impl ComponentTextViewStyle {
    /// 设置段落间距，默认为 1 rem。
    pub fn paragraph_gap(mut self, gap: Rems) -> Self {
        self.paragraph_gap = gap;
        self
    }

    /// 设置标题字体大小计算函数。
    pub fn heading_font_size<F>(mut self, f: F) -> Self
    where
        F: Fn(u8, Pixels) -> Pixels + Send + Sync + 'static,
    {
        self.heading_font_size = Some(Arc::new(f));
        self
    }

    /// 设置代码块样式。
    pub fn code_block(mut self, style: StyleRefinement) -> Self {
        self.code_block = style;
        self
    }

    /// 设置行内代码样式。
    pub fn inline_code(mut self, style: HighlightStyle) -> Self {
        self.inline_code = style;
        self
    }

    /// 设置表格容器样式。
    pub fn table(mut self, style: StyleRefinement) -> Self {
        self.table = style;
        self
    }

    /// 设置表格单元格样式。
    pub fn table_cell(mut self, style: StyleRefinement) -> Self {
        self.table_cell = style;
        self
    }
}

/// 组件文本类型，提供轻量级的文本内容封装。
///
/// 相比原生 [`Text`](crate::Text) 元素，此类型更贴近组件库的使用习惯，
/// 支持从字符串与共享字符串直接构造。
#[derive(IntoElement, Clone)]
pub enum ComponentText {
    /// 普通字符串文本
    String(SharedString),
}

impl From<SharedString> for ComponentText {
    fn from(s: SharedString) -> Self {
        Self::String(s)
    }
}

impl From<&str> for ComponentText {
    fn from(s: &str) -> Self {
        Self::String(SharedString::from(s.to_string()))
    }
}

impl From<String> for ComponentText {
    fn from(s: String) -> Self {
        Self::String(s.into())
    }
}

impl ComponentText {
    /// 设置样式（当前字符串变体不生效）。
    pub fn style(self, _style: ComponentTextViewStyle) -> Self {
        match self {
            Self::String(s) => Self::String(s),
        }
    }

    /// 获取文本内容。
    pub fn get_text(&self, _cx: &App) -> SharedString {
        match self {
            Self::String(s) => s.clone(),
        }
    }
}

impl RenderOnce for ComponentText {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        match self {
            Self::String(s) => s.into_any_element(),
        }
    }
}
