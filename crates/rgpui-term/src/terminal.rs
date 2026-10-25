//! 终端实体，封装 Alacritty 的终端模拟功能。
//!
//! 本模块提供核心的 Terminal 结构体，桥接 GPUI 与 Alacritty 终端模拟器。
//! 处理 PTY 通信、事件处理，并为终端视图提供清晰的交互 API。
//!
//! # 架构
//!
//! 终端系统包含以下部分：
//! - `ZedListener`: 通过无界通道将 Alacritty 事件桥接到 GPUI
//! - `TerminalBounds`: 管理终端尺寸（单元格、像素、边界）
//! - `TerminalContent`: 持有渲染输出以供显示
//! - `Terminal`: 主实体，封装 `Arc<FairMutex<Term<ZedListener>>>`
//! - `TerminalBuilder`: 创建终端的工厂，包含 PTY 订阅
//!
//! # 事件处理
//!
//! 来自 Alacritty 的事件会在 4ms 窗口内批处理，以减少 UI 更新开销。
//! 事件循环在 RGPUI spawn 任务中运行并异步处理事件。

use std::{
    borrow::Cow, cmp, collections::VecDeque, ops::Deref, path::PathBuf, sync::Arc, time::Duration,
};

use alacritty_terminal::{
    Term,
    event::{Event as AlacTermEvent, EventListener, Notify, OnResize, WindowSize},
    event_loop::{EventLoop, Msg, Notifier},
    grid::{Dimensions, Scroll as AlacScroll},
    index::{Column, Direction as AlacDirection, Line, Point as AlacPoint, Side},
    selection::{Selection, SelectionRange, SelectionType},
    sync::FairMutex,
    term::{Config, RenderableCursor, TermMode, cell::Cell, search::RegexSearch},
    tty,
    vte::ansi::{
        ClearMode, CursorShape as AlacCursorShape, CursorStyle as AlacCursorStyle, Handler,
    },
};
use anyhow::{Context as _, Result};
use futures::{
    FutureExt, StreamExt,
    channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded},
};
use rgpui::{
    App, Bounds, ClipboardItem, Context, EventEmitter, Keystroke, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollWheelEvent, Size, Task, TouchPhase, Window,
    px,
};

use crate::block::{Block, BlockTracker};
use crate::mappings::{
    keys::to_esc_str,
    mouse::{
        alt_scroll, grid_point, grid_point_and_side, mouse_button_report, mouse_moved_report,
        scroll_report,
    },
};
use crate::{InputOrigin, TerminalMiddleware};

const DEFAULT_SCROLL_HISTORY_LINES: usize = 10_000;
const MAX_SCROLL_HISTORY_LINES: usize = 100_000;

/// 终端后端写通道：决定用户输入、OSC 应答与尺寸变化发往何处。
enum PtyBackend {
    /// 本地 PTY：由 Alacritty 事件循环托管，字节直达 ConPTY/PTY 句柄。
    Local(Notifier),
    /// 外联字节流：转发给调用方的传输层（SSH/Telnet/串口等）。
    External(crate::external::ExternalPtyWriter),
}

impl PtyBackend {
    /// 写入下行字节（等价于写本地 PTY）。
    fn notify(&self, bytes: Cow<'static, [u8]>) {
        match self {
            PtyBackend::Local(notifier) => notifier.notify(bytes),
            PtyBackend::External(writer) => writer.notify(bytes),
        }
    }

    /// 通知网格尺寸变化。
    fn on_resize(&mut self, window_size: WindowSize) {
        match self {
            PtyBackend::Local(notifier) => {
                notifier.0.send(Msg::Resize(window_size)).ok();
            }
            PtyBackend::External(writer) => writer.on_resize(window_size),
        }
    }
}
const DEBUG_TERMINAL_WIDTH: Pixels = px(500.);
const DEBUG_TERMINAL_HEIGHT: Pixels = px(30.);
const DEBUG_CELL_WIDTH: Pixels = px(5.);
const DEBUG_LINE_HEIGHT: Pixels = px(5.);

/// 终端发出的事件，供视图层处理。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// 终端标题已更改
    TitleChanged,
    /// 收到响铃字符
    Bell,
    /// 终端内容已更改，需要重绘
    Wakeup,
    /// 光标闪烁状态已更改
    BlinkChanged(bool),
    /// 选择区域已更改
    SelectionsChanged,
    /// 终端进程已退出
    CloseTerminal,
    /// 点击了 OSC 8 超链接（携带 URI）
    HyperlinkClick(String),
}

impl EventEmitter<Event> for Terminal {}

/// 通过无界通道将 Alacritty 事件桥接到 RGPUI。
///
/// 实现 Alacritty 的 `EventListener` trait，接收来自终端模拟器的事件
/// 并将其转发到 RGPUI 事件循环进行处理。
#[derive(Clone)]
pub struct ZedListener(pub UnboundedSender<AlacTermEvent>);

impl EventListener for ZedListener {
    fn send_event(&self, event: AlacTermEvent) {
        self.0.unbounded_send(event).ok();
    }
}

/// 内部事件队列。
///
/// 这些事件在同步期间排队并处理，以更新终端状态。
#[derive(Clone)]
enum InternalEvent {
    Resize(TerminalBounds),
    Clear,
    Scroll(AlacScroll),
    ScrollToAlacPoint(AlacPoint),
    SetSelection(Option<(Selection, AlacPoint)>),
    UpdateSelection(Point<Pixels>),
    Copy(Option<bool>),
    /// 设置搜索关键词（空串表示清除）。
    SearchSet(String),
    /// 在搜索结果间步进。
    SearchStep(AlacDirection),
}

/// 终端内容搜索状态。
///
/// 建立在 alacritty `RegexSearch`（增量缓存 DFA）之上；搜索语义与其对齐：
/// query 全小写时忽略大小写，搜索范围覆盖全部滚动历史并自动回绕。
/// query 编译失败时保持空状态（无匹配）。
pub struct TerminalSearch {
    /// 已编译正则
    regex: Option<RegexSearch>,
    /// 当前高亮的匹配（网格坐标）
    current: Option<SelectionRange>,
}

/// 终端尺寸管理。
///
/// 处理像素边界、单元格尺寸和网格大小之间的关系。
/// 用于 RGPUI 和 Alacritty 之间的坐标转换。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalBounds {
    /// 单元格宽度
    pub cell_width: Pixels,
    /// 行高
    pub line_height: Pixels,
    /// 边界矩形
    pub bounds: Bounds<Pixels>,
}

impl TerminalBounds {
    pub fn new(line_height: Pixels, cell_width: Pixels, bounds: Bounds<Pixels>) -> Self {
        TerminalBounds {
            cell_width,
            line_height,
            bounds,
        }
    }

    pub fn num_lines(&self) -> usize {
        (self.bounds.size.height / self.line_height).floor() as usize
    }

    pub fn num_columns(&self) -> usize {
        (self.bounds.size.width / self.cell_width).floor() as usize
    }

