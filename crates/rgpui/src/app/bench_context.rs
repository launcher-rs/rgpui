use std::{
    cell::{OnceCell, RefCell},
    future::Future,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use anyhow::{Result, anyhow};
use hdrhistogram::Histogram;

use crate::{
    AnyView, AnyWindowHandle, App, AppCell, AppContext, BackgroundExecutor, Bounds, Context, Empty,
    Entity, EntityId, Focusable, ForegroundExecutor, Global, Platform, PlatformHeadlessRenderer,
    PlatformTextSystem, Render, Reservation, Task, TestPlatform, ThreadedDispatcher, VisualContext,
    Window, WindowBounds, WindowHandle, WindowOptions,
    app::GpuiBorrow,
    profiler::{self, FrameTiming, FrameTimingCollector},
};

/// 返回由当前线程共享分发器支持的基准测试平台。
///
/// 该平台使用当前线程的共享多线程 [`ThreadedDispatcher`]，因此后台任务
/// 以生产环境的并发度实时运行。分发器按线程缓存，在多次基准测试调用间
/// 复用，使得工作线程和定时器线程在整个进程生命周期内持续存在，
/// 而不是在每次 Criterion 校准轮次中重新创建。
///
/// 文本使用提供的平台文本系统进行排版。由 `#[rgpui::bench]` 生成的
/// 基准测试使用当前平台的文本系统，因此文本密集型基准测试的测量结果
/// 包含了生产环境的文本排版和字形光栅化。
///
/// `headless_renderer_factory` 为基准测试窗口提供渲染器，
/// 例如 `rgpui_platform::current_headless_renderer`。当提供时，
/// 基准测试绘制的场景会通过真实的精灵图集光栅化并在呈现时提交给 GPU，
/// 因此四边形/精灵的性能回归会体现在测量结果中。
/// 当为 `None` 时，呈现会丢弃场景。目前只有 macOS 提供无头渲染器
///（Metal），因此其他平台的基准测试测量不包含 GPU 提交。
pub fn bench_platform(
    headless_renderer_factory: Option<Box<dyn Fn() -> Option<Box<dyn PlatformHeadlessRenderer>>>>,
    text_system: Arc<dyn PlatformTextSystem>,
) -> Rc<dyn Platform> {
    thread_local! {
        static DISPATCHER: OnceCell<Arc<ThreadedDispatcher>> = const { OnceCell::new() };
    }
    let dispatcher = DISPATCHER.with(|cell| {
        cell.get_or_init(|| Arc::new(ThreadedDispatcher::new()))
            .clone()
    });
    let background_executor = BackgroundExecutor::new(dispatcher.clone());
    let foreground_executor = ForegroundExecutor::new(dispatcher);
    TestPlatform::with_platform(
        background_executor,
        foreground_executor,
        text_system,
        headless_renderer_factory,
    ) as Rc<dyn Platform>
}

/// 默认目标帧率，当基准测试未指定 `fps = N` 时使用。
const DEFAULT_FPS: u64 = 120;

const NANOS_PER_SECOND: u128 = 1_000_000_000;

/// RGPUI 基准测试生成的小型报告。
#[derive(Clone)]
pub struct BenchReport {
    frame_snapshot: Rc<RefCell<WindowFrameSnapshot>>,
    frame_budget_nanos: u128,
}

impl Default for BenchReport {
    fn default() -> Self {
        Self::with_fps(DEFAULT_FPS)
    }
}

impl BenchReport {
    /// 创建一个报告，其每帧预算为 `fps` 对应的一帧时长，
    /// 用于统计帧预算超支。
    pub fn with_fps(fps: u64) -> Self {
        assert!(fps > 0, "frame rate must be greater than zero");
        Self::with_frame_budget_nanos(NANOS_PER_SECOND / fps as u128)
    }

    /// 创建一个报告，将 `frame_budget_nanos` 视为每帧预算，
    /// 用于统计帧预算超支。
    pub fn with_frame_budget_nanos(frame_budget_nanos: u128) -> Self {
        Self {
            frame_snapshot: Rc::new(RefCell::new(WindowFrameSnapshot::new())),
            frame_budget_nanos,
        }
    }

    fn record_frame_timings<'i>(&self, timings: impl IntoIterator<Item = &'i FrameTiming>) {
        let mut snapshot = self.frame_snapshot.borrow_mut();
        // `.ok()` on `record`: this operation is infallible (the histograms auto-resize).
        for timing in timings {
            snapshot
                .draw
                .record(timing.draw_duration().as_nanos() as u64)
                .ok();
            if let Some(dirty_to_draw) = timing.dirty_to_draw_duration() {
                snapshot
                    .dirty_to_draw
                    .record(dirty_to_draw.as_nanos() as u64)
                    .ok();
            }
            if timing.invalidations > 0 {
                snapshot
                    .invalidations_per_frame
                    .record(timing.invalidations)
                    .ok();
            }
        }
    }

    fn total_budget_overruns(&self, histogram: &Histogram<u64>) -> u64 {
        histogram
            .iter_recorded()
            .map(|value| {
                self.budget_overruns(Duration::from_nanos(value.value_iterated_to()))
                    * value.count_at_value()
            })
            .sum()
    }

    /// 返回 `foreground_time` 超出每帧预算的完整帧数。
    /// 这是丢帧的合成代理指标：基准测试工具没有垂直同步，
    /// 因此它计算前台线程繁忙期间有多少帧截止时间已过期。
    fn budget_overruns(&self, foreground_time: Duration) -> u64 {
        let foreground_nanos = foreground_time.as_nanos();
        if foreground_nanos <= self.frame_budget_nanos {
            return 0;
        }

        let over_budget_nanos = foreground_nanos - self.frame_budget_nanos;
        over_budget_nanos.div_ceil(self.frame_budget_nanos) as u64
    }

    /// 将此报告打印到 stderr。
    pub fn print(&self, benchmark_name: Option<&'static str>) {
        let frame_snapshot = self.frame_snapshot.borrow();
        if frame_snapshot.is_empty() {
            return;
        }

        let benchmark_name = benchmark_name.unwrap_or("unknown benchmark");
        eprintln!("GPUI bench report (all observed iterations): {benchmark_name}");
        eprintln!("  note: includes Criterion warmup/calibration");
        self.print_histogram("window dirty-to-draw", &frame_snapshot.dirty_to_draw);
        self.print_histogram("window draw", &frame_snapshot.draw);
        if !frame_snapshot.invalidations_per_frame.is_empty() {
            eprintln!(
                "  invalidations per frame: mean {:.2}, max {}",
                frame_snapshot.invalidations_per_frame.mean(),
                frame_snapshot.invalidations_per_frame.max()
            );
        }
    }

    fn print_histogram(&self, name: &str, histogram: &Histogram<u64>) {
        if histogram.is_empty() {
            return;
        }

        let max_foreground_time = Duration::from_nanos(histogram.max());
        eprintln!("  {name}:");
        eprintln!("    samples: {}", histogram.len());
        eprintln!(
            "    mean: {}",
            format_duration(Duration::from_nanos(histogram.mean() as u64))
        );
        eprintln!(
            "    p50: {}",
            format_duration(Duration::from_nanos(histogram.value_at_quantile(0.50)))
        );
        eprintln!(
            "    p90: {}",
            format_duration(Duration::from_nanos(histogram.value_at_quantile(0.90)))
        );
        eprintln!(
            "    p95: {}",
            format_duration(Duration::from_nanos(histogram.value_at_quantile(0.95)))
        );
        eprintln!(
            "    p99: {}",
            format_duration(Duration::from_nanos(histogram.value_at_quantile(0.99)))
        );
        eprintln!("    max: {}", format_duration(max_foreground_time));
        eprintln!(
            "    frame budget overruns total: {}",
            self.total_budget_overruns(histogram)
        );
        eprintln!(
            "    frame budget overruns max: {}",
            self.budget_overruns(max_foreground_time)
        );
    }
}

