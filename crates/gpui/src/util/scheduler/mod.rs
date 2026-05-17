mod clock;
mod executor;
mod test_scheduler;
#[cfg(test)]
mod tests;

pub use clock::*;
pub use executor::*;
pub use test_scheduler::*;

use async_task::Runnable;
use futures::channel::oneshot;
use std::{
    future::Future,
    panic::Location,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

/// 后台任务的任务优先级。
///
/// 高优先级任务更可能在低优先级任务之前被调度，
/// 但这不是严格保证——调度器可能会交错不同优先级的任务
/// 以防止饥饿。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Priority {
    /// 实时优先级
    ///
    /// 使用此优先级生成的任务将在专用于该任务的单独线程上运行。仅用于音频。
    RealtimeAudio,
    /// 高优先级——用于对用户体验/响应能力至关重要的任务。
    High,
    /// 中优先级——适用于大多数用例。
    #[default]
    Medium,
    /// 低优先级——用于可以降优先级的后台工作。
    Low,
}

impl Priority {
    /// 返回此优先级级别的相对概率权重。
    /// 由调度器用于确定任务选择概率。
    pub const fn weight(self) -> u32 {
        match self {
            Priority::High => 60,
            Priority::Medium => 30,
            Priority::Low => 10,
            // realtime priorities are not considered for probability scheduling
            Priority::RealtimeAudio => 0,
        }
    }
}

/// 附加到可运行对象的元数据，用于调试和分析。
#[derive(Clone)]
pub struct RunnableMeta {
    /// 任务生成的源位置。
    pub location: &'static Location<'static>,
}

impl std::fmt::Debug for RunnableMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunnableMeta")
            .field("location", &self.location)
            .finish()
    }
}

pub trait Scheduler: Send + Sync {
    /// 阻塞直到给定未来对象完成或超时发生。
    ///
    /// 如果未来对象完成则返回 `true`，超时则返回 `false`。
    /// 未来对象作为固定可变引用传递，因此调用者
    /// 保留所有权并可以在超时后继续轮询或返回它。
    fn block(
        &self,
        session_id: Option<SessionId>,
        future: Pin<&mut dyn Future<Output = ()>>,
        timeout: Option<Duration>,
    ) -> bool;

    fn schedule_foreground(&self, session_id: SessionId, runnable: Runnable<RunnableMeta>);

    /// 调度具有给定优先级的后台任务。
    fn schedule_background_with_priority(
        &self,
        runnable: Runnable<RunnableMeta>,
        priority: Priority,
    );

    /// 在专用的实时线程上生成用于音频处理的闭包。
    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>);

    /// 使用默认（中）优先级调度后台任务。
    fn schedule_background(&self, runnable: Runnable<RunnableMeta>) {
        self.schedule_background_with_priority(runnable, Priority::default());
    }

    #[track_caller]
    fn timer(&self, timeout: Duration) -> Timer;
    fn clock(&self) -> Arc<dyn Clock>;

    fn as_test(&self) -> Option<&TestScheduler> {
        None
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SessionId(u16);

impl SessionId {
    pub fn new(id: u16) -> Self {
        SessionId(id)
    }
}

pub struct Timer(oneshot::Receiver<()>);

impl Timer {
    pub fn new(rx: oneshot::Receiver<()>) -> Self {
        Timer(rx)
    }
}

impl Future for Timer {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<()> {
        match Pin::new(&mut self.0).poll(cx) {
            Poll::Ready(_) => Poll::Ready(()),
            Poll::Pending => Poll::Pending,
        }
    }
}
