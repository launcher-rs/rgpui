//! LSP 客户端核心 trait 定义。
//!
//! 本模块提供 Language Server Protocol 的抽象层，允许编辑器组件
//! 与任意 LSP 服务器通信，而无需关心底层传输细节。
//!
//! # 架构
//!
//! ```text
//! EditorState
//!   └── LspState
//!         ├── client: Box<dyn LspClient>
//!         ├── completions: CompletionState
//!         ├── hover: HoverState
//!         ├── definitions: DefinitionState
//!         ├── diagnostics: DiagnosticsState
//!         └── semantic_tokens: SemanticTokensState
//! ```
//!
//! 每个子系统通过 trait 解耦，应用层可按需提供实现。

mod completions;
mod completions_ui;
mod definitions;
mod diagnostics;
mod diagnostics_ui;
mod hover;
mod semantic_tokens;
mod types;

pub use completions::*;
pub use completions_ui::*;
pub use definitions::*;
pub use diagnostics::*;
pub use diagnostics_ui::*;
pub use hover::*;
pub use semantic_tokens::*;
pub use types::*;
