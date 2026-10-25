//! 外联终端后端：由调用方提供双向字节流的终端接入（SSH、Telnet、串口、测试注入等）。
//!
//! 本模块不依赖任何具体传输协议，也不产生本地进程；终端模拟（Alacritty `Term` +
//! vte 解析）与本地 PTY 版本完全一致，差别仅在字节来源与去向：
//!
//! - **下行（远端 → 终端）**：传输层通过 [`FeedHandle::feed`] 注入远端返回的字节，
//!   内部线程驱动与 alacritty `event_loop` 相同的 vte 解析路径（含同步输出/
//!   synchronized update 语义），最终渲染到屏幕。
//! - **上行（终端 → 远端）**：用户键入、OSC 应答（OSC 52 / 颜色查询 /
//!   TextAreaSizeRequest 等）和尺寸变化通过 `alacritty::event::Notify` /
//!   `OnResize` 接口写入 [`PtyBoundMessage`] 通道，由传输层转发到远端。
//!
//! 会话结束时调用 [`FeedHandle::end_session`]（或直接丢弃 `feed_tx`），
//! 内部线程会以 `Term::exit()` 收尾并发出 `Event::CloseTerminal`，UI 表现
//! 与本地进程退出完全一致。
//!
//! # 与本地 PTY 后端的差异
//!
//! - 没有 OS 级轮询（polling）：字节经通道传入，线程阻塞在 `recv` 上即可，
//!   无需 `EventedReadWrite` 的 unsafe 注册。
//! - 截至本版本，feed 通道是无界的；对极端持续输出（如 `cat 大文件`）
//!   的背压由传输层自行控制（SSH 协议本身自带流控窗口）。

use std::{
    borrow::Cow,
    sync::{Arc, mpsc},
    time::Instant,
};

use alacritty_terminal::{
    event::{Event as AlacTermEvent, EventListener, Notify, OnResize, WindowSize},
    sync::FairMutex,
    term::Term,
    vte::ansi,
};

use crate::terminal::ZedListener;

/// 与 alacritty `event_loop` 一致的单轮持锁解析字节上限。
const MAX_LOCKED_READ: usize = u16::MAX as usize;

/// 终端 → 传输层 方向的消息（等价于写本地 PTY 的字节与尺寸控制）。
///
/// 注：`WindowSize` 未实现 `PartialEq`，故整体仅 derive `Clone`/`Debug`。
#[derive(Clone, Debug)]
pub enum PtyBoundMessage {
    /// 需要写入远端的字节（用户键入、鼠标报告、OSC 应答、焦点上报等）。
    Input(Vec<u8>),
    /// 终端网格尺寸（行列数 + 单元格像素尺寸），远端需要据此 resize 其 PTY。
    Resize(WindowSize),
}

/// 内部循环消息。
enum LoopMsg {
    /// 来自远端的原始字节。
    Feed(Vec<u8>),
    /// 远端会话已结束。
    Exited,
}

/// 传输层 → 终端 的注入句柄（下行字节流入口）。
///
/// 由 [`crate::TerminalBuilder::new_external`] 创建，rulink 的 SSH 引擎持有。
#[derive(Clone)]
pub struct FeedHandle {
    tx: mpsc::Sender<LoopMsg>,
}

impl FeedHandle {
    /// 注入远端返回的原始字节（SSH channel 的 stdout/stderr 混合流）。
    ///
    /// 返回 `false` 表示终端会话已结束、注入被丢弃。
    pub fn feed(&self, bytes: impl Into<Vec<u8>>) -> bool {
        self.tx.send(LoopMsg::Feed(bytes.into())).is_ok()
    }

    /// 结束终端会话（等价远端进程退出）。
    /// 返回 `false` 表示会话本就不再活跃。
    pub fn end_session(&self) -> bool {
        self.tx.send(LoopMsg::Exited).is_ok()
    }
}

