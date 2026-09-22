//! Android 前后台任务分发器。
//!
//! 双队列模型（与 Linux 一致）：
//! * **前台任务**：经 pipe 唤醒的 `ALooper` 回调，跑在 Android 主线程
//!   （拥有 `ANativeWindow`、处理输入的同一线程）；
//! * **后台任务**：Rust 线程池；
//! * **延后任务**：`tick()` 由主循环每轮驱动（见 `bridge`）。
//!
//! 主机（单测/文档构建）无 `ALooper`：前台任务堆在队列里，
//! 由 `flush_main_thread_tasks()` 手动排空。

use rgpui::{PlatformDispatcher, Priority, RunnableVariant};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// 可装箱的主线程任务。
type BoxedTask = Box<dyn FnOnce() + Send + 'static>;

/// 延后任务（到点后转交线程池）。
struct DelayedTask {
    /// 到期时间。
    due: Instant,
    /// 任务体。
    task: BoxedTask,
}

/// 极简固定线程池（后台任务用）。
struct ThreadPool {
    /// 投递端（Tasks 进，工作线程出）。
    sender: std::sync::mpsc::Sender<BoxedTask>,
}

impl ThreadPool {
    /// 建 `threads` 个工作线程的池。
    fn new(threads: usize) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel::<BoxedTask>();
        let receiver = Arc::new(parking_lot::Mutex::new(receiver));
        for index in 0..threads {
            let queue = Arc::clone(&receiver);
            thread::Builder::new()
                .name(format!("rgpui-bg-{index}"))
                .spawn(move || {
                    loop {
                        let task = { queue.lock().recv() };
                        match task {
                            Ok(task) => task(),
                            Err(_) => break,
                        }
                    }
                })
                .expect("后台线程创建失败");
        }
        Self { sender }
    }

    /// 投递后台任务（池关闭时静默丢弃）。
    fn dispatch(&self, task: BoxedTask) {
        let _ = self.sender.send(task);
    }
}

/// Android 任务分发器。
pub struct AndroidDispatcher {
    /// 主线程任务队列。
    main_queue: parking_lot::Mutex<VecDeque<BoxedTask>>,
    /// 后台线程池。
    pool: ThreadPool,
    /// 延后任务（按到期升序）。
    delayed: parking_lot::Mutex<Vec<DelayedTask>>,
    /// 构造线程标识（主机回退的主线程判定；Android 用 looper 指针）。
    main_thread_id: thread::ThreadId,
    /// 关闭标记（置位后不再接新任务）。
    shutdown: AtomicBool,
    /// Android 主线程 looper（主机为空）。
    #[cfg(target_os = "android")]
    looper: *mut std::ffi::c_void,
    /// 唤醒管写端（Android 有 looper 时有效，否则 -1）。
    #[cfg(target_os = "android")]
    wake_write_fd: std::os::unix::io::RawFd,
    /// 唤醒管读端（Android 有 looper 时有效，否则 -1；拥有以便关闭）。
    #[cfg(target_os = "android")]
    wake_read_fd: std::os::unix::io::RawFd,
}

// SAFETY：`looper` 裸指针只在主线程使用（注册/注销），
// 跨线程只调文档注明线程安全的 `ALooper_wake`；其余字段经锁/`Arc` 共享。
#[cfg(target_os = "android")]
unsafe impl Send for AndroidDispatcher {}
#[cfg(target_os = "android")]
unsafe impl Sync for AndroidDispatcher {}

impl AndroidDispatcher {
    /// 在当前线程构造分发器（Android 下必须在主线程，取其 looper）。
    pub fn new() -> Arc<Self> {
        let pool_threads = thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(4)
            .max(2);
        let this = Arc::new(Self {
            main_queue: parking_lot::Mutex::new(VecDeque::new()),
            pool: ThreadPool::new(pool_threads),
            delayed: parking_lot::Mutex::new(Vec::new()),
            main_thread_id: thread::current().id(),
            shutdown: AtomicBool::new(false),
            #[cfg(target_os = "android")]
            looper: std::ptr::null_mut(),
            #[cfg(target_os = "android")]
            wake_write_fd: -1,
            #[cfg(target_os = "android")]
            wake_read_fd: -1,
        });
        #[cfg(target_os = "android")]
        this.register_with_looper();
        this
    }

    /// 头模式构造（无真实 looper；与 [`Self::new`] 同语义，专供单测命名）。
    pub fn new_headless() -> Arc<Self> {
        Self::new()
    }

    /// 调用线程是否主线程。
    pub fn is_main_thread(&self) -> bool {
        #[cfg(target_os = "android")]
        {
            // 同一 looper 指针即同一线程；空 looper 回退线程标识比较。
            let current = unsafe { android_looper_for_thread() };
            if current.is_null() || self.looper.is_null() {
                return thread::current().id() == self.main_thread_id;
            }
            current == self.looper
        }
        #[cfg(not(target_os = "android"))]
        {
            thread::current().id() == self.main_thread_id
        }
    }

