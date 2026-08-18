//! # rgpui-markdown
//!
//! rgpui 的 Markdown 渲染组件库。
//!
//! 独立成库（见 `docs/ui-crate-plan.md` §5 方案 A）：重依赖（pulldown-cmark）不污染
//! rgpui 核心默认构建；单用途自包含，符合 rgpui-term / rgpui-3d 的独立库模式。
//!
//! 提供能力：
//! - [`Markdown`]：将 Markdown 源码解析并渲染为富文本元素。
//! - [`rich_text`]：富文本块模型（段落/标题/列表/表格/引用/代码块）与渲染函数。
//! - [`code_block`]：带行号、复制按钮与简易高亮的代码块组件。

pub mod code_block;
pub mod markdown;
pub mod rich_text;

pub use code_block::{CodeBlock, CodeBlockCopyState};
pub use markdown::Markdown;
pub use rich_text::{LinkClickHandler, ListItem, RichBlock, RichInline, TableAlignment};