/// 终端 → 传输层 的接收端（上行消息出口）。
///
/// 传输层以独立线程/任务循环 `recv()` 驱动；`recv` 阻塞语义，
/// 异步传输层可用 `try_recv` 轮询或接桥接线程。
pub struct PtyBoundReceiver {
    rx: mpsc::Receiver<PtyBoundMessage>,
}

impl PtyBoundReceiver {
    /// 阻塞接收一条上行消息；返回 `None` 表示终端已释放。
    pub fn recv(&self) -> Option<PtyBoundMessage> {
        self.rx.recv().ok()
    }

    /// 非阻塞接收一条上行消息。
    pub fn try_recv(&self) -> Option<PtyBoundMessage> {
        self.rx.try_recv().ok()
    }
}

/// 外联传输层句柄集合（由 [`crate::TerminalBuilder::take_external_handles`] 取走）。
pub struct ExternalHandles {
    /// 下行注入句柄：传输层持有，远端字节由此进入终端。
    pub feed: FeedHandle,
    /// 上行接收端：传输层驱动此通道，转发用户输入与尺寸变化到远端。
    pub rx: PtyBoundReceiver,
}

/// 外联后端的写入端，实现 `Notify`（上行字节）与 `OnResize`（尺寸）。
///
/// 与本地 PTY 的 `Notifier` 对等，供 `Terminal` 统一分发。
#[derive(Clone)]
pub(crate) struct ExternalPtyWriter {
    tx: mpsc::Sender<PtyBoundMessage>,
}

impl ExternalPtyWriter {
    /// 以上行通道构造写入端。
    pub(crate) fn new(tx: mpsc::Sender<PtyBoundMessage>) -> Self {
        ExternalPtyWriter { tx }
    }
}

impl Notify for ExternalPtyWriter {
    fn notify<B: Into<Cow<'static, [u8]>>>(&self, bytes: B) {
        let bytes = bytes.into();
        // 与 alacritty `Notifier` 相同语义：0 字节不发，避免挂起对端。
        if bytes.is_empty() {
            return;
        }
        let _ = self.tx.send(PtyBoundMessage::Input(bytes.into_owned()));
    }
}

impl OnResize for ExternalPtyWriter {
    fn on_resize(&mut self, window_size: WindowSize) {
        let _ = self.tx.send(PtyBoundMessage::Resize(window_size));
    }
}

/// 外联循环的解析状态（镜像 alacritty `event_loop::State` 的解析部分）。
#[derive(Default)]
struct ExternalLoopState {
    parser: ansi::Processor,
}