    /// 下一个延后任务的到期时间（主循环据此休眠，避免空转）。
    pub fn next_delayed_due(&self) -> Option<Instant> {
        self.delayed.lock().first().map(|task| task.due)
    }

    /// 跑一轮延后任务（到期者转交线程池；主循环每轮调用）。
    pub fn tick(&self) {
        let now = Instant::now();
        let mut ready = Vec::new();
        {
            let mut delayed = self.delayed.lock();
            while delayed.first().is_some_and(|task| task.due <= now) {
                ready.push(delayed.remove(0).task);
            }
        }
        for task in ready {
            self.pool.dispatch(task);
        }
    }

    /// 同步排空主线程队列（返回执行的任务数；无真实 looper 时的驱动方式）。
    pub fn flush_main_thread_tasks(&self) -> usize {
        let mut ran = 0;
        loop {
            let task = { self.main_queue.lock().pop_front() };
            match task {
                Some(task) => {
                    task();
                    ran += 1;
                }
                None => break,
            }
        }
        ran
    }

    /// 推一个主线程任务（Android 下同时写唤醒管叫醒 looper）。
    fn push_main_task(&self, task: BoxedTask) {
        if self.shutdown.load(Ordering::Relaxed) {
            return;
        }
        self.main_queue.lock().push_back(task);
        #[cfg(target_os = "android")]
        self.wake_looper();
    }

    /// 推一个延后任务（并按到期排序；Android 下叫醒 looper 重算超时）。
    fn push_delayed_task(&self, delay: Duration, task: BoxedTask) {
        if self.shutdown.load(Ordering::Relaxed) {
            return;
        }
        {
            let mut delayed = self.delayed.lock();
            delayed.push(DelayedTask {
                due: Instant::now() + delay,
                task,
            });
            delayed.sort_by_key(|task| task.due);
        }
        #[cfg(target_os = "android")]
        self.wake_looper();
    }
}

impl Default for AndroidDispatcher {
    /// 默认构造（与 [`Self::new`] 同，要求主线程语义见 `new`）。
    fn default() -> Self {
        // Arc 拆包：默认构造仅主机单测用，主线程即构造线程。
        Self {
            main_queue: parking_lot::Mutex::new(VecDeque::new()),
            pool: ThreadPool::new(2),
            delayed: parking_lot::Mutex::new(Vec::new()),
            main_thread_id: thread::current().id(),
            shutdown: AtomicBool::new(false),
            #[cfg(target_os = "android")]
            looper: std::ptr::null_mut(),
            #[cfg(target_os = "android")]
            wake_write_fd: -1,
            #[cfg(target_os = "android")]
            wake_read_fd: -1,
        }
    }
}

impl PlatformDispatcher for AndroidDispatcher {
    fn is_main_thread(&self) -> bool {
        AndroidDispatcher::is_main_thread(self)
    }

    fn dispatch(&self, runnable: RunnableVariant, _priority: Priority) {
        if self.shutdown.load(Ordering::Relaxed) {
            return;
        }
        self.pool.dispatch(Box::new(move || {
            runnable.run();
        }));
    }

    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, _priority: Priority) {
        self.push_main_task(Box::new(move || {
            runnable.run();
        }));
    }

    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
        self.push_delayed_task(
            duration,
            Box::new(move || {
                runnable.run();
            }),
        );
    }

    fn spawn_realtime(&self, task: Box<dyn FnOnce() + Send>) {
        thread::Builder::new()
            .name("rgpui-realtime".to_string())
            .spawn(task)
            .expect("实时线程创建失败");
    }
}

// ── Android looper 接线（仅真机） ─────────────────────────────────────────────

/// 当前线程 looper（`ALooper_forThread`，空表无）。
#[cfg(target_os = "android")]
unsafe fn android_looper_for_thread() -> *mut std::ffi::c_void {
    unsafe { ndk_sys::ALooper_forThread() as *mut std::ffi::c_void }
}

#[cfg(target_os = "android")]
mod looper_glue {
    use super::*;
    use std::os::unix::io::RawFd;

    /// looper 事件：fd 可读。
    const EVENT_INPUT: i32 = 1;

    /// looper 回调：排空唤醒管并执行全部主线程任务（返回 1 表继续注册）。
    unsafe extern "C" fn main_queue_callback(
        fd: RawFd,
        _events: i32,
        data: *mut std::ffi::c_void,
    ) -> i32 {
        // data 为 Arc<Mutex<VecDeque>> 裸指针，用后不释放归属（注册期有效）。
        let queue = unsafe { &*(data as *const parking_lot::Mutex<VecDeque<BoxedTask>>) };
        let mut discard = [0u8; 64];
        loop {
            let read = unsafe {
                libc::read(
                    fd,
                    discard.as_mut_ptr() as *mut std::ffi::c_void,
                    discard.len(),
                )
            };
            if read <= 0 {
                break;
            }
        }
        loop {
            let task = { queue.lock().pop_front() };
            match task {
                Some(task) => task(),
                None => break,
            }
        }
        1
    }