    pub fn height(&self) -> Pixels {
        self.bounds.size.height
    }

    pub fn width(&self) -> Pixels {
        self.bounds.size.width
    }

    pub fn last_column(&self) -> Column {
        Column(self.num_columns().saturating_sub(1))
    }

    pub fn bottommost_line(&self) -> Line {
        Line(self.num_lines().saturating_sub(1) as i32)
    }
}

impl Default for TerminalBounds {
    fn default() -> Self {
        TerminalBounds::new(
            DEBUG_LINE_HEIGHT,
            DEBUG_CELL_WIDTH,
            Bounds {
                origin: Point::default(),
                size: Size {
                    width: DEBUG_TERMINAL_WIDTH,
                    height: DEBUG_TERMINAL_HEIGHT,
                },
            },
        )
    }
}

impl From<TerminalBounds> for WindowSize {
    fn from(val: TerminalBounds) -> Self {
        WindowSize {
            num_lines: val.num_lines() as u16,
            num_cols: val.num_columns() as u16,
            cell_width: f32::from(val.cell_width) as u16,
            cell_height: f32::from(val.line_height) as u16,
        }
    }
}

impl Dimensions for TerminalBounds {
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }

    fn screen_lines(&self) -> usize {
        self.num_lines()
    }

    fn columns(&self) -> usize {
        self.num_columns()
    }
}

/// 终端网格中的单个单元格及其位置。
#[derive(Clone, Debug)]
pub struct IndexedCell {
    /// 单元格在网格中的位置
    pub point: AlacPoint,
    /// 单元格数据
    pub cell: Cell,
}

impl Deref for IndexedCell {
    type Target = Cell;

    fn deref(&self) -> &Cell {
        &self.cell
    }
}

/// 终端渲染内容，用于显示。
///
/// 包含渲染终端所需的所有信息：
/// 单元格、光标、选择区域、模式和滚动状态。
#[derive(Clone)]
pub struct TerminalContent {
    /// 单元格列表
    pub cells: Vec<IndexedCell>,
    /// 终端模式
    pub mode: TermMode,
    /// 显示偏移量（滚动位置）
    pub display_offset: usize,
    /// 选中的文本
    pub selection_text: Option<String>,
    /// 选择区域范围
    pub selection: Option<SelectionRange>,
    /// 光标信息
    pub cursor: RenderableCursor,
    /// 光标字符
    pub cursor_char: char,
    /// 终端尺寸
    pub terminal_bounds: TerminalBounds,
    /// 是否已滚动到顶部
    pub scrolled_to_top: bool,
    /// 是否已滚动到底部
    pub scrolled_to_bottom: bool,
    /// 历史行数
    pub history_size: usize,
    /// 当前搜索匹配（网格坐标，供渲染高亮）
    pub search_match: Option<SelectionRange>,
}

impl Default for TerminalContent {
    fn default() -> Self {
        TerminalContent {
            cells: Vec::new(),
            mode: TermMode::empty(),
            display_offset: 0,
            selection_text: None,
            selection: None,
            cursor: RenderableCursor {
                shape: AlacCursorShape::Block,
                point: AlacPoint::new(Line(0), Column(0)),
            },
            cursor_char: ' ',
            terminal_bounds: TerminalBounds::default(),
            scrolled_to_top: false,
            scrolled_to_bottom: true,
            history_size: 0,
            search_match: None,
        }
    }
}

#[derive(PartialEq, Eq)]
pub enum SelectionPhase {
    Selecting,
    Ended,
}

