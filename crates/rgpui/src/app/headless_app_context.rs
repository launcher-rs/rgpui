//! Cross-platform headless app context for tests that need real text shaping.
//!
//! This replaces the macOS-only `HeadlessMetalAppContext` with a platform-neutral
//! implementation backed by `TestPlatform`. Tests supply a real `PlatformTextSystem`
//! (e.g. `DirectWriteTextSystem` on Windows, `MacTextSystem` on macOS) to get
//! accurate glyph measurements while keeping everything else deterministic.
//!
//! Optionally, a renderer factory can be provided to enable real GPU rendering
//! and screenshot capture via [`HeadlessAppContext::capture_screenshot`].

use crate::{
    AnyView, AnyWindowHandle, App, AppCell, AppContext, AssetSource, BackgroundExecutor, Bounds,
    Context, Entity, EntityId, ForegroundExecutor, Global, Pixels, PlatformHeadlessRenderer,
    PlatformTextSystem, Render, Reservation, Size, Task, TestDispatcher, TestPlatform, TextSystem,
    Window, WindowBounds, WindowHandle, WindowOptions,
    app::{GpuiBorrow, GpuiMode},
};
use anyhow::Result;
use image::RgbaImage;
use std::{future::Future, rc::Rc, sync::Arc, time::Duration};

/// 一个跨平台的无头应用上下文，用于需要真实文本排版的测试。
///
/// 与旧的 `HeadlessMetalAppContext` 不同，此实现可在任何平台上工作。它使用
/// `TestPlatform` 进行确定性调度，并接受可插拔的
/// `PlatformTextSystem`，以便测试获得真实的字形测量。
///
/// # 用法
///
/// ```ignore
/// let text_system = Arc::new(rgpui_wgpu::CosmicTextSystem::new("fallback"));
/// let mut cx = HeadlessAppContext::with_platform(
///     text_system,
///     Arc::new(Assets),
///     || rgpui_platform::current_headless_renderer(),
/// );
/// ```
pub struct HeadlessAppContext {
    /// 底层的应用单元。
    pub app: Rc<AppCell>,
    /// 用于运行异步任务的后台执行器。
    pub background_executor: BackgroundExecutor,
    /// 用于在主线程运行任务的前台执行器。
    pub foreground_executor: ForegroundExecutor,
    dispatcher: TestDispatcher,
    text_system: Arc<TextSystem>,
}

impl HeadlessAppContext {
    /// 使用给定文本系统创建新的无头应用上下文。
    pub fn new(platform_text_system: Arc<dyn PlatformTextSystem>) -> Self {
        Self::with_platform(platform_text_system, Arc::new(()), || None)
    }

    /// 使用自定义文本系统和资源源创建新的无头应用上下文。
    pub fn with_asset_source(
        platform_text_system: Arc<dyn PlatformTextSystem>,
        asset_source: Arc<dyn AssetSource>,
    ) -> Self {
        Self::with_platform(platform_text_system, asset_source, || None)
    }

