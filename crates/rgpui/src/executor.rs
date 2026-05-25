use crate::scheduler::Instant;
use crate::scheduler::Scheduler;
use crate::{App, PlatformDispatcher, PlatformScheduler};
use crate::{TryFutureExt, TryFutureExtBacktrace};
use futures::channel::mpsc;
use futures::prelude::*;
use std::{future::Future, marker::PhantomData, mem, pin::Pin, rc::Rc, sync::Arc, time::Duration};

pub use crate::scheduler::{
    FallibleTask, ForegroundExecutor as SchedulerForegroundExecutor, Priority, Task,
};

/// 指向当前正在运行的执行器的指针，
/// 用于生成后台任务。
#[derive(Clone)]
pub struct BackgroundExecutor {
    inner: crate::scheduler::BackgroundExecutor,
    dispatcher: Arc<dyn PlatformDispatcher>,
}

/// 指向当前正在运行的执行器的指针，
/// 用于在主线程上生成任务。
#[derive(Clone)]
pub struct ForegroundExecutor {
    inner: crate::scheduler::ForegroundExecutor,
    dispatcher: Arc<dyn PlatformDispatcher>,
    not_send: PhantomData<Rc<()>>,
}

/// `Task<Result<T, E>>` 的扩展 trait，添加了带有 `&App` 上下文的 `detach_and_log_err`。
///
/// 此 trait 自动为所有 `Task<Result<T, E>>` 类型实现。
pub trait TaskExt<T, E> {
    /// 在后台运行任务至完成并记录发生的任何错误。
    fn detach_and_log_err(self, cx: &App);
    /// 类似于 [`Self::detach_and_log_err`]，但在失败时使用 `{:?}` 格式化，以便 `anyhow::Error`
    /// 值发出完整的回溯信息。除非需要回溯信息，否则优先使用 `detach_and_log_err`。
    fn detach_and_log_err_with_backtrace(self, cx: &App);
}

impl<T, E> TaskExt<T, E> for Task<Result<T, E>>
where
    T: 'static,
    E: 'static + std::fmt::Display + std::fmt::Debug,
{
    #[track_caller]
    fn detach_and_log_err(self, cx: &App) {
        let location = core::panic::Location::caller();
        cx.foreground_executor()
            .spawn(self.log_tracked_err(*location))
            .detach();
    }

    #[track_caller]
    fn detach_and_log_err_with_backtrace(self, cx: &App) {
        let location = *core::panic::Location::caller();
        cx.foreground_executor()
            .spawn(self.log_tracked_err_with_backtrace(location))
            .detach();
    }
}

impl BackgroundExecutor {
    /// 从给定的 PlatformDispatcher 创建新的 BackgroundExecutor。
    pub fn new(dispatcher: Arc<dyn PlatformDispatcher>) -> Self {
        #[cfg(any(test, feature = "test-support"))]
        let scheduler: Arc<dyn Scheduler> = if let Some(test_dispatcher) = dispatcher.as_test() {
            test_dispatcher.scheduler().clone()
        } else {
            Arc::new(PlatformScheduler::new(dispatcher.clone()))
        };

        #[cfg(not(any(test, feature = "test-support")))]
        let scheduler: Arc<dyn Scheduler> = Arc::new(PlatformScheduler::new(dispatcher.clone()));

        Self {
            inner: crate::scheduler::BackgroundExecutor::new(scheduler),
            dispatcher,
        }
    }

    /// 返回底层的 crate::scheduler::BackgroundExecutor。
    ///
    /// 这用于 Ex 将执行器传递给线程/工作树代码。
    pub fn scheduler_executor(&self) -> crate::scheduler::BackgroundExecutor {
        self.inner.clone()
    }

    /// 将给定的未来任务加入队列，在后台线程上运行至完成。
    #[track_caller]
    pub fn spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static,
    {
        self.spawn_with_priority(Priority::default(), future.boxed())
    }

    /// 将给定的未来任务以给定优先级加入队列，在后台线程上运行至完成。
    ///
    /// 当使用 `Priority::RealtimeAudio` 时，任务在具有实时调度优先级的专用线程上运行，
    /// 适用于音频处理。
    #[track_caller]
    pub fn spawn_with_priority<R>(
        &self,
        priority: Priority,
        future: impl Future<Output = R> + Send + 'static,
    ) -> Task<R>
    where
        R: Send + 'static,
    {
        if priority == Priority::RealtimeAudio {
            self.inner.spawn_realtime(future)
        } else {
            self.inner.spawn_with_priority(priority, future)
        }
    }

