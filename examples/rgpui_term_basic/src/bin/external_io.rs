//! 外接 IO 后端演示：不启动任何本地进程，用 [`TerminalBuilder::new_external`]
//! 让一条「远端」字节流驱动终端内核 —— 与接入 SSH / Telnet / 串口时的接线形状一致。
//!
//! 运行：`cargo run -p rgpui_term_basic --bin external_io`
//!
//! 两个方向都跑通：
//! - **下行**（远端 → 终端）：`spawn_simulated_remote` 线程把登录脚本与回显字节经
//!   [`FeedHandle::feed`] 注入，走与本地 PTY 完全相同的 vte 解析路径。
//! - **上行**（终端 → 远端）：键入的字节、OSC 应答与网格尺寸以 [`PtyBoundMessage`]
//!   出现在 [`PtyBoundReceiver`] 上，本例把它当作远端要处理的输入：字面回显，
//!   `Resize` 则回注一行提示（真实传输层会把它转成 SSH channel 的
//!   `pty-req` / `window-change`）。
//!
//! 输入 `exit` 回车：远端结束会话（[`FeedHandle::end_session`]），终端以与本地
//! 进程退出相同的方式收场。

use std::{
    thread,
    time::{Duration, Instant},
};

use rgpui::{
    App, AppContext as _, Bounds, Context, Entity, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, SharedString, Styled, TitlebarOptions, Window,
    WindowBackgroundAppearance, WindowBounds, WindowOptions, div, px, rgb, size,
};
use rgpui_platform::application;
use rgpui_term::{
    FeedHandle, PtyBoundMessage, PtyBoundReceiver, Terminal, TerminalBuilder, TerminalView,
};

/// 「远端」登录后回放的 ANSI 脚本（含颜色、加粗、OSC 8 超链接）。
const REMOTE_BANNER: &[&str] = &[
    "\x1b[1;36mrgpui-term\x1b[0m 外接 IO 后端 · \x1b[2m无本地 PTY\x1b[0m\r\n",
    "\x1b[33m$ \x1b[0m这条会话的字节由 \x1b[1mFeedHandle::feed\x1b[0m 注入，解析路径与 ssh 一致\r\n",
    "\x1b[33m$ \x1b[0m试试：输入任意字符看回显、缩放窗口看远端 resize、Ctrl+F 搜内容\r\n",
    "\x1b[33m$ \x1b[0m超链接：\x1b]8;;https://crates.io/crates/rgpui-term\x1b\\\x1b[4;34mrgpui-term on crates.io\x1b[0m\x1b]8;;\x1b\\\r\n",
    "\x1b[33m$ \x1b[0m",
];

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(860.0), px(520.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(SharedString::from("rgpui-term · 外接 IO 后端")),
                ..Default::default()
            }),
            window_background: WindowBackgroundAppearance::Opaque,
            ..Default::default()
        };

        if let Err(error) = cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| ExternalSession::new(window, cx));
            view.read(cx).focus_handle(cx).focus(window, cx);
            window.activate_window();
            view
        }) {
            eprintln!("建窗失败：{error:?}");
        }
    });
}

/// 演示根视图：持有一个外联终端与它的「远端」线程。
struct ExternalSession {
    terminal_view: Entity<TerminalView>,
    /// 状态行文本（说明会话走的是外接通道）
    status: SharedString,
}

impl ExternalSession {
    /// 建外联终端并拉起模拟远端。
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut builder = TerminalBuilder::new_external(Some(2000)).expect("外接终端创建失败");
        let handles = builder
            .take_external_handles()
            .expect("new_external 必须给出外联句柄");

        // 订阅后终端事件才会汇入 RGPUI 事件循环（与本地 PTY 同一条管道）
        let terminal: Entity<Terminal> = cx.new(|cx| builder.subscribe(cx));
        let terminal_view = cx.new(|cx| TerminalView::new(terminal, window, cx));

        // 「远端」线程：先回放登录脚本，再持续消费上行消息
        spawn_simulated_remote(handles.feed, handles.rx);

        Self {
            terminal_view,
            status: "外接后端已连接（本地无进程）· 键入与 resize 都会回到「远端」线程".into(),
        }
    }
}

impl Focusable for ExternalSession {
    /// 焦点转发给终端视图：应用一打开键入就直送「远端」。
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.terminal_view.read(cx).focus_handle(cx)
    }
}

impl Render for ExternalSession {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x141414))
            .child(
                div()
                    .px(px(10.0))
                    .py(px(4.0))
                    .text_sm()
                    .text_color(rgb(0x8a8a8a))
                    .child(self.status.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(self.terminal_view.clone()),
            )
    }
}

/// 拉起模拟远端：注入横幅，然后把上行字节回显回去。
///
/// 真实传输层在这里做的是「写 SSH channel」，形状与 `rx.recv()` 完全一致。
fn spawn_simulated_remote(feed: FeedHandle, rx: PtyBoundReceiver) {
    thread::spawn(move || {
        // 登录脚本逐条注入，留一点间隔让「流式到达」可见
        for chunk in REMOTE_BANNER {
            if !feed.feed(*chunk) {
                return;
            }
            thread::sleep(Duration::from_millis(60));
        }

        let mut line: Vec<u8> = Vec::new();
        let mut last_resize = Instant::now();

        while let Some(message) = rx.recv() {
            match message {
                PtyBoundMessage::Input(bytes) => {
                    for byte in bytes {
                        // 回显：把键入的字符当作远端的 echo 输出送回来
                        if !feed.feed(vec![byte]) {
                            return;
                        }
                        match byte {
                            b'\r' | b'\n' => {
                                let typed = std::mem::take(&mut line);
                                let typed = String::from_utf8_lossy(&typed).to_string();
                                if typed.trim().eq_ignore_ascii_case("exit") {
                                    feed.feed("\r\n\x1b[2m会话结束\x1b[0m\r\n");
                                    // 远端主动收尾：与 ssh 断开同一条路径
                                    feed.end_session();
                                    return;
                                }
                                feed.feed(prompt_for(&typed));
                            }
                            0x7f => {
                                line.pop();
                            }
                            _ if byte.is_ascii_graphic() || byte == b' ' => {
                                line.push(byte);
                            }
                            _ => {}
                        }
                    }
                }
                PtyBoundMessage::Resize(size) => {
                    // resize 由 UI 侧高频触发，限频避免刷屏
                    if last_resize.elapsed() < Duration::from_millis(200) {
                        continue;
                    }
                    last_resize = Instant::now();
                    feed.feed(format!(
                        "\r\n\x1b[2m[远端] window-change → {}×{}\x1b[0m\r\n\x1b[33m$ \x1b[0m",
                        size.num_lines, size.num_cols
                    ));
                }
            }
        }
    });
}

/// 极简「远端命令」应答：够演示双向通道，不模拟 shell 语义。
fn prompt_for(typed: &str) -> Vec<u8> {
    let prompt = "\x1b[33m$ \x1b[0m";
    match typed.trim() {
        "" => format!("\r\n{prompt}").into_bytes(),
        "help" => format!("\r\n可用：help / date / echo <文本> / exit\r\n{prompt}").into_bytes(),
        "date" => format!(
            "\r\n{}\r\n{prompt}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_secs())
                .unwrap_or_default()
        )
        .into_bytes(),
        other if other.starts_with("echo ") => {
            format!("\r\n{}\r\n{prompt}", &other["echo ".len()..]).into_bytes()
        }
        other => format!("\r\n\x1b[31m远端不认：\x1b[0m{other}\r\n{prompt}").into_bytes(),
    }
}
