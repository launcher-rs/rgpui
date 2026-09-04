//! LSP 悬停支持。

use lsp_types::{Hover, HoverContents, MarkedString};

use crate::App;

/// 悬停提供者 trait。
pub trait HoverProvider {
    /// 请求悬停信息。
    fn hover(
        &self,
        text: &ropey::Rope,
        offset: usize,
        window: &mut crate::Window,
        cx: &mut App,
    ) -> crate::Task<anyhow::Result<Option<HoverResponse>>>;
}

/// 悬停响应（简化版）。
#[derive(Debug, Clone)]
pub struct HoverResponse {
    /// 悬停范围。
    pub range: std::ops::Range<usize>,
    /// 格式化内容。
    pub contents: Vec<HoverContent>,
}

/// 悬停内容项。
#[derive(Debug, Clone)]
pub enum HoverContent {
    /// 纯文本。
    Text(String),
    /// Markdown 内容。
    Markdown(String),
    /// 语言代码块。
    CodeBlock {
        /// 语言标识符。
        language: String,
        /// 代码内容。
        code: String,
    },
}

impl HoverResponse {
    /// 从 LSP Hover 构建。
    pub fn from_lsp(hover: Hover, offset: usize) -> Self {
        let range = hover.range.map(|r| {
            let start = r.start.line as usize * 100 + r.start.character as usize;
            let end = r.end.line as usize * 100 + r.end.character as usize;
            start..end
        }).unwrap_or(offset..offset);

        let contents = match hover.contents {
            HoverContents::Scalar(MarkedString::String(s)) => {
                vec![HoverContent::Markdown(s)]
            }
            HoverContents::Scalar(MarkedString::LanguageString(block)) => {
                vec![HoverContent::CodeBlock {
                    language: block.language,
                    code: block.value,
                }]
            }
            HoverContents::Array(items) => items
                .into_iter()
                .map(|item| match item {
                    MarkedString::String(s) => HoverContent::Markdown(s),
                    MarkedString::LanguageString(block) => HoverContent::CodeBlock {
                        language: block.language,
                        code: block.value,
                    },
                })
                .collect(),
            HoverContents::Markup(content) => {
                vec![HoverContent::Markdown(content.value)]
            }
        };

        Self { range, contents }
    }
}

/// 悬停状态。
#[derive(Default)]
pub struct HoverState {
    /// 当前悬停响应。
    pub response: Option<HoverResponse>,
    /// 悬停位置偏移。
    pub offset: Option<usize>,
    /// 是否正在显示。
    pub visible: bool,
}

impl HoverState {
    /// 清空悬停状态。
    pub fn clear(&mut self) {
        self.response = None;
        self.offset = None;
        self.visible = false;
    }
}