    /// Scoped 允许你启动多个任务并等待
    /// 它们全部完成后再返回。
    pub async fn scoped<'scope, F>(&self, scheduler: F)
    where
        F: FnOnce(&mut Scope<'scope>),
    {
        let mut scope = Scope::new(self.clone(), Priority::default());
        (scheduler)(&mut scope);
        let spawned = mem::take(&mut scope.futures)
            .into_iter()
            .map(|f| self.spawn_with_priority(scope.priority, f))
            .collect::<Vec<_>>();
        for task in spawned {
            task.await;
        }
    }

    /// Scoped 允许你启动多个任务并等待
    /// 它们全部完成后再返回。
    pub async fn scoped_priority<'scope, F>(&self, priority: Priority, scheduler: F)
    where
        F: FnOnce(&mut Scope<'scope>),
    {
        let mut scope = Scope::new(self.clone(), priority);
        (scheduler)(&mut scope);
        let spawned = mem::take(&mut scope.futures)
            .into_iter()
            .map(|f| self.spawn_with_priority(scope.priority, f))
            .collect::<Vec<_>>();
        for task in spawned {
            task.await;
        }
    }

    /// 获取当前时间。
    ///
    /// 调用此方法而不是 `std::time::Instant::now` 允许在测试中使用
    /// 虚假计时器。
    pub fn now(&self) -> Instant {
        self.inner.scheduler().clock().now()
    }

    /// 返回一个任务，将在给定持续时间后可用。
    /// 取决于其他并发任务，实际经过的持续时间可能比请求的更长。
    #[track_caller]
    pub fn timer(&self, duration: Duration) -> Task<()> {
        if duration.is_zero() {
            return Task::ready(());
        }
        self.spawn(self.inner.scheduler().timer(duration))
    }

    /// 在测试中，运行任意数量的任务（由 SEED 环境变量确定）
    #[cfg(any(test, feature = "test-support"))]
    pub fn simulate_random_delay(&self) -> impl Future<Output = ()> + use<> {
        self.dispatcher.as_test().unwrap().simulate_random_delay()
    }

    /// 在测试中，推进时间。这不会运行任何任务，但会使 `timer` 准备就绪。
    #[cfg(any(test, feature = "test-support"))]
    pub fn advance_clock(&self, duration: Duration) {
        self.dispatcher.as_test().unwrap().advance_clock(duration)
    }

    /// 在测试中，运行一个任务。
    #[cfg(any(test, feature = "test-support"))]
    pub fn tick(&self) -> bool {
        self.dispatcher.as_test().unwrap().scheduler().tick()
    }

    /// 在测试中，运行任务直到调度器将进入休眠状态。
    ///
    /// 在调度器支持的测试调度器下，`tick()` 不会推进时钟，因此待处理的
    /// 计时器可以在所有当前可运行任务都已排空后仍保持 `has_pending_tasks()` 为 true。
    /// 为了保留测试所依赖的历史语义（排空所有可以取得进展的工作），
    /// 当没有可运行任务时，我们将时钟推进到下一个计时器。
    #[cfg(any(test, feature = "test-support"))]
    pub fn run_until_parked(&self) {
        let scheduler = self.dispatcher.as_test().unwrap().scheduler();
        scheduler.run();
    }

    /// 在测试中，防止 `run_until_parked` 在有未完成任务时恐慌。
    #[cfg(any(test, feature = "test-support"))]
    pub fn allow_parking(&self) {
        self.dispatcher
            .as_test()
            .unwrap()
            .scheduler()
            .allow_parking();

        if std::env::var("GPUI_RUN_UNTIL_PARKED_LOG").ok().as_deref() == Some("1") {
            log::warn!("[rgpui::executor] allow_parking: enabled");
        }
    }

    /// 设置 block_on 中运行前要执行的 tick 范围。
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_block_on_ticks(&self, range: std::ops::RangeInclusive<usize>) {
        self.dispatcher
            .as_test()
            .unwrap()
            .scheduler()
            .set_timeout_ticks(range);
    }

    /// 撤销 [`Self::allow_parking`] 的效果。
    #[cfg(any(test, feature = "test-support"))]
    pub fn forbid_parking(&self) {
        self.dispatcher
            .as_test()
            .unwrap()
            .scheduler()
            .forbid_parking();
    }

    /// 在测试中，返回调度器使用的 rng。
    #[cfg(any(test, feature = "test-support"))]
    pub fn rng(&self) -> crate::scheduler::SharedRng {
        self.dispatcher.as_test().unwrap().scheduler().rng()
    }

    /// 调度器有多少个 CPU 可用。
    pub fn num_cpus(&self) -> usize {
        #[cfg(any(test, feature = "test-support"))]
        if let Some(test) = self.dispatcher.as_test() {
            return test.num_cpus_override().unwrap_or(4);
        }
        num_cpus::get()
    }

    /// 在测试中覆盖此执行器报告的 CPU 数量。
    /// 如果不是在测试执行器上调用则会恐慌。
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_num_cpus(&self, count: usize) {
        self.dispatcher
            .as_test()
            .expect("set_num_cpus can only be called on a test executor")
            .set_num_cpus(count);
    }

    /// 我们是否在主线程上。
    pub fn is_main_thread(&self) -> bool {
        self.dispatcher.is_main_thread()
    }

    #[doc(hidden)]
    pub fn dispatcher(&self) -> &Arc<dyn PlatformDispatcher> {
        &self.dispatcher
    }
}

impl ForegroundExecutor {
    /// 从给定的 PlatformDispatcher 创建新的 ForegroundExecutor。
    pub fn new(dispatcher: Arc<dyn PlatformDispatcher>) -> Self {
        #[cfg(any(test, feature = "test-support"))]
        let (scheduler, session_id): (Arc<dyn Scheduler>, _) =
            if let Some(test_dispatcher) = dispatcher.as_test() {
                (
                    test_dispatcher.scheduler().clone(),
                    test_dispatcher.session_id(),
                )
            } else {
                let platform_scheduler = Arc::new(PlatformScheduler::new(dispatcher.clone()));
                let session_id = platform_scheduler.allocate_session_id();
                (platform_scheduler, session_id)
            };

        #[cfg(not(any(test, feature = "test-support")))]
        let (scheduler, session_id): (Arc<dyn Scheduler>, _) = {
            let platform_scheduler = Arc::new(PlatformScheduler::new(dispatcher.clone()));
            let session_id = platform_scheduler.allocate_session_id();
            (platform_scheduler, session_id)
        };

        let inner = crate::scheduler::ForegroundExecutor::new(session_id, scheduler);

        Self {
            inner,
            dispatcher,
            not_send: PhantomData,
        }
    }

    /// 将给定的任务加入队列，在主线程上运行。
    #[track_caller]
    pub fn spawn<R>(&self, future: impl Future<Output = R> + 'static) -> Task<R>
    where
        R: 'static,
    {
        self.inner.spawn(future.boxed_local())
    }

    /// 将给定的任务以给定优先级加入队列，在主线程上运行。
    #[track_caller]
    pub fn spawn_with_priority<R>(
        &self,
        _priority: Priority,
        future: impl Future<Output = R> + 'static,
    ) -> Task<R>
    where
        R: 'static,
    {
        // 前台任务忽略优先级 - 它们按顺序在主线程上运行
        self.inner.spawn(future)
    }

    /// 测试工具用于以同步方式运行异步测试。
    #[cfg(any(test, feature = "test-support"))]
    #[track_caller]
    pub fn block_test<R>(&self, future: impl Future<Output = R>) -> R {
        use std::cell::Cell;

        let scheduler = self.inner.scheduler();

        let output = Cell::new(None);
        let future = async {
            output.set(Some(future.await));
        };
        let mut future = std::pin::pin!(future);

        // In async GPUI tests, we must allow foreground tasks scheduled by the test itself
        // (which are associated with the test session) to make progress while we block.
        // Otherwise, awaiting futures that depend on same-session foreground work can deadlock.
        scheduler.block(None, future.as_mut(), None);

        output.take().expect("block_test future did not complete")
    }

    /// 阻塞当前线程直到给定的未来任务解析。
    /// 考虑改用 `block_with_timeout`。
    pub fn block_on<R>(&self, future: impl Future<Output = R>) -> R {
        self.inner.block_on(future)
    }

    /// 阻塞当前线程直到给定的未来任务解析或超时。
    pub fn block_with_timeout<R, Fut: Future<Output = R>>(
        &self,
        duration: Duration,
        future: Fut,
    ) -> Result<R, impl Future<Output = R> + use<R, Fut>> {
        self.inner.block_with_timeout(duration, future)
    }

    #[doc(hidden)]
    pub fn dispatcher(&self) -> &Arc<dyn PlatformDispatcher> {
        &self.dispatcher
    }

    #[doc(hidden)]
    pub fn scheduler_executor(&self) -> SchedulerForegroundExecutor {
        self.inner.clone()
    }
}

/// Scope 管理一组一起入队并等待的任务。参见 [`BackgroundExecutor::scoped`]。
pub struct Scope<'a> {
    executor: BackgroundExecutor,
    priority: Priority,
    futures: Vec<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>,
    tx: Option<mpsc::Sender<()>>,
    rx: mpsc::Receiver<()>,
    lifetime: PhantomData<&'a ()>,
}

impl<'a> Scope<'a> {
    fn new(executor: BackgroundExecutor, priority: Priority) -> Self {
        let (tx, rx) = mpsc::channel(1);
        Self {
            executor,
            priority,
            tx: Some(tx),
            rx,
            futures: Default::default(),
            lifetime: PhantomData,
        }
    }

    /// 调度器有多少个 CPU 可用。
    pub fn num_cpus(&self) -> usize {
        self.executor.num_cpus()
    }

    /// Spawn a future into this scope.
    #[track_caller]
    pub fn spawn<F>(&mut self, f: F)
    where
        F: Future<Output = ()> + Send + 'a,
    {
        let tx = self.tx.clone().unwrap();

        // SAFETY: The 'a lifetime is guaranteed to outlive any of these futures because
        // dropping this `Scope` blocks until all of the futures have resolved.
        let f = unsafe {
            mem::transmute::<
                Pin<Box<dyn Future<Output = ()> + Send + 'a>>,
                Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
            >(Box::pin(async move {
                f.await;
                drop(tx);
            }))
        };
        self.futures.push(f);
    }
}

impl Drop for Scope<'_> {
    fn drop(&mut self) {
        self.tx.take().unwrap();

        // Wait until the channel is closed, which means that all of the spawned
        // futures have resolved.
        let future = async {
            self.rx.next().await;
        };
        let mut future = std::pin::pin!(future);
        self.executor
            .inner
            .scheduler()
            .block(None, future.as_mut(), None);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{App, TestDispatcher, TestPlatform};
    use std::cell::RefCell;

    /// 创建测试辅助工具。
    /// 返回 (dispatcher, background_executor, app)。
    fn create_test_app() -> (TestDispatcher, BackgroundExecutor, Rc<crate::AppCell>) {
        let dispatcher = TestDispatcher::new(0);
        let arc_dispatcher = Arc::new(dispatcher.clone());
        let background_executor = BackgroundExecutor::new(arc_dispatcher.clone());
        let foreground_executor = ForegroundExecutor::new(arc_dispatcher);

        let platform = TestPlatform::new(background_executor.clone(), foreground_executor.clone());
        let asset_source = Arc::new(());
        #[cfg(feature = "test-support")]
        let http_client = crate::http_client::FakeHttpClient::with_404_response();
        #[cfg(not(feature = "test-support"))]
        let http_client = Arc::new(crate::http_client::BlockedHttpClient::new())
            as Arc<dyn crate::http_client::HttpClient>;

        let app = App::new_app(platform, asset_source, http_client);
        (dispatcher, background_executor, app)
    }

    #[test]
    fn sanity_test_tasks_run() {
        let (dispatcher, _background_executor, app) = create_test_app();
        let foreground_executor = app.borrow().foreground_executor.clone();

        let task_ran = Rc::new(RefCell::new(false));

        foreground_executor
            .spawn({
                let task_ran = Rc::clone(&task_ran);
                async move {
                    *task_ran.borrow_mut() = true;
                }
            })
            .detach();

        // 在 app 仍然存活时运行调度器
        dispatcher.run_until_parked();

        // 任务应该已经运行
        assert!(
            *task_ran.borrow(),
            "Task should run normally when app is alive"
        );
    }
}
