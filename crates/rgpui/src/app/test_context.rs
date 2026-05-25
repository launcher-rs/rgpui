use crate::{
    Action, AnyView, AnyWindowHandle, App, AppCell, AppContext, AsyncApp, AvailableSpace,
    BackgroundExecutor, BorrowAppContext, Bounds, Capslock, ClipboardItem, DrawPhase, Drawable,
    Element, Empty, EntityId, EventEmitter, ForegroundExecutor, Global, InputEvent, Keystroke,
    Modifiers, ModifiersChangedEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, Platform, Point, Render, Result, Size, Task, TestDispatcher, TestPlatform,
    TestScreenCaptureSource, TestWindow, TextSystem, VisualContext, Window, WindowBounds,
    WindowHandle, WindowOptions, app::GpuiMode, window::ElementArenaScope,
};
use anyhow::{anyhow, bail};
use futures::{Stream, StreamExt, channel::oneshot};

use std::{
    cell::RefCell, future::Future, ops::Deref, path::PathBuf, rc::Rc, sync::Arc, time::Duration,
};

/// `TestAppContext` 提供给使用 `#[rgpui::test]` 创建的测试，它提供了
/// `Context` 的实现，并带有在测试中有用的额外方法。
#[derive(Clone)]
pub struct TestAppContext {
    #[doc(hidden)]
    pub background_executor: BackgroundExecutor,
    #[doc(hidden)]
    pub foreground_executor: ForegroundExecutor,
    #[doc(hidden)]
    pub dispatcher: TestDispatcher,
    test_platform: Rc<TestPlatform>,
    text_system: Arc<TextSystem>,
    fn_name: Option<&'static str>,
    on_quit: Rc<RefCell<Vec<Box<dyn FnOnce() + 'static>>>>,
    #[doc(hidden)]
    pub app: Rc<AppCell>,
}

impl AppContext for TestAppContext {
    fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T> {
        let mut app = self.app.borrow_mut();
        app.new(build_entity)
    }

    fn reserve_entity<T: 'static>(&mut self) -> crate::Reservation<T> {
        let mut app = self.app.borrow_mut();
        app.reserve_entity()
    }

    fn insert_entity<T: 'static>(
        &mut self,
        reservation: crate::Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T> {
        let mut app = self.app.borrow_mut();
        app.insert_entity(reservation, build_entity)
    }

    fn update_entity<T: 'static, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R {
        let mut app = self.app.borrow_mut();
        app.update_entity(handle, update)
    }

    fn as_mut<'a, T>(&'a mut self, _: &Entity<T>) -> super::GpuiBorrow<'a, T>
    where
        T: 'static,
    {
        panic!("Cannot use as_mut with a test app context. Try calling update() first")
    }

    fn read_entity<T, R>(&self, handle: &Entity<T>, read: impl FnOnce(&T, &App) -> R) -> R
    where
        T: 'static,
    {
        let app = self.app.borrow();
        app.read_entity(handle, read)
    }

    fn update_window<T, F>(&mut self, window: AnyWindowHandle, f: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        let mut lock = self.app.borrow_mut();
        lock.update_window(window, f)
    }

    fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R> {
        let mut lock = self.app.borrow_mut();
        lock.with_window(entity_id, f)
    }

    fn read_window<T, R>(
        &self,
        window: &WindowHandle<T>,
        read: impl FnOnce(Entity<T>, &App) -> R,
    ) -> Result<R>
    where
        T: 'static,
    {
        let app = self.app.borrow();
        app.read_window(window, read)
    }

    fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static,
    {
        self.background_executor.spawn(future)
    }

    fn read_global<G, R>(&self, callback: impl FnOnce(&G, &App) -> R) -> R
    where
        G: Global,
    {
        let app = self.app.borrow();
        app.read_global(callback)
    }
}

