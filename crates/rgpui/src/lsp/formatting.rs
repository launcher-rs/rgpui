//! LSP 格式化支持（整篇格式化 / 范围格式化）。
//!
//! 传输层自理：provider 拿到文本（`Rope`）后自行请求语言服务器，把返回的
//! [`TextEdit`] 列表交给编辑器应用（编辑器按字节偏移倒序应用，见
//! `EditorState::request_formatting`）。

use lsp_types::{Range, TextEdit};
use ropey::Rope;

use crate::{App, Task, Window};

/// 格式化提供者 trait。
///
/// 应用层实现此 trait 即可让编辑器获得「格式化文档 / 格式化选区」能力。
pub trait FormattingProvider {
    /// 整篇格式化：返回要应用的文本编辑。
    fn formatting(
        &self,
        text: &Rope,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Vec<TextEdit>>>;

    /// 范围格式化（默认不支持，返回空编辑）。
    fn range_formatting(
        &self,
        text: &Rope,
        range: Range,
        window: &mut Window,
        _cx: &mut App,
    ) -> Task<anyhow::Result<Vec<TextEdit>>> {
        let _ = (text, range, window);
        Task::ready(Ok(Vec::new()))
    }
}
