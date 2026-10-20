//! 控制通道 —— 本机 JSON-RPC 2.0 over TCP（密钥鉴权），把命令注册表暴露给
//! CLI、脚本与外部自动化；同一张方法表也承载 MCP 服务端（见 [`crate::mcp`]）。
//!
//! ## 为什么需要它
//!
//! 命令注册表（[`crate::commands`]）统一了「UI / 快捷键 / 命令面板」的分发入口，
//! 但外部进程（编辑器插件、自动化脚本、MCP 客户端）还缺一条稳定的通道。
//! 本模块提供最小可用的一条：本机回环 TCP + 换行分隔的 JSON-RPC 2.0，
//! 每条请求必须带密钥（默认随机生成），避免同机其他程序误连。
//!
//! ## 协议
//!
//! ```text
//! -> {"jsonrpc":"2.0","id":1,"key":"<密钥>","method":"commands.list"}
//! <- {"jsonrpc":"2.0","id":1,"result":{"commands":[...]}}
//! ```
//!
//! 方法表：
//!
//! | 方法 | 参数 | 说明 |
//! |------|------|------|
//! | `ping` | — | 连通性探测 |
//! | `app.info` | — | 版本、平台、命令数、窗口数 |
//! | `commands.list` | — | 全部命令元数据 |
//! | `commands.get` | `{"id":"..."}` | 单条命令元数据 |
//! | `command.dispatch` | `{"id":"...","params":{...}}` | 按 id 派发动作 |
//! | `window.list` | — | 窗口 id 与焦点态 |
//! | `initialize` / `tools/list` / `tools/call` | 见 [`crate::mcp`] | MCP 方法 |
//!
//! ## 线程模型
//!
//! TCP 读写在 Tokio 线程池（[`crate::tokio`]）上跑，真正的动作派发必须回到
//! 主线程：请求经 [`RpcDispatcher`] 投递到前台执行器的任务里，由它调用
//! [`App::dispatch_command`]，再经 oneshot 把结果送回连接。
//!
//! ## 用法
//!
//! ```text
//! let channel = cx.start_control_channel(ControlChannelOptions::default())?;
//! println!("{} {}", channel.addr(), channel.key());
//! // 句柄丢弃即关闭监听
//! ```
//!
//! > feature 门控：`control-channel`（默认关闭）。启用后隐含 `tokio` feature。

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Context as _;
use serde_json::{Value, json};

use crate::commands::{CommandDescriptor, CommandRegistry};
use crate::tokio::Tokio;
use crate::{App, Task};

/// 控制通道配置。
#[derive(Clone, Debug)]
pub struct ControlChannelOptions {
    /// 监听地址；默认 `127.0.0.1:0`（回环 + 系统分配端口）。
    ///
    /// 绑到 `0.0.0.0` 会让同网段其他机器可连，除非确有必要请保持回环。
    pub bind_addr: SocketAddr,
    /// 访问密钥；`None` 表示自动生成 32 位十六进制密钥。
    pub key: Option<String>,
    /// 最大并发连接数（默认 4）；超出的连接直接断开。
    pub max_connections: usize,
}

impl Default for ControlChannelOptions {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            key: None,
            max_connections: 4,
        }
    }
}

impl ControlChannelOptions {
    /// 指定监听地址（链式构造）。
    pub fn with_bind_addr(mut self, bind_addr: SocketAddr) -> Self {
        self.bind_addr = bind_addr;
        self
    }

    /// 指定访问密钥（链式构造）。
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// 指定最大并发连接数（链式构造）。
    pub fn with_max_connections(mut self, max_connections: usize) -> Self {
        self.max_connections = max_connections;
        self
    }
}

/// 运行中的控制通道；丢弃句柄即停止监听（已建立的连接在下一次请求时退出）。
pub struct ControlChannel {
    addr: SocketAddr,
    key: String,
    /// 主线程调度器：持有前台任务，丢弃即停止泵送。
    _dispatcher: RpcDispatcher,
    /// 接受连接的任务：丢弃即 abort。
    _accept: Task<Result<anyhow::Result<()>, crate::tokio::JoinError>>,
}

