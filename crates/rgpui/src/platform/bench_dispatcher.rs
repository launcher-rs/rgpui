use std::{
    collections::BinaryHeap,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use parking_lot::{Condvar, Mutex};

use crate::{
    PlatformDispatcher, Priority, RunnableVariant, profiler,
    queue::{PriorityQueueReceiver, PriorityQueueSender},
};

const MIN_THREADS: usize = 2;

/// 用于基准测试的多线程 [`PlatformDispatcher`]
///
/// 后台任务在工作线程池上并行运行，计时器在专用计时器线程上实时触发，镜像生产环境的调度器（参见 `LinuxDispatcher`）。主线程任务在基准测试线程通过 [`Self::run_until_idle`] 排空它们之前排队，因为没有平台运行循环来泵送它们
///
/// 与在单线程上使用虚拟时钟运行所有内容的 [`TestDispatcher`](crate::TestDispatcher) 不同，通过此调度器分派的工作以生产环境并发性执行，因此墙钟测量反映真实的并行性
pub struct BenchDispatcher {
    background_sender: PriorityQueueSender<RunnableVariant>,
    main_sender: PriorityQueueSender<RunnableVariant>,
    main_receiver: Mutex<PriorityQueueReceiver<RunnableVariant>>,
    timers: Arc<TimerQueue>,
    idle: Arc<IdleTracker>,
    main_thread_id: thread::ThreadId,
}

/// 跟踪有多少后台和计时器 runnable 排队或运行中，以便 [`BenchDispatcher::run_until_idle`] 知道何时停止等待
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

    /// 返回一个 guard，当被丢弃时递减飞行中计数，以便即使正在执行的 runnable 发生 panic，计数也保持正确
    fn decrement_on_drop(&self) -> impl Drop + '_ {
        rgpui_util::defer(|| self.decrement())
    }

    /// 在持有飞行中锁时通知等待者。`run_until_idle` 在此锁下重新检查其唤醒条件，然后再等待，因此通知不会在其检查和等待之间丢失
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
        // 反转以便最早到期时间的条目（按插入顺序断开平局）位于最大堆顶部
        other
            .due
            .cmp(&self.due)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

impl Default for BenchDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchDispatcher {
    /// 创建一个主线程是调用线程的调度器
    ///
    /// 工作线程和计时器线程在进程的生命周期内存在；调度器预期被创建一次并在基准测试之间重用
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
                .name(format!("BenchWorker-{i}"))
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
                .expect("failed to spawn benchmark worker thread");
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
                .name("BenchTimer".to_owned())
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
                        // 在释放锁之前将触发的计时器计为飞行中，以便它可以生成 `run_until_idle` 将等待的后续工作。锁顺序始终是计时器状态，然后飞行中计数；`run_until_idle` 永远不会以相反顺序获取它们
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
                .expect("failed to spawn benchmark timer thread");
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

    /// 运行排队的主线程任务并等待直到没有后台或计时器工作排队、运行中或已到期
    ///
    /// 尚未达到到期时间的计时器*不*被等待：调度器实时运行，无法像 `TestDispatcher` 的虚拟时钟那样跳过 ahead，因此等待未来的计时器将阻塞其完整的实际持续时间。在此类计时器上休眠的任务被认为处于空闲状态。必须在创建此调度器的线程上调用
    pub fn run_until_idle(&self) {
        assert!(
            self.is_main_thread(),
            "run_until_idle must be called on the benchmark main thread"
        );
        loop {
            if self.drain_main_queue() {
                continue;
            }

            // 在获取飞行中锁之前检查；计时器线程以相反顺序锁定它们，因此嵌套将导致死锁
            if self.has_due_timer() {
                // 简短轮询：触发的计时器在注册为飞行中之前离开堆
                let mut inflight = self.idle.inflight.lock();
                self.idle
                    .condvar
                    .wait_for(&mut inflight, Duration::from_millis(1));
                continue;
            }

            let mut inflight = self.idle.inflight.lock();
            // 在 `dispatch_on_main_thread` 通知下的锁下重新检查，因此通知不会丢失
            if self.main_queue_has_work() {
                continue;
            }
            if *inflight == 0 {
                // 主线程发送发生在飞行中递减之前，递减发生在此锁下，因此上面的检查观察到了所有已完成的工作
                return;
            }
            // 当主线程工作到达或飞行中计数达到零时唤醒；两者都在此锁下通知
            self.idle.condvar.wait(&mut inflight);
        }
    }

    /// 遗忘所有待处理的计时器，以便一个基准测试武装的计时器不会在共享此进程生命周期调度器的后续基准测试期间触发
    ///
    /// runnable 被泄漏而不是丢弃，因为丢弃一个会唤醒等待的任务，就像计时器已触发一样
    pub fn forget_pending_timers(&self) {
        let mut state = self.timers.state.lock();
        for entry in state.heap.drain() {
            std::mem::forget(entry.runnable);
        }
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
            // 仅在弹出时加锁，以便 runnable 可以在运行时通过 sender 重入分派更多主线程工作
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

impl PlatformDispatcher for BenchDispatcher {
    fn is_main_thread(&self) -> bool {
        thread::current().id() == self.main_thread_id
    }

    fn dispatch(&self, runnable: RunnableVariant, priority: Priority) {
        self.idle.increment();
        self.background_sender
            .send(priority, runnable)
            .unwrap_or_else(|_| panic!("benchmark worker threads are no longer running"));
    }

    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority) {
        if let Err(error) = self.main_sender.send(priority, runnable) {
            // 主接收器与此调度器的生命周期一样长，因此发送失败意味着我们正在拆解中。runnable 可能包装了 !Send future，因此忘记它而不是在此线程上丢弃它（镜像 LinuxDispatcher）
            std::mem::forget(error);
            return;
        }
        // 如果 `run_until_idle` 正在等待主线程工作，则唤醒它
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
        // 基准测试不需要实时调度优先级；普通线程使其可移植
        thread::Builder::new()
            .name("BenchRealtime".to_owned())
            .spawn(f)
            .expect("failed to spawn benchmark realtime thread");
    }

    fn as_bench(&self) -> Option<&BenchDispatcher> {
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
        let dispatcher = Arc::new(BenchDispatcher::new());
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
        let dispatcher = Arc::new(BenchDispatcher::new());
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
    fn forget_pending_timers_prevents_stale_timers_from_firing() {
        let dispatcher = Arc::new(BenchDispatcher::new());
        let background = BackgroundExecutor::new(dispatcher.clone());

        let fired = Arc::new(AtomicBool::new(false));
        let timer = background.timer(Duration::from_millis(250));
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
        dispatcher.forget_pending_timers();

        thread::sleep(Duration::from_millis(400));
        dispatcher.run_until_idle();
        assert!(!fired.load(Ordering::SeqCst));
    }
}