impl TestAppContext {
    /// Creates a new `TestAppContext`. Usually you can rely on `#[rgpui::test]` to do this for you.
    pub fn build(dispatcher: TestDispatcher, fn_name: Option<&'static str>) -> Self {
        let arc_dispatcher = Arc::new(dispatcher.clone());
        let background_executor = BackgroundExecutor::new(arc_dispatcher.clone());
        let foreground_executor = ForegroundExecutor::new(arc_dispatcher);
        let platform = TestPlatform::new(background_executor.clone(), foreground_executor.clone());
        let asset_source = Arc::new(());
        #[cfg(feature = "test-support")]
        let http_client = crate::http_client::FakeHttpClient::with_404_response();
        #[cfg(not(feature = "test-support"))]
        let http_client = Arc::new(crate::http_client::BlockedHttpClient::new()) as Arc<dyn crate::http_client::HttpClient>;
        let text_system = Arc::new(TextSystem::new(platform.text_system()));

        let app = App::new_app(platform.clone(), asset_source, http_client);
        app.borrow_mut().mode = GpuiMode::test();

        Self {
            app,
            background_executor,
            foreground_executor,
            dispatcher,
            test_platform: platform,
            text_system,
            fn_name,
            on_quit: Rc::new(RefCell::new(Vec::default())),
        }
    }

    /// 跳过此测试期间的所有绘制操作。
    pub fn skip_drawing(&mut self) {
        self.app.borrow_mut().mode = GpuiMode::Test { skip_drawing: true };
    }

    /// 创建单个 TestAppContext，用于非多客户端测试
    pub fn single() -> Self {
        let dispatcher = TestDispatcher::new(0);
        Self::build(dispatcher, None)
    }

