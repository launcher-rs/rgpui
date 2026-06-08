//! 对话框模块
//!
//! 提供模态对话框相关的组件，包括基础对话框、警告对话框、
//! 以及对话框的各个组成部分（头部、标题、描述、内容、底部）。

mod alert_dialog;
mod content;
mod description;
mod dialog;
mod footer;
mod header;
mod title;

pub use alert_dialog::*;
pub use content::DialogContent;
pub use description::DialogDescription;
pub use dialog::*;
pub use footer::*;
pub use header::DialogHeader;
pub use title::DialogTitle;
