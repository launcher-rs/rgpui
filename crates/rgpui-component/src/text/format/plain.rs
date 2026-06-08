use rgpui::SharedString;

use crate::text::{
    document::ParsedDocument,
    node::{BlockNode, NodeContext, Paragraph, Span},
};

/// 解析纯文本，将每一行作为段落处理。
pub(crate) fn parse(
    source: &str,
    node_cx: &mut NodeContext,
) -> Result<ParsedDocument, SharedString> {
    let mut blocks = Vec::new();
    let mut current_offset = node_cx.offset;

    for line in source.lines() {
        let line_start = current_offset;
        let line_end = line_start + line.len();

        let mut paragraph = Paragraph::new(line.to_string());
        paragraph.set_span(Span {
            start: line_start,
            end: line_end,
        });
        blocks.push(BlockNode::Paragraph(paragraph));

        // +1 for the newline character
        current_offset = line_end + 1;
    }

    // 处理空文本的情况
    if blocks.is_empty() && !source.is_empty() {
        let mut paragraph = Paragraph::new(source.to_string());
        paragraph.set_span(Span {
            start: node_cx.offset,
            end: node_cx.offset + source.len(),
        });
        blocks.push(BlockNode::Paragraph(paragraph));
    }

    Ok(ParsedDocument {
        source: source.to_string().into(),
        blocks,
    })
}