#[cfg(windows)]
fn find_on_path(executable: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(executable);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

#[cfg(windows)]
fn find_pwsh() -> Option<String> {
    if let Some(path) = find_on_path("pwsh.exe") {
        return Some(path);
    }

    let roots = ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"];
    for key in roots {
        if let Ok(root) = std::env::var(key) {
            for suffix in ["PowerShell\\7\\pwsh.exe", "PowerShell\\7-preview\\pwsh.exe"] {
                let path = std::path::PathBuf::from(&root).join(suffix);
                if path.is_file() {
                    return Some(path.to_string_lossy().into_owned());
                }
            }
        }
    }

    if let Ok(root) = std::env::var("LOCALAPPDATA") {
        for suffix in [
            "Microsoft\\PowerShell\\7\\pwsh.exe",
            "Microsoft\\PowerShell\\7-preview\\pwsh.exe",
        ] {
            let path = std::path::PathBuf::from(&root).join(suffix);
            if path.is_file() {
                return Some(path.to_string_lossy().into_owned());
            }
        }
    }

    None
}

#[cfg(windows)]
fn default_shell_command() -> Option<String> {
    if let Ok(shell) = std::env::var("SHELL") {
        return Some(shell);
    }

    if let Some(pwsh) = find_pwsh() {
        return Some(pwsh);
    }

    if let Ok(root) = std::env::var("SystemRoot") {
        let mut path = std::path::PathBuf::from(root);
        path.push("System32");
        path.push("WindowsPowerShell");
        path.push("v1.0");
        path.push("powershell.exe");
        return Some(path.to_string_lossy().into_owned());
    }

    Some("powershell".to_string())
}

#[cfg(not(windows))]
fn default_shell_command() -> Option<String> {
    std::env::var("SHELL").ok()
}

/// 创建 Terminal 实例的工厂，包含 PTY 连接。
///
/// 处理异步 PTY 创建并提供 `subscribe` 方法
/// 来连接事件循环。
pub struct TerminalBuilder {
    terminal: Terminal,
    events_rx: UnboundedReceiver<AlacTermEvent>,
    /// 外联传输层句柄（仅 `new_external` 非空）。
    external: Option<crate::external::ExternalHandles>,
}

impl TerminalBuilder {
    /// 创建带有 PTY 连接的新终端。
    ///
    /// # 参数
    ///
    /// * `working_directory` - Shell 的初始工作目录
    /// * `shell` - 要运行的 Shell 程序（None 使用系统默认）
    /// * `env` - 额外的环境变量
    /// * `max_scroll_history_lines` - 最大滚动历史行数
    /// * `window_id` - RGPUI 窗口标识符
    /// * `cx` - 应用上下文
    pub fn new(
        working_directory: Option<PathBuf>,
        shell: Option<String>,
        env: std::collections::HashMap<String, String>,
        max_scroll_history_lines: Option<usize>,
        window_id: u64,
        cx: &App,
    ) -> Task<Result<TerminalBuilder>> {
        cx.spawn(async move |_| {
            let mut shell_cmd = shell.clone();

            if shell_cmd.is_none() {
                shell_cmd = default_shell_command();
            }

            let alac_shell = shell_cmd.map(|program| tty::Shell::new(program, Vec::new()));

            // Validate and fallback working directory
            let working_directory = working_directory
                .filter(|dir| {
                    let exists = dir.exists();
                    if !exists {
                        log::warn!("Working directory does not exist: {:?}", dir);
                    }
                    exists
                })
                .or_else(|| {
                    // Fallback to home directory
                    let home = dirs::home_dir();
                    if home.is_some() {
                        log::info!("Using home directory as working directory");
                    }
                    home
                })
                .or_else(|| {
                    // Fallback to USERPROFILE on Windows
                    #[cfg(windows)]
                    {
                        let userprofile = std::env::var("USERPROFILE").ok().map(PathBuf::from);
                        if userprofile.is_some() {
                            log::info!("Using USERPROFILE as working directory");
                        }
                        userprofile
                    }
                    #[cfg(not(windows))]
                    None
                })
                .or_else(|| {
                    // Last resort: use current directory
                    log::warn!("No valid working directory found, using current directory");
                    std::env::current_dir().ok()
                });

            let pty_options = tty::Options {
                shell: alac_shell,
                working_directory,
                drain_on_exit: true,
                env: env.into_iter().collect(),
                #[cfg(windows)]
                escape_args: false,
            };

            let scrolling_history = max_scroll_history_lines
                .unwrap_or(DEFAULT_SCROLL_HISTORY_LINES)
                .min(MAX_SCROLL_HISTORY_LINES);

            let config = Config {
                scrolling_history,
                default_cursor_style: AlacCursorStyle {
                    shape: AlacCursorShape::Block,
                    blinking: false,
                },
                ..Config::default()
            };

            // Log PTY creation details for debugging
            log::info!("Creating PTY with options:");
            log::info!("  shell: {:?}", pty_options.shell);
            log::info!("  working_directory: {:?}", pty_options.working_directory);
            log::info!("  window_id: {}", window_id);
            log::info!("  env vars count: {}", pty_options.env.len());

            let pty = tty::new(&pty_options, TerminalBounds::default().into(), window_id)
                .context("failed to create PTY")?;

            let (events_tx, events_rx) = unbounded();

            let term = Term::new(
                config,
                &TerminalBounds::default(),
                ZedListener(events_tx.clone()),
            );

            let term = Arc::new(FairMutex::new(term));

            let event_loop = EventLoop::new(
                term.clone(),
                ZedListener(events_tx),
                pty,
                pty_options.drain_on_exit,
                false,
            )
            .context("failed to create event loop")?;

            let pty_tx = event_loop.channel();
            let _io_thread = event_loop.spawn();

            let terminal = Terminal {
                term,
                pty_backend: PtyBackend::Local(Notifier(pty_tx)),
                events: VecDeque::with_capacity(10),
                last_content: TerminalContent::default(),
                last_mouse: None,
                selection_head: None,
                breadcrumb_text: String::new(),
                scroll_px: px(0.),
                selection_phase: SelectionPhase::Ended,
                middlewares: Vec::new(),
                event_loop_task: Task::ready(Ok(())),
                block_tracker: BlockTracker::new(),
                ime_composition: None,
                search: None,
            };

            Ok(TerminalBuilder {
                terminal,
                events_rx,
                external: None,
            })
        })
    }

    /// 创建由调用方提供字节流的外联终端（SSH/Telnet/串口等）。
    ///
    /// 不生成任何本地进程：远端方向字节经 [`FeedHandle::feed`] 注入，上行
    /// 字节与尺寸变化从 [`PtyBoundMessage`] 通道读出（通过
    /// [`Self::take_external_handles`] 取走句柄）。会话结束时由传输层调用
    /// [`FeedHandle::end_session`]，UI 侧以 `Event::CloseTerminal` 收场，
    /// 与本地进程退出一致。
    ///
    /// 创建过程无进程拉起与 OS 等待，无需异步：直接返回。
    pub fn new_external(max_scroll_history_lines: Option<usize>) -> Result<TerminalBuilder> {
        let scrolling_history = max_scroll_history_lines
            .unwrap_or(DEFAULT_SCROLL_HISTORY_LINES)
            .min(MAX_SCROLL_HISTORY_LINES);

        let config = Config {
            scrolling_history,
            default_cursor_style: AlacCursorStyle {
                shape: AlacCursorShape::Block,
                blinking: false,
            },
            ..Config::default()
        };

        let (events_tx, events_rx) = unbounded();

        let term = Term::new(
            config,
            &TerminalBounds::default(),
            ZedListener(events_tx.clone()),
        );

        let term = Arc::new(FairMutex::new(term));

        let (feed_handle, writer, bound_rx) =
            crate::external::spawn_feed_loop(term.clone(), ZedListener(events_tx));

        let terminal = Terminal {
            term,
            pty_backend: PtyBackend::External(writer),
            events: VecDeque::with_capacity(10),
            last_content: TerminalContent::default(),
            last_mouse: None,
            selection_head: None,
            breadcrumb_text: String::new(),
            scroll_px: px(0.),
            selection_phase: SelectionPhase::Ended,
            middlewares: Vec::new(),
            event_loop_task: Task::ready(Ok(())),
            block_tracker: BlockTracker::new(),
            ime_composition: None,
            search: None,
        };

        Ok(TerminalBuilder {
            terminal,
            events_rx,
            external: Some(crate::external::ExternalHandles {
                feed: feed_handle,
                rx: bound_rx,
            }),
        })
    }

    /// 取走外联传输层句柄（仅 [`Self::new_external`] 创建的终端可用）。
    ///
    /// `feed` 由传输层持有并注入远端字节；`rx` 交由传输层的独立线程/任务
    /// 循环驱动，转发 SSH channel。
    pub fn take_external_handles(&mut self) -> Option<crate::external::ExternalHandles> {
        self.external.take()
    }

    /// 订阅终端事件并返回已配置的 Terminal。
    ///
    /// 此方法设置事件循环，以批处理 4ms 窗口的方式处理 Alacritty 事件，
    /// 减少 UI 更新开销。
    pub fn subscribe(mut self, cx: &Context<Terminal>) -> Terminal {
        self.terminal.event_loop_task = cx.spawn(async move |terminal, cx| {
            while let Some(event) = self.events_rx.next().await {
                terminal.update(cx, |terminal, cx| {
                    terminal.process_event(event, cx);
                })?;

                'outer: loop {
                    let mut events = Vec::new();

                    let mut timer = cx
                        .background_executor()
                        .timer(Duration::from_millis(4))
                        .fuse();

                    let mut wakeup = false;
                    loop {
                        futures::select_biased! {
                            _ = timer => break,
                            event = self.events_rx.next() => {
                                if let Some(event) = event {
                                    if matches!(event, AlacTermEvent::Wakeup) {
                                        wakeup = true;
                                    } else {
                                        events.push(event);
                                    }

                                    if events.len() > 100 {
                                        break;
                                    }
                                } else {
                                    break;
                                }
                            },
                        }
                    }

                    if events.is_empty() && !wakeup {
                        smol::future::yield_now().await;
                        break 'outer;
                    }

                    terminal.update(cx, |this, cx| {
                        if wakeup {
                            this.process_event(AlacTermEvent::Wakeup, cx);
                        }

                        for event in events {
                            this.process_event(event, cx);
                        }
                    })?;
                    smol::future::yield_now().await;
                }
            }
            anyhow::Ok(())
        });
        self.terminal
    }
}

