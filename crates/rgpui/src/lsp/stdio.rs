//! stdio LSP 传输（O1）：子进程 + JSON-RPC Content-Length 帧 + 请求路由 + 通知分发。
//!
//! - 只做 stdio（TCP/websocket 不做）；无新依赖（`std::process` + 线程 + 通道）；
//! - wasm 不可用（无进程原语），模块整体 cfg 门控；
//! - 进程异常退出则在途请求全失败、**不守护重启**（重启是应用层策略，文档即契约）；
//! - `Drop` 时 `kill` 子进程（`try_wait` 顺手回收，阻塞式 `wait` 不做，避免卡 UI 线程）。

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicI64, Ordering},
};

use anyhow::{Context as _, Result};
use futures::channel::oneshot;
use lsp_types::{InitializeParams, InitializeResult, ServerCapabilities};
use serde_json::Value;

use super::types::LspClient;
use crate::{App, Task};

/// 服务端 → 客户端通知（方法 + 参数，如 `textDocument/publishDiagnostics`）。
///
/// 应用层经 [`StdioLspClient::try_recv_notification`] 轮询；通道无界，
/// 不轮询只会积压内存，不会阻塞服务端读取线程。
#[derive(Debug, Clone)]
pub struct ServerNotification {
    /// 通知方法名。
    pub method: String,
    /// 通知参数。
    pub params: Value,
}

/// JSON-RPC 响应错误（`{ code, message }`）。
#[derive(Debug, Clone)]
pub struct ResponseError {
    /// 错误码。
    pub code: i64,
    /// 错误信息。
    pub message: String,
}

impl std::fmt::Display for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "lsp error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for ResponseError {}

/// 从 `reader` 读一帧（Content-Length 头 + 体；流正常结束返回 `Ok(None)`）。
///
/// 只认 `Content-Length`（LSP 传输语义，不支持纯换行分隔）；头大小写不敏感，
/// 未知头跳过；体长度与声明不符（提前 EOF）返回错误。
pub fn read_message(reader: &mut impl BufRead) -> std::io::Result<Option<Vec<u8>>> {
    let mut content_length: Option<usize> = None;
    let mut line = Vec::new();
    loop {
        line.clear();
        let n = reader.read_until(b'\n', &mut line)?;
        if n == 0 {
            // 头未读完即 EOF：有半截头算损坏，无头算正常结束。
            if content_length.is_some() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "lsp 头部截断",
                ));
            }
            return Ok(None);
        }
        let text = String::from_utf8_lossy(&line);
        let trimmed = text.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed
            .strip_prefix("Content-Length:")
            .or_else(|| trimmed.strip_prefix("content-length:"))
        {
            content_length = value.trim().parse::<usize>().ok();
        }
    }
    let Some(len) = content_length else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "lsp 缺少 Content-Length 头",
        ));
    };
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    Ok(Some(body))
}

/// 编码一帧（`Content-Length` 头 + `\r\n` + 体）。
pub fn encode_message(body: &[u8]) -> Vec<u8> {
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend_from_slice(body);
    out
}

/// stdio 传输（读写线程 + 请求路由；`Send + Sync`，多处共享）。
struct Transport {
    /// 下一请求 id（1 起）。
    next_id: AtomicI64,
    /// 在途请求（id → 完成通道）。
    pending: Mutex<HashMap<i64, oneshot::Sender<Result<Value, ResponseError>>>>,
    /// 写队列（写线程单写者；服务端已死时发送失败，记日志丢弃）。
    writer: Mutex<std::sync::mpsc::Sender<Vec<u8>>>,
    /// 服务端通知队列（无界；应用层轮询取走）。
    notifications: Mutex<std::sync::mpsc::Receiver<ServerNotification>>,
}

