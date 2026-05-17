use crate::{
    Action, AnyView, AnyWindowHandle, App, AppCell, AppContext, AssetSource, BackgroundExecutor,
    Bounds, ClipboardItem, Context, Entity, EntityId, ForegroundExecutor, Global, InputEvent,
    Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Platform, Point, Render, Result, Size, Task, TestDispatcher, TextSystem, VisualTestPlatform,
    Window, WindowBounds, WindowHandle, WindowOptions, app::GpuiMode,
};
use anyhow::anyhow;
use image::RgbaImage;
use std::{future::Future, rc::Rc, sync::Arc, time::Duration};

/// 使用真实 macOS 渲染而非模拟渲染的测试上下文。
/// 用于需要捕获实际截图的视觉测试。
///
/// 与使用 `TestPlatform` 和模拟渲染的 `TestAppContext` 不同，
/// `VisualTestAppContext` 使用真实的 `MacPlatform` 生成实际渲染输出。
///
/// 通过此上下文创建的窗口位于屏幕外（如 -10000, -10000 坐标），
/// 因此对用户不可见，但仍由合成器完全渲染。
#[derive(Clone)]
pub struct VisualTestAppContext {
    /// The underlying app cell
    pub app: Rc<AppCell>,
    /// The background executor for running async tasks
    pub background_executor: BackgroundExecutor,
    /// The foreground executor for running tasks on the main thread
    pub foreground_executor: ForegroundExecutor,
    /// The test dispatcher for deterministic task scheduling
    dispatcher: TestDispatcher,
    platform: Rc<dyn Platform>,
    text_system: Arc<TextSystem>,
}

impl VisualTestAppContext {
    /// 创建带有真实 macOS 平台渲染的 `VisualTestAppContext`，
    /// 但通过 TestDispatcher 进行确定性任务调度。
    ///
    /// 提供：
    /// - 真实 Metal/合成器渲染以获得准确的截图
    /// - 通过 TestDispatcher 进行确定性任务调度
    /// - 通过 `advance_clock` 控制时间
    ///
    /// 注意：这使用无操作资产源，因此 SVG 图标不会渲染。
    /// 使用 `with_asset_source` 提供真实资产以渲染图标。
    pub fn new(platform: Rc<dyn Platform>) -> Self {
        Self::with_asset_source(platform, Arc::new(()))
    }

