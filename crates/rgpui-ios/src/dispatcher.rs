//! iOS 前后台任务分发器（M1 桩实现，M4 换 GCD 真实现）。

use rgpui::{PlatformDispatcher, Priority, RunnableVariant};
use std::thread;
use std::time::Duration;

/// iOS 任务分发器。
pub struct IosDispatcher {
    /// 构造时的主线程标识。
    main_thread_id: thread::ThreadId,
}

impl IosDispatcher {
    /// 在当前线程构造分发器，并将当前线程视为主线程。
    pub fn new() -> Self {
        Self {
            main_thread_id: thread::current().id(),
        }
    }
}

impl Default for IosDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformDispatcher for IosDispatcher {
    fn is_main_thread(&self) -> bool {
        thread::current().id() == self.main_thread_id
    }

    fn dispatch(&self, runnable: RunnableVariant, _priority: Priority) {
        thread::spawn(move || {
            runnable.run();
        });
    }

    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, _priority: Priority) {
        if self.is_main_thread() {
            runnable.run();
        } else {
            thread::spawn(move || {
                runnable.run();
            });
        }
    }

    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
        thread::spawn(move || {
            thread::sleep(duration);
            runnable.run();
        });
    }

    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
        thread::spawn(move || {
            f();
        });
    }
}