impl Transport {
    /// 由已建好的字节流启动传输（读/写/stderr 各一线程）。
    ///
    /// `stderr` 为 `None` 时跳过日志线程（测试用）。
    fn new(
        reader: Box<dyn BufRead + Send>,
        writer: Box<dyn Write + Send>,
        stderr: Option<Box<dyn Read + Send>>,
        program: &str,
    ) -> Arc<Self> {
        let (bytes_tx, bytes_rx) = std::sync::mpsc::channel::<Vec<u8>>();
        let (notify_tx, notify_rx) = std::sync::mpsc::channel::<ServerNotification>();
        let this = Arc::new(Self {
            next_id: AtomicI64::new(1),
            pending: Mutex::new(HashMap::new()),
            writer: Mutex::new(bytes_tx),
            notifications: Mutex::new(notify_rx),
        });
        std::thread::Builder::new()
            .name(format!("lsp-reader-{program}"))
            .spawn({
                let this = this.clone();
                move || {
                    Self::read_loop(reader, &this, notify_tx);
                    // 读循环结束 = 流断了：在途请求全失败，避免永远挂起。
                    this.fail_all_pending();
                }
            })
            .ok();
        std::thread::Builder::new()
            .name(format!("lsp-writer-{program}"))
            .spawn(move || {
                Self::write_loop(writer, bytes_rx);
            })
            .ok();
        if let Some(stderr) = stderr {
            let program = program.to_string();
            std::thread::Builder::new()
                .name(format!("lsp-stderr-{program}"))
                .spawn(move || {
                    Self::stderr_loop(stderr, &program);
                })
                .ok();
        }
        this
    }

    /// 读循环：分帧 → 有 id 走响应路由，无 id 有 method 走通知，其余记日志丢弃。
    fn read_loop(
        mut reader: Box<dyn BufRead + Send>,
        this: &Arc<Self>,
        notify_tx: std::sync::mpsc::Sender<ServerNotification>,
    ) {
        loop {
            let frame = match read_message(&mut reader) {
                Ok(Some(body)) => body,
                Ok(None) => break,
                Err(error) => {
                    log::warn!("lsp 读帧失败: {error}");
                    break;
                }
            };
            let value: Value = match serde_json::from_slice(&frame) {
                Ok(value) => value,
                Err(error) => {
                    log::warn!("lsp 非 JSON 帧丢弃: {error}");
                    continue;
                }
            };
            if let Some(id) = value.get("id").and_then(Value::as_i64) {
                let result = if let Some(error) = value.get("error") {
                    Err(ResponseError {
                        code: error.get("code").and_then(Value::as_i64).unwrap_or(-1),
                        message: error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("未知错误")
                            .to_string(),
                    })
                } else {
                    Ok(value.get("result").cloned().unwrap_or(Value::Null))
                };
                if let Some(tx) = this.pending.lock().unwrap().remove(&id) {
                    let _ = tx.send(result);
                }
            } else if let Some(method) = value.get("method").and_then(Value::as_str) {
                let notification = ServerNotification {
                    method: method.to_string(),
                    params: value.get("params").cloned().unwrap_or(Value::Null),
                };
                if notify_tx.send(notification).is_err() {
                    break;
                }
            } else {
                log::warn!("lsp 无 id 无 method 帧丢弃");
            }
        }
    }

    /// 写循环：队列字节逐帧刷出；对端断开即退出。
    fn write_loop(mut writer: Box<dyn Write + Send>, rx: std::sync::mpsc::Receiver<Vec<u8>>) {
        for bytes in rx {
            if writer
                .write_all(&bytes)
                .and_then(|_| writer.flush())
                .is_err()
            {
                break;
            }
        }
    }

    /// stderr 透日志（server 崩溃可查；行缓冲，退出即停）。
    fn stderr_loop(reader: Box<dyn Read + Send>, program: &str) {
        let mut reader = BufReader::new(reader);
        let mut line = Vec::new();
        loop {
            line.clear();
            match reader.read_until(b'\n', &mut line) {
                Ok(0) => break,
                Ok(_) => {
                    log::warn!(
                        "lsp[{program}] {}",
                        String::from_utf8_lossy(&line).trim_end()
                    );
                }
                Err(_) => break,
            }
        }
    }

    /// 在途请求全失败（读循环结束时调用）。
    fn fail_all_pending(&self) {
        let mut pending = self.pending.lock().unwrap();
        for (_, tx) in pending.drain() {
            let _ = tx.send(Err(ResponseError {
                code: -1,
                message: "lsp 传输已断开".to_string(),
            }));
        }
    }