    /// 创建此 `TestAppContext` 的测试函数名
    pub fn test_function_name(&self) -> Option<&'static str> {
        self.fn_name
    }

    /// 检查平台是否收到任何新的路径提示。
    pub fn did_prompt_for_new_path(&self) -> bool {
        self.test_platform.did_prompt_for_new_path()
    }

    /// 返回一个新的 `TestAppContext`，重用相同的执行器以交错任务。
    pub fn new_app(&self) -> TestAppContext {
        Self::build(self.dispatcher.clone(), self.fn_name)
    }

    /// 由测试助手调用以结束测试。
    /// 公开以便宏可以调用。
    pub fn quit(&self) {
        self.on_quit.borrow_mut().drain(..).for_each(|f| f());
        self.app.borrow_mut().shutdown();
    }

    /// 注册清理操作以在测试结束时运行。
    pub fn on_quit(&mut self, f: impl FnOnce() + 'static) {
        self.on_quit.borrow_mut().push(Box::new(f));
    }

    /// Schedules all windows to be redrawn on the next effect cycle.
    pub fn refresh(&mut self) -> Result<()> {
        let mut app = self.app.borrow_mut();
        app.refresh_windows();
        Ok(())
    }

    /// 返回执行器（用于在后台运行任务）
    pub fn executor(&self) -> BackgroundExecutor {
        self.background_executor.clone()
    }

    /// 返回执行器（用于在主线程上运行任务）
    pub fn foreground_executor(&self) -> &ForegroundExecutor {
        &self.foreground_executor
    }

    /// 在闭包期间提供 `&mut App`
    pub fn update<R>(&self, f: impl FnOnce(&mut App) -> R) -> R {
        let mut cx = self.app.borrow_mut();
        cx.update(f)
    }

    /// 在闭包期间提供 `&App`
    pub fn read<R>(&self, f: impl FnOnce(&App) -> R) -> R {
        let cx = self.app.borrow();
        f(&cx)
    }

    /// 添加新窗口。窗口将始终由 `TestWindow` 支持，
    /// 可以通过 `self.test_window(handle)` 检索
    pub fn add_window<F, V>(&mut self, build_window: F) -> WindowHandle<V>
    where
        F: FnOnce(&mut Window, &mut Context<V>) -> V,
        V: 'static + Render,
    {
        let mut cx = self.app.borrow_mut();

        // 某些测试依赖窗口大小匹配测试显示的边界
        let bounds = Bounds::maximized(None, &cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| build_window(window, cx)),
        )
        .unwrap()
    }

    /// Opens a new window with a specific size.
    ///
    /// Unlike `add_window` which uses maximized bounds, this allows controlling
    /// the window dimensions, which is important for layout-sensitive tests.
    pub fn open_window<F, V>(
        &mut self,
        window_size: Size<Pixels>,
        build_window: F,
    ) -> WindowHandle<V>
    where
        F: FnOnce(&mut Window, &mut Context<V>) -> V,
        V: 'static + Render,
    {
        let mut cx = self.app.borrow_mut();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: window_size,
                })),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| build_window(window, cx)),
        )
        .unwrap()
    }

    /// 添加没有内容的新窗口。
    pub fn add_empty_window(&mut self) -> &mut VisualTestContext {
        let mut cx = self.app.borrow_mut();
        let bounds = Bounds::maximized(None, &cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
            .unwrap();
        drop(cx);
        let cx = VisualTestContext::from_window(*window.deref(), self).into_mut();
        cx.run_until_parked();
        cx
    }

    /// 添加新窗口，并返回其根视图和 `VisualTestContext`，
    /// 可在测试的其余部分用作 `Window` 和 `App`。通常你会将此上下文阴影化为
    /// 返回的上下文。`let (view, cx) = cx.add_window_view(...);`
    pub fn add_window_view<F, V>(
        &mut self,
        build_root_view: F,
    ) -> (Entity<V>, &mut VisualTestContext)
    where
        F: FnOnce(&mut Window, &mut Context<V>) -> V,
        V: 'static + Render,
    {
        let mut cx = self.app.borrow_mut();
        let bounds = Bounds::maximized(None, &cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| build_root_view(window, cx)),
            )
            .unwrap();
        drop(cx);
        let view = window.root(self).unwrap();
        let cx = VisualTestContext::from_window(*window.deref(), self).into_mut();
        cx.run_until_parked();

        // 可能值得尝试在每个测试结束时清理这些。
        (view, cx)
    }

    /// 返回 TextSystem
    pub fn text_system(&self) -> &Arc<TextSystem> {
        &self.text_system
    }

    /// 模拟写入平台剪贴板
    pub fn write_to_clipboard(&self, item: ClipboardItem) {
        self.test_platform.write_to_clipboard(item)
    }

    /// 模拟从平台剪贴板读取。
    /// 这将返回 `write_to_clipboard` 的最新值。
    pub fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.test_platform.read_from_clipboard()
    }

    /// 模拟在平台的"打开"对话框中选择文件。
    pub fn simulate_new_path_selection(
        &self,
        select_path: impl FnOnce(&std::path::Path) -> Option<std::path::PathBuf>,
    ) {
        self.test_platform.simulate_new_path_selection(select_path);
    }

    /// 模拟在平台级警报对话框中点击按钮。
    #[track_caller]
    pub fn simulate_prompt_answer(&self, button: &str) {
        self.test_platform.simulate_prompt_answer(button);
    }

    /// 如果存在警报对话框则返回 true。
    pub fn has_pending_prompt(&self) -> bool {
        self.test_platform.has_pending_prompt()
    }

    /// 如果存在警报对话框则返回 true。
    pub fn pending_prompt(&self) -> Option<(String, String)> {
        self.test_platform.pending_prompt()
    }

    /// 此测试期间使用 cx.open_url() 打开的所有 URL。
    pub fn opened_url(&self) -> Option<String> {
        self.test_platform.opened_url.borrow().clone()
    }

    /// 模拟用户将窗口调整到新大小。
    pub fn simulate_window_resize(&self, window_handle: AnyWindowHandle, size: Size<Pixels>) {
        self.test_window(window_handle).simulate_resize(size);
    }

    /// 如果存在警报对话框则返回 true。
    pub fn expect_restart(&self) -> oneshot::Receiver<Option<PathBuf>> {
        let (tx, rx) = futures::channel::oneshot::channel();
        self.test_platform.expect_restart.borrow_mut().replace(tx);
        rx
    }

    /// 如果应用程序查询屏幕捕获源，则使给定源被返回。
    pub fn set_screen_capture_sources(&self, sources: Vec<TestScreenCaptureSource>) {
        self.test_platform.set_screen_capture_sources(sources);
    }

    /// 返回测试中打开的所有窗口。
    pub fn windows(&self) -> Vec<AnyWindowHandle> {
        self.app.borrow().windows()
    }

    /// 在主线程上运行给定任务。
    #[track_caller]
    pub fn spawn<Fut, R>(&self, f: impl FnOnce(AsyncApp) -> Fut) -> Task<R>
    where
        Fut: Future<Output = R> + 'static,
        R: 'static,
    {
        self.foreground_executor.spawn(f(self.to_async()))
    }

    /// 如果给定全局存在则返回 true
    pub fn has_global<G: Global>(&self) -> bool {
        let app = self.app.borrow();
        app.has_global::<G>()
    }

    /// 使用全局的引用运行给定闭包
    /// 如果 `has_global` 返回 false 则 panic。
    pub fn read_global<G: Global, R>(&self, read: impl FnOnce(&G, &App) -> R) -> R {
        let app = self.app.borrow();
        read(app.global(), &app)
    }

    /// 使用全局的引用运行给定闭包（如果已设置）
    pub fn try_read_global<G: Global, R>(&self, read: impl FnOnce(&G, &App) -> R) -> Option<R> {
        let lock = self.app.borrow();
        Some(read(lock.try_global()?, &lock))
    }

    /// 在此上下文中设置全局。
    pub fn set_global<G: Global>(&mut self, global: G) {
        let mut lock = self.app.borrow_mut();
        lock.update(|cx| cx.set_global(global))
    }

    /// 更新此上下文中的全局。（如果 `has_global` 返回 false 则 panic）
    pub fn update_global<G: Global, R>(&mut self, update: impl FnOnce(&mut G, &mut App) -> R) -> R {
        let mut lock = self.app.borrow_mut();
        lock.update(|cx| cx.update_global(update))
    }

    /// 返回 `AsyncApp`，可用于在测试中当前线程运行期望在后台线程的任务。
    pub fn to_async(&self) -> AsyncApp {
        AsyncApp {
            app: Rc::downgrade(&self.app),
            background_executor: self.background_executor.clone(),
            foreground_executor: self.foreground_executor.clone(),
        }
    }

    /// 等待直到没有更多待处理任务。
    pub fn run_until_parked(&self) {
        self.dispatcher.run_until_parked();
    }

    /// 模拟将动作分派到窗口中当前聚焦的节点。
    pub fn dispatch_action<A>(&mut self, window: AnyWindowHandle, action: A)
    where
        A: Action,
    {
        window
            .update(self, |_, window, cx| {
                window.dispatch_action(action.boxed_clone(), cx)
            })
            .unwrap();

        self.background_executor.run_until_parked()
    }

    /// simulate_keystrokes 接受以空格分隔的要输入的键列表。
    /// cx.simulate_keystrokes("cmd-shift-p b k s p enter")
    /// 在 Zed 中，这将通过命令面板在当前编辑器上运行退格。
    /// 这还会运行后台执行器直到它被挂起。
    pub fn simulate_keystrokes(&mut self, window: AnyWindowHandle, keystrokes: &str) {
        for keystroke in keystrokes
            .split(' ')
            .map(Keystroke::parse)
            .map(Result::unwrap)
        {
            self.dispatch_keystroke(window, keystroke);
        }

        self.background_executor.run_until_parked()
    }

    /// simulate_input 接受要输入的文本字符串。
    /// cx.simulate_input("abc")
    /// 将在当前编辑器中输入 abc
    /// 这还会运行后台执行器直到它被挂起。
    pub fn simulate_input(&mut self, window: AnyWindowHandle, input: &str) {
        for keystroke in input.split("").map(Keystroke::parse).map(Result::unwrap) {
            self.dispatch_keystroke(window, keystroke);
        }

        self.background_executor.run_until_parked()
    }

    /// 分派单个击键（另请参阅 `simulate_keystrokes` 和 `simulate_input`）
    pub fn dispatch_keystroke(&mut self, window: AnyWindowHandle, keystroke: Keystroke) {
        self.update_window(window, |_, window, cx| {
            window.dispatch_keystroke(keystroke, cx)
        })
        .unwrap();
    }

    /// 返回给定句柄背后的 `TestWindow`。
    pub(crate) fn test_window(&self, window: AnyWindowHandle) -> TestWindow {
        self.app
            .borrow_mut()
            .windows
            .get_mut(window.id)
            .unwrap()
            .as_deref_mut()
            .unwrap()
            .platform_window
            .as_test()
            .unwrap()
            .clone()
    }

    /// 返回实体每次更新时的通知流。
    pub fn notifications<T: 'static>(
        &mut self,
        entity: &Entity<T>,
    ) -> impl Stream<Item = ()> + use<T> {
        let (tx, rx) = futures::channel::mpsc::unbounded();
        self.update(|cx| {
            cx.observe(entity, {
                let tx = tx.clone();
                move |_, _| {
                    let _ = tx.unbounded_send(());
                }
            })
            .detach();
            cx.observe_release(entity, move |_, _| tx.close_channel())
                .detach()
        });
        rx
    }

    /// 返回给定实体发出的事件流。
    pub fn events<Evt, T: 'static + EventEmitter<Evt>>(
        &mut self,
        entity: &Entity<T>,
    ) -> futures::channel::mpsc::UnboundedReceiver<Evt>
    where
        Evt: 'static + Clone,
    {
        let (tx, rx) = futures::channel::mpsc::unbounded();
        entity
            .update(self, |_, cx: &mut Context<T>| {
                cx.subscribe(entity, move |_entity, _handle, event, _cx| {
                    let _ = tx.unbounded_send(event.clone());
                })
            })
            .detach();
        rx
    }

    /// 运行直到给定条件变为 true。（如果不需要在特定时间介入，优先使用 `run_until_parked`）。
    pub async fn condition<T: 'static>(
        &mut self,
        entity: &Entity<T>,
        mut predicate: impl FnMut(&mut T, &mut Context<T>) -> bool,
    ) {
        let timer = self.executor().timer(Duration::from_secs(3));
        let mut notifications = self.notifications(entity);

        use futures::FutureExt as _;
        use futures_concurrency::future::Race as _;

        (
            async {
                loop {
                    if entity.update(self, &mut predicate) {
                        return Ok(());
                    }

                    if notifications.next().await.is_none() {
                        bail!("entity dropped")
                    }
                }
            },
            timer.map(|_| Err(anyhow!("condition timed out"))),
        )
            .race()
            .await
            .unwrap();
    }

    /// 为此 App 设置名称。
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_name(&mut self, name: &'static str) {
        self.update(|cx| cx.name = Some(name))
    }
}

