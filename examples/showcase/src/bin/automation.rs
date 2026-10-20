//! 控制通道 / MCP 演示：外部进程经本机 TCP 调命令，AI 客户端经 stdio 调 MCP 工具。
//!
//! 启动后控制台打印监听地址与密钥（同时显示在窗口里）：
//!
//! ```text
//! RGPUI_CONTROL_CHANNEL=127.0.0.1:52341
//! RGPUI_CONTROL_KEY=0123456789abcdef0123456789abcdef
//! ```
//!
//! PowerShell 里手动试一条（把地址/密钥换成上面打印的值）：
//!
//! ```powershell
//! $client = [System.Net.Sockets.TcpClient]::new('127.0.0.1', 52341)
//! $stream = $client.GetStream()
//! $writer = [System.IO.StreamWriter]::new($stream)
//! $writer.WriteLine('{"jsonrpc":"2.0","id":1,"key":"<密钥>","method":"commands.list"}')
//! $writer.Flush()
//! [System.IO.StreamReader]::new($stream).ReadLine()
//! ```
//!
//! 带 `--mcp` 启动时额外在 stdin/stdout 上跑 MCP 服务端（握手信息改打到 stderr，
//! 保证 stdout 只有 JSON-RPC）：
//!
//! ```text
//! cargo run -p showcase --bin automation -- --mcp
//! -> {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}
//! <- {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18",...}}
//! -> {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"automation_demo__Increment","arguments":{}}}
//! ```
//!
//! 窗口里的计数器会随 `command.dispatch`（TCP）或 `tools/call`（MCP）实时变化。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::control_channel::{ControlChannel, ControlChannelOptions};
use rgpui::mcp::McpServer;
use rgpui::{
    App, Bounds, CommandDescriptor, Context, FocusHandle, Render, Window, WindowBounds,
    WindowOptions, actions, div, h_flex, prelude::*, px, register_command, size, v_flex,
};
use rgpui_platform::application;

actions!(automation_demo, [Increment, Decrement, Reset]);

/// 「增加」命令：外部脚本 / MCP 工具调它。
fn cmd_increment() -> CommandDescriptor {
    CommandDescriptor::new("automation_demo::Increment", "增加")
        .with_category("演示")
        .with_description("把计数器加一")
        .with_keystroke("ctrl-i")
}

/// 「减少」命令。
fn cmd_decrement() -> CommandDescriptor {
    CommandDescriptor::new("automation_demo::Decrement", "减少")
        .with_category("演示")
        .with_description("把计数器减一")
}

/// 「归零」命令。
fn cmd_reset() -> CommandDescriptor {
    CommandDescriptor::new("automation_demo::Reset", "归零")
        .with_category("演示")
        .with_description("把计数器清零")
}

register_command!(cmd_increment);
register_command!(cmd_decrement);
register_command!(cmd_reset);

/// 演示视图：持有控制通道与（可选的）MCP 服务端句柄。
struct AutomationDemo {
    count: i32,
    focus_handle: FocusHandle,
    /// 控制通道句柄：随视图存活，视图释放即关闭监听。
    _channel: ControlChannel,
    /// MCP 服务端句柄（仅在 `--mcp` 时存在）。
    _mcp: Option<McpServer>,
    addr: String,
    key: String,
    mcp_enabled: bool,
}

impl AutomationDemo {
    fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        channel: ControlChannel,
        mcp: Option<McpServer>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        let addr = channel.addr().to_string();
        let key = channel.key().to_string();
        let mcp_enabled = mcp.is_some();
        Self {
            count: 0,
            focus_handle,
            _channel: channel,
            _mcp: mcp,
            addr,
            key,
            mcp_enabled,
        }
    }

    fn on_increment(&mut self, _: &Increment, _window: &mut Window, cx: &mut Context<Self>) {
        self.count += 1;
        cx.notify();
    }

    fn on_decrement(&mut self, _: &Decrement, _window: &mut Window, cx: &mut Context<Self>) {
        self.count -= 1;
        cx.notify();
    }

    fn on_reset(&mut self, _: &Reset, _window: &mut Window, cx: &mut Context<Self>) {
        self.count = 0;
        cx.notify();
    }
}

