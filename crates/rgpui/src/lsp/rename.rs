//! LSP 重命名支持。
//!
//! 传输层自理：provider 拿到文本（`Rope`）与光标位置后自行请求语言服务器，
//! 把返回的 [`WorkspaceEdit`] 交给编辑器应用（编辑器按字节偏移倒序应用，见
//! `EditorState::request_rename`）。

use lsp_types::WorkspaceEdit;
use ropey::Rope;

use crate::{App, Task, Window};

/// 重命名提供者 trait。
///
/// 应用层实现此 trait 即可让编辑器获得「重命名符号」能力。
pub trait RenameProvider {
    /// 重命名光标处的符号：返回工作区编辑（`None` 表示服务端不支持或位置无符号）。
    fn rename(
        &self,
        text: &Rope,
        offset: usize,
        new_name: &str,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Option<WorkspaceEdit>>>;
}