    /// 发请求（不阻塞；经后台执行器等待响应）。
    ///
    /// 返回的 future 需执行器驱动（如 `cx.background_executor().spawn`）；
    /// 对端已死时写队列发送失败，直接返回错误。
    fn request(
        self: &Arc<Self>,
        method: &str,
        params: Value,
    ) -> impl Future<Output = Result<Value>> + Send + 'static {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let queued = self
            .writer
            .lock()
            .unwrap()
            .send(encode_message(
                &serde_json::to_vec(&body).unwrap_or_default(),
            ))
            .is_ok();
        if !queued {
            self.pending.lock().unwrap().remove(&id);
        }
        async move {
            if !queued {
                anyhow::bail!("lsp 写队列已断开");
            }
            rx.await
                .map_err(|_| anyhow::anyhow!("lsp 请求无响应（传输已断开）"))?
                .map_err(anyhow::Error::from)
        }
    }

    /// 发通知（不阻塞；失败只记日志）。
    fn notify(&self, method: &str, params: Value) {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        if self
            .writer
            .lock()
            .unwrap()
            .send(encode_message(
                &serde_json::to_vec(&body).unwrap_or_default(),
            ))
            .is_err()
        {
            log::warn!("lsp 通知发送失败（传输已断开）: {method}");
        }
    }

    /// 取一条服务端通知（无则 `None`，不阻塞）。
    fn try_recv_notification(&self) -> Option<ServerNotification> {
        self.notifications.lock().unwrap().try_recv().ok()
    }
}

/// stdio LSP 客户端（`LspClient` 的进程传输实现）。
///
/// 能力快照语义：`spawn` 时为空默认能力；`initialize()` 的 `Task` 完成后，
/// 应用层用返回的 `InitializeResult.capabilities` 自行判断（`server_capabilities`
/// 签名只读，客户端不代写）；持有具体类型时可用 [`Self::set_capabilities`] 跟随。
pub struct StdioLspClient {
    /// 传输（线程 + 路由）。
    transport: Arc<Transport>,
    /// 能力快照（`set_capabilities` 显式跟随）。
    capabilities: ServerCapabilities,
    /// 子进程句柄（`Drop` 时 `kill`，不守护重启）。
    child: Mutex<Option<Child>>,
}

impl StdioLspClient {
    /// 拉起子进程并接管 stdio（`stderr` 透日志）。
    ///
    /// 程序不存在/无权限即返回错误；进程内存活与否不管（首个请求失败即见分晓）。
    pub fn spawn(command: &str, args: &[&str]) -> Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("lsp 启动失败: {command}"))?;
        let stdin: ChildStdin = child.stdin.take().context("lsp stdin 管道缺失")?;
        let stdout: ChildStdout = child.stdout.take().context("lsp stdout 管道缺失")?;
        let stderr = child.stderr.take();
        let transport = Transport::new(
            Box::new(BufReader::new(stdout)),
            Box::new(stdin),
            stderr.map(|stderr| Box::new(stderr) as Box<dyn Read + Send>),
            command,
        );
        Ok(Self {
            transport,
            capabilities: ServerCapabilities::default(),
            child: Mutex::new(Some(child)),
        })
    }

    /// 跟随服务端能力（`initialize` 成功后应用层调用）。
    pub fn set_capabilities(&mut self, capabilities: ServerCapabilities) {
        self.capabilities = capabilities;
    }

    /// 取一条服务端通知（无则 `None`，不阻塞；如诊断推送）。
    pub fn try_recv_notification(&self) -> Option<ServerNotification> {
        self.transport.try_recv_notification()
    }

    /// 发请求（`LspClient` 方法族的公共底层；`cx` 仅供取后台执行器）。
    fn request(&self, method: &str, params: Value, cx: &mut App) -> Task<Result<Value>> {
        cx.background_executor()
            .spawn(self.request_future(method, params))
    }

    /// 发请求的纯 future 版（不经执行器；测试用 `block_on` 直驱）。
    fn request_future(
        &self,
        method: &str,
        params: Value,
    ) -> impl Future<Output = Result<Value>> + Send + 'static {
        self.transport.request(method, params)
    }
}

impl Drop for StdioLspClient {
    /// 杀子进程（`try_wait` 顺手回收已退出的，阻塞式 `wait` 不做）。
    fn drop(&mut self) {
        if let Some(mut child) = self.child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.try_wait();
        }
    }
}

