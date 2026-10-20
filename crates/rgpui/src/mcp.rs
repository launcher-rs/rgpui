//! MCP 服务端 —— 把命令注册表暴露成 MCP 工具（tools），供 AI 客户端调用。
//!
//! ## 传输
//!
//! 标准 MCP 的 stdio 传输：客户端把我们当子进程启动，逐行读写 JSON-RPC 2.0。
//! 本模块只实现服务端最小子集：`initialize` / `tools/list` / `tools/call`。
//! 请求经 [`crate::control_channel::RpcDispatcher`] 投递到主线程执行，与 TCP
//! 控制通道共用同一张方法表（见 [`crate::control_channel`]）。
//!
//! ## 工具映射
//!
//! 一个命令 = 一个工具：
//!
//! - 工具名：命令 id 中 `[A-Za-z0-9_-]` 之外的字符替换为 `_`，并截断到 64 字符
//!   （MCP 工具名限制），如 `editor::Save` → `editor__Save`；
//! - 工具描述：命令的 `description`，缺省回退为 `label`；
//! - `inputSchema`：动作的 JSON Schema（[`App::action_schemas`]），无参数动作为
//!   `{"type":"object"}`。
//!
//! ## 用法
//!
//! ```text
//! // 应用启动后（需在事件循环之前/之内均可）
//! let _mcp = McpServer::start_stdio(cx)?;
//! ```
//!
//! 客户端配置示例（Claude Desktop 等）：
//!
//! ```text
//! { "mcpServers": { "rgpui": { "command": "path/to/app" } } }
//! ```
//!
//! > feature 门控：随 `control-channel` 一起开启。

use std::io::Write;

use serde_json::{Value, json};

use crate::App;
use crate::commands::CommandRegistry;
use crate::control_channel::{RpcDispatcher, RpcError, RpcHandle, error_response};

/// 本服务端支持的 MCP 协议版本（最后一个为最新）。
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18"];

/// MCP 工具名长度上限（协议规定 1..=64）。
const MAX_TOOL_NAME_LEN: usize = 64;

/// 处理 MCP 方法；非 MCP 方法返回 `None`，交回控制通道分发。
pub(crate) fn handle(
    cx: &mut App,
    method: &str,
    params: &Value,
) -> Option<Result<Value, RpcError>> {
    match method {
        "initialize" => Some(Ok(initialize(params))),
        "tools/list" => Some(Ok(tools_list(cx))),
        "tools/call" => Some(tools_call(cx, params)),
        // 客户端握手后的通知：没有 id，正常不会走到这里；带了 id 也按空结果回。
        other if other.starts_with("notifications/") => Some(Ok(json!({}))),
        _ => None,
    }
}

/// `initialize`：协商协议版本并声明 tools 能力。
fn initialize(params: &Value) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let protocol_version = if SUPPORTED_PROTOCOL_VERSIONS.contains(&requested) {
        requested
    } else {
        SUPPORTED_PROTOCOL_VERSIONS
            .last()
            .copied()
            .unwrap_or("2025-06-18")
    };

    json!({
        "protocolVersion": protocol_version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": "rgpui",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": "本服务暴露 rgpui 应用已登记的命令；tools/call 的 name 为命令 id 的转义形式，arguments 为动作参数。",
    })
}

/// `tools/list`：每个命令一个工具，`inputSchema` 取自动作 schema。
fn tools_list(cx: &App) -> Value {
    let registry = CommandRegistry::global();
    // 动作 schema 一次生成、按命令 id 索引（未注册动作的命令没有 schema）。
    let mut generator = schemars::SchemaGenerator::default();
    let schemas: std::collections::HashMap<&'static str, Value> = cx
        .action_schemas(&mut generator)
        .into_iter()
        .filter_map(|(name, schema)| {
            schema.and_then(|schema| serde_json::to_value(schema).ok().map(|json| (name, json)))
        })
        .collect();
    // 工具名经过转义，可能撞名；统一按 `_2`、`_3`… 消歧，保证与 `tools/call` 一致。
    let names: std::collections::HashMap<&str, String> =
        unique_tool_names(registry.iter().map(|descriptor| descriptor.id().as_ref()))
            .into_iter()
            .map(|(name, id)| (id, name))
            .collect();

    let tools = registry
        .iter()
        .map(|descriptor| {
            let id = descriptor.id().as_ref();
            let name = names.get(id).cloned().unwrap_or_else(|| tool_name(id));
            let description = descriptor
                .description()
                .map(|text| text.to_string())
                .unwrap_or_else(|| descriptor.label().to_string());
            let input_schema = schemas
                .get(id)
                .cloned()
                .unwrap_or_else(|| json!({ "type": "object" }));
            json!({
                "name": name,
                "description": description,
                "inputSchema": input_schema,
            })
        })
        .collect::<Vec<_>>();

    json!({ "tools": tools })
}