/// 外联终端主循环：吸收远端字节，驱动解析并通知 RGPUI 重绘。
///
/// 与 alacritty `event_loop::spawn` 行为对齐（同步超时、缓冲上限强制锁定、
/// Wakeup 触发条件一致），但以通道代替 OS 轮询，无需 `EventedReadWrite`。
fn external_loop(
    terminal: Arc<FairMutex<Term<ZedListener>>>,
    proxy: ZedListener,
    feed_rx: mpsc::Receiver<LoopMsg>,
) {
    let mut state = ExternalLoopState::default();

    loop {
        // 同步超时窗口内等待新字节；超时则结束同步输出（镜像 event_loop L229-249）。
        let handler = state.parser.sync_timeout();
        let timeout = handler
            .sync_timeout()
            .map(|st| st.saturating_duration_since(Instant::now()));

        let first = match timeout {
            Some(timeout) => match feed_rx.recv_timeout(timeout) {
                Ok(message) => Some(message),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    state.parser.stop_sync(&mut *terminal.lock());
                    proxy.send_event(AlacTermEvent::Wakeup);
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // 传输层丢弃了注入端，视同会话结束。
                    break;
                }
            },
            None => match feed_rx.recv() {
                Ok(message) => Some(message),
                Err(_) => break,
            },
        };

        let mut exited = match first {
            Some(LoopMsg::Exited) | None => true,
            Some(LoopMsg::Feed(_)) => false,
        };

        // 收集当前已就绪的字节块（至多 MAX_LOCKED_READ），单轮持锁解析完；
        // 超出部分留在通道里下一轮继续（与 event_loop 的持锁上限一致）。
        let mut pending: Vec<Vec<u8>> = Vec::new();
        let mut budget = 0usize;
        if !exited {
            if let Some(LoopMsg::Feed(bytes)) = first {
                budget += bytes.len();
                pending.push(bytes);
            }
            while budget < MAX_LOCKED_READ {
                match feed_rx.try_recv() {
                    Ok(LoopMsg::Feed(bytes)) => {
                        budget += bytes.len();
                        pending.push(bytes);
                    }
                    Ok(LoopMsg::Exited) => {
                        exited = true;
                        break;
                    }
                    Err(_) => break,
                }
            }
        }

        if !pending.is_empty() {
            // 先租约再解锁：防止 UI 线程在本轮解析前反复抢占（镜像 event_loop）。
            let _lease = terminal.lease();
            let mut term = terminal
                .try_lock_unfair()
                .unwrap_or_else(|| terminal.lock());

            let mut processed = 0usize;
            for chunk in &pending {
                state.parser.advance(&mut *term, chunk);
                processed += chunk.len();
            }

            // 全部字节都被收入同步缓冲时没有可见变化，无需重绘（镜像 event_loop L166）。
            if state.parser.sync_bytes_count() < processed && processed > 0 {
                proxy.send_event(AlacTermEvent::Wakeup);
            }
        }

        if exited {
            break;
        }
    }

    // 远端会话结束：退出终端并通知 UI（与 alacritty child-exit 路径一致）。
    terminal.lock().exit();
    proxy.send_event(AlacTermEvent::Wakeup);
}

