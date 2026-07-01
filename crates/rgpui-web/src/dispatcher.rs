use rgpui::{
    PlatformDispatcher, Priority, PriorityQueueReceiver, PriorityQueueSender, RunnableVariant,
};
#[cfg(feature = "multithreaded")]
use std::sync::Arc;
#[cfg(feature = "multithreaded")]
use std::sync::atomic::AtomicI32;
use std::time::Duration;
use wasm_bindgen::prelude::*;
use web_time::Instant;

#[cfg(feature = "multithreaded")]
const MIN_BACKGROUND_THREADS: usize = 2;

/// 检查当前环境是否支持 SharedArrayBuffer 和 Atomics
#[cfg(feature = "multithreaded")]
fn shared_memory_supported() -> bool {
    let global = js_sys::global();
    let has_shared_array_buffer =
        js_sys::Reflect::has(&global, &JsValue::from_str("SharedArrayBuffer")).unwrap_or(false);
    let has_atomics = js_sys::Reflect::has(&global, &JsValue::from_str("Atomics")).unwrap_or(false);
    let memory = js_sys::WebAssembly::Memory::from(wasm_bindgen::memory());
    let buffer = memory.buffer();
    let is_shared_buffer = buffer.is_instance_of::<js_sys::SharedArrayBuffer>();
    has_shared_array_buffer && has_atomics && is_shared_buffer
}

/// 主线程任务项枚举
#[cfg(feature = "multithreaded")]
enum MainThreadItem {
    Runnable(RunnableVariant),
    Delayed {
        runnable: RunnableVariant,
        millis: i32,
    },
    // TODO-Wasm：这些是否应该在它们专用的线程上运行？
    RealtimeFunction(Box<dyn FnOnce() + Send>),
}

/// 主线程邮箱，用于从其他线程向主线程发送任务
#[cfg(feature = "multithreaded")]
struct MainThreadMailbox {
    sender: PriorityQueueSender<MainThreadItem>,
    receiver: parking_lot::Mutex<PriorityQueueReceiver<MainThreadItem>>,
    signal: AtomicI32,
}

#[cfg(feature = "multithreaded")]
impl MainThreadMailbox {
    fn new() -> Self {
        let (sender, receiver) = PriorityQueueReceiver::new();
        Self {
            sender,
            receiver: parking_lot::Mutex::new(receiver),
            signal: AtomicI32::new(0),
        }
    }

    fn post(&self, priority: Priority, item: MainThreadItem) {
        if self.sender.spin_send(priority, item).is_err() {
            log::error!("MainThreadMailbox::send failed: receiver disconnected");
        }

        // TODO-Wasm：验证此无锁协议
        let view = self.signal_view();
        js_sys::Atomics::store(&view, 0, 1).ok();
        js_sys::Atomics::notify(&view, 0).ok();
    }

    fn drain(&self, window: &web_sys::Window) {
        let mut receiver = self.receiver.lock();
        loop {
            // 我们需要这些 `spin` 变体，因为无法在主线程上获取锁
            // TODO-WASM：我们应该做不同的处理吗？
            match receiver.spin_try_pop() {
                Ok(Some(item)) => execute_on_main_thread(window, item),
                Ok(None) => break,
                Err(_) => break,
            }
        }
    }

    fn signal_view(&self) -> js_sys::Int32Array {
        let byte_offset = self.signal.as_ptr() as u32;
        let memory = js_sys::WebAssembly::Memory::from(wasm_bindgen::memory());
        js_sys::Int32Array::new_with_byte_offset_and_length(&memory.buffer(), byte_offset, 1)
    }