impl ControlChannel {
    /// 启动控制通道。
    ///
    /// 若尚未初始化 Tokio 运行时，会先调用 [`crate::tokio::init`]。返回的句柄
    /// 需要由调用方持有（例如存进应用状态）；丢弃后不再接受新连接。
    pub fn start(cx: &mut App, options: ControlChannelOptions) -> anyhow::Result<Self> {
        if !crate::tokio::is_initialized(cx) {
            crate::tokio::init(cx);
        }

        let listener = std::net::TcpListener::bind(options.bind_addr)
            .with_context(|| format!("控制通道无法绑定 {}", options.bind_addr))?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let key = options.key.unwrap_or_else(generate_key);

        let dispatcher = RpcDispatcher::new(cx);
        let handle = dispatcher.handle();
        let active = Arc::new(AtomicUsize::new(0));
        let accept = Tokio::spawn(
            cx,
            accept_loop(
                listener,
                key.clone(),
                handle,
                options.max_connections.max(1),
                active,
            ),
        );

        Ok(Self {
            addr,
            key,
            _dispatcher: dispatcher,
            _accept: accept,
        })
    }

    /// 实际监听地址（端口由系统分配时用它取回）。
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// 访问密钥；客户端每条请求都要带上。
    pub fn key(&self) -> &str {
        &self.key
    }
}

impl App {
    /// 启动控制通道（本机 TCP + 密钥）。
    ///
    /// 等价于 [`ControlChannel::start`]，方便在应用启动流程里直接调用。
    pub fn start_control_channel(
        &mut self,
        options: ControlChannelOptions,
    ) -> anyhow::Result<ControlChannel> {
        ControlChannel::start(self, options)
    }
}

/// 生成 32 位十六进制访问密钥。
fn generate_key() -> String {
    format!(
        "{:016x}{:016x}",
        rand::random::<u64>(),
        rand::random::<u64>()
    )
}

/// 一条待主线程处理的 JSON-RPC 请求。
pub(crate) struct RpcJob {
    request: Value,
    reply: futures::channel::oneshot::Sender<Option<Value>>,
}

/// 把后台线程的请求投递到主线程执行的调度器。
///
/// 内部持有一个前台任务：从通道取出请求 → 调用 [`handle_request`] → 回送结果。
pub(crate) struct RpcDispatcher {
    requests: async_channel::Sender<RpcJob>,
    /// 泵送任务；丢弃即取消（连接侧随后读不到结果并退出）。
    _pump: Task<()>,
}

impl RpcDispatcher {
    pub(crate) fn new(cx: &mut App) -> Self {
        let (requests, jobs) = async_channel::unbounded::<RpcJob>();
        let pump = cx.spawn(async move |cx| {
            while let Ok(job) = jobs.recv().await {
                // 动作派发必须在主线程：`update` 会借用 App 并刷新副作用。
                let response = cx.update(|cx| handle_request(cx, &job.request));
                let _ = job.reply.send(response);
            }
        });
        Self {
            requests,
            _pump: pump,
        }
    }

    pub(crate) fn handle(&self) -> RpcHandle {
        RpcHandle {
            requests: self.requests.clone(),
        }
    }
}

/// 可跨线程移动的请求句柄：只负责把请求投递到主线程。
#[derive(Clone)]
pub(crate) struct RpcHandle {
    requests: async_channel::Sender<RpcJob>,
}

impl RpcHandle {
    /// 异步等待主线程处理完一条请求（通知类请求返回 `None`）。
    pub(crate) async fn request(&self, request: Value) -> Option<Value> {
        let (reply, response) = futures::channel::oneshot::channel();
        self.requests.send(RpcJob { request, reply }).await.ok()?;
        response.await.ok().flatten()
    }

    /// 阻塞等待主线程处理完一条请求（供非异步线程使用）。
    pub(crate) fn request_blocking(&self, request: Value) -> Option<Value> {
        futures::executor::block_on(self.request(request))
    }
}

/// 接受连接的循环；客户端断开或监听失败时返回。
async fn accept_loop(
    listener: std::net::TcpListener,
    key: String,
    handle: RpcHandle,
    max_connections: usize,
    active: Arc<AtomicUsize>,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::from_std(listener)?;
    loop {
        let (stream, _peer) = listener.accept().await?;
        if active.load(Ordering::Relaxed) >= max_connections {
            // 超限：直接丢弃，避免外部脚本把连接池占满。
            drop(stream);
            continue;
        }
        active.fetch_add(1, Ordering::Relaxed);
        let handle = handle.clone();
        let key = key.clone();
        let active = active.clone();
        tokio::spawn(async move {
            let _ = serve_connection(stream, key, handle).await;
            active.fetch_sub(1, Ordering::Relaxed);
        });
    }
}