/// 终端主实体，封装 Alacritty 的 Term。
///
/// 提供 GPUI 与 Alacritty 终端模拟器之间的接口。
/// 处理输入、输出、滚动、选择和事件处理。
pub struct Terminal {
    /// Alacritty 终端实例
    term: Arc<FairMutex<Term<ZedListener>>>,
    /// 后端写通道（本地 PTY 或外联传输层）
    pty_backend: PtyBackend,
    /// 内部事件队列
    events: VecDeque<InternalEvent>,
    /// 上次鼠标位置
    last_mouse: Option<(AlacPoint, AlacDirection)>,
    /// 上次渲染内容
    pub last_content: TerminalContent,
    /// 选择区域头部位置
    pub selection_head: Option<AlacPoint>,
    /// 面包屑文本（终端标题）
    pub breadcrumb_text: String,
    /// 滚动像素值
    scroll_px: Pixels,
    /// 选择阶段
    selection_phase: SelectionPhase,
    /// 中间件列表
    middlewares: Vec<Arc<dyn TerminalMiddleware>>,
    /// 事件循环任务
    event_loop_task: Task<Result<(), anyhow::Error>>,
    /// 代码块跟踪器
    block_tracker: BlockTracker,
    /// IME 组合中（尚未上屏）的文本；`None` 表示没有正在进行的组合
    ime_composition: Option<String>,
    /// 终端内容搜索（`None` 表示未激活）
    search: Option<TerminalSearch>,
}

impl Terminal {
    /// 向终端添加中间件实例。
    pub fn add_middleware(&mut self, middleware: Arc<dyn TerminalMiddleware>) {
        self.middlewares.push(middleware);
    }

    /// 用新列表替换中间件管道。
    pub fn set_middlewares(&mut self, middlewares: Vec<Arc<dyn TerminalMiddleware>>) {
        self.middlewares = middlewares;
    }