/// 为给定终端拉起外联解析循环，返回（注入句柄, 写入端, 上行接收端）。
///
/// `terminal` 与 `proxy` 由 `terminal.rs` 构造（与本地 PTY 版本完全一致）。
pub(crate) fn spawn_feed_loop(
    terminal: Arc<FairMutex<Term<ZedListener>>>,
    proxy: ZedListener,
) -> (FeedHandle, ExternalPtyWriter, PtyBoundReceiver) {
    let (feed_tx, feed_rx) = mpsc::channel::<LoopMsg>();
    let (bound_tx, bound_rx) = mpsc::channel::<PtyBoundMessage>();

    // 外联循环无需与 UI 同步启动；线程创建失败极罕见，记日志并继续（后续
    // feed 会因通道无接收端而失败，最终以 CloseTerminal 收场）。
    if std::thread::Builder::new()
        .name("rgpui-term-external".to_string())
        .spawn(move || external_loop(terminal, proxy, feed_rx))
        .is_err()
    {
        log::error!("failed to spawn rgpui-term external feed loop thread");
    }

    (
        FeedHandle { tx: feed_tx },
        ExternalPtyWriter::new(bound_tx),
        PtyBoundReceiver { rx: bound_rx },
    )
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, thread::sleep, time::Duration};

    use alacritty_terminal::{
        event::Event as AlacTermEvent,
        index::{Column, Line},
        sync::FairMutex,
        term::{Config, Term},
    };
    use futures::channel::mpsc::unbounded;

    use super::*;
    use crate::terminal::{TerminalBounds, ZedListener};

    /// 构造默认网格的 Term + 事件桥（与真实 UI 侧结构一致）。
    fn make_term() -> (
        Arc<FairMutex<Term<ZedListener>>>,
        ZedListener,
        futures::channel::mpsc::UnboundedReceiver<AlacTermEvent>,
    ) {
        let (tx, rx) = unbounded();
        let proxy = ZedListener(tx);
        let config = Config {
            scrolling_history: 1000,
            ..Config::default()
        };
        let term = Term::new(config, &TerminalBounds::default(), proxy.clone());
        (Arc::new(FairMutex::new(term)), proxy, rx)
    }

    /// 轮询网格第零行从 `start` 列起是否构成目标文本。
    fn wait_row_text(terminal: &Arc<FairMutex<Term<ZedListener>>>, start: usize, expected: &str) {
        for _ in 0..500 {
            sleep(Duration::from_millis(10));
            let term = terminal.lock();
            let row = &term.grid()[Line(0)];
            let text: String = (start..start + expected.len())
                .map(|i| row[Column(i)].c)
                .collect();
            if text == expected {
                return;
            }
        }
        panic!("grid text {:?} never appeared", expected);
    }

    /// 拉取事件直至匹配；返回匹配后已消耗的事件计数。
    fn wait_event(
        events: &mut futures::channel::mpsc::UnboundedReceiver<AlacTermEvent>,
        want: &dyn Fn(&AlacTermEvent) -> bool,
    ) {
        for _ in 0..500 {
            match events.try_recv() {
                Ok(event) => {
                    if want(&event) {
                        return;
                    }
                }
                Err(_) => sleep(Duration::from_millis(10)),
            }
        }
        panic!("expected terminal event never arrived");
    }

    #[test]
    fn external_feed_parses_ansi() {
        let (terminal, proxy, _events) = make_term();
        let (feed, _writer, _rx) = spawn_feed_loop(terminal.clone(), proxy);

        feed.feed("hello ".as_bytes());
        feed.feed("\x1b[31mworld".as_bytes());

        wait_row_text(&terminal, 0, "hello");
        // ANSI SGR 红色字节被消费，"world"（第 6 列起）纯文本上屏
        wait_row_text(&terminal, 6, "world");
    }

    #[test]
    fn notify_reaches_bound_receiver() {
        let (terminal, proxy, _events) = make_term();
        let (_feed, writer, rx) = spawn_feed_loop(terminal, proxy);

        writer.notify(b"ls\r\n");

        match rx.recv() {
            Some(PtyBoundMessage::Input(bytes)) => assert_eq!(bytes, b"ls\r\n".to_vec()),
            other => panic!("expected Input message, got {:?}", other),
        }
    }

    #[test]
    fn on_resize_reaches_bound_receiver() {
        let (terminal, proxy, _events) = make_term();
        let (_feed, mut writer, rx) = spawn_feed_loop(terminal, proxy);

        writer.on_resize(WindowSize {
            num_lines: 24,
            num_cols: 80,
            cell_width: 9,
            cell_height: 18,
        });

        match rx.recv() {
            Some(PtyBoundMessage::Resize(size)) => {
                assert_eq!(size.num_lines, 24);
                assert_eq!(size.num_cols, 80);
            }
            other => panic!("expected Resize message, got {:?}", other),
        }
    }

    #[test]
    fn end_session_sends_exit() {
        let (terminal, proxy, mut events) = make_term();
        let (feed, _writer, _rx) = spawn_feed_loop(terminal, proxy);

        feed.end_session();

        wait_event(&mut events, &|event| matches!(event, AlacTermEvent::Exit));
    }

    #[test]
    fn dropping_feed_handle_also_exits() {
        let (terminal, proxy, mut events) = make_term();
        let (feed, _writer, _rx) = spawn_feed_loop(terminal, proxy);

        drop(feed);

        wait_event(&mut events, &|event| matches!(event, AlacTermEvent::Exit));
    }

    #[test]
    fn writer_keeps_live_after_terminal_use() {
        let (terminal, proxy, mut events) = make_term();
        let (feed, writer, rx) = spawn_feed_loop(terminal, proxy);

        // 注入 + 上行各一次后双方都应继续工作
        feed.feed("a".as_bytes());
        writer.notify(b"b");
        assert!(matches!(rx.recv(), Some(PtyBoundMessage::Input(ref bytes)) if bytes == b"b"));

        wait_event(&mut events, &|event| matches!(event, AlacTermEvent::Wakeup));
    }
}
