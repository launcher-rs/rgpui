# scheduler

任务调度系统，支持优先级调度和多执行器模型。

## 描述

`scheduler` 提供了一个灵活的任务调度抽象层，支持前台任务、后台优先级调度和实时音频处理。通过 `Scheduler` trait 定义了统一的调度接口，并提供 `ForegroundExecutor` 和 `BackgroundExecutor` 两种执行器类型。

## 主要功能与类型

### 优先级

- **`Priority`** — 任务优先级枚举
  - `RealtimeAudio` — 实时优先级，在专用线程上运行（仅用于音频处理）
  - `High` — 高优先级，用于对用户体验关键的任务（权重 60）
  - `Medium` — 中优先级，默认级别（权重 30）
  - `Low` — 低优先级，用于可延迟的后台工作（权重 10）

### Scheduler Trait

- **`Scheduler`** — 核心调度器 trait
  - `block()` — 阻塞等待 future 完成或超时
  - `schedule_foreground()` — 调度前台任务
  - `schedule_background()` — 调度后台任务（默认中优先级）
  - `schedule_background_with_priority()` — 按优先级调度后台任务
  - `spawn_realtime()` — 在实时线程上执行闭包
  - `timer()` — 创建定时器
  - `clock()` — 获取时钟
  - `as_test()` — 获取测试调度器引用

### 执行器

- **`ForegroundExecutor`** — 前台任务执行器
  - 与特定 `SessionId` 关联
  - 不是 `Send` 类型，只能在创建的线程上使用
  - `spawn(future)` — 生成前台任务
  - `block_on(future)` — 阻塞等待 future 完成
  - `block_with_timeout(timeout, future)` — 带超时的阻塞等待
  - `timer(duration)` — 创建定时器
  - `now()` — 获取当前时间

- **`BackgroundExecutor`** — 后台任务执行器
  - `Send + Sync`，可在线程间传递
  - `spawn(future)` — 生成后台任务（默认中优先级）
  - `spawn_with_priority(priority, future)` — 按优先级生成任务
  - `spawn_retime(future)` — 在实时线程上生成任务
  - `timer(duration)` — 创建定时器
  - `now()` — 获取当前时间

### Task

- **`Task<T>`** — 异步任务包装器
  - 实现 `Future`，可 `.await`
  - drop 时自动取消
  - `ready(val)` — 创建已完成的任务
  - `is_ready()` — 检查是否已完成
  - `detach()` — 分离任务，使其在后台继续运行
  - `fallible()` — 转换为返回 `Option<T>` 的可失败任务

- **`FallibleTask<T>`** — 可失败任务，取消时返回 `None` 而非 panic
  - `ready(val)` — 创建已完成的可失败任务
  - `detach()` — 分离任务

### 时钟

- **`Clock`** — 时钟 trait
  - `now()` — 获取单调时间（`Instant`）
  - `utc_now()` — 获取 UTC 时间

- **`TestClock`** — 可控制的测试时钟
  - `new()` — 创建测试时钟（默认时间 2025-07-01T23:59:58）
  - `advance(duration)` — 推进时间
  - `set_utc_now(datetime)` — 设置 UTC 时间

### 其他类型

- **`SessionId`** — 会话标识符，用于前台任务分组
- **`Timer`** — 定时器 future，超时后完成
- **`RunnableMeta`** — 任务元数据，包含源位置信息用于调试
- **`Instant`** — 单调时间类型（来自 `web_time`）

## 使用示例

```rust
use scheduler::{Scheduler, ForegroundExecutor, BackgroundExecutor, Priority, SessionId};
use std::sync::Arc;

// 假设已有一个 Scheduler 实现
fn example(scheduler: Arc<dyn Scheduler>) {
    let session = SessionId::new(1);
    
    // 前台执行器
    let fg_executor = ForegroundExecutor::new(session, scheduler.clone());
    let task = fg_executor.spawn(async {
        // 在前台线程执行的任务
        println!("running on foreground thread");
        42
    });
    
    // 后台执行器
    let bg_executor = BackgroundExecutor::new(scheduler.clone());
    let bg_task = bg_executor.spawn_with_priority(Priority::High, async {
        // 高优先级后台任务
        do_heavy_work().await
    });
    
    // 阻塞等待
    let result = fg_executor.block_on(async {
        task.await + bg_task.await
    });
    
    // 带超时的阻塞
    match fg_executor.block_with_timeout(
        std::time::Duration::from_secs(5),
        some_slow_operation()
    ) {
        Ok(result) => println!("completed: {:?}", result),
        Err(future) => println!("timed out"),
    }
}
```

## 依赖关系

- `async-task` — 异步任务原语
- `futures` — Future 工具（oneshot 通道）
- `flume` (0.11) — 高性能多生产者多消费者通道
- `parking_lot` — 高性能锁（用于 TestClock）
- `chrono` — 日期时间处理
- `web-time` — 跨平台单调时间
- `backtrace` — 回溯信息
- `rand` — 随机数生成