struct WindowFrameSnapshot {
    dirty_to_draw: Histogram<u64>,
    draw: Histogram<u64>,
    invalidations_per_frame: Histogram<u64>,
}

impl WindowFrameSnapshot {
    fn new() -> Self {
        Self {
            dirty_to_draw: Histogram::new(3).expect("3 significant digits is valid"),
            draw: Histogram::new(3).expect("3 significant digits is valid"),
            invalidations_per_frame: Histogram::new(3).expect("3 significant digits is valid"),
        }
    }

    fn is_empty(&self) -> bool {
        self.dirty_to_draw.is_empty() && self.draw.is_empty()
    }
}

fn format_duration(duration: Duration) -> String {
    format!("{:.3}ms", duration.as_secs_f64() * 1000.)
}

/// 在测量期间启用帧追踪并收集其中记录的帧。
/// 之前的追踪状态会在 drop 时恢复，因此 panic 的测量
/// 不会使无关代码（如同进程中后续的基准测试）的追踪保持启用。
struct FrameTraceScope {
    collector: FrameTimingCollector,
    was_already_enabled: bool,
}

impl FrameTraceScope {
    fn start() -> Self {
        let was_already_enabled = !profiler::set_frame_trace_enabled(true);
        Self {
            collector: FrameTimingCollector::new(),
            was_already_enabled,
        }
    }

