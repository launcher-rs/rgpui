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
    sync::{Arc, atomic::Ordering, mpsc},
    time::{Duration, Instant},
};

use alacritty_terminal::{
    event::{Event as AlacTermEvent, EventListener, Notify, OnResize, WindowSize},
    sync::FairMutex,
    term::Term,
    vte::ansi,
};

use crate::terminal::{SyncStats, ZedListener};

/// 与 alacritty `event_loop` 一致的单轮持锁解析字节上限。
const MAX_LOCKED_READ: usize = u16::MAX as usize;

/// 单轮解析最多暂存的字节量。拖动时解析会在每个 PARSE_SLICE_BYTES 边界暂停，
/// 暂存上限只控制预取量，不决定 UI 等多久。
const UI_BUSY_LOCKED_READ: usize = 256 * 1024;

/// 单次持锁解析的字节片大小。
///
/// UI 帧取锁用的是 `try_lock_unfair`（不等），能不能拿到全看解析线程什么时候
/// 松手。原来一整批 64KB 一次解析完才放锁，解析耗时随内容波动（带颜色/长行的
/// docker 日志实测单批可到 5~20ms），这期间 UI 的每一帧都必然落空 —— 2500+ 次
/// 取锁失败、每秒丢十几帧就是这么来的。
///
/// 切成 8KB 一片、片间释放数据锁，将单次持锁时间降到亚毫秒级。拖动期间每批解析
/// 后还会等 UI 实际成功拿锁，避免解析线程重取锁过快，让 UI 每帧都错过这些短间隙。
/// 片间只做一次 drop + 重新加锁（几十纳秒），吞吐代价可以忽略。
const PARSE_SLICE_BYTES: usize = 8192;