/// `tools/call`：按工具名找回命令 id 并派发；执行失败以 `isError` 回报。
fn tools_call(cx: &mut App, params: &Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::invalid_params("缺少字符串参数 `name`"))?;
    let id = command_id_for_tool(name)
        .ok_or_else(|| RpcError::invalid_params(format!("未知工具：{name}")))?;

    let arguments = match params.get("arguments") {
        None | Some(Value::Null) => None,
        Some(Value::Object(map)) if map.is_empty() => None,
        Some(value) => Some(value.clone()),
    };

    Ok(match cx.dispatch_command(&id, arguments) {
        Ok(()) => json!({
            "content": [{ "type": "text", "text": format!("已执行 {id}") }],
            "isError": false,
        }),
        Err(error) => json!({
            "content": [{ "type": "text", "text": format!("执行 {id} 失败：{error:#}") }],
            "isError": true,
        }),
    })
}

/// 命令 id → MCP 工具名（非法字符替换为 `_`，截断到 64 字符）。
pub fn tool_name(id: &str) -> String {
    let mut name: String = id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect();
    // 只含 ASCII，截断不会切到字符边界。
    name.truncate(MAX_TOOL_NAME_LEN);
    name
}

/// MCP 工具名 → 命令 id（按注册表顺序、与 `tools/list` 相同的消歧规则找匹配项）。
fn command_id_for_tool(name: &str) -> Option<String> {
    let registry = CommandRegistry::global();
    unique_tool_names(registry.iter().map(|descriptor| descriptor.id().as_ref()))
        .into_iter()
        .find(|(tool_name, _)| tool_name == name)
        .map(|(_, id)| id.to_string())
}

/// 命令 id 列表 → `(工具名, 命令 id)`；转义后撞名时追加 `_2`、`_3`…
///
/// MCP 要求工具名在服务端唯一，而 `a::b` 与 `a__b` 都会转义成 `a__b`，
/// 所以这里统一消歧，`tools/list` 与 `tools/call` 共用同一套结果。
fn unique_tool_names<'a>(ids: impl IntoIterator<Item = &'a str>) -> Vec<(String, &'a str)> {
    let mut used = std::collections::HashSet::new();
    let mut entries = Vec::new();
    for id in ids {
        let base = tool_name(id);
        let mut name = base.clone();
        let mut index = 1;
        while !used.insert(name.clone()) {
            index += 1;
            name = suffixed_tool_name(&base, index);
        }
        entries.push((name, id));
    }
    entries
}

/// 在工具名后追加 `_2`、`_3`…，并保证总长不超过 64 字符。
fn suffixed_tool_name(base: &str, index: usize) -> String {
    let suffix = format!("_{index}");
    let keep = MAX_TOOL_NAME_LEN.saturating_sub(suffix.len());
    let mut name: String = base.chars().take(keep).collect();
    name.push_str(&suffix);
    name
}

/// 运行中的 MCP stdio 服务端；丢弃句柄即停止泵送（读线程随 stdin 关闭退出）。
pub struct McpServer {
    /// 主线程调度器：持有前台任务。
    _dispatcher: RpcDispatcher,
    /// stdin 读线程。
    _reader: std::thread::JoinHandle<()>,
}

impl McpServer {
    /// 在 stdin/stdout 上启动 MCP 服务端。
    ///
    /// 读线程逐行解析请求并阻塞等待主线程处理，再把回复写回 stdout；
    /// 因此必须在 rgpui 事件循环运行期间使用（否则请求会一直排队）。
    pub fn start_stdio(cx: &mut App) -> anyhow::Result<Self> {
        let dispatcher = RpcDispatcher::new(cx);
        let handle = dispatcher.handle();
        let reader = std::thread::Builder::new()
            .name("rgpui-mcp".to_string())
            .spawn(move || {
                use std::io::BufRead;
                let stdin = std::io::stdin();
                let mut stdout = std::io::stdout();
                for line in stdin.lock().lines() {
                    let Ok(line) = line else { break };
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    if let Some(response) = respond(&handle, line) {
                        let text = serde_json::to_string(&response).unwrap_or_default();
                        if writeln!(stdout, "{text}").is_err() {
                            break;
                        }
                        let _ = stdout.flush();
                    }
                }
            })?;

        Ok(Self {
            _dispatcher: dispatcher,
            _reader: reader,
        })
    }
}

