use crate::scheduler::Instant;
use crate::scheduler::{Clock, Scheduler, SessionId, TestScheduler, TestSchedulerConfig, Yield};
use crate::{PlatformDispatcher, Priority, RunnableVariant};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

/// TestDispatcher 为测试提供确定性的异步执行。
///
/// 此实现将任务调度委托给 scheduler crate 的 `TestScheduler`。
/// 通过 `scheduler()` 直接访问调度器，以控制时钟、随机数和暂停。
#[doc(hidden)]
pub struct TestDispatcher {
    session_id: SessionId,
    scheduler: Arc<TestScheduler>,
    num_cpus_override: Arc<AtomicUsize>,
}

impl TestDispatcher {
    /// 使用给定的种子创建新的 TestDispatcher
    pub fn new(seed: u64) -> Self {
        let scheduler = Arc::new(TestScheduler::new(TestSchedulerConfig {
            seed,
            randomize_order: true,
            allow_parking: false,
            capture_pending_traces: std::env::var("PENDING_TRACES")
                .map_or(false, |var| var == "1" || var == "true"),
            timeout_ticks: 0..=1000,
        }));
        Self::from_scheduler(scheduler)
    }

    /// 从现有的 TestScheduler 创建 TestDispatcher
    pub fn from_scheduler(scheduler: Arc<TestScheduler>) -> Self {
        TestDispatcher {
            session_id: scheduler.allocate_session_id(),
            scheduler,
            num_cpus_override: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// 返回对底层 TestScheduler 的引用
    pub fn scheduler(&self) -> &Arc<TestScheduler> {
        &self.scheduler
    }

    /// 返回会话 ID
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// 排空所有待处理的任务
    pub fn drain_tasks(&self) {
        self.scheduler.drain_tasks();
    }

    /// 将时钟推进指定的时间
    pub fn advance_clock(&self, by: Duration) {
        self.scheduler.advance_clock(by);
    }

    /// 将时钟推进到下一个定时器
    pub fn advance_clock_to_next_timer(&self) -> bool {
        self.scheduler.advance_clock_to_next_timer()
    }

    /// 模拟随机延迟
    pub fn simulate_random_delay(&self) -> Yield {
        self.scheduler.yield_random()
    }

    /// 执行一个 tick，返回是否有任务被执行
    pub fn tick(&self, background_only: bool) -> bool {
        if background_only {
            self.scheduler.tick_background_only()
        } else {
            self.scheduler.tick()
        }
    }

    /// 运行直到所有任务都完成
    pub fn run_until_parked(&self) {
        while self.tick(false) {}
    }

    /// 允许线程暂停
    pub fn allow_parking(&self) {
        self.scheduler.allow_parking();
    }

    /// 禁止线程暂停
    pub fn forbid_parking(&self) {
        self.scheduler.forbid_parking();
    }

    /// 在测试中覆盖 `BackgroundExecutor::num_cpus()` 的返回值。
    /// 值为 0 表示不覆盖（使用默认值 4）。
    pub fn set_num_cpus(&self, count: usize) {
        self.num_cpus_override.store(count, Ordering::SeqCst);
    }

    /// 返回覆盖的 CPU 数量，如果没有设置覆盖则返回 `None`。
    pub fn num_cpus_override(&self) -> Option<usize> {
        match self.num_cpus_override.load(Ordering::SeqCst) {
            0 => None,
            n => Some(n),
        }
    }
}

impl Clone for TestDispatcher {
    fn clone(&self) -> Self {
        let session_id = self.scheduler.allocate_session_id();
        Self {
            session_id,
            scheduler: self.scheduler.clone(),
            num_cpus_override: self.num_cpus_override.clone(),
        }
    }
}

impl PlatformDispatcher for TestDispatcher {
    fn is_main_thread(&self) -> bool {
        self.scheduler.is_main_thread()
    }

    fn now(&self) -> Instant {
        self.scheduler.clock().now()
    }

    fn dispatch(&self, runnable: RunnableVariant, priority: Priority) {
        self.scheduler
            .schedule_background_with_priority(runnable, priority);
    }

    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, _priority: Priority) {
        self.scheduler
            .schedule_foreground(self.session_id, runnable);
    }

    fn dispatch_after(&self, _duration: Duration, _runnable: RunnableVariant) {
        panic!(
            "dispatch_after should not be called in tests. \
            Use BackgroundExecutor::timer() which uses the scheduler's native timer."
        );
    }

    fn as_test(&self) -> Option<&TestDispatcher> {
        Some(self)
    }

    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
        std::thread::spawn(move || {
            f();
        });
    }
}