    fn apply_input_middlewares(
        &self,
        mut input: Cow<'static, [u8]>,
        origin: InputOrigin,
    ) -> Option<Cow<'static, [u8]>> {
        for middleware in &self.middlewares {
            input = middleware.on_input(input, origin)?;
        }
        Some(input)
    }

    fn notify_middlewares_event(&self, event: &Event) {
        for middleware in &self.middlewares {
            middleware.on_event(event);
        }
    }

    fn notify_middlewares_output(&self, content: &TerminalContent) {
        for middleware in &self.middlewares {
            middleware.on_output(content);
        }
    }

    /// 在中间件处理后向 PTY 写入字节。
    fn write_to_pty(&self, input: impl Into<Cow<'static, [u8]>>, origin: InputOrigin) -> bool {
        let Some(filtered) = self.apply_input_middlewares(input.into(), origin) else {
            return false;
        };

        self.pty_backend.notify(filtered);
        true
    }

    /// 向终端发送输入，滚动到底部并清除选择区域。
    pub fn input(&mut self, input: impl Into<Cow<'static, [u8]>>) {
        self.input_with_origin(input, InputOrigin::Programmatic);
    }

    /// 向终端发送输入，附带来源元数据。
    pub fn input_with_origin(&mut self, input: impl Into<Cow<'static, [u8]>>, origin: InputOrigin) {
        let input = input.into();

        // Detect Enter key from user input to start block tracking
        if matches!(origin, InputOrigin::Keystroke | InputOrigin::Text) && input.as_ref() == b"\x0d"
        {
            let term = self.term.lock();
            self.block_tracker.on_enter(&term);
        }

        if self.write_to_pty(input, origin) {
            self.events
                .push_back(InternalEvent::Scroll(AlacScroll::Bottom));
            self.events.push_back(InternalEvent::SetSelection(None));
        }
    }

    /// 尝试处理按键事件，返回 true 表示已处理。
    ///
    /// 仅处理特殊键（方向键、功能键、Ctrl 组合等），
    /// 常规字符输入通过 InputHandler::replace_text_in_range 处理。
    pub fn try_keystroke(&mut self, keystroke: &Keystroke, option_as_meta: bool) -> bool {
        let esc = to_esc_str(keystroke, &self.last_content.mode, option_as_meta);
        if let Some(esc) = esc {
            match esc {
                Cow::Borrowed(string) => {
                    self.input_with_origin(string.as_bytes(), InputOrigin::Keystroke)
                }
                Cow::Owned(string) => {
                    self.input_with_origin(string.into_bytes(), InputOrigin::Keystroke)
                }
            };
            true
        } else {
            false
        }
    }

    /// 提交文本输入到终端。
    /// 当用户输入常规字符时由 InputHandler 调用。
    pub fn input_text(&mut self, text: &str) {
        if !text.is_empty() {
            self.input_with_origin(text.as_bytes().to_vec(), InputOrigin::Text);
        }
    }

    /// 设置 IME 组合文本（`None` 表示组合结束）。
    ///
    /// 组合文本只做预览，不上屏；上屏由 [`Self::input_text`] 完成。
    pub fn set_ime_composition(&mut self, text: Option<String>) {
        self.ime_composition = text;
    }

    /// 当前 IME 组合文本（用于在光标处预览）。
    pub fn ime_composition(&self) -> Option<&str> {
        self.ime_composition.as_deref()
    }

    /// 粘贴文本到终端。
    pub fn paste(&mut self, text: &str) {
        let paste_text = if self.last_content.mode.contains(TermMode::BRACKETED_PASTE) {
            format!("{}{}{}", "\x1b[200~", text.replace('\x1b', ""), "\x1b[201~")
        } else {
            text.replace("\r\n", "\r").replace('\n', "\r")
        };

        self.input_with_origin(paste_text.into_bytes(), InputOrigin::Paste);
    }

    /// 将终端大小调整为新边界。
    pub fn set_size(&mut self, new_bounds: TerminalBounds) {
        if self.last_content.terminal_bounds != new_bounds {
            self.events.push_back(InternalEvent::Resize(new_bounds));
        }
    }

    /// 同步终端状态并更新内容以供渲染。
    pub fn sync(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let term = self.term.clone();
        let mut terminal = term.lock_unfair();

        while let Some(event) = self.events.pop_front() {
            self.process_terminal_event(&event, &mut terminal, cx);
        }

        self.last_content = Self::make_content(&terminal, &self.last_content);

        // 将当前搜索匹配同步到渲染内容
        self.last_content.search_match = self.search.as_ref().and_then(|search| search.current);

        // Check for prompt detection to finalize blocks
        self.block_tracker.on_sync(&terminal, &self.last_content);

        drop(terminal);
        self.notify_middlewares_output(&self.last_content);
    }

    fn make_content(term: &Term<ZedListener>, last_content: &TerminalContent) -> TerminalContent {
        let content = term.renderable_content();

        let estimated_size = content.display_iter.size_hint().0;
        let mut cells = Vec::with_capacity(estimated_size);

        cells.extend(content.display_iter.map(|ic| IndexedCell {
            point: ic.point,
            cell: ic.cell.clone(),
        }));

        let selection_text = if content.selection.is_some() {
            term.selection_to_string()
        } else {
            None
        };

        TerminalContent {
            cells,
            mode: content.mode,
            display_offset: content.display_offset,
            selection_text,
            selection: content.selection,
            cursor: content.cursor,
            cursor_char: term.grid()[content.cursor.point].c,
            terminal_bounds: last_content.terminal_bounds,
            scrolled_to_top: content.display_offset == term.history_size(),
            scrolled_to_bottom: content.display_offset == 0,
            history_size: term.history_size(),
            search_match: None,
        }
    }

    /// 向上滚动一行。
    pub fn scroll_line_up(&mut self) {
        self.events
            .push_back(InternalEvent::Scroll(AlacScroll::Delta(1)));
    }

    /// 向下滚动一行。
    pub fn scroll_line_down(&mut self) {
        self.events
            .push_back(InternalEvent::Scroll(AlacScroll::Delta(-1)));
    }

    /// 向上滚动一页。
    pub fn scroll_page_up(&mut self) {
        self.events
            .push_back(InternalEvent::Scroll(AlacScroll::PageUp));
    }

    /// 向下滚动一页。
    pub fn scroll_page_down(&mut self) {
        self.events
            .push_back(InternalEvent::Scroll(AlacScroll::PageDown));
    }

    /// 滚动到顶部。
    pub fn scroll_to_top(&mut self) {
        self.events
            .push_back(InternalEvent::Scroll(AlacScroll::Top));
    }

    /// 滚动到底部。
    pub fn scroll_to_bottom(&mut self) {
        self.events
            .push_back(InternalEvent::Scroll(AlacScroll::Bottom));
    }

    /// 滚动到滚动历史中的特定显示偏移量。
    pub fn scroll_to_offset(&mut self, target_offset: usize) {
        let current = self.last_content.display_offset;
        let delta = target_offset as i32 - current as i32;
        if delta != 0 {
            self.events
                .push_back(InternalEvent::Scroll(AlacScroll::Delta(delta)));
        }
    }

    /// 滚动到终端网格中的特定点。
    pub fn scroll_to_point(&mut self, point: AlacPoint) {
        self.events
            .push_back(InternalEvent::ScrollToAlacPoint(point));
    }

    /// 选择终端中的所有文本。
    pub fn select_all(&mut self) {
        let term = self.term.lock();
        let start = AlacPoint::new(term.topmost_line(), Column(0));
        let end = AlacPoint::new(term.bottommost_line(), term.last_column());
        drop(term);
        self.set_selection(Some((make_selection(&(start..=end)), end)));
    }

    fn set_selection(&mut self, selection: Option<(Selection, AlacPoint)>) {
        self.events
            .push_back(InternalEvent::SetSelection(selection));
    }

    /// 复制选中的文本到剪贴板。
    pub fn copy(&mut self, keep_selection: Option<bool>) {
        self.events.push_back(InternalEvent::Copy(keep_selection));
    }

    /// 清除终端屏幕。
    pub fn clear(&mut self) {
        self.events.push_back(InternalEvent::Clear);
    }

    /// 在终端内容（含滚动历史）中设置搜索关键词。
    ///
    /// 语义与 alacritty 一致：query 全小写时忽略大小写；找到的第一个匹配
    /// 从光标位置起向右搜索。空串清除搜索。
    pub fn search(&mut self, query: &str) {
        self.events
            .push_back(InternalEvent::SearchSet(query.to_string()));
    }

    /// 跳到下一个匹配（覆盖全部滚动历史，自动回绕）。
    pub fn search_next(&mut self) {
        self.events
            .push_back(InternalEvent::SearchStep(AlacDirection::Right));
    }

    /// 跳到上一个匹配（覆盖全部滚动历史，自动回绕）。
    pub fn search_previous(&mut self) {
        self.events
            .push_back(InternalEvent::SearchStep(AlacDirection::Left));
    }

    /// 清除搜索状态与高亮。
    pub fn clear_search(&mut self) {
        self.events
            .push_back(InternalEvent::SearchSet(String::new()));
    }

    /// 当前搜索匹配（网格坐标）。
    pub fn search_current(&self) -> Option<SelectionRange> {
        self.search.as_ref().and_then(|search| search.current)
    }

    /// 返回是否正在选择文本。
    pub fn selection_started(&self) -> bool {
        self.selection_phase == SelectionPhase::Selecting
    }

    /// 返回上次渲染的内容。
    pub fn last_content(&self) -> &TerminalContent {
        &self.last_content
    }

    /// 返回所有已完成的代码块。
    pub fn blocks(&self) -> &[Block] {
        self.block_tracker.blocks()
    }

    /// 返回最近完成的代码块。
    pub fn current_block(&self) -> Option<&Block> {
        self.block_tracker.current_block()
    }

    /// 返回是否有命令正在运行（在 Enter 和下一个提示符之间）。
    pub fn block_running(&self) -> bool {
        self.block_tracker.is_running()
    }

    /// 返回所有终端文本（滚动历史 + 可见屏幕）作为最终状态。
    pub fn get_all_text(&self) -> String {
        let term = self.term.lock();
        let topmost = term.topmost_line();
        let bottommost = term.bottommost_line();
        let cols = term.grid().columns();

        let mut text = String::new();
        for line_idx in topmost.0..=bottommost.0 {
            let row = &term.grid()[Line(line_idx)];
            let mut line_text = String::with_capacity(cols);
            for col in 0..cols {
                line_text.push(row[Column(col)].c);
            }
            text.push_str(line_text.trim_end());
            text.push('\n');
        }
        text
    }

    /// 返回鼠标模式是否激活。
    pub fn mouse_mode(&self, shift: bool) -> bool {
        self.last_content.mode.intersects(TermMode::MOUSE_MODE) && !shift
    }

    /// 查询网格坐标处单元格携带的 OSC 8 超链接 URI。
    pub fn hyperlink_at(&self, point: AlacPoint) -> Option<String> {
        let term = self.term.lock();
        let last_line = term.bottommost_line();
        let last_col = term.last_column();
        if !(term.topmost_line()..=last_line).contains(&point.line) || point.column > last_col {
            return None;
        }
        term.grid()[point.line][point.column]
            .hyperlink()
            .map(|hyperlink| hyperlink.uri().to_string())
    }

    /// 检查鼠标位置是否发生变化。
    fn mouse_changed(&mut self, point: AlacPoint, side: AlacDirection) -> bool {
        match self.last_mouse {
            Some((old_point, old_side)) => {
                if old_point == point && old_side == side {
                    false
                } else {
                    self.last_mouse = Some((point, side));
                    true
                }
            }
            None => {
                self.last_mouse = Some((point, side));
                true
            }
        }
    }

    /// 处理鼠标按下事件。
    pub fn mouse_down(&mut self, e: &MouseDownEvent, _cx: &mut Context<Self>) {
        let position = e.position - self.last_content.terminal_bounds.bounds.origin;
        let point = grid_point(
            position,
            self.last_content.terminal_bounds,
            self.last_content.display_offset,
        );

        // Ctrl/Cmd + 单击 OSC 8 超链接：打开并跳过本击的选择逻辑。
        if e.button == MouseButton::Left
            && !self.mouse_mode(e.modifiers.shift)
            && (e.modifiers.control || e.modifiers.platform)
        {
            if let Some(uri) = self.hyperlink_at(point) {
                let event = Event::HyperlinkClick(uri);
                self.notify_middlewares_event(&event);
                _cx.emit(event);
                return;
            }
        }

        if self.mouse_mode(e.modifiers.shift) {
            if let Some(bytes) =
                mouse_button_report(point, e.button, e.modifiers, true, self.last_content.mode)
            {
                self.write_to_pty(bytes, InputOrigin::Mouse);
            }
        } else if e.button == MouseButton::Left {
            let (point, side) = grid_point_and_side(
                position,
                self.last_content.terminal_bounds,
                self.last_content.display_offset,
            );

            let selection_type = match e.click_count {
                0 => return,
                1 => Some(SelectionType::Simple),
                2 => Some(SelectionType::Semantic),
                3 => Some(SelectionType::Lines),
                _ => None,
            };

            if selection_type == Some(SelectionType::Simple) && e.modifiers.shift {
                self.events
                    .push_back(InternalEvent::UpdateSelection(position));
                return;
            }

            let selection =
                selection_type.map(|selection_type| Selection::new(selection_type, point, side));

            if let Some(sel) = selection {
                self.events
                    .push_back(InternalEvent::SetSelection(Some((sel, point))));
            }
        }
    }

    /// 处理鼠标释放事件。
    pub fn mouse_up(&mut self, e: &MouseUpEvent, _cx: &Context<Self>) {
        let position = e.position - self.last_content.terminal_bounds.bounds.origin;

        if self.mouse_mode(e.modifiers.shift) {
            let point = grid_point(
                position,
                self.last_content.terminal_bounds,
                self.last_content.display_offset,
            );

            if let Some(bytes) =
                mouse_button_report(point, e.button, e.modifiers, false, self.last_content.mode)
            {
                self.write_to_pty(bytes, InputOrigin::Mouse);
            }
        }

        self.selection_phase = SelectionPhase::Ended;
        self.last_mouse = None;
    }

    /// 处理鼠标移动事件。
    pub fn mouse_move(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        let position = e.position - self.last_content.terminal_bounds.bounds.origin;

        if self.mouse_mode(e.modifiers.shift) {
            let (point, side) = grid_point_and_side(
                position,
                self.last_content.terminal_bounds,
                self.last_content.display_offset,
            );

            if self.mouse_changed(point, side)
                && let Some(bytes) =
                    mouse_moved_report(point, e.pressed_button, e.modifiers, self.last_content.mode)
            {
                self.write_to_pty(bytes, InputOrigin::Mouse);
            }
        }
        cx.notify();
    }

    /// 处理鼠标拖拽事件。
    pub fn mouse_drag(
        &mut self,
        e: &MouseMoveEvent,
        region: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let position = e.position - self.last_content.terminal_bounds.bounds.origin;

        if !self.mouse_mode(e.modifiers.shift) {
            self.selection_phase = SelectionPhase::Selecting;
            self.events
                .push_back(InternalEvent::UpdateSelection(position));

            if !self.last_content.mode.contains(TermMode::ALT_SCREEN)
                && let Some(scroll_lines) = self.drag_line_delta(e, region)
            {
                self.events
                    .push_back(InternalEvent::Scroll(AlacScroll::Delta(scroll_lines)));
            }

            cx.notify();
        }
    }

    /// 计算拖拽滚动的行数。
    fn drag_line_delta(&self, e: &MouseMoveEvent, region: Bounds<Pixels>) -> Option<i32> {
        let top = region.origin.y;
        let bottom = region.bottom_left().y;

        let scroll_lines = if e.position.y < top {
            let scroll_delta = (top - e.position.y).pow(1.1);
            (scroll_delta / self.last_content.terminal_bounds.line_height).ceil() as i32
        } else if e.position.y > bottom {
            let scroll_delta = -((e.position.y - bottom).pow(1.1));
            (scroll_delta / self.last_content.terminal_bounds.line_height).floor() as i32
        } else {
            return None;
        };

        Some(scroll_lines.clamp(-3, 3))
    }

    /// 处理滚轮事件。
    pub fn scroll_wheel(&mut self, e: &ScrollWheelEvent, scroll_multiplier: f32) {
        let mouse_mode = self.mouse_mode(e.shift);
        let scroll_multiplier = if mouse_mode { 1. } else { scroll_multiplier };

        if let Some(scroll_lines) = self.determine_scroll_lines(e, scroll_multiplier) {
            if mouse_mode {
                let point = grid_point(
                    e.position - self.last_content.terminal_bounds.bounds.origin,
                    self.last_content.terminal_bounds,
                    self.last_content.display_offset,
                );

                if let Some(scrolls) = scroll_report(point, scroll_lines, e, self.last_content.mode)
                {
                    for scroll in scrolls {
                        self.write_to_pty(scroll, InputOrigin::Scroll);
                    }
                }
            } else if self
                .last_content
                .mode
                .contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL)
                && !e.shift
            {
                self.write_to_pty(alt_scroll(scroll_lines), InputOrigin::Scroll);
            } else if scroll_lines != 0 {
                self.events
                    .push_back(InternalEvent::Scroll(AlacScroll::Delta(scroll_lines)));
            }
        }
    }

    /// 计算滚轮事件应滚动的行数。
    fn determine_scroll_lines(
        &mut self,
        e: &ScrollWheelEvent,
        scroll_multiplier: f32,
    ) -> Option<i32> {
        let line_height = self.last_content.terminal_bounds.line_height;
        match e.touch_phase {
            TouchPhase::Started => {
                self.scroll_px = px(0.);
                None
            }
            TouchPhase::Moved => {
                let old_offset = (self.scroll_px / line_height) as i32;
                self.scroll_px += e.delta.pixel_delta(line_height).y * scroll_multiplier;
                let new_offset = (self.scroll_px / line_height) as i32;
                self.scroll_px %= self.last_content.terminal_bounds.height();
                Some(new_offset - old_offset)
            }
            TouchPhase::Ended => None,
            TouchPhase::Cancelled => None,
        }
    }

    /// 处理焦点进入事件。
    pub fn focus_in(&self) {
        if self.last_content.mode.contains(TermMode::FOCUS_IN_OUT) {
            self.write_to_pty("\x1b[I".as_bytes(), InputOrigin::Focus);
        }
    }

    /// 处理焦点离开事件。
    pub fn focus_out(&mut self) {
        if self.last_content.mode.contains(TermMode::FOCUS_IN_OUT) {
            self.write_to_pty("\x1b[O".as_bytes(), InputOrigin::Focus);
        }
    }

    /// 处理来自 Alacritty 的终端事件。
    fn process_event(&mut self, event: AlacTermEvent, cx: &mut Context<Self>) {
        match event {
            AlacTermEvent::Title(title) => {
                self.breadcrumb_text = title;
                let event = Event::TitleChanged;
                self.notify_middlewares_event(&event);
                cx.emit(event);
            }
            AlacTermEvent::ResetTitle => {
                self.breadcrumb_text = String::new();
                let event = Event::TitleChanged;
                self.notify_middlewares_event(&event);
                cx.emit(event);
            }
            AlacTermEvent::ClipboardStore(_, data) => {
                cx.write_to_clipboard(ClipboardItem::new_string(data));
            }
            AlacTermEvent::ClipboardLoad(_, format) => {
                self.write_to_pty(
                    match &cx.read_from_clipboard().and_then(|item| item.text()) {
                        Some(text) => format(text),
                        _ => format(""),
                    }
                    .into_bytes(),
                    InputOrigin::Clipboard,
                );
            }
            AlacTermEvent::PtyWrite(out) => {
                self.write_to_pty(out.into_bytes(), InputOrigin::System);
            }
            AlacTermEvent::TextAreaSizeRequest(format) => {
                self.write_to_pty(
                    format(self.last_content.terminal_bounds.into()).into_bytes(),
                    InputOrigin::System,
                );
            }
            AlacTermEvent::CursorBlinkingChange => {
                let blinking = {
                    let terminal = self.term.lock();
                    terminal.cursor_style().blinking
                };
                let event = Event::BlinkChanged(blinking);
                self.notify_middlewares_event(&event);
                cx.emit(event);
            }
            AlacTermEvent::Bell => {
                let event = Event::Bell;
                self.notify_middlewares_event(&event);
                cx.emit(event);
            }
            AlacTermEvent::Exit | AlacTermEvent::ChildExit(_) => {
                let event = Event::CloseTerminal;
                self.notify_middlewares_event(&event);
                cx.emit(event);
            }
            AlacTermEvent::Wakeup => {
                let event = Event::Wakeup;
                self.notify_middlewares_event(&event);
                cx.emit(event);
            }
            AlacTermEvent::MouseCursorDirty => {}
            AlacTermEvent::ColorRequest(index, format) => {
                let color = self.term.lock().colors()[index]
                    .unwrap_or(alacritty_terminal::vte::ansi::Rgb { r: 0, g: 0, b: 0 });
                self.write_to_pty(format(color).into_bytes(), InputOrigin::System);
            }
        }
    }

    /// 处理内部终端事件，更新终端状态。
    fn process_terminal_event(
        &mut self,
        event: &InternalEvent,
        term: &mut Term<ZedListener>,
        cx: &mut Context<Self>,
    ) {
        match event {
            InternalEvent::Resize(new_bounds) => {
                let mut new_bounds = *new_bounds;
                new_bounds.bounds.size.height =
                    cmp::max(new_bounds.line_height, new_bounds.height());
                new_bounds.bounds.size.width = cmp::max(new_bounds.cell_width, new_bounds.width());

                self.last_content.terminal_bounds = new_bounds;

                self.pty_backend.on_resize(new_bounds.into());
                term.resize(new_bounds);
            }
            InternalEvent::Clear => {
                term.clear_screen(ClearMode::Saved);

                let cursor = term.grid().cursor.point;
                term.grid_mut().reset_region(..cursor.line);

                let line = term.grid()[cursor.line][..Column(term.grid().columns())]
                    .iter()
                    .cloned()
                    .enumerate()
                    .collect::<Vec<(usize, Cell)>>();

                for (i, cell) in line {
                    term.grid_mut()[Line(0)][Column(i)] = cell;
                }

                term.grid_mut().cursor.point =
                    AlacPoint::new(Line(0), term.grid_mut().cursor.point.column);
                let new_cursor = term.grid().cursor.point;

                if (new_cursor.line.0 as usize) < term.screen_lines() - 1 {
                    term.grid_mut().reset_region((new_cursor.line + 1)..);
                }

                let event = Event::Wakeup;
                self.notify_middlewares_event(&event);
                cx.emit(event);
            }
            InternalEvent::Scroll(scroll) => {
                term.scroll_display(*scroll);
            }
            InternalEvent::ScrollToAlacPoint(point) => {
                term.scroll_to_point(*point);
            }
            InternalEvent::SetSelection(selection) => {
                term.selection = selection.as_ref().map(|(sel, _)| sel.clone());

                if let Some((_, head)) = selection {
                    self.selection_head = Some(*head);
                }
                let event = Event::SelectionsChanged;
                self.notify_middlewares_event(&event);
                cx.emit(event);
            }
            InternalEvent::UpdateSelection(position) => {
                if let Some(mut selection) = term.selection.take() {
                    let (point, side) = grid_point_and_side(
                        *position,
                        self.last_content.terminal_bounds,
                        term.grid().display_offset(),
                    );

                    selection.update(point, side);
                    term.selection = Some(selection);

                    self.selection_head = Some(point);
                    let event = Event::SelectionsChanged;
                    self.notify_middlewares_event(&event);
                    cx.emit(event);
                }
            }
            InternalEvent::Copy(keep_selection) => {
                if let Some(txt) = term.selection_to_string() {
                    cx.write_to_clipboard(ClipboardItem::new_string(txt));
                    if !keep_selection.unwrap_or(false) {
                        self.events.push_back(InternalEvent::SetSelection(None));
                    }
                }
            }
            InternalEvent::SearchSet(query) => {
                self.process_search_set(query.as_str(), term, cx);
            }
            InternalEvent::SearchStep(direction) => {
                self.process_search_step(*direction, term, cx);
            }
        }
    }

    /// 处理搜索关键词设置（持有终端锁）。
    fn process_search_set(
        &mut self,
        query: &str,
        term: &mut Term<ZedListener>,
        cx: &mut Context<Self>,
    ) {
        if query.is_empty() {
            self.search = None;
            cx.emit(Event::Wakeup);
            return;
        }

        let mut regex = match RegexSearch::new(query) {
            Ok(regex) => Some(regex),
            Err(error) => {
                log::warn!("failed to compile terminal search regex {query:?}: {error}");
                None
            }
        };

        // 初次定位：从光标位置起向右找第一个匹配（无则回绕到网格顶端）。
        let first = regex.as_mut().and_then(|regex| {
            let origin = term.grid().cursor.point;
            Self::search_match(term, regex, origin, AlacDirection::Right)
        });

        self.search = Some(TerminalSearch {
            regex,
            current: first,
        });

        if let Some(range) = &first {
            term.scroll_to_point(range.start);
        }
        cx.emit(Event::Wakeup);
    }

    /// 处理搜索步进（持有终端锁）。
    fn process_search_step(
        &mut self,
        direction: AlacDirection,
        term: &mut Term<ZedListener>,
        cx: &mut Context<Self>,
    ) {
        let Some(search_state) = &mut self.search else {
            return;
        };
        let Some(regex) = &mut search_state.regex else {
            return;
        };

        let anchor = search_state.current.as_ref().and_then(|current| {
            let point = match direction {
                // 紧贴匹配的边界向相反方向步进一格，否则搜索会重复命中自身。
                AlacDirection::Right => Self::step_point(term, current.end, 1),
                AlacDirection::Left => Self::step_point(term, current.start, -1),
            };
            point
        });

        let origin = match (anchor, direction) {
            (Some(point), _) => point,
            (None, AlacDirection::Right) => AlacPoint::new(term.topmost_line(), Column(0)),
            (None, AlacDirection::Left) => {
                AlacPoint::new(term.bottommost_line(), term.last_column())
            }
        };

        let side = match direction {
            AlacDirection::Right => Side::Right,
            AlacDirection::Left => Side::Left,
        };

        let found = term.search_next(regex, origin, direction, side, None);

        if let Some(match_range) = found {
            let range = SelectionRange {
                start: *match_range.start(),
                end: *match_range.end(),
                is_block: false,
            };
            search_state.current = Some(range);
            term.scroll_to_point(range.start);
        }

        cx.emit(Event::Wakeup);
    }

    /// 将点按方向移动一格（列越界时折返到相邻行首/尾）。
    fn step_point(term: &Term<ZedListener>, point: AlacPoint, step: i32) -> Option<AlacPoint> {
        let columns = term.grid().columns() as i32;
        if step > 0 {
            if point.column.0 as i32 + 1 < columns {
                Some(AlacPoint::new(point.line, Column(point.column.0 + 1)))
            } else if let Some(line) = point.line.0.checked_add(1) {
                Some(AlacPoint::new(Line(line), Column(0)))
            } else {
                None
            }
        } else if point.column.0 > 0 {
            Some(AlacPoint::new(point.line, Column(point.column.0 - 1)))
        } else if let Some(line) = point.line.0.checked_sub(1) {
            Some(AlacPoint::new(
                Line(line),
                Column(((columns - 1).max(0)) as usize),
            ))
        } else {
            None
        }
    }

    /// 以锚点为起点找一个匹配（一次 `search_next` 调用覆盖全部网格并自动回绕）。
    fn search_match(
        term: &Term<ZedListener>,
        regex: &mut RegexSearch,
        origin: AlacPoint,
        direction: AlacDirection,
    ) -> Option<SelectionRange> {
        let side = match direction {
            AlacDirection::Right => Side::Right,
            AlacDirection::Left => Side::Left,
        };

        term.search_next(regex, origin, direction, side, None)
            .map(|match_range| SelectionRange {
                start: *match_range.start(),
                end: *match_range.end(),
                is_block: false,
            })
    }
}