/// 处理一行 JSON-RPC 文本；解析失败按 `-32700` 回复。
fn respond(handle: &RpcHandle, line: &str) -> Option<Value> {
    match serde_json::from_str::<Value>(line) {
        Ok(request) => handle.request_blocking(request),
        Err(error) => Some(error_response(
            Value::Null,
            -32700,
            format!("JSON 解析失败：{error}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommandDescriptor, TestAppContext, register_command};

    /// 演示命令：无参数动作。
    fn mcp_demo_command() -> CommandDescriptor {
        CommandDescriptor::new("rgpui_mcp_test::Save", "保存")
            .with_category("文档")
            .with_description("保存当前文档")
    }

    register_command!(mcp_demo_command);

    #[test]
    fn initialize_echoes_supported_version() {
        let result = initialize(&json!({ "protocolVersion": "2025-03-26" }));
        assert_eq!(result["protocolVersion"], "2025-03-26");
        assert_eq!(result["capabilities"]["tools"]["listChanged"], false);
        assert_eq!(result["serverInfo"]["name"], "rgpui");
    }

    #[test]
    fn initialize_falls_back_to_latest_version() {
        let result = initialize(&json!({ "protocolVersion": "1999-01-01" }));
        assert_eq!(
            result["protocolVersion"],
            *SUPPORTED_PROTOCOL_VERSIONS.last().expect("至少一个版本")
        );
    }

    #[test]
    fn tool_name_replaces_illegal_characters() {
        assert_eq!(tool_name("editor::Save"), "editor__Save");
        assert_eq!(tool_name("a b/c"), "a_b_c");
        assert_eq!(tool_name("ok-name_1"), "ok-name_1");
        assert_eq!(tool_name(&"x".repeat(100)).len(), MAX_TOOL_NAME_LEN);
    }

    #[test]
    fn colliding_tool_names_get_numeric_suffix() {
        let entries = unique_tool_names(["a::b", "a__b", "a::b"]);
        assert_eq!(
            entries,
            vec![
                ("a__b".to_string(), "a::b"),
                ("a__b_2".to_string(), "a__b"),
                ("a__b_3".to_string(), "a::b"),
            ]
        );
        // 超长名追加后缀后仍不超过 64 字符。
        let long = "x".repeat(MAX_TOOL_NAME_LEN);
        let entries = unique_tool_names([long.as_str(), long.as_str()]);
        assert_eq!(entries[1].0.len(), MAX_TOOL_NAME_LEN);
        assert!(entries[1].0.ends_with("_2"));
    }

    #[rgpui::test]
    fn tools_list_names_are_unique(cx: &mut TestAppContext) {
        let result = cx.update(|cx| tools_list(cx));
        let names = result["tools"]
            .as_array()
            .expect("tools 是数组")
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();
        let unique = names
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), names.len(), "MCP 工具名必须唯一：{names:?}");
    }

    #[rgpui::test]
    fn tools_list_exposes_commands(cx: &mut TestAppContext) {
        let result = cx.update(|cx| tools_list(cx));
        let tools = result["tools"].as_array().expect("tools 是数组");
        let save = tools
            .iter()
            .find(|tool| tool["name"] == "rgpui_mcp_test__Save")
            .expect("演示命令映射成工具");
        assert_eq!(save["description"], "保存当前文档");
        assert_eq!(save["inputSchema"]["type"], "object");
    }

    #[rgpui::test]
    fn tools_call_unknown_tool_is_invalid_params(cx: &mut TestAppContext) {
        let result = cx.update(|cx| tools_call(cx, &json!({ "name": "no_such_tool" })));
        assert!(result.is_err_and(|error| error.code == -32602));
    }

    #[rgpui::test]
    fn tools_call_reports_tool_error(cx: &mut TestAppContext) {
        // 演示命令没有对应动作，派发必然失败 —— 按 MCP 约定用 isError 回报。
        let result = cx
            .update(|cx| tools_call(cx, &json!({ "name": "rgpui_mcp_test__Save" })))
            .expect("未知工具才会走 JSON-RPC 错误");
        assert_eq!(result["isError"], true);
        assert!(
            result["content"][0]["text"]
                .as_str()
                .is_some_and(|text| text.contains("rgpui_mcp_test::Save"))
        );
    }

    #[rgpui::test]
    fn notifications_are_accepted(cx: &mut TestAppContext) {
        let result = cx
            .update(|cx| handle(cx, "notifications/initialized", &Value::Null))
            .expect("通知属于 MCP 方法");
        assert!(result.is_ok());
    }

    #[rgpui::test]
    fn non_mcp_methods_fall_through(cx: &mut TestAppContext) {
        assert!(
            cx.update(|cx| handle(cx, "commands.list", &Value::Null))
                .is_none()
        );
    }
}
