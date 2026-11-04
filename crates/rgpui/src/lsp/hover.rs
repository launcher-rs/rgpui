//! LSP 悬停支持。

use lsp_types::{Hover, HoverContents, MarkedString};

use crate::App;
use crate::lsp::PositionMapping;

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
    ///
    /// `text` 是发起请求时缓冲区的内容快照：LSP 返回的是「行 + 列」，
    /// 偏移必须经由 `PositionMapping` 在这份文本上换算，
    /// 不能拿行列直接算数（原实现写的是 `line * 100 + character`）。
    pub fn from_lsp(hover: Hover, offset: usize, text: &ropey::Rope) -> Self {
        let range = hover
            .range
            .map(|r| {
                let start = PositionMapping::position_to_offset(text, r.start);
                let end = PositionMapping::position_to_offset(text, r.end);
                // 个别服务端会给倒序区间，夹紧后再返回，避免下游按 start..end 切片时 panic
                start.min(end)..end.max(start)
            })
            .unwrap_or(offset..offset);

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

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{MarkupContent, MarkupKind, Position, Range};
    use ropey::Rope;

    /// hover 区间必须按文本真实字节位置换算。
    ///
    /// 回归测试：原实现写的是 `line * 100 + character`，与缓冲区实际字节无关。
    #[test]
    fn from_lsp_maps_range_to_real_offsets() {
        let text = Rope::from("第一行\n第二行有 emoji\n");
        let range = Range::new(Position::new(1, 0), Position::new(1, 3));
        let hover = Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: "**签名**".into(),
            }),
            range: Some(range),
        };

        let response = HoverResponse::from_lsp(hover, 0, &text);
        let expected = PositionMapping::position_to_offset(&text, range.start)
            ..PositionMapping::position_to_offset(&text, range.end);
        assert_eq!(response.range, expected);
        // 旧算术会得到 100..103，而全文不足 30 字节
        assert_ne!(response.range, 100..103);
        let all = text.to_string();
        // LSP 缺省按 UTF-16 码元计列：第 1 行第 0..3 列是「第二行」三个汉字，
        // 不是 3 个字节（1 个汉字 3 字节）——按字节算只会切出「第」。
        assert_eq!(
            &all[response.range.clone()],
            "第二行",
            "换算出的区间要能按字节切出内容"
        );
        assert!(matches!(
            response.contents.first(),
            Some(HoverContent::Markdown(value)) if value == "**签名**"
        ));
    }

    /// 服务端给出倒序区间时夹紧为升序，避免下游按 `start..end` 切片时 panic。
    #[test]
    fn from_lsp_clamps_reversed_range() {
        let text = Rope::from("abcdef\n");
        let hover = Hover {
            contents: HoverContents::Scalar(MarkedString::String("x".into())),
            range: Some(Range::new(Position::new(0, 4), Position::new(0, 1))),
        };

        let response = HoverResponse::from_lsp(hover, 0, &text);
        assert_eq!(response.range, 1..4);
    }

    /// 服务端不给区间时退回请求处的偏移点。
    #[test]
    fn from_lsp_without_range_falls_back_to_offset() {
        let text = Rope::from("abc\n");
        let hover = Hover {
            contents: HoverContents::Array(vec![
                MarkedString::String("一".into()),
                MarkedString::LanguageString(lsp_types::LanguageString {
                    language: "rust".into(),
                    value: "fn a()".into(),
                }),
            ]),
            range: None,
        };

        let response = HoverResponse::from_lsp(hover, 2, &text);
        assert_eq!(response.range, 2..2);
        assert_eq!(response.contents.len(), 2);
        assert!(matches!(
            response.contents[1],
            HoverContent::CodeBlock { .. }
        ));
    }
}
