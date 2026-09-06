//! 防抖原语：连续触发时只执行最后一次请求。
//!
//! 典型场景是输入即搜（search-as-type）：每次按键都调 [`Debouncer::debounce`]，
//! 只有在 `duration` 内不再有新请求时，最后一次的回调才会运行。
//! 世代计数保证旧任务即使后完成也不会覆盖新结果。

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use crate::executor::BackgroundExecutor;

/// 防抖器：连续触发时只执行最后一次请求。
///
/// 无输入焦点要求，可放在任何实体/状态结构体里长期持有。
/// 回调在后台任务中运行，不得捕获非 `Send` 数据（如 `&mut Context`）；
/// 需要写回 UI 时，请在回调内经 channel/Atomic 传递，由 UI 线程消费，
/// 或用 `cx.update_entity`（实体存活时）写回。
pub struct Debouncer {
    generation: Arc<AtomicU64>,
}

impl Debouncer {
    /// 创建新的防抖器。
    pub fn new() -> Self {
        Self {
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 请求执行一次回调。
    ///
    /// 取消之前尚未执行的请求；若 `duration` 内没有更新的请求，
    /// 则在后台执行 `callback`。连续调用时只有最后一次生效。
    pub fn debounce(
        &self,
        executor: &BackgroundExecutor,
        duration: Duration,
        callback: impl FnOnce() + Send + 'static,
    ) {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let current = self.generation.clone();
        let timer = executor.clone();
        executor
            .spawn(async move {
                timer.timer(duration).await;
                if current.load(Ordering::SeqCst) == generation {
                    callback();
                }
            })
            .detach();
    }
}

impl Default for Debouncer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::TestDispatcher;

    /// 连续三次请求，只有最后一次的回调运行。
    #[test]
    fn only_last_request_runs() {
        let dispatcher = TestDispatcher::new(0);
        let executor = BackgroundExecutor::new(std::sync::Arc::new(dispatcher.clone()));
        let debouncer = Debouncer::new();
        let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        for _ in 0..3 {
            let hits = hits.clone();
            debouncer.debounce(&executor, Duration::from_millis(50), move || {
                hits.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        }

        dispatcher.advance_clock(Duration::from_millis(200));
        dispatcher.run_until_parked();

        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    /// 超时后又有新请求，旧回调被取消，只有新回调运行。
    #[test]
    fn stale_request_is_cancelled() {
        let dispatcher = TestDispatcher::new(0);
        let executor = BackgroundExecutor::new(std::sync::Arc::new(dispatcher.clone()));
        let debouncer = Debouncer::new();
        let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let hits_first = hits.clone();
        debouncer.debounce(&executor, Duration::from_millis(50), move || {
            hits_first.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        dispatcher.advance_clock(Duration::from_millis(200));
        dispatcher.run_until_parked();

        let hits_second = hits.clone();
        debouncer.debounce(&executor, Duration::from_millis(50), move || {
            hits_second.fetch_add(10, std::sync::atomic::Ordering::SeqCst);
        });
        dispatcher.advance_clock(Duration::from_millis(200));
        dispatcher.run_until_parked();

        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 11);
    }
}
