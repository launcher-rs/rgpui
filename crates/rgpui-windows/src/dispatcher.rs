use std::{
    cell::RefCell,
    sync::atomic::{AtomicBool, Ordering},
    thread::{ThreadId, current},
    time::Duration,
};

use anyhow::Context;
use rgpui::util::ResultExt;
use windows::{
    System::Threading::{
        ThreadPool, ThreadPoolTimer, TimerElapsedHandler, WorkItemHandler, WorkItemPriority,
    },
    Win32::{
        Foundation::{LPARAM, WPARAM},
        Media::{timeBeginPeriod, timeEndPeriod},
        System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL},
        UI::WindowsAndMessaging::PostMessageW,
    },
};

use crate::{HWND, SafeHwnd, WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD};
use rgpui::{
    PlatformDispatcher, Priority, PriorityQueueSender, RunnableVariant, TimerResolutionGuard,
};

/// Windows 平台任务调度器
///
/// 负责在 Windows 线程池上调度任务，支持不同优先级和延迟执行
pub(crate) struct WindowsDispatcher {
    /// 标记是否已发送唤醒消息
    pub(crate) wake_posted: AtomicBool,
    /// 主线程任务发送器
    main_sender: PriorityQueueSender<RunnableVariant>,
    /// 主线程 ID
    main_thread_id: ThreadId,
    /// 平台窗口句柄
    pub(crate) platform_window_handle: SafeHwnd,
    /// 验证编号，用于消息验证
    validation_number: usize,
}

impl WindowsDispatcher {
    /// 创建新的 Windows 调度器
    ///
    /// # 参数
    /// * `main_sender` - 主线程任务发送器
    /// * `platform_window_handle` - 平台窗口句柄
    /// * `validation_number` - 验证编号
    pub(crate) fn new(
        main_sender: PriorityQueueSender<RunnableVariant>,
        platform_window_handle: HWND,
        validation_number: usize,
    ) -> Self {
        let main_thread_id = current().id();
        let platform_window_handle = platform_window_handle.into();

        WindowsDispatcher {
            main_sender,
            main_thread_id,
            platform_window_handle,
            validation_number,
            wake_posted: AtomicBool::new(false),
        }
    }

    /// 在线程池上调度任务
    ///
    /// # 参数
    /// * `priority` - 任务优先级
    /// * `runnable` - 可执行任务
    fn dispatch_on_threadpool(&self, priority: WorkItemPriority, runnable: RunnableVariant) {
        let handler = {
            let task_wrapper = RefCell::new(Some(runnable));
            WorkItemHandler::new(move |_| {
                let runnable = task_wrapper.borrow_mut().take().unwrap();
                Self::execute_runnable(runnable);
                Ok(())
            })
        };

        ThreadPool::RunWithPriorityAsync(&handler, priority).log_err();
    }

    /// 在线程池上延迟调度任务
    ///
    /// # 参数
    /// * `runnable` - 可执行任务
    /// * `duration` - 延迟时间
    fn dispatch_on_threadpool_after(&self, runnable: RunnableVariant, duration: Duration) {
        let handler = {
            let task_wrapper = RefCell::new(Some(runnable));
            TimerElapsedHandler::new(move |_| {
                let runnable = task_wrapper.borrow_mut().take().unwrap();
                Self::execute_runnable(runnable);
                Ok(())
            })
        };
        ThreadPoolTimer::CreateTimer(&handler, duration.into()).log_err();
    }

    /// 执行可执行任务，并记录任务耗时
    ///
    /// # 参数
    /// * `runnable` - 要执行的任务
    #[inline(always)]
    pub(crate) fn execute_runnable(runnable: RunnableVariant) {
        let location = runnable.metadata().location;
        let spawned = runnable.metadata().spawned;
        rgpui::profiler::update_running_task(spawned, location);
        runnable.run();
        rgpui::profiler::save_task_timing();
    }
}

impl PlatformDispatcher for WindowsDispatcher {
    fn is_main_thread(&self) -> bool {
        current().id() == self.main_thread_id
    }

    fn dispatch(&self, runnable: RunnableVariant, priority: Priority) {
        let priority = match priority {
            Priority::RealtimeAudio => {
                panic!("RealtimeAudio 优先级应使用 spawn_realtime，而非 dispatch")
            }
            Priority::High => WorkItemPriority::High,
            Priority::Medium => WorkItemPriority::Normal,
            Priority::Low => WorkItemPriority::Low,
        };
        self.dispatch_on_threadpool(priority, runnable);
    }

    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority) {
        match self.main_sender.send(priority, runnable) {
            Ok(_) => {
                if !self.wake_posted.swap(true, Ordering::AcqRel) {
                    unsafe {
                        PostMessageW(
                            Some(self.platform_window_handle.as_raw()),
                            WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD,
                            WPARAM(self.validation_number),
                            LPARAM(0),
                        )
                        .log_err();
                    }
                }
            }
            Err(runnable) => {
                // 注意：Runnable 可能包装了 !Send 的 Future。
                //
                // 这通常是安全的，因为我们只在主线程上轮询它。
                // 但如果发送失败，我们知道：
                // 1. main_receiver 已被丢弃（意味着应用正在关闭）
                // 2. 我们当前在后台线程上。
                // 在错误的线程上丢弃 !Send 的对象是不安全的，而且
                // 应用即将退出，所以我们必须遗忘这个 runnable。
                std::mem::forget(runnable);
            }
        }
    }

    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
        self.dispatch_on_threadpool_after(runnable, duration);
    }

    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
        std::thread::spawn(move || {
            // 安全：始终安全可调用
            let thread_handle = unsafe { GetCurrentThread() };

            // 安全：thread_handle 是当前线程的有效句柄
            unsafe { SetThreadPriority(thread_handle, THREAD_PRIORITY_TIME_CRITICAL) }
                .context("thread priority")
                .log_err();

            f();
        });
    }

    fn increase_timer_resolution(&self) -> TimerResolutionGuard {
        unsafe {
            timeBeginPeriod(1);
        }
        rgpui::defer(Box::new(|| unsafe {
            timeEndPeriod(1);
        }))
    }
}