    /// 使用给定文本系统、资源源和可选的渲染器工厂创建新的无头应用上下文，
    /// 以支持截图功能。
    pub fn with_platform(
        platform_text_system: Arc<dyn PlatformTextSystem>,
        asset_source: Arc<dyn AssetSource>,
        renderer_factory: impl Fn() -> Option<Box<dyn PlatformHeadlessRenderer>> + 'static,
    ) -> Self {
        let seed = std::env::var("SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let dispatcher = TestDispatcher::new(seed);
        let arc_dispatcher = Arc::new(dispatcher.clone());
        let background_executor = BackgroundExecutor::new(arc_dispatcher.clone());
        let foreground_executor = ForegroundExecutor::new(arc_dispatcher);

        let renderer_factory: Box<dyn Fn() -> Option<Box<dyn PlatformHeadlessRenderer>>> =
            Box::new(renderer_factory);
        let platform = TestPlatform::with_platform(
            background_executor.clone(),
            foreground_executor.clone(),
            platform_text_system.clone(),
            Some(renderer_factory),
        );

        let text_system = Arc::new(TextSystem::new(platform_text_system));
        #[cfg(feature = "test-support")]
        let http_client = crate::http_client::FakeHttpClient::with_404_response();
        #[cfg(not(feature = "test-support"))]
        let http_client = Arc::new(crate::http_client::BlockedHttpClient::new())
            as Arc<dyn crate::http_client::HttpClient>;
        let app = App::new_app(platform, asset_source, http_client);
        app.borrow_mut().mode = GpuiMode::test();

        Self {
            app,
            background_executor,
            foreground_executor,
            dispatcher,
            text_system,
        }
    }

    /// 打开一个用于无头渲染的窗口。
    pub fn open_window<V: Render + 'static>(
        &mut self,
        size: Size<Pixels>,
        build_root: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
    ) -> Result<WindowHandle<V>> {
        use crate::{point, px};

        let bounds = Bounds {
            origin: point(px(0.0), px(0.0)),
            size,
        };

        let mut cx = self.app.borrow_mut();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: false,
                show: false,
                ..Default::default()
            },
            build_root,
        )
    }

    /// 运行所有待处理的任务直到暂停。
    pub fn run_until_parked(&self) {
        self.dispatcher.run_until_parked();
    }

    /// 推进模拟时钟。
    pub fn advance_clock(&self, duration: Duration) {
        self.dispatcher.advance_clock(duration);
    }

    /// 启用暂停模式，允许在真实 I/O 上阻塞（例如异步资源加载）。
    pub fn allow_parking(&self) {
        self.dispatcher.allow_parking();
    }

    /// 禁用暂停模式，返回确定性测试执行。
    pub fn forbid_parking(&self) {
        self.dispatcher.forbid_parking();
    }

    /// 更新应用状态。
    pub fn update<R>(&mut self, f: impl FnOnce(&mut App) -> R) -> R {
        let mut app = self.app.borrow_mut();
        f(&mut app)
    }

    /// 更新窗口并调用绘制进行渲染。
    pub fn update_window<R>(
        &mut self,
        window: AnyWindowHandle,
        f: impl FnOnce(AnyView, &mut Window, &mut App) -> R,
    ) -> Result<R> {
        let mut app = self.app.borrow_mut();
        app.update_window(window, f)
    }

    /// 从窗口捕获截图。
    ///
    /// 需要上下文通过 [`HeadlessAppContext::with_platform`] 使用返回 `Some` 的渲染器工厂创建。
    pub fn capture_screenshot(&mut self, window: AnyWindowHandle) -> Result<RgbaImage> {
        let mut app = self.app.borrow_mut();
        app.update_window(window, |_, window, _| window.render_to_image())?
    }

    /// 返回文本系统。
    pub fn text_system(&self) -> &Arc<TextSystem> {
        &self.text_system
    }

    /// 返回后台执行器。
    pub fn background_executor(&self) -> &BackgroundExecutor {
        &self.background_executor
    }

    /// 返回前台执行器。
    pub fn foreground_executor(&self) -> &ForegroundExecutor {
        &self.foreground_executor
    }
}

impl Drop for HeadlessAppContext {
    fn drop(&mut self) {
        // 关闭应用以便在 LeakDetector 运行之前关闭窗口并释放实体句柄。
        self.app.borrow_mut().shutdown();
    }
}

impl AppContext for HeadlessAppContext {
    fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T> {
        let mut app = self.app.borrow_mut();
        app.new(build_entity)
    }

    fn reserve_entity<T: 'static>(&mut self) -> Reservation<T> {
        let mut app = self.app.borrow_mut();
        app.reserve_entity()
    }

    fn insert_entity<T: 'static>(
        &mut self,
        reservation: Reservation<T>,
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

    fn as_mut<'a, T>(&'a mut self, _: &Entity<T>) -> GpuiBorrow<'a, T>
    where
        T: 'static,
    {
        panic!("Cannot use as_mut with HeadlessAppContext. Call update() instead.")
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