impl<T: 'static> Entity<T> {
    /// 阻塞直到实体发出下一个事件，然后返回它。
    pub fn next_event<Event>(&self, cx: &mut TestAppContext) -> impl Future<Output = Event>
    where
        Event: Send + Clone + 'static,
        T: EventEmitter<Event>,
    {
        let (tx, mut rx) = oneshot::channel();
        let mut tx = Some(tx);
        let subscription = self.update(cx, |_, cx| {
            cx.subscribe(self, move |_, _, event, _| {
                if let Some(tx) = tx.take() {
                    _ = tx.send(event.clone());
                }
            })
        });

        async move {
            let event = rx.await.expect("no event emitted");
            drop(subscription);
            event
        }
    }
}

impl<V: 'static> Entity<V> {
    /// 返回视图下次更新时解析的未来。
    pub fn next_notification(
        &self,
        advance_clock_by: Duration,
        cx: &TestAppContext,
    ) -> impl Future<Output = ()> {
        use postage::prelude::{Sink as _, Stream as _};

        let (mut tx, mut rx) = postage::mpsc::channel(1);
        let subscription = cx.app.borrow_mut().observe(self, move |_, _| {
            tx.try_send(()).ok();
        });

        cx.executor().advance_clock(advance_clock_by);

        async move {
            rx.recv()
                .await
                .expect("entity dropped while test was waiting for its next notification");
            drop(subscription);
        }
    }
}