/// 请求结果 JSON 反序列化（`null` 按 `Default` 处理，LSP 可选字段惯例）。
fn from_result<T: serde::de::DeserializeOwned + Default>(value: Value) -> Result<T> {
    if value.is_null() {
        return Ok(T::default());
    }
    Ok(serde_json::from_value(value)?)
}

impl LspClient for StdioLspClient {
    fn server_capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    fn initialize(&self, params: InitializeParams, cx: &mut App) -> Task<Result<InitializeResult>> {
        let task = self.request(
            "initialize",
            serde_json::to_value(params).unwrap_or(Value::Null),
            cx,
        );
        cx.background_executor().spawn(async move {
            let value = task.await?;
            Ok(serde_json::from_value(value)?)
        })
    }

    fn shutdown(&self, cx: &mut App) -> Task<Result<()>> {
        let task = self.request("shutdown", Value::Null, cx);
        let transport = self.transport.clone();
        cx.background_executor().spawn(async move {
            task.await?;
            // `shutdown` 响应后按协议发 `exit` 通知。
            transport.notify("exit", Value::Null);
            Ok(())
        })
    }

    fn completions(
        &self,
        params: lsp_types::CompletionParams,
        cx: &mut App,
    ) -> Task<Result<lsp_types::CompletionResponse>> {
        let task = self.request(
            "textDocument/completion",
            serde_json::to_value(params).unwrap_or(Value::Null),
            cx,
        );
        cx.background_executor().spawn(async move {
            let value = task.await?;
            if value.is_null() {
                return Ok(lsp_types::CompletionResponse::Array(Vec::new()));
            }
            Ok(serde_json::from_value(value)?)
        })
    }

    fn hover(
        &self,
        params: lsp_types::HoverParams,
        cx: &mut App,
    ) -> Task<Result<Option<lsp_types::Hover>>> {
        let task = self.request(
            "textDocument/hover",
            serde_json::to_value(params).unwrap_or(Value::Null),
            cx,
        );
        cx.background_executor()
            .spawn(async move { Ok(serde_json::from_value(task.await?)?) })
    }

    fn definition(
        &self,
        params: lsp_types::GotoDefinitionParams,
        cx: &mut App,
    ) -> Task<Result<lsp_types::GotoDefinitionResponse>> {
        let task = self.request(
            "textDocument/definition",
            serde_json::to_value(params).unwrap_or(Value::Null),
            cx,
        );
        cx.background_executor().spawn(async move {
            let value = task.await?;
            if value.is_null() {
                return Ok(lsp_types::GotoDefinitionResponse::Array(Vec::new()));
            }
            Ok(serde_json::from_value(value)?)
        })
    }

    fn references(
        &self,
        params: lsp_types::ReferenceParams,
        cx: &mut App,
    ) -> Task<Result<Vec<lsp_types::Location>>> {
        let task = self.request(
            "textDocument/references",
            serde_json::to_value(params).unwrap_or(Value::Null),
            cx,
        );
        cx.background_executor()
            .spawn(async move { from_result(task.await?) })
    }

    fn diagnostics(
        &self,
        params: lsp_types::DocumentDiagnosticParams,
        cx: &mut App,
    ) -> Task<Result<lsp_types::DocumentDiagnosticReport>> {
        let task = self.request(
            "textDocument/diagnostic",
            serde_json::to_value(params).unwrap_or(Value::Null),
            cx,
        );
        cx.background_executor()
            .spawn(async move { Ok(serde_json::from_value(task.await?)?) })
    }

    fn semantic_tokens_full(
        &self,
        params: lsp_types::SemanticTokensParams,
        cx: &mut App,
    ) -> Task<Result<Option<lsp_types::SemanticTokensResult>>> {
        let task = self.request(
            "textDocument/semanticTokens/full",
            serde_json::to_value(params).unwrap_or(Value::Null),
            cx,
        );
        cx.background_executor()
            .spawn(async move { Ok(serde_json::from_value(task.await?)?) })
    }

    fn semantic_tokens_range(
        &self,
        params: lsp_types::SemanticTokensRangeParams,
        cx: &mut App,
    ) -> Task<Result<Option<lsp_types::SemanticTokensRangeResult>>> {
        let task = self.request(
            "textDocument/semanticTokens/range",
            serde_json::to_value(params).unwrap_or(Value::Null),
            cx,
        );
        cx.background_executor()
            .spawn(async move { Ok(serde_json::from_value(task.await?)?) })
    }

