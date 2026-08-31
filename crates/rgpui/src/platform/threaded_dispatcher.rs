use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use crate::collections::BinaryHeap;

use parking_lot::{Condvar, Mutex};

use crate::{
    PlatformDispatcher, Priority, RunnableVariant, profiler,
    queue::{PriorityQueueReceiver, PriorityQueueSender},
};

const MIN_THREADS: usize = 2;

/// 用于基准测试的多线程 [`PlatformDispatcher`]。
///
/// 后台任务在工作线程池上并行运行，计时器在专用计时器线程上实时触发，
/// 镜像生产调度器（参见 `LinuxDispatcher`）。主线程任务排队直到
/// 基准测试线程通过 [`Self::run_until_idle`] 清空它们，因为没有
/// 平台运行循环来泵送它们。
///
/// 与 [`TestDispatcher`](crate::TestDispatcher) 不同，后者在单个线程上
/// 使用虚拟时钟运行所有内容，通过此调度器分派的工作以生产并发性执行，
/// 因此挂钟测量反映真实的并行性。
pub struct ThreadedDispatcher {
    background_sender: PriorityQueueSender<RunnableVariant>,
    main_sender: PriorityQueueSender<RunnableVariant>,
    main_receiver: Mutex<PriorityQueueReceiver<RunnableVariant>>,
    timers: Arc<TimerQueue>,
    idle: Arc<IdleTracker>,
    main_thread_id: thread::ThreadId,
}

/// 跟踪有多少后台和计时器可运行对象排队或正在运行，以便
/// [`ThreadedDispatcher::run_until_idle`] 知道何时停止等待。
#[derive(Default)]
struct IdleTracker {
    inflight: Mutex<usize>,
    condvar: Condvar,
}

impl IdleTracker {
    fn increment(&self) {
        *self.inflight.lock() += 1;
    }

    fn decrement(&self) {
        let mut inflight = self.inflight.lock();
        *inflight -= 1;
        if *inflight == 0 {
            self.condvar.notify_all();
        }
    }

    /// 返回一个守卫，当被丢弃时减少进行中计数，这样即使正在执行的
    /// 可运行对象发生 panic，计数也能保持正确。
    fn decrement_on_drop(&self) -> impl Drop + '_ {
        crate::rgpui_util::defer(|| self.decrement())
    }

    /// 在持有进行中锁时通知等待者。`run_until_idle`
    /// 在等待前在此锁下重新检查其唤醒条件，因此
    /// 通知不会在其检查和等待之间溜走而丢失。
    fn notify_under_lock(&self) {
        let _inflight = self.inflight.lock();
        self.condvar.notify_all();
    }
}

struct TimerQueue {
    state: Mutex<TimerQueueState>,
    condvar: Condvar,
}

struct TimerQueueState {
    heap: BinaryHeap<TimerEntry>,
    next_seq: u64,
}

struct TimerEntry {
    due: Instant,
    seq: u64,
    runnable: RunnableVariant,
}

impl PartialEq for TimerEntry {
    fn eq(&self, other: &Self) -> bool {
        self.due == other.due && self.seq == other.seq
    }
}

impl Eq for TimerEntry {}