    fn finish(mut self) -> Vec<FrameTiming> {
        self.collector.collect_unseen()
        // Dropping `self` restores the previous tracing state.
    }
}

impl Drop for FrameTraceScope {
    fn drop(&mut self) {
        if !self.was_already_enabled {
            profiler::set_frame_trace_enabled(false);
        }
    }
}

/// Criterion 基准测试的 RGPUI 应用上下文。
///
/// `BenchAppContext` 与 `TestAppContext` 是分开的：它拥有一个
/// 基准测试应用实例，仅暴露基准测试设置所需的应用/窗口操作。
/// Criterion 通过其 `Bencher` API 负责被测量的循环。
#[derive(Clone)]
pub struct BenchAppContext<'a, 'measurement> {
    app: Rc<AppCell>,
    background_executor: BackgroundExecutor,
    foreground_executor: ForegroundExecutor,
    benchmark_name: Option<&'static str>,
    bencher: Rc<RefCell<Option<&'a mut criterion::Bencher<'measurement>>>>,
    report: BenchReport,
}

impl<'a, 'measurement> BenchAppContext<'a, 'measurement> {
    /// 创建一个由提供的平台支持的新的基准测试应用上下文。
    ///
    /// 平台的执行器必须由 [`ThreadedDispatcher`] 支持
    ///（参见 [`bench_platform`]），以便上下文能通过
    /// [`Self::run_until_idle`] 排空前台任务；否则会 panic。
    pub fn new(
        platform: Rc<dyn Platform>,
        benchmark_name: Option<&'static str>,
        bencher: &'a mut criterion::Bencher<'measurement>,
    ) -> Self {
        Self::build(platform, benchmark_name, bencher, BenchReport::default())
    }

    /// 创建一个由提供的平台支持的新的基准测试应用上下文。
    ///
    /// 平台的执行器必须由 [`ThreadedDispatcher`] 支持
    ///（参见 [`bench_platform`]），以便上下文能通过
    /// [`Self::run_until_idle`] 排空前台任务；否则会 panic。
    #[doc(hidden)]
    pub fn new_with_platform_and_report(
        platform: Rc<dyn Platform>,
        benchmark_name: Option<&'static str>,
        bencher: &'a mut criterion::Bencher<'measurement>,
        report: BenchReport,
    ) -> Self {
        Self::build(platform, benchmark_name, bencher, report)
    }

