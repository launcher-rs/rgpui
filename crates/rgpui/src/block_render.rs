//! 块级渲染组件 —— 文档块级元素的渲染支持。
//!
//! # 示例
//!
//! ```rust,ignore
//! use rgpui::block_render::{BlockRenderer, BlockElement, BlockType};
//!
//! let mut renderer = BlockRenderer::new();
//! renderer.register(BlockType::Heading, |block| {
//!     // 自定义标题渲染
//! });
//! ```

use std::collections::HashMap;

/// 块类型。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BlockType {
    /// 标题。
    Heading(u8),
    /// 段落。
    Paragraph,
    /// 列表。
    List,
    /// 代码块。
    CodeBlock,
    /// 引用块。
    Blockquote,
    /// 水平线。
    HorizontalRule,
    /// 表格。
    Table,
    /// 图片。
    Image,
    /// 自定义块。
    Custom(String),
}

/// 块元素。
#[derive(Debug, Clone)]
pub struct BlockElement {
    /// 块类型。
    pub block_type: BlockType,
    /// 块内容。
    pub content: String,
    /// 块属性。
    pub attributes: HashMap<String, String>,
    /// 子块。
    pub children: Vec<BlockElement>,
}

impl BlockElement {
    /// 创建新的块元素。
    pub fn new(block_type: BlockType, content: impl Into<String>) -> Self {
        Self {
            block_type,
            content: content.into(),
            attributes: HashMap::new(),
            children: Vec::new(),
        }
    }

    /// 添加属性。
    pub fn with_attr(mut self, key: &str, value: &str) -> Self {
        self.attributes.insert(key.to_string(), value.to_string());
        self
    }

    /// 添加子块。
    pub fn child(mut self, child: BlockElement) -> Self {
        self.children.push(child);
        self
    }
}

/// 块渲染器。
pub struct BlockRenderer {
    /// 自定义渲染器。
    renderers: HashMap<BlockType, Box<dyn Fn(&BlockElement) -> String>>,
}

impl BlockRenderer {
    /// 创建新的块渲染器。
    pub fn new() -> Self {
        Self {
            renderers: HashMap::new(),
        }
    }

    /// 注册自定义渲染器。
    pub fn register<F>(&mut self, block_type: BlockType, renderer: F)
    where
        F: Fn(&BlockElement) -> String + 'static,
    {
        self.renderers.insert(block_type, Box::new(renderer));
    }

    /// 渲染块元素。
    pub fn render(&self, block: &BlockElement) -> String {
        if let Some(renderer) = self.renderers.get(&block.block_type) {
            renderer(block)
        } else {
            self.default_render(block)
        }
    }

    /// 默认渲染逻辑。
    fn default_render(&self, block: &BlockElement) -> String {
        match &block.block_type {
            BlockType::Heading(level) => {
                format!("<h{level}>{}</h{level}>", block.content)
            }
            BlockType::Paragraph => {
                format!("<p>{}</p>", block.content)
            }
            BlockType::CodeBlock => {
                let lang = block.attributes.get("language").map(|s| s.as_str()).unwrap_or("");
                format!("<pre><code class=\"language-{lang}\">{}</code></pre>", block.content)
            }
            BlockType::Blockquote => {
                format!("<blockquote>{}</blockquote>", block.content)
            }
            BlockType::HorizontalRule => {
                "<hr />".to_string()
            }
            BlockType::List => {
                format!("<ul>{}</ul>", block.content)
            }
            BlockType::Table => {
                format!("<table>{}</table>", block.content)
            }
            BlockType::Image => {
                let src = block.attributes.get("src").map(|s| s.as_str()).unwrap_or("");
                let alt = block.attributes.get("alt").map(|s| s.as_str()).unwrap_or("");
                format!("<img src=\"{src}\" alt=\"{alt}\" />")
            }
            BlockType::Custom(name) => {
                format!("<div class=\"custom-{name}\">{}</div>", block.content)
            }
        }
    }

    /// 解析 Markdown 块级元素。
    pub fn parse_markdown(&self, input: &str) -> Vec<BlockElement> {
        let mut blocks = Vec::new();
        let mut current_paragraph = String::new();

        for line in input.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                if !current_paragraph.is_empty() {
                    blocks.push(BlockElement::new(BlockType::Paragraph, current_paragraph.clone()));
                    current_paragraph.clear();
                }
                continue;
            }

            // 标题
            if let Some(level) = trimmed.strip_prefix('#').map(|s| s.len() as u8) {
                if level <= 6 {
                    let content = trimmed.trim_start_matches('#').trim();
                    blocks.push(BlockElement::new(BlockType::Heading(level), content));
                    continue;
                }
            }

            // 水平线
            if trimmed.starts_with("---") || trimmed.starts_with("***") || trimmed.starts_with("___") {
                blocks.push(BlockElement::new(BlockType::HorizontalRule, ""));
                continue;
            }

            // 代码块
            if trimmed.starts_with("```") {
                let lang = trimmed.strip_prefix("```").unwrap_or("").trim();
                blocks.push(BlockElement::new(BlockType::CodeBlock, "").with_attr("language", lang));
                continue;
            }

            // 引用块
            if let Some(content) = trimmed.strip_prefix('>') {
                blocks.push(BlockElement::new(BlockType::Blockquote, content.trim()));
                continue;
            }

            // 累积段落
            if !current_paragraph.is_empty() {
                current_paragraph.push(' ');
            }
            current_paragraph.push_str(trimmed);
        }

        // 处理最后一段
        if !current_paragraph.is_empty() {
            blocks.push(BlockElement::new(BlockType::Paragraph, current_paragraph));
        }

        blocks
    }
}

impl Default for BlockRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paragraph_render() {
        let renderer = BlockRenderer::new();
        let block = BlockElement::new(BlockType::Paragraph, "Hello");
        assert_eq!(renderer.render(&block), "<p>Hello</p>");
    }

    #[test]
    fn test_heading_render() {
        let renderer = BlockRenderer::new();
        let h1 = BlockElement::new(BlockType::Heading(1), "Title");
        assert_eq!(renderer.render(&h1), "<h1>Title</h1>");
    }

    #[test]
    fn test_parse_markdown() {
        let renderer = BlockRenderer::new();
        let blocks = renderer.parse_markdown("# Title\n\nPara\n\n---");
        assert_eq!(blocks.len(), 3);
        assert!(matches!(blocks[0].block_type, BlockType::Heading(1)));
        assert!(matches!(blocks[1].block_type, BlockType::Paragraph));
        assert!(matches!(blocks[2].block_type, BlockType::HorizontalRule));
    }
}