    fn run_waker_loop(self: &Arc<Self>, window: web_sys::Window) {
        if !shared_memory_supported() {
            log::warn!("SharedArrayBuffer not available; main thread mailbox waker loop disabled");
            return;
        }

        let mailbox = Arc::clone(self);
        wasm_bindgen_futures::spawn_local(async move {
            let view = mailbox.signal_view();
            loop {
                js_sys::Atomics::store(&view, 0, 0).expect("Atomics.store failed");

                let result = match js_sys::Atomics::wait_async(&view, 0, 0) {
                    Ok(result) => result,
                    Err(error) => {
                        log::error!("Atomics.waitAsync failed: {error:?}");
                        break;
                    }
                };

                let is_async = js_sys::Reflect::get(&result, &JsValue::from_str("async"))
                    .ok()
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if !is_async {
                    log::error!("Atomics.waitAsync returned synchronously; waker loop exiting");
                    break;
                }

                let promise: js_sys::Promise =
                    js_sys::Reflect::get(&result, &JsValue::from_str("value"))
                        .expect("waitAsync result missing 'value'")
                        .unchecked_into();

                let _ = wasm_bindgen_futures::JsFuture::from(promise).await;

                mailbox.drain(&window);
            }
        });
    }
}

/// Web 平台任务调度器，负责在主线程和后台线程之间分发任务
pub struct WebDispatcher {
    main_thread_id: std::thread::ThreadId,
    browser_window: web_sys::Window,
    background_sender: PriorityQueueSender<RunnableVariant>,
    #[cfg(feature = "multithreaded")]
    main_thread_mailbox: Arc<MainThreadMailbox>,
    supports_threads: bool,
    #[cfg(feature = "multithreaded")]
    _background_threads: Vec<wasm_thread::JoinHandle<()>>,
}

// 安全：`web_sys::Window` 仅从主线程访问
// 所有其他字段通过构造保证是 `Send + Sync`
unsafe impl Send for WebDispatcher {}
unsafe impl Sync for WebDispatcher {}