impl<V> Entity<V> {
    /// 返回条件变为 true 时解析的未来。
    pub fn condition<Evt>(
        &self,
        cx: &TestAppContext,
        mut predicate: impl FnMut(&V, &App) -> bool,
    ) -> impl Future<Output = ()>
    where
        Evt: 'static,
        V: EventEmitter<Evt>,
    {
        use postage::prelude::{Sink as _, Stream as _};

        let (tx, mut rx) = postage::mpsc::channel(1024);

        let mut cx = cx.app.borrow_mut();
        let subscriptions = (
            cx.observe(self, {
                let mut tx = tx.clone();
                move |_, _| {
                    tx.blocking_send(()).ok();
                }
            }),
            cx.subscribe(self, {
                let mut tx = tx;
                move |_, _: &Evt, _| {
                    tx.blocking_send(()).ok();
                }
            }),
        );

        let cx = cx.this.upgrade().unwrap();
        let handle = self.downgrade();

        async move {
            loop {
                {
                    let cx = cx.borrow();
                    let cx = &*cx;
                    if predicate(
                        handle
                            .upgrade()
                            .expect("view dropped with pending condition")
                            .read(cx),
                        cx,
                    ) {
                        break;
                    }
                }

                rx.recv()
                    .await
                    .expect("view dropped with pending condition");
            }
            drop(subscriptions);
        }
    }
}