    fn build(
        platform: Rc<dyn Platform>,
        benchmark_name: Option<&'static str>,
        bencher: &'a mut criterion::Bencher<'measurement>,
        report: BenchReport,
    ) -> Self {
        let background_executor = platform.background_executor();
        // Validate up front so misconfiguration fails at construction with a
        // clear message instead of deep inside `run_until_idle`.
        assert!(
            background_executor.dispatcher().as_threaded().is_some(),
            "BenchAppContext requires a platform whose executors are backed by a \
             ThreadedDispatcher; construct one with rgpui::bench_platform"
        );
        let foreground_executor = platform.foreground_executor();
        let asset_source = Arc::new(());
        let http_client = crate::http_client::FakeHttpClient::with_404_response();
        let app = App::new_app(platform, asset_source, http_client);

        Self {
            app,
            background_executor,
            foreground_executor,
            benchmark_name,
            bencher: Rc::new(RefCell::new(Some(bencher))),
            report,
        }
    }

    /// 创建此上下文的基准测试函数名。
    pub fn benchmark_name(&self) -> Option<&'static str> {
        self.benchmark_name
    }

    /// 返回此基准测试应用使用的后台执行器。
    pub fn background_executor(&self) -> &BackgroundExecutor {
        &self.background_executor
    }

    /// 返回此基准测试应用使用的前台执行器。
    pub fn foreground_executor(&self) -> &ForegroundExecutor {
        &self.foreground_executor
    }

    /// 更新应用并在之后刷新同步的 RGPUI 效果。
    pub fn update<R>(&mut self, update: impl FnOnce(&mut App) -> R) -> R {
        let mut app = self.app.borrow_mut();
        app.update(update)
    }

    /// 读取应用状态。
    pub fn read<R>(&self, read: impl FnOnce(&App) -> R) -> R {
        let app = self.app.borrow();
        read(&app)
    }

    /// 在当前线程上运行排队的前台任务，并等待进行中的后台任务完成。
    /// 尚未到期的定时器不会被等待（参见 [`ThreadedDispatcher::run_until_idle`]）。
    pub fn run_until_idle(&self) {
        self.background_executor
            .dispatcher()
            .as_threaded()
            .expect("validated in BenchAppContext::build")
            .run_until_idle();
    }

    /// 使用 Criterion 的迭代循环测量通用基准测试负载。
    ///
    /// 闭包在每次 Criterion 迭代时被调用，并传入此基准测试应用上下文，
    /// 以便它可以更新 RGPUI 状态。
    ///
    /// 由负载触发的任何窗口绘制会通过 RGPUI 帧分析器
    /// 记录到基准测试的帧报告中。
    pub fn bench_iter(&mut self, mut benchmark: impl FnMut(&mut Self)) {
        let bencher = self.take_bencher("bench_iter");
        let collector = FrameTraceScope::start();
        let mut benchmark = || benchmark(self);
        bencher.iter(&mut benchmark);
        self.report.record_frame_timings(collector.finish().iter());
        self.replace_bencher(bencher);
    }

    /// 在当前窗口中更新 RGPUI 实体后测量帧延迟。
    ///
    /// 每次迭代在实体的当前窗口中运行 `update`。在基准测试构建中，
    /// 刷新更新的效果会同步绘制脏窗口。该实体应是窗口渲染树的一部分，
    /// 例如根视图或其子视图。
    ///
    /// 帧计时通过 RGPUI 帧分析器收集
    ///（[`crate::profiler::record_frame_timing`]），在测量期间启用。
    pub fn bench_renderer<V>(
        &mut self,
        view: Entity<V>,
        mut update: impl FnMut(&mut V, &mut Window, &mut Context<V>),
    ) where
        V: 'static + Render,
    {
        let bencher = self.take_bencher("bench_renderer");
        let window_id = self
            .with_window(view.entity_id(), |window, _| {
                window.window_handle().window_id()
            })
            .expect("cannot benchmark renderer for entity without a current window");

        let collector = FrameTraceScope::start();

        let mut benchmark = || {
            self.with_window(view.entity_id(), |window, cx| {
                view.update(cx, |view, cx| update(view, window, cx));
            })
            .expect("cannot benchmark renderer for entity without a current window");
            // Submit the frame drawn by the update's effect flush, mirroring
            // production where every drawn frame is presented. With a headless
            // renderer this includes scene submission to the GPU.
            self.with_window(view.entity_id(), |window, _| {
                window.present_if_needed();
            })
            .expect("cannot benchmark renderer for entity without a current window");
        };
        bencher.iter(&mut benchmark);

        let timings = collector.finish();
        self.report.record_frame_timings(
            timings
                .iter()
                .filter(|timing| timing.window_id == window_id),
        );
        self.replace_bencher(bencher);
    }

    /// 添加一个带有空根视图的窗口用于基准测试设置。
    pub fn add_empty_window(&mut self) -> BenchWindowContext<'a, 'measurement> {
        let bounds = {
            let app = self.app.borrow();
            Bounds::maximized(None, &app)
        };
        let window = {
            let mut app = self.app.borrow_mut();
            let window: AnyWindowHandle = app
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        ..Default::default()
                    },
                    |_, cx| cx.new(|_| Empty),
                )
                .expect("failed to open benchmark window")
                .into();
            window
        };

        self.run_until_idle();
        BenchWindowContext {
            cx: self.clone(),
            window,
        }
    }

    fn take_bencher(&self, benchmark_kind: &str) -> &'a mut criterion::Bencher<'measurement> {
        self.bencher.borrow_mut().take().unwrap_or_else(|| {
            panic!("cannot start {benchmark_kind}: benchmark measurement is already running")
        })
    }

    fn replace_bencher(&self, bencher: &'a mut criterion::Bencher<'measurement>) {
        let previous = self.bencher.borrow_mut().replace(bencher);
        assert!(
            previous.is_none(),
            "benchmark bencher was unexpectedly present after measurement"
        );
    }

    /// 运行 RGPUI 基准测试的清理工作。
    ///
    /// 取消共享分发器上仍然激活的所有定时器，并排空取消操作所解除阻塞的
    /// 工作，使它们不会在后续基准测试中触发；假定当前线程上没有其他
    /// `BenchAppContext` 存活。
    pub fn teardown(mut self) {
        self.run_until_idle();
        self.update(|cx| {
            cx.quit();
        });
        self.run_until_idle();

        let dispatcher = self.background_executor.dispatcher();
        let dispatcher = dispatcher
            .as_threaded()
            .expect("validated in BenchAppContext::build");

        drop(self.app);
        drop(self.foreground_executor);

        for _ in 0..100 {
            if dispatcher.cancel_pending_timers() == 0 {
                return;
            }
            dispatcher.run_until_idle();
        }
        panic!(
            "benchmark teardown kept scheduling timers: {}",
            dispatcher.debug_state()
        );
    }
}