impl WebDispatcher {
    /// 创建新的 WebDispatcher 实例
    ///
    /// # 参数
    /// * `browser_window` - 浏览器窗口对象
    /// * `allow_threads` - 是否允许多线程调度
    pub fn new(browser_window: web_sys::Window, allow_threads: bool) -> Self {
        #[cfg(feature = "multithreaded")]
        let (background_sender, background_receiver) = PriorityQueueReceiver::new();
        #[cfg(not(feature = "multithreaded"))]
        let (background_sender, _) = PriorityQueueReceiver::new();

        #[cfg(feature = "multithreaded")]
        let main_thread_mailbox = Arc::new(MainThreadMailbox::new());

        #[cfg(feature = "multithreaded")]
        let supports_threads = allow_threads && shared_memory_supported();
        #[cfg(not(feature = "multithreaded"))]
        let supports_threads = {
            let _ = allow_threads;
            false
        };

        if supports_threads {
            #[cfg(feature = "multithreaded")]
            main_thread_mailbox.run_waker_loop(browser_window.clone());
        } else {
            log::warn!(
                "SharedArrayBuffer not available; falling back to single-threaded dispatcher"
            );
        }

        #[cfg(feature = "multithreaded")]
        let background_threads = if supports_threads {
            let thread_count = browser_window
                .navigator()
                .hardware_concurrency()
                .max(MIN_BACKGROUND_THREADS as f64) as usize;

            // TODO-Wasm：让 web worker 长时间阻塞是否有害？
            (0..thread_count)
                .map(|i| {
                    let mut receiver = background_receiver.clone();
                    wasm_thread::Builder::new()
                        .name(format!("background-worker-{i}"))
                        .spawn(move || {
                            loop {
                                let runnable: RunnableVariant = match receiver.pop() {
                                    Ok(runnable) => runnable,
                                    Err(_) => {
                                        log::info!(
                                            "background-worker-{i}: channel disconnected, exiting"
                                        );
                                        break;
                                    }
                                };

                                runnable.run();
                            }
                        })
                        .expect("failed to spawn background worker thread")
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        Self {
            main_thread_id: std::thread::current().id(),
            browser_window,
            background_sender,
            #[cfg(feature = "multithreaded")]
            main_thread_mailbox,
            supports_threads,
            #[cfg(feature = "multithreaded")]
            _background_threads: background_threads,
        }
    }

    /// 检查当前是否在主线程上运行
    fn on_main_thread(&self) -> bool {
        std::thread::current().id() == self.main_thread_id
    }
}

impl PlatformDispatcher for WebDispatcher {
    fn is_main_thread(&self) -> bool {
        self.on_main_thread()
    }

    fn dispatch(&self, runnable: RunnableVariant, priority: Priority) {
        if !self.supports_threads {
            self.dispatch_on_main_thread(runnable, priority);
            return;
        }

        let result = if self.on_main_thread() {
            self.background_sender.spin_send(priority, runnable)
        } else {
            self.background_sender.send(priority, runnable)
        };

        if let Err(error) = result {
            log::error!("dispatch: failed to send to background queue: {error:?}");
        }
    }

    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority) {
        if self.on_main_thread() {
            schedule_runnable(&self.browser_window, runnable, priority);
        } else {
            #[cfg(feature = "multithreaded")]
            self.main_thread_mailbox
                .post(priority, MainThreadItem::Runnable(runnable));
            #[cfg(not(feature = "multithreaded"))]
            schedule_runnable(&self.browser_window, runnable, priority);
        }
    }

    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
        let millis = duration.as_millis().min(i32::MAX as u128) as i32;
        if self.on_main_thread() {
            let callback = Closure::once_into_js(move || {
                runnable.run();
            });
            self.browser_window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    callback.unchecked_ref(),
                    millis,
                )
                .ok();
        } else {
            #[cfg(feature = "multithreaded")]
            self.main_thread_mailbox
                .post(Priority::High, MainThreadItem::Delayed { runnable, millis });
            #[cfg(not(feature = "multithreaded"))]
            {
                let callback = Closure::once_into_js(move || {
                    runnable.run();
                });
                self.browser_window
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        callback.unchecked_ref(),
                        millis,
                    )
                    .ok();
            }
        }
    }

    fn spawn_realtime(&self, function: Box<dyn FnOnce() + Send>) {
        if self.on_main_thread() {
            let callback = Closure::once_into_js(move || {
                function();
            });
            self.browser_window
                .queue_microtask(callback.unchecked_ref());
        } else {
            #[cfg(feature = "multithreaded")]
            self.main_thread_mailbox
                .post(Priority::High, MainThreadItem::RealtimeFunction(function));
            #[cfg(not(feature = "multithreaded"))]
            {
                let callback = Closure::once_into_js(move || {
                    function();
                });
                self.browser_window
                    .queue_microtask(callback.unchecked_ref());
            }
        }
    }

    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// 在主线程上执行任务项
#[cfg(feature = "multithreaded")]
fn execute_on_main_thread(window: &web_sys::Window, item: MainThreadItem) {
    match item {
        MainThreadItem::Runnable(runnable) => {
            runnable.run();
        }
        MainThreadItem::Delayed { runnable, millis } => {
            let callback = Closure::once_into_js(move || {
                runnable.run();
            });
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    callback.unchecked_ref(),
                    millis,
                )
                .ok();
        }
        MainThreadItem::RealtimeFunction(function) => {
            function();
        }
    }
}

/// 根据优先级调度任务执行
fn schedule_runnable(window: &web_sys::Window, runnable: RunnableVariant, priority: Priority) {
    let callback = Closure::once_into_js(move || {
        runnable.run();
    });
    let callback: &js_sys::Function = callback.unchecked_ref();

    match priority {
        Priority::RealtimeAudio => {
            window.queue_microtask(callback);
        }
        _ => {
            // TODO-Wasm：这里应该入队以便我们能以正确的优先级出队
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(callback, 0)
                .ok();
        }
    }
}