use derive_more::{Deref, DerefMut};

use super::{Context, Entity};
#[derive(Deref, DerefMut, Clone)]
/// VisualTestContext 是 `Window` 和 `App` 的测试等效物。它允许你
/// 运行特定于窗口的测试代码。它可以解引用为 `TestAppContext`。
pub struct VisualTestContext {
    #[deref]
    #[deref_mut]
    /// cx 是原始 TestAppContext（使用 Deref 可以更轻松地访问）
    pub cx: TestAppContext,
    window: AnyWindowHandle,
}

impl VisualTestContext {
    /// 在闭包期间提供 `Window` 和 `App`。
    pub fn update<R>(&mut self, f: impl FnOnce(&mut Window, &mut App) -> R) -> R {
        self.cx
            .update_window(self.window, |_, window, cx| f(window, cx))
            .unwrap()
    }

    /// 创建新的 VisualTestContext。你通常会将传入的
    /// TestAppContext 与此阴影化。
    /// `let cx = VisualTestContext::from_window(window, cx);`
    pub fn from_window(window: AnyWindowHandle, cx: &TestAppContext) -> Self {
        Self {
            cx: cx.clone(),
            window,
        }
    }

    /// 等待直到没有更多待处理任务。
    pub fn run_until_parked(&self) {
        self.cx.background_executor.run_until_parked();
    }

    /// 将动作分派到当前聚焦的节点。
    pub fn dispatch_action<A>(&mut self, action: A)
    where
        A: Action,
    {
        self.cx.dispatch_action(self.window, action)
    }

    /// 读取窗口标题（由 `Window#set_window_title` 设置）
    pub fn window_title(&mut self) -> Option<String> {
        self.cx.test_window(self.window).0.lock().title.clone()
    }

    /// 读取窗口的文档路径（由 `Window#set_document_path` 设置）
    pub fn document_path(&mut self) -> Option<std::path::PathBuf> {
        self.cx
            .test_window(self.window)
            .0
            .lock()
            .document_path
            .clone()
    }

    /// 模拟一系列击键 `cx.simulate_keystrokes("cmd-p escape")`
    /// 自动运行直到挂起。
    pub fn simulate_keystrokes(&mut self, keystrokes: &str) {
        self.cx.simulate_keystrokes(self.window, keystrokes)
    }

    /// 模拟输入文本 `cx.simulate_input("hello")`
    /// 自动运行直到挂起。
    pub fn simulate_input(&mut self, input: &str) {
        self.cx.simulate_input(self.window, input)
    }

    /// 模拟鼠标移动到给定点
    pub fn simulate_mouse_move(
        &mut self,
        position: Point<Pixels>,
        button: impl Into<Option<MouseButton>>,
        modifiers: Modifiers,
    ) {
        self.simulate_event(MouseMoveEvent {
            position,
            modifiers,
            pressed_button: button.into(),
        })
    }

    /// 模拟鼠标在给定点按下
    pub fn simulate_mouse_down(
        &mut self,
        position: Point<Pixels>,
        button: MouseButton,
        modifiers: Modifiers,
    ) {
        self.simulate_event(MouseDownEvent {
            position,
            modifiers,
            button,
            click_count: 1,
            first_mouse: false,
        })
    }

    /// 模拟鼠标在给定点释放
    pub fn simulate_mouse_up(
        &mut self,
        position: Point<Pixels>,
        button: MouseButton,
        modifiers: Modifiers,
    ) {
        self.simulate_event(MouseUpEvent {
            position,
            modifiers,
            button,
            click_count: 1,
        })
    }