impl PartialOrd for TimerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimerEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reversed so that the entry with the earliest due time (breaking ties
        // by insertion order) is at the top of the max-heap.
        other
            .due
            .cmp(&self.due)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

impl Default for ThreadedDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadedDispatcher {
    /// 创建一个主线程是调用线程的调度器。
    ///
    /// 工作线程和计时器线程的生命周期与进程相同；
    /// 调度器应创建一次并在多个基准测试中重用。
    pub fn new() -> Self {
        let (background_sender, background_receiver) = PriorityQueueReceiver::new();
        let (main_sender, main_receiver) = PriorityQueueReceiver::new();
        let idle = Arc::new(IdleTracker::default());

        let thread_count =
            thread::available_parallelism().map_or(MIN_THREADS, |i| i.get().max(MIN_THREADS));
        for i in 0..thread_count {
            let mut receiver: PriorityQueueReceiver<RunnableVariant> = background_receiver.clone();
            let idle = idle.clone();
            thread::Builder::new()
                .name(format!("ThreadedDispatcherWorker-{i}"))
                .spawn(move || {
                    while let Ok(runnable) = receiver.pop() {
                        let _decrement = idle.decrement_on_drop();
                        let location = runnable.metadata().location;
                        let spawned = runnable.metadata().spawned;
                        profiler::update_running_task(spawned, location);
                        runnable.run();
                        profiler::save_task_timing();
                    }
                })
                .expect("failed to spawn threaded dispatcher worker");
        }
        drop(background_receiver);

        let timers = Arc::new(TimerQueue {
            state: Mutex::new(TimerQueueState {
                heap: BinaryHeap::new(),
                next_seq: 0,
            }),
            condvar: Condvar::new(),
        });
        {
            let timers = timers.clone();
            let idle = idle.clone();
            thread::Builder::new()
                .name("ThreadedDispatcherTimer".to_owned())
                .spawn(move || {
                    let mut state = timers.state.lock();
                    loop {
                        let Some(entry) = state.heap.peek() else {
                            timers.condvar.wait(&mut state);
                            continue;
                        };
                        let due = entry.due;
                        if due > Instant::now() {
                            timers.condvar.wait_until(&mut state, due);
                            continue;
                        }
                        let Some(entry) = state.heap.pop() else {
                            continue;
                        };
                        // Count the firing timer as in-flight before releasing
                        // the lock so it can spawn follow-up work that
                        // `run_until_idle` will wait for. Lock order is always
                        // timer state, then in-flight count; `run_until_idle`
                        // never takes them in the opposite order.
                        idle.increment();
                        drop(state);

                        {
                            let _decrement = idle.decrement_on_drop();
                            let location = entry.runnable.metadata().location;
                            let spawned = entry.runnable.metadata().spawned;
                            profiler::update_running_task(spawned, location);
                            entry.runnable.run();
                            profiler::save_task_timing();
                        }

                        state = timers.state.lock();
                    }
                })
                .expect("failed to spawn threaded dispatcher timer");
        }

        Self {
            background_sender,
            main_sender,
            main_receiver: Mutex::new(main_receiver),
            timers,
            idle,
            main_thread_id: thread::current().id(),
        }
    }

    /// 运行排队的主线程任务，并等待直到没有后台或计时器
    /// 工作排队、运行或已到期。
    ///
    /// 尚未到达到期时间的计时器*不*等待：调度器实时运行，
    /// 不能像 `TestDispatcher` 的虚拟时钟那样跳过，因此等待
    /// 未来的计时器会阻塞其完整的实际持续时间。在此类计时器上
    /// 休眠的任务被视为空闲。必须在创建此调度器的线程上调用。
    pub fn run_until_idle(&self) {
        assert!(
            self.is_main_thread(),
            "run_until_idle must be called on the benchmark main thread"
        );
        loop {
            if self.drain_main_queue() {
                continue;
            }

            // Checked before taking the in-flight lock; the timer thread
            // locks them in the opposite order, so nesting would deadlock.
            if self.has_due_timer() {
                // Poll briefly: a firing timer leaves the heap just before it
                // registers as in-flight.
                let mut inflight = self.idle.inflight.lock();
                self.idle
                    .condvar
                    .wait_for(&mut inflight, Duration::from_millis(1));
                continue;
            }

            let mut inflight = self.idle.inflight.lock();
            // Re-checked under the lock that `dispatch_on_main_thread`
            // notifies under, so the notification can't be lost.
            if self.main_queue_has_work() {
                continue;
            }
            if *inflight == 0 {
                // Main-thread sends happen before in-flight decrements, and
                // decrements happen under this lock, so the check above
                // observed all completed work.
                return;
            }
            // Woken when main-thread work arrives or the in-flight count
            // reaches zero; both notify under this lock.
            self.idle.condvar.wait(&mut inflight);
        }
    }

    /// 取消所有待处理的计时器，使一个基准测试设置的计时器不会在
    /// 后续共享此进程生命周期调度器的基准测试中触发。
    ///
    /// 丢弃计时器可运行对象会丢弃其完成发送器，唤醒等待计时器的任务。
    /// 在此方法后调用 [`Self::run_until_idle`] 以清空取消解除阻塞的任何工作。
    pub fn cancel_pending_timers(&self) -> usize {
        let timers = {
            let mut state = self.timers.state.lock();
            let timers: Vec<_> = state.heap.drain().collect();
            self.timers.condvar.notify_all();
            timers
        };
        let canceled = timers.len();
        drop(timers);
        canceled
    }

    /// 描述调度器的空闲跟踪状态，用于诊断未能达到静止状态的基准测试。
    pub fn debug_state(&self) -> String {
        let inflight = *self.idle.inflight.lock();
        let timers = self.timers.state.lock().heap.len();
        let main_queue_has_work = self.main_queue_has_work();
        format!(
            "ThreadedDispatcher {{ inflight: {inflight}, pending_timers: {timers}, \
             main_queue_has_work: {main_queue_has_work} }}"
        )
    }

    fn has_due_timer(&self) -> bool {
        let state = self.timers.state.lock();
        state
            .heap
            .peek()
            .is_some_and(|entry| entry.due <= Instant::now())
    }

    fn main_queue_has_work(&self) -> bool {
        !self.main_receiver.lock().is_empty()
    }

    fn drain_main_queue(&self) -> bool {
        let mut ran_any = false;
        loop {
            // Lock only around the pop so runnables can re-entrantly dispatch
            // more main-thread work through the sender while they run.
            let runnable = self.main_receiver.lock().try_pop();
            match runnable {
                Ok(Some(runnable)) => {
                    let location = runnable.metadata().location;
                    let spawned = runnable.metadata().spawned;
                    profiler::update_running_task(spawned, location);
                    runnable.run();
                    profiler::save_task_timing();
                    ran_any = true;
                }
                Ok(None) | Err(_) => return ran_any,
            }
        }
    }
}

impl PlatformDispatcher for ThreadedDispatcher {
    fn is_main_thread(&self) -> bool {
        thread::current().id() == self.main_thread_id
    }

    fn dispatch(&self, runnable: RunnableVariant, priority: Priority) {
        self.idle.increment();
        self.background_sender
            .send(priority, runnable)
            .unwrap_or_else(|_| panic!("threaded dispatcher workers are no longer running"));
    }

    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority) {
        if let Err(error) = self.main_sender.send(priority, runnable) {
            // The main receiver lives as long as this dispatcher, so a failed
            // send means we're mid-teardown. The runnable may wrap a !Send
            // future, so forget it rather than dropping it on this thread
            // (mirrors LinuxDispatcher).
            std::mem::forget(error);
            return;
        }
        // Wake `run_until_idle` if it's waiting for main-thread work.
        self.idle.notify_under_lock();
    }

    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
        let mut state = self.timers.state.lock();
        let seq = state.next_seq;
        state.next_seq += 1;
        state.heap.push(TimerEntry {
            due: Instant::now() + duration,
            seq,
            runnable,
        });
        self.timers.condvar.notify_one();
    }

    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
        // Benchmarks don't need realtime scheduling priority; a plain thread
        // keeps this portable.
        thread::Builder::new()
            .name("ThreadedDispatcherRealtime".to_owned())
            .spawn(f)
            .expect("failed to spawn benchmark realtime thread");
    }

    fn as_threaded(&self) -> Option<&ThreadedDispatcher> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use crate::{BackgroundExecutor, ForegroundExecutor};

    #[test]
    fn run_until_idle_completes_background_to_main_handoffs() {
        let dispatcher = Arc::new(ThreadedDispatcher::new());
        let background = BackgroundExecutor::new(dispatcher.clone());
        let foreground = ForegroundExecutor::new(dispatcher.clone());

        let (sender, receiver) = futures::channel::oneshot::channel();
        background
            .spawn(async move {
                thread::sleep(Duration::from_millis(10));
                sender.send(()).ok();
            })
            .detach();

        let completed = Arc::new(AtomicBool::new(false));
        foreground
            .spawn({
                let completed = completed.clone();
                async move {
                    receiver.await.ok();
                    completed.store(true, Ordering::SeqCst);
                }
            })
            .detach();

        dispatcher.run_until_idle();
        assert!(completed.load(Ordering::SeqCst));
    }

    #[test]
    fn timers_fire_in_real_time() {
        let dispatcher = Arc::new(ThreadedDispatcher::new());
        let background = BackgroundExecutor::new(dispatcher);

        let fired = Arc::new(AtomicBool::new(false));
        let timer = background.timer(Duration::from_millis(10));
        background
            .spawn({
                let fired = fired.clone();
                async move {
                    timer.await;
                    fired.store(true, Ordering::SeqCst);
                }
            })
            .detach();

        let deadline = Instant::now() + Duration::from_secs(10);
        while !fired.load(Ordering::SeqCst) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn cancel_pending_timers_wakes_waiters_without_waiting_for_deadline() {
        let dispatcher = Arc::new(ThreadedDispatcher::new());
        let background = BackgroundExecutor::new(dispatcher.clone());

        let fired = Arc::new(AtomicBool::new(false));
        let timer = background.timer(Duration::from_secs(10));
        background
            .spawn({
                let fired = fired.clone();
                async move {
                    timer.await;
                    fired.store(true, Ordering::SeqCst);
                }
            })
            .detach();

        dispatcher.run_until_idle();
        assert_eq!(dispatcher.cancel_pending_timers(), 1);
        dispatcher.run_until_idle();

        assert!(fired.load(Ordering::SeqCst));
        assert_eq!(dispatcher.cancel_pending_timers(), 0);
    }
}