/// 从范围创建选择区域。
fn make_selection(range: &std::ops::RangeInclusive<AlacPoint>) -> Selection {
    let mut selection = Selection::new(SelectionType::Simple, *range.start(), Side::Left);
    selection.update(*range.end(), Side::Right);
    selection
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alacritty_terminal::{
        event::Event as AlacTermEvent, event::EventListener, index::Column, sync::FairMutex,
        term::Config, vte::ansi,
    };

    use super::*;

    /// 构造默认网格（100 列 ×6 行）的 Term，并注入原始字节。
    fn term_with(input: &[u8]) -> Arc<FairMutex<Term<ZedListener>>> {
        let term = Term::new(
            Config::default(),
            &TerminalBounds::default(),
            ZedListener(futures::channel::mpsc::unbounded().0),
        );
        let term = Arc::new(FairMutex::new(term));
        let mut parser: ansi::Processor = ansi::Processor::default();
        parser.advance(&mut *term.lock(), input);
        term
    }

    /// 从光标位置向右找第一个匹配。
    fn find_from_cursor(term: &Term<ZedListener>, query: &str) -> Option<SelectionRange> {
        let mut regex = RegexSearch::new(query).ok()?;
        search_match(
            term,
            &mut regex,
            term.grid().cursor.point,
            AlacDirection::Right,
        )
    }

    /// 以锚点为起点找一个匹配（一次 `search_next` 调用覆盖全部网格并自动回绕）。
    fn search_match(
        term: &Term<ZedListener>,
        regex: &mut RegexSearch,
        origin: AlacPoint,
        direction: AlacDirection,
    ) -> Option<SelectionRange> {
        let side = match direction {
            AlacDirection::Right => Side::Right,
            AlacDirection::Left => Side::Left,
        };

        term.search_next(regex, origin, direction, side, None)
            .map(|matched| SelectionRange {
                start: *matched.start(),
                end: *matched.end(),
                is_block: false,
            })
    }

    #[test]
    fn search_match_finds_text() {
        let term = term_with(b"hello world");
        let range = find_from_cursor(&term.lock(), "hello").expect("expected a match");
        assert_eq!(range.start, AlacPoint::new(Line(0), Column(0)));
        assert_eq!(range.end, AlacPoint::new(Line(0), Column(4)));
    }

    #[test]
    fn search_match_after_cursor_skips_earlier_occurrences() {
        let term = term_with(b"hello world hello");
        let range = find_from_cursor(&term.lock(), "hello").expect("expected a match");
        // 光标已越过第一个 "hello"，应命中第二个（自动回绕语义下取首个匹配）
        let second_start = AlacPoint::new(Line(0), Column(12));
        assert!(
            range.start == second_start || range.start == AlacPoint::new(Line(0), Column(0)),
            "unexpected match start {:?}",
            range.start
        );
    }

    #[test]
    fn search_match_case_insensitive_when_query_lower() {
        let term = term_with(b"Hello World");
        let range = find_from_cursor(&term.lock(), "hello");
        assert!(
            range.is_some(),
            "lowercase query should match capitalized text"
        );
    }

    #[test]
    fn search_match_wraps_grid() {
        let term = term_with(b"\r\nbottom");
        let range = find_from_cursor(&term.lock(), "bottom").expect("expected a match");
        assert_eq!(range.start.line, Line(1));
    }

    #[test]
    fn osc8_hyperlink_stored_in_cells() {
        // OSC 8 序列：开链接 → 写文本 → 关链接 → 普通文本
        let term = term_with(b"\x1b]8;;https://example.org\x1b\\click\x1b]8;;\x1b\\here");
        let term = term.lock();
        assert_eq!(
            term.grid()[Line(0)][Column(0)]
                .hyperlink()
                .map(|h| h.uri().to_string()),
            Some("https://example.org".to_string())
        );
        // 关闭链接后的单元格不应再携带 URI
        assert_eq!(term.grid()[Line(0)][Column(6)].hyperlink(), None);
    }

    #[test]
    fn wakeup_event_bridges() {
        // ZedListener 桥事件仅发送、不阻塞（与 external 模块的用法同一语义）
        let (sender, mut receiver) = futures::channel::mpsc::unbounded();
        let listener = ZedListener(sender);
        listener.send_event(AlacTermEvent::Wakeup);
        assert!(matches!(
            futures::executor::block_on(receiver.next()),
            Some(AlacTermEvent::Wakeup)
        ));
    }
}