    impl AndroidDispatcher {
        /// 向主线程 looper 注册唤醒管（构造时调用一次）。
        pub(super) fn register_with_looper(&self) {
            let looper = unsafe { super::android_looper_for_thread() };
            assert!(
                !looper.is_null(),
                "AndroidDispatcher 必须在 Android 主线程构造"
            );
            // 持有 looper 引用计数：拥有线程退出后 looper 才析构；
            // 分发器 Drop 时先注销再释放，保证注销永远落在活 looper 上
            // （返回键退出后同进程重进，旧线程已死，不持有会导致
            // `ALooper_removeFd` 碰已销毁 mutex 直接 SIGABRT）。
            unsafe {
                ndk_sys::ALooper_acquire(looper as *mut ndk_sys::ALooper);
            };
            let mut fds = [0 as RawFd; 2];
            assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0, "唤醒管创建失败");
            for fd in fds {
                let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
                if flags >= 0 {
                    unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
                }
            }
            // 裸指针写回（Send/Sync 已在外层保证；仅构造线程触碰）。
            let this = self as *const Self as *mut Self;
            unsafe {
                (*this).looper = looper;
                (*this).wake_read_fd = fds[0];
                (*this).wake_write_fd = fds[1];
            }
            let queue_ptr = &self.main_queue as *const parking_lot::Mutex<VecDeque<BoxedTask>>
                as *mut std::ffi::c_void;
            let added = unsafe {
                ndk_sys::ALooper_addFd(
                    looper as *mut ndk_sys::ALooper,
                    fds[0],
                    0,
                    EVENT_INPUT,
                    Some(main_queue_callback),
                    queue_ptr,
                )
            };
            if added != 1 {
                log::warn!("ALooper_addFd 返回 {added}，前台分发可能失效");
            }
        }

        /// 叫醒阻塞中的 looper（任意线程可调）。
        pub(super) fn wake_looper(&self) {
            if self.wake_write_fd >= 0 {
                let byte = [1u8; 1];
                unsafe {
                    libc::write(
                        self.wake_write_fd,
                        byte.as_ptr() as *const std::ffi::c_void,
                        1,
                    );
                }
            }
            if !self.looper.is_null() {
                // SAFETY：`ALooper_wake` 文档注明任意线程可调。
                unsafe { ndk_sys::ALooper_wake(self.looper as *mut ndk_sys::ALooper) };
            }
        }
    }

    impl Drop for AndroidDispatcher {
        /// 注销 looper 并关闭唤醒管，最后释放 looper 引用。
        fn drop(&mut self) {
            self.shutdown.store(true, Ordering::SeqCst);
            if !self.looper.is_null() {
                unsafe {
                    ndk_sys::ALooper_removeFd(
                        self.looper as *mut ndk_sys::ALooper,
                        self.wake_read_fd,
                    );
                }
            }
            for fd in [self.wake_read_fd, self.wake_write_fd] {
                if fd >= 0 {
                    unsafe { libc::close(fd) };
                }
            }
            // 与构造时的 acquire 配对：looper 至此才允许析构。
            if !self.looper.is_null() {
                unsafe {
                    ndk_sys::ALooper_release(self.looper as *mut ndk_sys::ALooper);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// 后台线程池确实执行任务。
    #[test]
    fn background_tasks_run() {
        let pool = ThreadPool::new(2);
        let counter = Arc::new(AtomicUsize::new(0));
        for _ in 0..10 {
            let counter = Arc::clone(&counter);
            pool.dispatch(Box::new(move || {
                counter.fetch_add(1, Ordering::Relaxed);
            }));
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while counter.load(Ordering::Relaxed) < 10 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(counter.load(Ordering::Relaxed), 10);
    }

    /// 未到期的延后任务不执行，到期后转交线程池。
    #[test]
    fn delayed_tasks_fire_after_due() {
        let dispatcher = AndroidDispatcher::new();
        let ran = Arc::new(AtomicBool::new(false));
        let probe = Arc::clone(&ran);
        dispatcher.push_delayed_task(
            Duration::from_millis(30),
            Box::new(move || {
                probe.store(true, Ordering::Relaxed);
            }),
        );
        assert!(dispatcher.next_delayed_due().is_some());
        dispatcher.tick();
        assert!(!ran.load(Ordering::Relaxed));
        thread::sleep(Duration::from_millis(60));
        dispatcher.tick();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !ran.load(Ordering::Relaxed) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(ran.load(Ordering::Relaxed));
    }

    /// 主线程队列可同步排空并计数。
    #[test]
    fn main_queue_flush_runs_tasks() {
        let dispatcher = AndroidDispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        for _ in 0..3 {
            let counter = Arc::clone(&counter);
            dispatcher.push_main_task(Box::new(move || {
                counter.fetch_add(1, Ordering::Relaxed);
            }));
        }
        assert_eq!(dispatcher.flush_main_thread_tasks(), 3);
        assert_eq!(counter.load(Ordering::Relaxed), 3);
        assert_eq!(dispatcher.flush_main_thread_tasks(), 0);
    }
}