    fn code_actions(
        &self,
        params: lsp_types::CodeActionParams,
        cx: &mut App,
    ) -> Task<Result<Vec<lsp_types::CodeActionOrCommand>>> {
        let task = self.request(
            "textDocument/codeAction",
            serde_json::to_value(params).unwrap_or(Value::Null),
            cx,
        );
        cx.background_executor()
            .spawn(async move { from_result(task.await?) })
    }

    fn formatting(
        &self,
        params: lsp_types::DocumentFormattingParams,
        cx: &mut App,
    ) -> Task<Result<Vec<lsp_types::TextEdit>>> {
        let task = self.request(
            "textDocument/formatting",
            serde_json::to_value(params).unwrap_or(Value::Null),
            cx,
        );
        cx.background_executor()
            .spawn(async move { from_result(task.await?) })
    }

    fn rename(
        &self,
        params: lsp_types::RenameParams,
        cx: &mut App,
    ) -> Task<Result<Option<lsp_types::WorkspaceEdit>>> {
        let task = self.request(
            "textDocument/rename",
            serde_json::to_value(params).unwrap_or(Value::Null),
            cx,
        );
        cx.background_executor()
            .spawn(async move { Ok(serde_json::from_value(task.await?)?) })
    }

    fn document_highlights(
        &self,
        params: lsp_types::DocumentHighlightParams,
        cx: &mut App,
    ) -> Task<Result<Vec<lsp_types::DocumentHighlight>>> {
        let task = self.request(
            "textDocument/documentHighlight",
            serde_json::to_value(params).unwrap_or(Value::Null),
            cx,
        );
        cx.background_executor()
            .spawn(async move { from_result(task.await?) })
    }

    fn signature_help(
        &self,
        params: lsp_types::SignatureHelpParams,
        cx: &mut App,
    ) -> Task<Result<Option<lsp_types::SignatureHelp>>> {
        let task = self.request(
            "textDocument/signatureHelp",
            serde_json::to_value(params).unwrap_or(Value::Null),
            cx,
        );
        cx.background_executor()
            .spawn(async move { Ok(serde_json::from_value(task.await?)?) })
    }

    fn did_change(
        &self,
        identifier: lsp_types::TextDocumentIdentifier,
        changes: Vec<lsp_types::TextDocumentContentChangeEvent>,
        _cx: &mut App,
    ) {
        self.transport.notify(
            "textDocument/didChange",
            serde_json::json!({
                "textDocument": identifier,
                "contentChanges": changes,
            }),
        );
    }