/// 单个连接：逐行读 JSON-RPC，逐行写回复。
async fn serve_connection(
    stream: tokio::net::TcpStream,
    key: String,
    handle: RpcHandle,
) -> std::io::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Value>(line) {
            Ok(request) if !authorized(&request, &key) => {
                Some(error_response(Value::Null, -32001, "控制通道密钥不匹配"))
            }
            // 主线程调度器停止（应用退出）时 `request` 返回 None，直接断开。
            Ok(request) => handle.request(request).await,
            Err(error) => Some(error_response(
                Value::Null,
                -32700,
                format!("JSON 解析失败：{error}"),
            )),
        };

        if let Some(response) = response {
            let mut text = serde_json::to_string(&response).map_err(std::io::Error::other)?;
            text.push('\n');
            writer.write_all(text.as_bytes()).await?;
            writer.flush().await?;
        }
    }
    Ok(())
}

/// 校验请求携带的密钥（请求顶层 `key` 字段）。
fn authorized(request: &Value, key: &str) -> bool {
    request
        .get("key")
        .and_then(Value::as_str)
        .is_some_and(|candidate| candidate == key)
}

/// 一条 JSON-RPC 方法执行的错误。
#[derive(Debug)]
pub(crate) struct RpcError {
    pub(crate) code: i64,
    pub(crate) message: String,
}

impl RpcError {
    pub(crate) fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("未知方法：{method}"),
        }
    }

    pub(crate) fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
        }
    }

    pub(crate) fn failed(message: impl Into<String>) -> Self {
        Self {
            code: -32000,
            message: message.into(),
        }
    }
}

/// 构造成功响应。
fn success_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// 构造错误响应。
pub(crate) fn error_response(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message.into() },
    })
}

/// 处理一条 JSON-RPC 请求；通知（无 `id`）返回 `None`。
pub(crate) fn handle_request(cx: &mut App, request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return id.map(|id| error_response(id, -32600, "缺少 method 字段"));
    };
    // 通知没有 id：只做副作用，不回复。
    let id = id?;
    let params = request.get("params").cloned().unwrap_or(Value::Null);

    Some(match dispatch_method(cx, method, &params) {
        Ok(result) => success_response(id, result),
        Err(error) => error_response(id, error.code, error.message),
    })
}

/// 方法分发：先交给 MCP 方法表，再处理通道自有方法。
fn dispatch_method(cx: &mut App, method: &str, params: &Value) -> Result<Value, RpcError> {
    if let Some(result) = crate::mcp::handle(cx, method, params) {
        return result;
    }

    match method {
        "ping" => Ok(json!({ "pong": true })),
        "app.info" => Ok(app_info(cx)),
        "commands.list" => Ok(json!({ "commands": commands_json() })),
        "commands.get" => {
            let id = command_id_param(params)?;
            let descriptor = CommandRegistry::global()
                .get(id)
                .ok_or_else(|| RpcError::invalid_params(format!("未登记的命令：{id}")))?;
            Ok(descriptor_json(descriptor))
        }
        "command.dispatch" => {
            let id = command_id_param(params)?;
            let arguments = match params.get("params") {
                None | Some(Value::Null) => None,
                Some(value) => Some(value.clone()),
            };
            cx.dispatch_command(id, arguments)
                .map_err(|error| RpcError::failed(format!("派发 `{id}` 失败：{error:#}")))?;
            Ok(json!({ "dispatched": id }))
        }
        "window.list" => Ok(windows_json(cx)),
        other => Err(RpcError::method_not_found(other)),
    }
}

/// 取 `params.id`（命令 id）。
fn command_id_param(params: &Value) -> Result<&str, RpcError> {
    params
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::invalid_params("缺少字符串参数 `id`"))
}

/// 应用信息（版本 / 平台 / 命令数 / 窗口数）。
fn app_info(cx: &App) -> Value {
    let registry = CommandRegistry::global();
    json!({
        "name": "rgpui",
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "commands": registry.len(),
        "categories": registry
            .categories()
            .iter()
            .map(|category| category.to_string())
            .collect::<Vec<_>>(),
        "windows": cx.windows().len(),
        "focused_window": cx.active_window().map(|handle| handle.window_id().as_u64()),
    })
}

/// 全部命令的元数据。
fn commands_json() -> Vec<Value> {
    CommandRegistry::global()
        .iter()
        .map(descriptor_json)
        .collect()
}

/// 单条命令元数据的 JSON 形态。
fn descriptor_json(descriptor: &CommandDescriptor) -> Value {
    json!({
        "id": descriptor.id().to_string(),
        "label": descriptor.label().to_string(),
        "category": descriptor.category().map(|category| category.to_string()),
        "description": descriptor.description().map(|text| text.to_string()),
        "keystrokes": descriptor
            .default_keystrokes()
            .iter()
            .map(|keystroke| keystroke.to_string())
            .collect::<Vec<_>>(),
        "keytip": descriptor.keytip().map(|keytip| keytip.to_string()),
    })
}