    /// 模拟在给定点的主要鼠标点击
    pub fn simulate_click(&mut self, position: Point<Pixels>, modifiers: Modifiers) {
        self.simulate_event(MouseDownEvent {
            position,
            modifiers,
            button: MouseButton::Left,
            click_count: 1,
            first_mouse: false,
        });
        self.simulate_event(MouseUpEvent {
            position,
            modifiers,
            button: MouseButton::Left,
            click_count: 1,
        });
    }

    /// 模拟修饰键更改事件
    pub fn simulate_modifiers_change(&mut self, modifiers: Modifiers) {
        self.simulate_event(ModifiersChangedEvent {
            modifiers,
            capslock: Capslock { on: false },
        })
    }

    /// 模拟大小写锁定更改事件
    pub fn simulate_capslock_change(&mut self, on: bool) {
        self.simulate_event(ModifiersChangedEvent {
            modifiers: Modifiers::none(),
            capslock: Capslock { on },
        })
    }

    /// 模拟用户将窗口调整到新大小。
    pub fn simulate_resize(&self, size: Size<Pixels>) {
        self.simulate_window_resize(self.window, size)
    }

    /// debug_bounds 返回具有给定选择器的元素的边界。
    pub fn debug_bounds(&mut self, selector: &'static str) -> Option<Bounds<Pixels>> {
        self.update(|window, _| window.rendered_frame.debug_bounds.get(selector).copied())
    }

    /// 向窗口绘制元素。用于模拟事件或动作
    pub fn draw<E>(
        &mut self,
        origin: Point<Pixels>,
        space: impl Into<Size<AvailableSpace>>,
        f: impl FnOnce(&mut Window, &mut App) -> E,
    ) -> (E::RequestLayoutState, E::PrepaintState)
    where
        E: Element,
    {
        self.update(|window, cx| {
            let _arena_scope = ElementArenaScope::enter(&cx.element_arena);

            window.invalidator.set_phase(DrawPhase::Prepaint);
            let mut element = Drawable::new(f(window, cx));
            element.layout_as_root(space.into(), window, cx);
            window.with_absolute_element_offset(origin, |window| element.prepaint(window, cx));

            window.invalidator.set_phase(DrawPhase::Paint);
            let (request_layout_state, prepaint_state) = element.paint(window, cx);

            window.invalidator.set_phase(DrawPhase::None);
            window.refresh();

            drop(element);
            cx.element_arena.borrow_mut().clear();

            (request_layout_state, prepaint_state)
        })
    }

    /// 模拟来自平台的事件，例如 ScrollWheelEvent
    /// 确保你已先调用 [VisualTestContext::draw]！
    pub fn simulate_event<E: InputEvent>(&mut self, event: E) {
        self.test_window(self.window)
            .simulate_input(event.to_platform_input());
        self.background_executor.run_until_parked();
    }

    /// 模拟用户模糊窗口。
    pub fn deactivate_window(&mut self) {
        if Some(self.window) == self.test_platform.active_window() {
            self.test_platform.set_active_window(None)
        }
        self.background_executor.run_until_parked();
    }

    /// 模拟用户关闭窗口。
    /// 如果窗口已关闭则返回 true。
    pub fn simulate_close(&mut self) -> bool {
        let handler = self
            .cx
            .update_window(self.window, |_, window, _| {
                window
                    .platform_window
                    .as_test()
                    .unwrap()
                    .0
                    .lock()
                    .should_close_handler
                    .take()
            })
            .unwrap();
        if let Some(mut handler) = handler {
            let should_close = handler();
            self.cx
                .update_window(self.window, |_, window, _| {
                    window.platform_window.on_should_close(handler);
                })
                .unwrap();
            should_close
        } else {
            false
        }
    }

    /// 获取 &mut VisualTestContext（这 mostly 是你需要传递给其他方法的内容）。
    /// 此方法在内部将 VisualTestContext 保留到测试结束。
    pub fn into_mut(self) -> &'static mut Self {
        let ptr = Box::into_raw(Box::new(self));
        // 安全：on_quit 将在测试完成后调用。
        // 执行器将确保所有与测试相关的任务都已停止。
        // 因此 on_quit 调用后无法访问 cx。
        // 注意：这在堆叠借用（也可能是树借用）下是不健全的
        // 可变引用会使 `ptr` 失效，而 `ptr` 稍后在闭包中使用
        let cx = unsafe { &mut *ptr };
        cx.on_quit(move || unsafe {
            drop(Box::from_raw(ptr));
        });
        cx
    }
}