impl Render for AutomationDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let request = format!(
            r#"{{"jsonrpc":"2.0","id":1,"key":"{}","method":"command.dispatch","params":{{"id":"automation_demo::Increment"}}}}"#,
            self.key
        );

        v_flex()
            .id("automation-demo")
            .track_focus(&self.focus_handle)
            .on_action(_cx.listener(Self::on_increment))
            .on_action(_cx.listener(Self::on_decrement))
            .on_action(_cx.listener(Self::on_reset))
            .size_full()
            .gap(px(10.0))
            .p(px(16.0))
            .bg(rgpui::rgb(0xf8fafc))
            .child(
                div()
                    .text_lg()
                    .font_weight(rgpui::FontWeight::BOLD)
                    .child("控制通道 / MCP 演示"),
            )
            .child(div().text_sm().child(format!("监听地址：{}", self.addr)))
            .child(
                div()
                    .text_xs()
                    .text_color(rgpui::rgb(0x64748b))
                    .child(format!("访问密钥：{}", self.key)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgpui::rgb(0x64748b))
                    .child(if self.mcp_enabled {
                        "MCP：已启用（stdin/stdout，握手信息在 stderr）"
                    } else {
                        "MCP：未启用（加 --mcp 开启）"
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgpui::rgb(0x334155))
                    .child(format!("计数器：{}", self.count)),
            )
            .child(
                div()
                    .rounded_md()
                    .bg(rgpui::rgb(0x0f172a))
                    .p(px(8.0))
                    .text_xs()
                    .text_color(rgpui::rgb(0xe2e8f0))
                    .child(request),
            )
            .child(
                h_flex()
                    .gap(px(8.0))
                    .items_center()
                    .child(
                        div()
                            .id("automation-increment")
                            .px(px(10.0))
                            .py(px(4.0))
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .child("增加（automation_demo::Increment）")
                            .on_click(|_, _window, cx| {
                                let _ = cx.dispatch_command("automation_demo::Increment", None);
                            }),
                    )
                    .child(
                        div()
                            .id("automation-reset")
                            .px(px(10.0))
                            .py(px(4.0))
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .child("归零")
                            .on_click(|_, _window, cx| {
                                let _ = cx.dispatch_command("automation_demo::Reset", None);
                            }),
                    ),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);

        let channel = cx
            .start_control_channel(ControlChannelOptions::default())
            .expect("控制通道启动失败");
        let addr = channel.addr();
        let key = channel.key().to_string();
        let mcp_enabled = std::env::args().any(|arg| arg == "--mcp");
        let mcp = if mcp_enabled {
            Some(McpServer::start_stdio(cx).expect("MCP 服务端启动失败"))
        } else {
            None
        };

        // `--mcp` 时 stdout 只能出现 JSON-RPC，握手信息改走 stderr。
        if mcp_enabled {
            eprintln!("RGPUI_CONTROL_CHANNEL={addr}");
            eprintln!("RGPUI_CONTROL_KEY={key}");
        } else {
            println!("RGPUI_CONTROL_CHANNEL={addr}");
            println!("RGPUI_CONTROL_KEY={key}");
        }

        let bounds = Bounds::centered(None, size(px(640.0), px(420.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| AutomationDemo::new(window, cx, channel, mcp)),
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}

#[cfg(test)]
mod tests {
    use rgpui::CommandRegistry;

    /// gate：本示例登记的命令清单自洽（id 唯一 / 标签与分类非空 / 快捷键可解析）。
    ///
    /// 应用照抄这一条即可把「命令清单」纳入 CI：漏填分类、快捷键拼错、
    /// 重复登记都会在测试阶段失败，而不是等用户点到 UI 才发现。
    #[test]
    fn registered_commands_are_valid() {
        let problems = CommandRegistry::global().validate();
        assert!(problems.is_empty(), "命令清单存在问题：{problems:?}");
    }
}