/// 窗口列表：id + 是否平台焦点窗口。
fn windows_json(cx: &App) -> Value {
    let focused = cx.active_window().map(|handle| handle.window_id());
    let windows = cx
        .windows()
        .iter()
        .map(|handle| {
            let id = handle.window_id();
            json!({ "id": id.as_u64(), "focused": focused == Some(id) })
        })
        .collect::<Vec<_>>();
    json!({ "windows": windows })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommandDescriptor, TestAppContext, register_command};

    /// 演示命令：供 `commands.list` / `tools.list` 断言。
    fn channel_demo_command() -> CommandDescriptor {
        CommandDescriptor::new("rgpui_channel_test::Open", "通道打开")
            .with_category("控制通道")
            .with_description("打开一个文件")
            .with_keystrokes(["ctrl-o"])
    }

    register_command!(channel_demo_command);

    /// 构造一条带 id 的请求。
    fn request(method: &str, params: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params })
    }

    #[test]
    fn key_must_match() {
        let request = json!({ "jsonrpc": "2.0", "id": 1, "key": "abc", "method": "ping" });
        assert!(authorized(&request, "abc"));
        assert!(!authorized(&request, "xyz"));
        assert!(!authorized(&json!({ "id": 1, "method": "ping" }), "abc"));
    }

    #[rgpui::test]
    fn ping_answers_pong(cx: &mut TestAppContext) {
        let response = cx.update(|cx| handle_request(cx, &request("ping", Value::Null)));
        let response = response.expect("ping 应有回复");
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 7);
        assert_eq!(response["result"]["pong"], true);
    }

    #[rgpui::test]
    fn notifications_have_no_response(cx: &mut TestAppContext) {
        let notification = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        let response = cx.update(|cx| handle_request(cx, &notification));
        assert!(response.is_none());
    }

    #[rgpui::test]
    fn unknown_method_reports_error(cx: &mut TestAppContext) {
        let response = cx.update(|cx| handle_request(cx, &request("nope/nope", Value::Null)));
        let response = response.expect("错误也要回复");
        assert_eq!(response["error"]["code"], -32601);
    }

    #[rgpui::test]
    fn commands_list_exposes_registry(cx: &mut TestAppContext) {
        let response = cx.update(|cx| handle_request(cx, &request("commands.list", Value::Null)));
        let commands = response.expect("应有回复")["result"]["commands"]
            .as_array()
            .expect("commands 是数组")
            .clone();
        let open = commands
            .iter()
            .find(|command| command["id"] == "rgpui_channel_test::Open")
            .expect("演示命令在列表里");
        assert_eq!(open["label"], "通道打开");
        assert_eq!(open["category"], "控制通道");
        assert_eq!(open["keystrokes"][0], "ctrl-o");
    }

    #[rgpui::test]
    fn commands_get_unknown_id_is_invalid_params(cx: &mut TestAppContext) {
        let response = cx.update(|cx| {
            handle_request(
                cx,
                &request("commands.get", json!({ "id": "missing::Cmd" })),
            )
        });
        assert_eq!(response.expect("应有回复")["error"]["code"], -32602);
    }

    #[rgpui::test]
    fn dispatch_unknown_command_fails(cx: &mut TestAppContext) {
        let response = cx.update(|cx| {
            handle_request(
                cx,
                &request("command.dispatch", json!({ "id": "missing::Cmd" })),
            )
        });
        let response = response.expect("应有回复");
        assert_eq!(response["error"]["code"], -32000);
        assert!(
            response["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("missing::Cmd"))
        );
    }

    #[rgpui::test]
    fn app_info_reports_version_and_counts(cx: &mut TestAppContext) {
        let response = cx.update(|cx| handle_request(cx, &request("app.info", Value::Null)));
        let info = &response.expect("应有回复")["result"];
        assert_eq!(info["name"], "rgpui");
        assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
        assert!(info["commands"].as_u64().is_some_and(|count| count >= 1));
        assert_eq!(info["windows"], 0);
    }

    #[rgpui::test]
    fn window_list_is_empty_in_test_app(cx: &mut TestAppContext) {
        let response = cx.update(|cx| handle_request(cx, &request("window.list", Value::Null)));
        assert_eq!(response.expect("应有回复")["result"]["windows"], json!([]));
    }
}