impl AppContext for BenchAppContext<'_, '_> {
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

    fn as_mut<'b, T>(&'b mut self, _: &Entity<T>) -> GpuiBorrow<'b, T>
    where
        T: 'static,
    {
        panic!("Cannot use as_mut with BenchAppContext. Call update() instead.")
    }

    fn read_entity<T, R>(&self, handle: &Entity<T>, read: impl FnOnce(&T, &App) -> R) -> R
    where
        T: 'static,
    {
        let app = self.app.borrow();
        app.read_entity(handle, read)
    }

    fn update_window<T, F>(&mut self, window: AnyWindowHandle, update: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        let mut app = self.app.borrow_mut();
        app.update_window(window, update)
    }

    fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        update: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R> {
        let mut app = self.app.borrow_mut();
        app.with_window(entity_id, update)
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

/// RGPUI 基准测试的窗口专用上下文。
///
/// 它与 `VisualTestContext` 是分开的：它提供对基准测试窗口的访问，
/// 但不暴露仅用于测试的辅助方法（如输入模拟）。
#[derive(Clone)]
pub struct BenchWindowContext<'a, 'measurement> {
    cx: BenchAppContext<'a, 'measurement>,
    window: AnyWindowHandle,
}

impl<'a, 'measurement> BenchWindowContext<'a, 'measurement> {
    /// 返回底层的基准测试应用上下文。
    pub fn app_context(&mut self) -> &mut BenchAppContext<'a, 'measurement> {
        &mut self.cx
    }

    /// 返回与此上下文关联的窗口。
    pub fn window_handle(&self) -> AnyWindowHandle {
        self.window
    }

    /// 在当前线程上运行排队的前台任务，并等待进行中的后台任务完成。
    /// 待处理的定时器不会被等待。
    pub fn run_until_idle(&self) {
        self.cx.run_until_idle();
    }

    /// 更新基准测试窗口。
    pub fn update<R>(&mut self, update: impl FnOnce(&mut Window, &mut App) -> R) -> R {
        self.cx
            .update_window(self.window, |_, window, cx| update(window, cx))
            .expect("benchmark window was unexpectedly closed")
    }
}

impl AppContext for BenchWindowContext<'_, '_> {
    fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T> {
        self.window
            .update(&mut self.cx, |_, _, cx| cx.new(build_entity))
            .expect("benchmark window was unexpectedly closed")
    }

    fn reserve_entity<T: 'static>(&mut self) -> Reservation<T> {
        self.cx.reserve_entity()
    }

    fn insert_entity<T: 'static>(
        &mut self,
        reservation: Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T> {
        self.window
            .update(&mut self.cx, |_, _, cx| {
                cx.insert_entity(reservation, build_entity)
            })
            .expect("benchmark window was unexpectedly closed")
    }

    fn update_entity<T: 'static, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R {
        self.cx.update_entity(handle, update)
    }

    fn as_mut<'b, T>(&'b mut self, handle: &Entity<T>) -> GpuiBorrow<'b, T>
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

    fn update_window<T, F>(&mut self, window: AnyWindowHandle, update: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        self.cx.update_window(window, update)
    }

    fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        update: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R> {
        self.cx.with_window(entity_id, update)
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

impl VisualContext for BenchWindowContext<'_, '_> {
    type Result<T> = Result<T>;

    fn window_handle(&self) -> AnyWindowHandle {
        self.window
    }

    fn update_window_entity<T: 'static, R>(
        &mut self,
        entity: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Window, &mut Context<T>) -> R,
    ) -> Result<R> {
        let entity = entity.clone();
        self.cx
            .app
            .borrow_mut()
            .with_window(entity.entity_id(), |window, app| {
                entity.update(app, |entity, cx| update(entity, window, cx))
            })
            .ok_or_else(|| {
                anyhow!("entity has no current window; use `update` instead of `update_in`")
            })
    }

    fn new_window_entity<T: 'static>(
        &mut self,
        build_entity: impl FnOnce(&mut Window, &mut Context<T>) -> T,
    ) -> Result<Entity<T>> {
        self.window.update(&mut self.cx, |_, window, cx| {
            cx.new(|cx| build_entity(window, cx))
        })
    }

    fn replace_root_view<V>(
        &mut self,
        build_view: impl FnOnce(&mut Window, &mut Context<V>) -> V,
    ) -> Result<Entity<V>>
    where
        V: 'static + Render,
    {
        self.window.update(&mut self.cx, |_, window, cx| {
            window.replace_root(cx, build_view)
        })
    }

    fn focus<V>(&mut self, entity: &Entity<V>) -> Result<()>
    where
        V: Focusable,
    {
        self.window.update(&mut self.cx, |_, window, cx| {
            entity.read(cx).focus_handle(cx).focus(window, cx)
        })
    }
}