    fn did_open(
        &self,
        identifier: lsp_types::TextDocumentIdentifier,
        language_id: &str,
        text: &str,
        _cx: &mut App,
    ) {
        self.transport.notify(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": identifier.uri,
                    "languageId": language_id,
                    "version": 0,
                    "text": text,
                },
            }),
        );
    }

    fn did_close(&self, identifier: lsp_types::TextDocumentIdentifier, _cx: &mut App) {
        self.transport.notify(
            "textDocument/didClose",
            serde_json::json!({ "textDocument": identifier }),
        );
    }

    fn did_save(
        &self,
        identifier: lsp_types::TextDocumentIdentifier,
        text: Option<String>,
        _cx: &mut App,
    ) {
        self.transport.notify(
            "textDocument/didSave",
            serde_json::json!({ "textDocument": identifier, "text": text }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::net::{TcpListener, TcpStream};

    /// 编码→解码往返（含多帧流与大小写头）。
    #[test]
    fn frame_roundtrip() {
        let bodies: Vec<Vec<u8>> = vec![b"{}".to_vec(), b"{\"id\":1}".to_vec()];
        let mut stream = Vec::new();
        for body in &bodies {
            stream.extend_from_slice(&encode_message(body));
        }
        // 第二帧改小写头，验证大小写不敏感。
        let lower =
            String::from_utf8_lossy(&stream).replacen("Content-Length", "content-length", 1);
        let mut reader = Cursor::new(lower.into_bytes());
        for body in &bodies {
            assert_eq!(read_message(&mut reader).unwrap(), Some(body.clone()));
        }
        assert_eq!(read_message(&mut reader).unwrap(), None);
    }

    /// 缺头/截断报错。
    #[test]
    fn frame_errors() {
        let mut reader = Cursor::new(b"{}\r\n\r\n".to_vec());
        assert!(read_message(&mut reader).is_err());
        let mut reader = Cursor::new(b"Content-Length: 10\r\n\r\nabc".to_vec());
        assert!(read_message(&mut reader).is_err());
    }

    /// 回环 TCP 对上的请求/响应/通知全链路（真线程 + 真路由）。
    ///
    /// 用回环 TCP 代内存 duplex（std 无 pipe）；传输层只认 `BufRead + Write`，
    /// 与子进程管道走同一代码。
    #[test]
    fn transport_request_response_notification() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut writer = stream;
            // 读请求，回固定响应；再主动推一条通知。
            let frame = read_message(&mut reader).unwrap().unwrap();
            let request: Value = serde_json::from_slice(&frame).unwrap();
            assert_eq!(request["method"], "textDocument/completion");
            let id = request["id"].clone();
            let response = serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": [] });
            writer
                .write_all(&encode_message(&serde_json::to_vec(&response).unwrap()))
                .unwrap();
            writer.flush().unwrap();
            let notification =
                serde_json::json!({ "jsonrpc": "2.0", "method": "m", "params": { "a": 1 } });
            writer
                .write_all(&encode_message(&serde_json::to_vec(&notification).unwrap()))
                .unwrap();
            writer.flush().unwrap();
        });
        let stream = TcpStream::connect(addr).unwrap();
        let transport = Transport::new(
            Box::new(BufReader::new(stream.try_clone().unwrap())),
            Box::new(stream),
            None,
            "test",
        );
        let value =
            futures::executor::block_on(transport.request("textDocument/completion", Value::Null))
                .unwrap();
        assert_eq!(value, Value::Array(vec![]));
        // 通知稍后到达：轮询等待。
        let mut notification = None;
        for _ in 0..100 {
            notification = transport.try_recv_notification();
            if notification.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let notification = notification.expect("服务端通知应到达");
        assert_eq!(notification.method, "m");
        server.join().unwrap();
    }

    /// 不存在的程序启动失败（不 panic，带程序名的错误）。
    #[test]
    fn spawn_missing_program_errors() {
        match StdioLspClient::spawn("rgpui-test-only-missing-program", &[]) {
            Ok(_) => panic!("不存在的程序应启动失败"),
            Err(error) => assert!(
                error
                    .to_string()
                    .contains("rgpui-test-only-missing-program")
            ),
        }
    }

    /// 补全方法映射（假服务端回固定补全项；方法名断言在服务端侧）。
    ///
    /// 普通 `#[test]`：传输线程的跨线程唤醒与确定性调度器不兼容
    /// （`#[rgpui::test]` 下 pump 会 panic），故只 `block_on` 纯 future，
    /// 全程不碰调度器。
    #[test]
    fn client_completions_mapping() {
        use std::time::Duration;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut writer = stream;
            while let Ok(Some(frame)) = read_message(&mut reader) {
                let request: Value = serde_json::from_slice(&frame).unwrap();
                assert_eq!(
                    request["method"], "textDocument/completion",
                    "方法名映射必须正确",
                );
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request["id"],
                    "result": [{ "label": "println" }],
                });
                writer
                    .write_all(&encode_message(&serde_json::to_vec(&response).unwrap()))
                    .unwrap();
                writer.flush().unwrap();
            }
        });
        let stream = std::net::TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let transport = Transport::new(
            Box::new(BufReader::new(stream.try_clone().unwrap())),
            Box::new(stream),
            None,
            "test",
        );
        let client = StdioLspClient {
            transport,
            capabilities: ServerCapabilities::default(),
            child: Mutex::new(None),
        };
        let params = serde_json::json!({
            "textDocument": { "uri": "file:///a.rs" },
            "position": { "line": 0, "character": 0 },
        });
        let value =
            futures::executor::block_on(client.request_future("textDocument/completion", params))
                .unwrap();
        let response: lsp_types::CompletionResponse = serde_json::from_value(value).unwrap();
        match response {
            lsp_types::CompletionResponse::Array(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].label, "println");
            }
            other => panic!("应为数组响应: {other:?}"),
        }
    }
}
