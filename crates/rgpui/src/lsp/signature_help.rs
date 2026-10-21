//! LSP 签名提示支持。
//!
//! 传输层自理：provider 拿到文本（`Rope`）与光标位置后自行请求语言服务器，
//! 把返回的 [`SignatureHelp`] 交给编辑器（编辑器只保存状态，浮层由应用渲染）。

use lsp_types::SignatureHelp;
use ropey::Rope;

use crate::{App, Task, Window};

/// 签名提示提供者 trait。
pub trait SignatureHelpProvider {
    /// 请求签名提示（`None` 表示当前位置没有可用签名）。
    fn signature_help(
        &self,
        text: &Rope,
        offset: usize,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Option<SignatureHelp>>>;
}

/// 签名提示状态（最近一次请求；应用层据此渲染浮层）。
#[derive(Debug, Clone, Default)]
pub struct SignatureHelpState {
    /// 服务端返回的签名提示。
    pub response: Option<SignatureHelp>,
    /// 当前激活的签名序号。
    pub active_signature: Option<u32>,
    /// 当前激活的参数序号。
    pub active_parameter: Option<u32>,
    /// 是否可见（无结果时为 `false`）。
    pub visible: bool,
}

impl SignatureHelpState {
    /// 清空状态。
    pub fn clear(&mut self) {
        self.response = None;
        self.active_signature = None;
        self.active_parameter = None;
        self.visible = false;
    }
}