const UI_GRACE_SLICE: Duration = Duration::from_millis(1);
const UI_YIELD_TIMEOUT: Duration = Duration::from_millis(25);
/// 拖动暂停解析的最长时间。正常松手会清 `ui_dragging`；这里兜底防标志卡死。
const UI_DRAG_PAUSE_MAX: Duration = Duration::from_secs(2);

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
/// 由 [`crate::TerminalBuilder::new_external`] 创建，由应用侧的传输层持有。
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
///
/// 与 alacritty 不同的一点：alacritty 的解析与 UI 在同一事件循环线程上，
/// 不存在锁争用；这里解析线程与 UI 线程隔着一把 FairMutex。持续刷屏时
/// 每批 64KB 背靠背解析会把数据锁几乎占满，UI 的 `try_lock_unfair` 长期
/// 抢不到 → 画面停更、排队的滚动/复制事件无人处理（拖动滚动条卡顿、
/// Ctrl+C 走陈旧选区的复制分支发不出 ^C）。因此加了一条让锁规则：
/// 只要上一段窗口里 UI 试过取锁却没取到（`SyncStats` 的 attempts 涨了
/// 而 successes 没涨）→ 在下批取锁前，在锁外站最多 24ms（超过一帧周期，
/// 保证盖住下一次取锁时点）等 UI 补上一帧，取到即走；UI 不缺锁时一毫秒
/// 都不多等。条件刻意不依赖「本批攒满 64KB」：docker logs 刷屏常被拆成
/// 小包，按批大小判断会让让锁在最需要它的场景里失效。
fn external_loop(
    terminal: Arc<FairMutex<Term<ZedListener>>>,
    proxy: ZedListener,
    feed_rx: mpsc::Receiver<LoopMsg>,
    sync_stats: Arc<SyncStats>,
) {
    let mut state = ExternalLoopState::default();
    let mut last_attempts = 0u64;
    let mut last_successes = 0u64;

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

        // UI 正在拖滚动条 / 刚滚过滚轮时，限制本轮预取字节数。拖动期间会在
        // 每个解析片前额外检查状态并暂停，滚轮则依靠较小批次尽快把锁交给 UI。
        let now_ms = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0)
        };
        let ui_busy = sync_stats.ui_dragging.load(Ordering::Relaxed)
            || now_ms().saturating_sub(sync_stats.last_scroll_ms.load(Ordering::Relaxed)) < 200;
        let locked_read_limit = if ui_busy {
            UI_BUSY_LOCKED_READ
        } else {
            MAX_LOCKED_READ
        };

        let mut exited = match first {
            Some(LoopMsg::Exited) | None => true,
            Some(LoopMsg::Feed(_)) => false,
        };

        // 收集当前已就绪的字节块（至多 locked_read_limit），单轮持锁解析完；
        // 超出部分留在通道里下一轮继续（与 event_loop 的持锁上限一致）。
        let mut pending: Vec<Vec<u8>> = Vec::new();
        let mut budget = 0usize;
        if !exited {
            if let Some(LoopMsg::Feed(bytes)) = first {
                budget += bytes.len();
                pending.push(bytes);
            }
            while budget < locked_read_limit {
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
            // 租约占的就是 `next` 锁，因此这里只能退到 `lock_unfair`（只等 data）：
            // parking_lot 不可重入，持租约再走 `lock()` 会二次加 `next` 自锁，
            // 解析线程就此永不消费，远端字节再也进不了网格。
            let mut lease = Some(terminal.lease());
            let mut term = terminal
                .try_lock_unfair()
                .unwrap_or_else(|| terminal.lock_unfair());

            let mut processed = 0usize;
            for chunk in pending.iter() {
                // 按片解析、片间松手：单次持锁从「一整批最多 64KB」降到
                // 「一片 8KB」。UI 侧的自旋重试正是靠这些间隙插进来的。
                for slice in chunk.chunks(PARSE_SLICE_BYTES) {
                    // 刷屏与鼠标事件并行时，等待一帧成功拿锁仍会让快速拖动偶尔
                    // 错过整帧。拖动期间直接暂停解析，并同时归还 data/next 两把锁；
                    // 远端字节留在 feed 通道里，松手后继续解析，不会丢日志。
                    if sync_stats.ui_dragging.load(Ordering::Relaxed) {
                        drop(term);
                        drop(lease.take());
                        // 兜底超时：正常松手会由 end_scrollbar_drag 清标志，但
                        // 万一某个鼠标事件路径漏了（窗口失焦、系统吞掉 mouse_up），
                        // 无限等待会让解析线程永久停摆——远端进程其实已被 ^C 杀掉，
                        // 画面却再也不刷新，表现为「Ctrl+C 终止不了」。
                        let deadline = Instant::now() + UI_DRAG_PAUSE_MAX;
                        while sync_stats.ui_dragging.load(Ordering::Relaxed)
                            && Instant::now() < deadline
                        {
                            std::thread::sleep(UI_GRACE_SLICE);
                        }
                        lease = Some(terminal.lease());
                        term = terminal.lock_unfair();
                    }
                    state.parser.advance(&mut *term, slice);
                    processed += slice.len();
                    // 释放数据锁后立刻重新获取：锁空出来的一瞬，UI 的
                    // try_lock 才可能成功。`_lease` 仍持着 next 队列锁，
                    // 所以这里只能用 lock_unfair（见上方租约注释）。
                    drop(term);
                    term = terminal.lock_unfair();
                }
            }

            // 全部字节都被收入同步缓冲时没有可见变化，无需重绘（镜像 event_loop L166）。
            if state.parser.sync_bytes_count() < processed && processed > 0 {
                proxy.send_event(AlacTermEvent::Wakeup);
            }
            drop(term);
            drop(lease);
        }

        let dragging = sync_stats.ui_dragging.load(Ordering::Relaxed);

        // 非拖动刷屏时让锁（见函数头注释）：只要上一段窗口里 UI 帧试过取锁却没抢到
        // （attempts 涨、successes 没涨），下一批取锁之前就在锁外站一会儿，
        // 让 UI 把帧画出来、把排队的滚动/复制事件处理掉。
        // 不再以「本批攒满 64KB」为前提：逐行小包刷屏时该条件几乎永不成立，
        // 让锁逻辑在这些最需要它的场景里从不生效。
        let attempts = sync_stats.attempts.load(Ordering::Relaxed);
        let successes = sync_stats.successes.load(Ordering::Relaxed);
        if !dragging && attempts > last_attempts && successes == last_successes {
            let deadline = Instant::now() + UI_YIELD_TIMEOUT;
            while sync_stats.successes.load(Ordering::Relaxed) == successes
                && Instant::now() < deadline
            {
                std::thread::sleep(UI_GRACE_SLICE);
            }
        }
        // 基线每批都推进（不只是让锁那批）：否则 UI 两次帧之间攒下的
        // attempts 会被反复当成「新饥饿」，形成无谓的站桩。
        last_attempts = sync_stats.attempts.load(Ordering::Relaxed);
        last_successes = sync_stats.successes.load(Ordering::Relaxed);

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
    sync_stats: Arc<SyncStats>,
) -> (FeedHandle, ExternalPtyWriter, PtyBoundReceiver) {
    let (feed_tx, feed_rx) = mpsc::channel::<LoopMsg>();
    let (bound_tx, bound_rx) = mpsc::channel::<PtyBoundMessage>();

    // 外联循环无需与 UI 同步启动；线程创建失败极罕见，记日志并继续（后续
    // feed 会因通道无接收端而失败，最终以 CloseTerminal 收场）。
    if std::thread::Builder::new()
        .name("rgpui-term-external".to_string())
        .spawn(move || external_loop(terminal, proxy, feed_rx, sync_stats))
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

    fn fresh_stats() -> Arc<SyncStats> {
        Arc::new(SyncStats::default())
    }

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
        let (feed, _writer, _rx) = spawn_feed_loop(terminal.clone(), proxy, fresh_stats());

        feed.feed("hello ".as_bytes());
        feed.feed("\x1b[31mworld".as_bytes());

        wait_row_text(&terminal, 0, "hello");
        // ANSI SGR 红色字节被消费，"world"（第 6 列起）纯文本上屏
        wait_row_text(&terminal, 6, "world");
    }

    /// 回归：UI 线程正持有 data 锁时到达的字节，解锁后必须仍被解析上屏。
    ///
    /// 解析线程先取租约（`next`），`try_lock_unfair` 失败后若退回 `lock()`
    /// 就是同线程二次加 `next`，parking_lot 不可重入 → 线程永久自锁，
    /// 通道照常收字节但网格再也不更新（外接 SSH/Telnet 会话"卡住不出字"）。
    #[test]
    fn feed_survives_contention_with_ui_lock() {
        let (terminal, proxy, _events) = make_term();
        let (feed, _writer, _rx) = spawn_feed_loop(terminal.clone(), proxy, fresh_stats());

        // 占住 data 锁，逼出 try_lock_unfair 的失败分支。
        let guard = terminal.lock_unfair();
        assert!(feed.feed("contended".as_bytes()));
        sleep(Duration::from_millis(120));
        drop(guard);

        // 用 try_lock 轮询：自锁的线程会一直占着 next，wait_row_text 的
        // 公平 lock() 反而会把测试挂死，这里要的是有限时间内失败。
        let mut appeared = false;
        for _ in 0..200 {
            sleep(Duration::from_millis(10));
            if let Some(term) = terminal.try_lock_unfair() {
                let text: String = (0..9).map(|i| term.grid()[Line(0)][Column(i)].c).collect();
                drop(term);
                if text == "contended" {
                    appeared = true;
                    break;
                }
            }
        }
        assert!(appeared, "UI 持锁期间到达的字节没有被解析上屏");
    }

    #[test]
    fn notify_reaches_bound_receiver() {
        let (terminal, proxy, _events) = make_term();
        let (_feed, writer, rx) = spawn_feed_loop(terminal, proxy, fresh_stats());

        writer.notify(b"ls\r\n");

        match rx.recv() {
            Some(PtyBoundMessage::Input(bytes)) => assert_eq!(bytes, b"ls\r\n".to_vec()),
            other => panic!("expected Input message, got {:?}", other),
        }
    }

    #[test]
    fn on_resize_reaches_bound_receiver() {
        let (terminal, proxy, _events) = make_term();
        let (_feed, mut writer, rx) = spawn_feed_loop(terminal, proxy, fresh_stats());

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
        let (feed, _writer, _rx) = spawn_feed_loop(terminal, proxy, fresh_stats());

        feed.end_session();

        wait_event(&mut events, &|event| matches!(event, AlacTermEvent::Exit));
    }

    #[test]
    fn dropping_feed_handle_also_exits() {
        let (terminal, proxy, mut events) = make_term();
        let (feed, _writer, _rx) = spawn_feed_loop(terminal, proxy, fresh_stats());

        drop(feed);

        wait_event(&mut events, &|event| matches!(event, AlacTermEvent::Exit));
    }

    /// 刷屏让锁回归：解析线程持续满负荷解析时，UI 也必须能按帧拿到数据锁。
    ///
    /// 旧实现只在 64KB 批次之间放锁，间隙以微秒计——模拟 UI 按 16ms 一帧
    /// `try_lock_unfair` 时整段刷屏期一次都进不来（画面停更、排队的滚动/
    /// 复制事件无人处理，即 issue #1 的「拖动卡顿」「Ctrl+C 发不出」）。
    #[test]
    fn flood_still_lets_the_ui_get_a_frame_lock() {
        let (terminal, proxy, _events) = make_term();
        let stats = fresh_stats();
        let (feed, _writer, _rx) = spawn_feed_loop(terminal.clone(), proxy, stats.clone());

        // 一次性灌 100MB：无论解析快慢，UI 的观察窗口都落在满负荷期内。
        let feeder_feed = feed.clone();
        std::thread::spawn(move || {
            let chunk = vec![b'a'; 64 * 1024];
            for _ in 0..1_600 {
                if !feeder_feed.feed(chunk.clone()) {
                    break;
                }
            }
        });

        // 模拟 UI：每 16ms 一帧，进门记 attempts，取到锁记 successes 并短暂持有。
        // stats / terminal 之后不再使用，直接移入线程，不做冗余 clone。
        let ui = std::thread::spawn(move || {
            let mut successes = 0u64;
            for _ in 0..12 {
                std::thread::sleep(Duration::from_millis(16));
                stats.attempts.fetch_add(1, Ordering::Relaxed);
                if terminal.try_lock_unfair().is_some() {
                    stats.successes.fetch_add(1, Ordering::Relaxed);
                    successes += 1;
                    std::thread::sleep(Duration::from_millis(2));
                }
            }
            successes
        });

        let successes = ui.join().expect("UI 模拟线程 panic");
        drop(feed);
        // 这里刻意让 UI 每帧只 try 一次（不自旋），专门检验解析线程的让锁：
        // 旧实现整段刷屏期一次都拿不到（0/12）。阈值取 1 而非更高，是因为
        // macOS CI 调度粒度粗、实测在 1~3/12 之间波动；只要能抢到一次即证明
        // 让锁生效，而旧实现的 0 仍会被捕获。真实 UI 的 sync() 还有 2ms 自旋
        // 重试，实际帧成功率远高于这个保守模拟。
        assert!(
            successes >= 1,
            "持续刷屏 200ms 内 UI 只抢到 {successes}/12 帧的数据锁，让锁失效"
        );
    }

    /// 逐行小包刷屏回归：docker logs -f 把历史日志一行行推过来时，
    /// 每批都远小于 64KB，旧实现要等「攒满上限」才让锁，于是整段刷屏
    /// 期间一次都不让，UI 连续丢帧、拖动滚动条卡住（issue #1 的形态）。
    #[test]
    fn small_packet_flood_still_lets_the_ui_get_a_frame_lock() {
        let (terminal, proxy, _events) = make_term();
        let stats = fresh_stats();
        let (feed, _writer, _rx) = spawn_feed_loop(terminal.clone(), proxy, stats.clone());

        // 每 1ms 一行 ~200B，持续约 200ms：解析线程一直有活干，但
        // try_recv 无论怎么攒都到不了 MAX_LOCKED_READ。
        let feeder = std::thread::spawn(move || {
            let line = b"2026-11-20T13:00:00 INFO container log line payload payload\n";
            for _ in 0..200 {
                sleep(Duration::from_millis(1));
                if !feed.feed(line.to_vec()) {
                    break;
                }
            }
        });

        let ui = std::thread::spawn(move || {
            let mut successes = 0u64;
            for _ in 0..12 {
                sleep(Duration::from_millis(16));
                stats.attempts.fetch_add(1, Ordering::Relaxed);
                if terminal.try_lock_unfair().is_some() {
                    stats.successes.fetch_add(1, Ordering::Relaxed);
                    successes += 1;
                    sleep(Duration::from_millis(2));
                }
            }
            successes
        });

        feeder.join().expect("feed 线程 panic");
        let successes = ui.join().expect("UI 模拟线程 panic");
        assert!(
            successes >= 4,
            "逐行小包刷屏 200ms 内 UI 只抢到 {successes}/12 帧的数据锁，让锁失效"
        );
    }

    /// 拖滚动条期间解析线程暂停，松手后接着解析且不丢字节。
    #[test]
    fn dragging_scrollbar_still_feeds_the_grid() {
        let (terminal, proxy, _events) = make_term();
        let stats = fresh_stats();
        let (feed, _writer, _rx) = spawn_feed_loop(terminal.clone(), proxy, stats.clone());

        stats.ui_dragging.store(true, Ordering::Relaxed);
        assert!(feed.feed("during drag".as_bytes()));

        // 拖动中终端网格保持稳定；释放拖动标志后，队列数据应恢复解析。
        sleep(Duration::from_millis(40));
        let term = terminal.lock();
        let text: String = (0..11).map(|i| term.grid()[Line(0)][Column(i)].c).collect();
        assert_ne!(text, "during drag", "拖动期间不应继续更新终端网格");
        drop(term);

        stats.ui_dragging.store(false, Ordering::Relaxed);
        let mut seen = false;
        for _ in 0..40 {
            sleep(Duration::from_millis(10));
            let term = terminal.lock();
            let text: String = (0..11).map(|i| term.grid()[Line(0)][Column(i)].c).collect();
            drop(term);
            if text == "during drag" {
                seen = true;
                break;
            }
        }
        assert!(seen, "松开拖动后应继续解析并更新网格");
    }

    /// 拖动标志卡住（鼠标事件丢失、窗口失焦等）时，解析线程靠超时自恢复。
    ///
    /// 没有这个兜底，`external_loop` 会永久停在 `while ui_dragging { sleep }`：
    /// ^C 其实已经发到远端把进程杀掉了，但画面再也不刷新、shell 提示符回不来，
    /// 表现就是「Ctrl+C 终止不了」（拖出终端右缘松手最容易触发）。
    #[test]
    fn stuck_drag_flag_resumes_parsing_after_timeout() {
        let (terminal, proxy, _events) = make_term();
        let stats = fresh_stats();
        let (feed, _writer, _rx) = spawn_feed_loop(terminal.clone(), proxy, stats.clone());

        // 模拟松手漏清标志：置位后既不喂新字节，也不主动清。
        stats.ui_dragging.store(true, Ordering::Relaxed);
        assert!(feed.feed("after stuck drag".as_bytes()));

        // 兜底超时是 2s；留足余量等解析恢复并把字节写进网格。
        let mut seen = false;
        for _ in 0..400 {
            sleep(Duration::from_millis(10));
            let term = terminal.lock();
            let text: String = (0..16).map(|i| term.grid()[Line(0)][Column(i)].c).collect();
            drop(term);
            if text == "after stuck drag" {
                seen = true;
                break;
            }
        }
        assert!(
            seen,
            "拖动标志卡住时解析线程应在超时后自恢复，否则画面冻结、Ctrl+C 看似失效"
        );
    }

    /// 拖动滚动条 + 持续刷屏：解析暂停期间 UI 每帧都能拿到数据锁。
    ///
    /// 这是 issue #1「拖一段卡一下、再卡一下」的直接量化。拖动时 UI 帧要
    /// 连续拿到锁（接近 60Hz）；一旦隔帧才拿到一次，就表现为「动一段、卡一段」。
    /// 检查拖动 + 洪泛时没有后台解析与 UI 争用数据锁。
    #[test]
    fn dragging_scrollbar_under_flood_keeps_frame_cadence() {
        let (terminal, proxy, _events) = make_term();
        let stats = fresh_stats();
        let (feed, _writer, _rx) = spawn_feed_loop(terminal.clone(), proxy, stats.clone());

        // 一次性灌 100MB：无论解析快慢，整个观察窗口里解析线程都满载有活干。
        let feeder = std::thread::spawn(move || {
            let chunk = vec![b'a'; 64 * 1024];
            for _ in 0..1_600 {
                if !feed.feed(chunk.clone()) {
                    break;
                }
            }
        });

        // 模拟「正在拖滚动条」并等待解析线程检查到标志。
        stats.ui_dragging.store(true, Ordering::Relaxed);
        sleep(Duration::from_millis(10));

        // 模拟 UI：60Hz 一帧，进门记 attempts，取到锁记 successes 并按帧重绘持锁
        // ~2ms。同时记录最长连续丢帧段，量化「卡」的成段程度。
        let ui_stats = stats.clone();
        let ui = std::thread::spawn(move || {
            const FRAMES: u32 = 30;
            let mut successes = 0u32;
            let mut cur_miss = 0u32;
            let mut max_miss = 0u32;
            for _ in 0..FRAMES {
                sleep(Duration::from_millis(16));
                ui_stats.attempts.fetch_add(1, Ordering::Relaxed);
                if terminal.try_lock_unfair().is_some() {
                    ui_stats.successes.fetch_add(1, Ordering::Relaxed);
                    successes += 1;
                    cur_miss = 0;
                    sleep(Duration::from_millis(2));
                } else {
                    cur_miss += 1;
                    if cur_miss > max_miss {
                        max_miss = cur_miss;
                    }
                }
            }
            (successes, FRAMES, max_miss)
        });

        let (successes, frames, max_miss) = ui.join().expect("UI 模拟线程 panic");
        feeder.join().expect("feed 线程 panic");
        stats.ui_dragging.store(false, Ordering::Relaxed);
        assert!(
            successes == frames,
            "拖动刷屏时 UI 只抢到 {successes}/{frames} 帧（最长连丢 {max_miss} 帧），拖动仍会成段卡顿"
        );
    }

    #[test]
    fn writer_keeps_live_after_terminal_use() {
        let (terminal, proxy, mut events) = make_term();
        let (feed, writer, rx) = spawn_feed_loop(terminal, proxy, fresh_stats());

        // 注入 + 上行各一次后双方都应继续工作
        feed.feed("a".as_bytes());
        writer.notify(b"b");
        assert!(matches!(rx.recv(), Some(PtyBoundMessage::Input(ref bytes)) if bytes == b"b"));

        wait_event(&mut events, &|event| matches!(event, AlacTermEvent::Wakeup));
    }
}