impl AppContext for VisualTestContext {
    fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T> {
        self.window
            .update(&mut self.cx, |_, _, cx| cx.new(build_entity))
            .expect("window was unexpectedly closed")
    }

    fn reserve_entity<T: 'static>(&mut self) -> crate::Reservation<T> {
        self.cx.reserve_entity()
    }

    fn insert_entity<T: 'static>(
        &mut self,
        reservation: crate::Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T> {
        self.window
            .update(&mut self.cx, |_, _, cx| {
                cx.insert_entity(reservation, build_entity)
            })
            .expect("window was unexpectedly closed")
    }

    fn update_entity<T, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R
    where
        T: 'static,
    {
        self.cx.update_entity(handle, update)
    }

    fn as_mut<'a, T>(&'a mut self, handle: &Entity<T>) -> super::GpuiBorrow<'a, T>
    where
        T: 'static,
    {
        self.cx.as_mut(handle)
    }

    fn read_entity<T, R>(&self, handle: &Entity<T>, read: impl FnOnce(&T, &App) -> R) -> R
    where
        T: 'static,
    {
        self.cx.read_entity(handle, read)
    }

    fn update_window<T, F>(&mut self, window: AnyWindowHandle, f: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        self.cx.update_window(window, f)
    }

    fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R> {
        self.cx.with_window(entity_id, f)
    }

    fn read_window<T, R>(
        &self,
        window: &WindowHandle<T>,
        read: impl FnOnce(Entity<T>, &App) -> R,
    ) -> Result<R>
    where
        T: 'static,
    {
        self.cx.read_window(window, read)
    }

    fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static,
    {
        self.cx.background_spawn(future)
    }

    fn read_global<G, R>(&self, callback: impl FnOnce(&G, &App) -> R) -> R
    where
        G: Global,
    {
        self.cx.read_global(callback)
    }
}

impl VisualContext for VisualTestContext {
    type Result<T> = T;

    /// Get the underlying window handle underlying this context.
    fn window_handle(&self) -> AnyWindowHandle {
        self.window
    }

    fn new_window_entity<T: 'static>(
        &mut self,
        build_entity: impl FnOnce(&mut Window, &mut Context<T>) -> T,
    ) -> Entity<T> {
        self.window
            .update(&mut self.cx, |_, window, cx| {
                cx.new(|cx| build_entity(window, cx))
            })
            .expect("window was unexpectedly closed")
    }

    fn update_window_entity<V: 'static, R>(
        &mut self,
        view: &Entity<V>,
        update: impl FnOnce(&mut V, &mut Window, &mut Context<V>) -> R,
    ) -> R {
        let view = view.clone();
        self.cx
            .app
            .borrow_mut()
            .with_window(view.entity_id(), |window, app| {
                view.update(app, |v, cx| update(v, window, cx))
            })
            .expect("entity has no current window; use `update` instead of `update_in`")
    }

    fn replace_root_view<V>(
        &mut self,
        build_view: impl FnOnce(&mut Window, &mut Context<V>) -> V,
    ) -> Entity<V>
    where
        V: 'static + Render,
    {
        self.window
            .update(&mut self.cx, |_, window, cx| {
                window.replace_root(cx, build_view)
            })
            .expect("window was unexpectedly closed")
    }

    fn focus<V: crate::Focusable>(&mut self, view: &Entity<V>) {
        self.window
            .update(&mut self.cx, |_, window, cx| {
                view.read(cx).focus_handle(cx).focus(window, cx)
            })
            .expect("window was unexpectedly closed")
    }
}

impl AnyWindowHandle {
    /// 在此窗口中创建给定视图。
    pub fn build_entity<V: Render + 'static>(
        &self,
        cx: &mut TestAppContext,
        build_view: impl FnOnce(&mut Window, &mut Context<V>) -> V,
    ) -> Entity<V> {
        self.update(cx, |_, window, cx| cx.new(|cx| build_view(window, cx)))
            .unwrap()
    }
}
