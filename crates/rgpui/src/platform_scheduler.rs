use crate::{PlatformDispatcher, RunnableMeta};
use async_task::Runnable;
use chrono::{DateTime, Utc};
use futures::channel::oneshot;
use crate::scheduler::Instant;
use crate::scheduler::{Clock, Priority, Scheduler, SessionId, TestScheduler, Timer};
#[cfg(not(target_family = "wasm"))]
use std::task::{Context, Poll};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU16, Ordering},
    },
    time::Duration,
};

/// [`Scheduler`] 的生产实现，包装 [`PlatformDispatcher`]。
///
/// 这允许 GPUI 将调度器 crate 的执行器类型与平台的
/// 原生调度机制一起使用（例如 macOS 上的 Grand Central Dispatch）。
pub struct PlatformScheduler {
    dispatcher: Arc<dyn PlatformDispatcher>,
    clock: Arc<PlatformClock>,
    next_session_id: AtomicU16,
}

impl PlatformScheduler {
    pub fn new(dispatcher: Arc<dyn PlatformDispatcher>) -> Self {
        Self {
            dispatcher: dispatcher.clone(),
            clock: Arc::new(PlatformClock { dispatcher }),
            next_session_id: AtomicU16::new(0),
        }
    }

    pub fn allocate_session_id(&self) -> SessionId {
        SessionId::new(self.next_session_id.fetch_add(1, Ordering::SeqCst))
    }
}

impl Scheduler for PlatformScheduler {
    fn block(
        &self,
        _session_id: Option<SessionId>,
        #[cfg_attr(target_family = "wasm", allow(unused_mut))] mut future: Pin<
            &mut dyn Future<Output = ()>,
        >,
        #[cfg_attr(target_family = "wasm", allow(unused_variables))] timeout: Option<Duration>,
    ) -> bool {
        #[cfg(target_family = "wasm")]
        {
            let _ = (&future, &timeout);
            panic!("Cannot block on wasm")
        }
        #[cfg(not(target_family = "wasm"))]
        {
            use waker_fn::waker_fn;
            let deadline = timeout.map(|t| Instant::now() + t);
            let parker = parking::Parker::new();
            let unparker = parker.unparker();
            let waker = waker_fn(move || {
                unparker.unpark();
            });
            let mut cx = Context::from_waker(&waker);
            if let Poll::Ready(()) = future.as_mut().poll(&mut cx) {
                return true;
            }

            let park_deadline = |deadline: Instant| {
                // Windows 上默认情况下定时器过期仅每约 15.6 毫秒传递一次。
                // 我们在此等待期间提高分辨率，以便短超时保持合理短。
                let _timer_guard = self.dispatcher.increase_timer_resolution();
                parker.park_deadline(deadline)
            };

            loop {
                match deadline {
                    Some(deadline) if !park_deadline(deadline) && deadline <= Instant::now() => {
                        return false;
                    }
                    Some(_) => (),
                    None => parker.park(),
                }
                if let Poll::Ready(()) = future.as_mut().poll(&mut cx) {
                    break true;
                }
            }
        }
    }

    fn schedule_foreground(&self, _session_id: SessionId, runnable: Runnable<RunnableMeta>) {
        self.dispatcher
            .dispatch_on_main_thread(runnable, Priority::default());
    }

    fn schedule_background_with_priority(
        &self,
        runnable: Runnable<RunnableMeta>,
        priority: Priority,
    ) {
        self.dispatcher.dispatch(runnable, priority);
    }

    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
        self.dispatcher.spawn_realtime(f);
    }

    #[track_caller]
    fn timer(&self, duration: Duration) -> Timer {
        let (tx, rx) = oneshot::channel();
        let dispatcher = self.dispatcher.clone();

        // 创建将发送完成信号的可运行任务
        let location = std::panic::Location::caller();
        let (runnable, _task) = async_task::Builder::new()
            .metadata(RunnableMeta { location })
            .spawn(
                move |_| async move {
                    let _ = tx.send(());
                },
                move |runnable| {
                    dispatcher.dispatch_after(duration, runnable);
                },
            );
        runnable.schedule();

        Timer::new(rx)
    }

    fn clock(&self) -> Arc<dyn Clock> {
        self.clock.clone()
    }

    fn as_test(&self) -> Option<&TestScheduler> {
        None
    }
}

/// 使用平台调度器时间的生产时钟。
struct PlatformClock {
    dispatcher: Arc<dyn PlatformDispatcher>,
}

impl Clock for PlatformClock {
    fn utc_now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn now(&self) -> Instant {
        self.dispatcher.now()
    }
}
