use std::future::Future;

use rgpui::{App, AppContext, Global, ReadGlobal, Task};
use rgpui::defer;

pub use tokio::task::JoinError;

/// 初始化 Tokio 包装器，使用带有 2 个工作线程的新 Tokio 运行时。
///
/// 如果需要更多线程（或在 GPUI 外部访问运行时），可以自行创建运行时
/// 并将 Handle 传递给 `init_from_handle`。
pub fn init(cx: &mut App) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        // 由于现在有两个执行器，尽量保持占用最小
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("初始化 Tokio 失败");

    let handle = runtime.handle().clone();
    cx.set_global(GlobalTokio {
        owned_runtime: Some(runtime),
        handle,
    });
}

/// 使用 Tokio 运行时句柄初始化包装器。
pub fn init_from_handle(cx: &mut App, handle: tokio::runtime::Handle) {
    cx.set_global(GlobalTokio {
        owned_runtime: None,
        handle,
    });
}

/// 全局 Tokio 运行时包装器
struct GlobalTokio {
    /// 拥有的运行时（如果由本 crate 创建）
    owned_runtime: Option<tokio::runtime::Runtime>,
    /// 运行时句柄
    handle: tokio::runtime::Handle,
}

impl Global for GlobalTokio {}

impl Drop for GlobalTokio {
    /// 销毁时关闭运行时
    fn drop(&mut self) {
        if let Some(runtime) = self.owned_runtime.take() {
            runtime.shutdown_background();
        }
    }
}

/// Tokio 异步任务包装器
pub struct Tokio {}

impl Tokio {
    /// 在 Tokio 线程池上生成给定的 Future，并通过 GPUI 任务返回结果
    /// 注意：如果 GPUI 任务被丢弃，Tokio 任务也会被取消
    pub fn spawn<C, Fut, R>(cx: &C, f: Fut) -> Task<Result<R, JoinError>>
    where
        C: AppContext,
        Fut: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        cx.read_global(|tokio: &GlobalTokio, cx| {
            let join_handle = tokio.handle.spawn(f);
            let abort_handle = join_handle.abort_handle();
            let cancel = defer(move || {
                abort_handle.abort();
            });
            cx.background_spawn(async move {
                let result = join_handle.await;
                drop(cancel);
                result
            })
        })
    }

    /// 在 Tokio 线程池上生成给定的 Future，并通过 GPUI 任务返回 Result
    /// 注意：如果 GPUI 任务被丢弃，Tokio 任务也会被取消
    pub fn spawn_result<C, Fut, R>(cx: &C, f: Fut) -> Task<anyhow::Result<R>>
    where
        C: AppContext,
        Fut: Future<Output = anyhow::Result<R>> + Send + 'static,
        R: Send + 'static,
    {
        cx.read_global(|tokio: &GlobalTokio, cx| {
            let join_handle = tokio.handle.spawn(f);
            let abort_handle = join_handle.abort_handle();
            let cancel = defer(move || {
                abort_handle.abort();
            });
            cx.background_spawn(async move {
                let result = join_handle.await?;
                drop(cancel);
                result
            })
        })
    }

    /// 获取 Tokio 运行时句柄
    pub fn handle(cx: &App) -> tokio::runtime::Handle {
        GlobalTokio::global(cx).handle.clone()
    }
}