    /// 创建带有自定义资产源的 `VisualTestAppContext`。
    ///
    /// 当你需要 SVG 图标在视觉测试中正确渲染时使用此方法。
    /// 传递真实的 `Assets` 结构体以启用图标渲染。
    pub fn with_asset_source(
        platform: Rc<dyn Platform>,
        asset_source: Arc<dyn AssetSource>,
    ) -> Self {
        // 使用种子 RNG 以获得确定性行为
        let seed = std::env::var("SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // 创建结合真实 Mac 渲染的视觉测试平台
        // 以及可控的 TestDispatcher 用于确定性任务调度
        let platform = Rc::new(VisualTestPlatform::new(platform, seed));

        // 从平台获取分发器和执行器
        let dispatcher = platform.dispatcher().clone();
        let background_executor = platform.background_executor();
        let foreground_executor = platform.foreground_executor();

        let text_system = Arc::new(TextSystem::new(platform.text_system()));

        let http_client = crate::http_client::FakeHttpClient::with_404_response();

        let mut app = App::new_app(platform.clone(), asset_source, http_client);
        app.borrow_mut().mode = GpuiMode::test();

        Self {
            app,
            background_executor,
            foreground_executor,
            dispatcher,
            platform,
            text_system,
        }
    }

    /// 打开位于屏幕外的窗口以进行不可见渲染。
    ///
    /// 窗口定位在 (-10000, -10000)，因此在任何显示上都不可见，
    /// 但仍由合成器完全渲染，并可通过 ScreenCaptureKit 捕获。
    ///
    /// # 参数
    /// * `size` - 要创建的窗口大小
    /// * `build_root` - 构建窗口根视图的闭包
    pub fn open_offscreen_window<V: Render + 'static>(
        &mut self,
        size: Size<Pixels>,
        build_root: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
    ) -> Result<WindowHandle<V>> {
        use crate::{point, px};

        let bounds = Bounds {
            origin: point(px(-10000.0), px(-10000.0)),
            size,
        };

        let mut cx = self.app.borrow_mut();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: false,
                show: true,
                ..Default::default()
            },
            build_root,
        )
    }

    /// 打开默认大小 (1280x800) 的屏幕外窗口。
    pub fn open_offscreen_window_default<V: Render + 'static>(
        &mut self,
        build_root: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
    ) -> Result<WindowHandle<V>> {
        use crate::{px, size};
        self.open_offscreen_window(size(px(1280.0), px(800.0)), build_root)
    }

    /// 返回此平台是否支持屏幕捕获。
    pub fn is_screen_capture_supported(&self) -> bool {
        self.platform.is_screen_capture_supported()
    }

    /// 返回此上下文使用的文本系统。
    pub fn text_system(&self) -> &Arc<TextSystem> {
        &self.text_system
    }

    /// 返回后台执行器。
    pub fn executor(&self) -> BackgroundExecutor {
        self.background_executor.clone()
    }

    /// 返回前台执行器。
    pub fn foreground_executor(&self) -> ForegroundExecutor {
        self.foreground_executor.clone()
    }

    /// 运行所有待处理的前台和后台任务，直到没有剩余任务。
    /// 这对于处理异步操作（如工具提示计时器）至关重要。
    pub fn run_until_parked(&self) {
        self.dispatcher.run_until_parked();
    }

    /// 将模拟时钟前进给定持续时间，并处理任何
    /// 变为就绪的任务。这对于测试基于时间的行为（如
    /// 工具提示延迟）至关重要。
    pub fn advance_clock(&self, duration: Duration) {
        self.dispatcher.advance_clock(duration);
    }

    /// 更新应用状态。
    pub fn update<R>(&mut self, f: impl FnOnce(&mut App) -> R) -> R {
        let mut app = self.app.borrow_mut();
        f(&mut app)
    }

    /// 读取应用状态。
    pub fn read<R>(&self, f: impl FnOnce(&App) -> R) -> R {
        let app = self.app.borrow();
        f(&app)
    }

    /// 更新窗口。
    pub fn update_window<T, F>(&mut self, window: AnyWindowHandle, f: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        let mut lock = self.app.borrow_mut();
        lock.update_window(window, f)
    }

    /// 在前台执行器上生成任务。
    pub fn spawn<F, R>(&self, f: F) -> Task<R>
    where
        F: Future<Output = R> + 'static,
        R: 'static,
    {
        self.foreground_executor.spawn(f)
    }

    /// 检查是否存在 G 类型的全局。
    pub fn has_global<G: Global>(&self) -> bool {
        let app = self.app.borrow();
        app.has_global::<G>()
    }

    /// 读取全局值。
    pub fn read_global<G: Global, R>(&self, f: impl FnOnce(&G, &App) -> R) -> R {
        let app = self.app.borrow();
        f(app.global::<G>(), &app)
    }

    /// 设置全局值。
    pub fn set_global<G: Global>(&mut self, global: G) {
        let mut app = self.app.borrow_mut();
        app.set_global(global);
    }

    /// 更新全局值。
    pub fn update_global<G: Global, R>(&mut self, f: impl FnOnce(&mut G, &mut App) -> R) -> R {
        let mut lock = self.app.borrow_mut();
        lock.update(|cx| {
            let mut global = cx.lease_global::<G>();
            let result = f(&mut global, cx);
            cx.end_global_lease(global);
            result
        })
    }

    /// 模拟在给定窗口上的一系列击键。
    ///
    /// 击键指定为以空格分隔的字符串，例如 "cmd-p escape"。
    pub fn simulate_keystrokes(&mut self, window: AnyWindowHandle, keystrokes: &str) {
        for keystroke_text in keystrokes.split_whitespace() {
            let keystroke = Keystroke::parse(keystroke_text)
                .unwrap_or_else(|_| panic!("无效的击键: {}", keystroke_text));
            self.dispatch_keystroke(window, keystroke);
        }
        self.run_until_parked();
    }

    /// 向窗口分派单个击键。
    pub fn dispatch_keystroke(&mut self, window: AnyWindowHandle, keystroke: Keystroke) {
        self.update_window(window, |_, window, cx| {
            window.dispatch_keystroke(keystroke, cx);
        })
        .ok();
    }

    /// 模拟在给定窗口上输入文本。
    pub fn simulate_input(&mut self, window: AnyWindowHandle, input: &str) {
        for char in input.chars() {
            let key = char.to_string();
            let keystroke = Keystroke {
                modifiers: Modifiers::default(),
                key: key.clone(),
                key_char: Some(key),
            };
            self.dispatch_keystroke(window, keystroke);
        }
        self.run_until_parked();
    }

    /// 模拟鼠标移动事件。
    pub fn simulate_mouse_move(
        &mut self,
        window: AnyWindowHandle,
        position: Point<Pixels>,
        button: impl Into<Option<MouseButton>>,
        modifiers: Modifiers,
    ) {
        self.simulate_event(
            window,
            MouseMoveEvent {
                position,
                modifiers,
                pressed_button: button.into(),
            },
        );
    }

    /// 模拟鼠标按下事件。
    pub fn simulate_mouse_down(
        &mut self,
        window: AnyWindowHandle,
        position: Point<Pixels>,
        button: MouseButton,
        modifiers: Modifiers,
    ) {
        self.simulate_event(
            window,
            MouseDownEvent {
                position,
                modifiers,
                button,
                click_count: 1,
                first_mouse: false,
            },
        );
    }

    /// 模拟鼠标释放事件。
    pub fn simulate_mouse_up(
        &mut self,
        window: AnyWindowHandle,
        position: Point<Pixels>,
        button: MouseButton,
        modifiers: Modifiers,
    ) {
        self.simulate_event(
            window,
            MouseUpEvent {
                position,
                modifiers,
                button,
                click_count: 1,
            },
        );
    }

    /// 模拟点击（鼠标按下后释放）。
    pub fn simulate_click(
        &mut self,
        window: AnyWindowHandle,
        position: Point<Pixels>,
        modifiers: Modifiers,
    ) {
        self.simulate_mouse_down(window, position, MouseButton::Left, modifiers);
        self.simulate_mouse_up(window, position, MouseButton::Left, modifiers);
    }

    /// 模拟在给定窗口上的输入事件。
    pub fn simulate_event<E: InputEvent>(&mut self, window: AnyWindowHandle, event: E) {
        self.update_window(window, |_, window, cx| {
            window.dispatch_event(event.to_platform_input(), cx);
        })
        .ok();
        self.run_until_parked();
    }

    /// 向给定窗口分派动作。
    pub fn dispatch_action(&mut self, window: AnyWindowHandle, action: impl Action) {
        self.update_window(window, |_, window, cx| {
            window.dispatch_action(action.boxed_clone(), cx);
        })
        .ok();
        self.run_until_parked();
    }

    /// 写入剪贴板。
    pub fn write_to_clipboard(&self, item: ClipboardItem) {
        self.platform.write_to_clipboard(item);
    }

    /// 从剪贴板读取。
    pub fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.platform.read_from_clipboard()
    }

    /// 等待条件变为 true，带有超时。
    pub async fn wait_for<T: 'static>(
        &mut self,
        entity: &Entity<T>,
        predicate: impl Fn(&T) -> bool,
        timeout: Duration,
    ) -> Result<()> {
        let start = web_time::Instant::now();
        loop {
            {
                let app = self.app.borrow();
                if predicate(entity.read(&app)) {
                    return Ok(());
                }
            }

            if start.elapsed() > timeout {
                return Err(anyhow!("等待条件超时"));
            }

            self.run_until_parked();
            self.background_executor
                .timer(Duration::from_millis(10))
                .await;
        }
    }

    /// 使用直接纹理捕获捕获指定窗口的截图。
    ///
    /// 这将场景渲染到 Metal 纹理并直接读取像素，
    /// 不需要窗口在屏幕上可见。
    #[cfg(any(test, feature = "test-support"))]
    pub fn capture_screenshot(&mut self, window: AnyWindowHandle) -> Result<RgbaImage> {
        self.update_window(window, |_, window, _cx| window.render_to_image())?
    }

    /// 等待动画完成，通过等待几帧。
    pub async fn wait_for_animations(&self) {
        self.background_executor
            .timer(Duration::from_millis(32))
            .await;
        self.run_until_parked();
    }
}

impl AppContext for VisualTestAppContext {
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

    fn as_mut<'a, T>(&'a mut self, _: &Entity<T>) -> crate::GpuiBorrow<'a, T>
    where
        T: 'static,
    {
        panic!("Cannot use as_mut with a visual test app context. Try calling update() first")
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
        callback(app.global::<G>(), &app)
    }
}
