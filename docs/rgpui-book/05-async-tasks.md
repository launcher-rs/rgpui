# 异步任务

> 后台任务、前台执行器与定时器。

## 概述

rgpui 提供了两套异步任务执行器：
- **BackgroundExecutor** - 在后台线程池中执行任务
- **ForegroundExecutor** - 在主线程上执行任务

两者都基于 `futures` 库，支持 `async/await` 语法。

## 后台执行器（BackgroundExecutor）

用于执行不阻塞 UI 的后台任务：

```rust
use rgpui::Task;

// 生成后台任务
let task: Task<String> = cx.background_spawn(async {
    // 耗时操作，如网络请求、文件读取
    let result = fetch_data_from_server().await;
    result
});

// 等待任务完成
let data = task.await;
```

### 优先级

后台任务支持不同的优先级：

```rust
use rgpui::{BackgroundExecutor, Priority};

// 默认优先级
cx.background_spawn(async { /* ... */ });

// 高优先级任务
cx.background_spawn_with_priority(Priority::High, async { /* ... */ });

// 实时音频优先级（专用线程）
cx.background_spawn_with_priority(Priority::RealtimeAudio, async { /* ... */ });
```

### 作用域任务

批量启动多个后台任务并等待全部完成：

```rust
cx.background_scoped(|scope| {
    scope.spawn(async { /* 任务 1 */ });
    scope.spawn(async { /* 任务 2 */ });
    scope.spawn(async { /* 任务 3 */ });
}).await;
// 所有任务在此处完成
```

## 前台执行器（ForegroundExecutor）

用于在主线程上执行需要访问 UI 状态的任务：

```rust
// 生成前台任务
let task = cx.spawn(async |cx| {
    // 可以访问 cx（App 上下文）
    let data = fetch_data().await;
    
    // 在前台更新 UI
    cx.update(|cx| {
        // 更新实体状态
    });
});

task.await;
```

### 异步上下文

前台任务可以使用 `AsyncApp` 或 `AsyncWindowContext`：

```rust
// App 级别的异步上下文
let task = cx.spawn(async |cx: &mut AsyncApp| {
    let data = fetch_data().await;
    cx.update(|cx| {
        // 更新全局状态
    });
});

// 窗口级别的异步上下文
let task = window.spawn(cx, async |window, cx| {
    let data = fetch_data().await;
    window.update(cx, |view, cx| {
        // 更新视图状态
    });
});
```

## 错误处理

### detach_and_log_err

在后台运行任务并自动记录错误：

```rust
use rgpui::TaskExt;

cx.background_spawn(async {
    fetch_data().await
})
.detach_and_log_err(cx);
```

### 带回溯信息的错误处理

```rust
cx.background_spawn(async {
    fetch_data().await
})
.detach_and_log_err_with_backtrace(cx);
```

## 定时器

### 基本定时器

```rust
// 在指定时间后执行
cx.timer(Duration::from_secs(1)).await;
println!("1 秒后执行");

// 周期性定时器
loop {
    cx.timer(Duration::from_millis(100)).await;
    // 每 100ms 执行一次
    update_animation();
}
```

### 带超时的操作

```rust
use rgpui::Timeout;

// 为异步操作添加超时
let result = cx.timeout(
    Duration::from_secs(5),
    async {
        fetch_data().await
    }
).await;

match result {
    Ok(data) => println!("获取成功"),
    Err(_) => println!("操作超时"),
}
```

## Task 类型

`Task<T>` 是 rgpui 中异步任务的返回类型：

```rust
// 发送任务（可以跨线程）
let task: Task<String> = cx.background_spawn(async { "hello".into() });

// 接收任务（必须在主线程）
let value: String = task.await;

// 取消任务（drop Task 即可取消）
let task = cx.background_spawn(async { /* ... */ });
drop(task); // 任务被取消
```

## 测试支持

rgpui 提供了测试用的执行器工具：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_async_operation() {
        let mut app = TestApp::new();
        
        app.update(|cx| {
            // 推进时间
            cx.advance_clock(Duration::from_secs(1));
            
            // 运行所有待处理任务
            cx.run_until_parked();
            
            // 手动 tick
            cx.tick();
        });
    }
}
```

### 测试方法

| 方法 | 说明 |
|------|------|
| `advance_clock(duration)` | 推进虚拟时钟 |
| `run_until_parked()` | 运行任务直到调度器休眠 |
| `tick()` | 运行一个任务 |
| `simulate_random_delay()` | 模拟随机延迟 |
| `allow_parking()` | 允许调度器休眠 |

## 最佳实践

1. **区分执行器**：UI 操作用前台执行器，耗时操作用后台执行器
2. **错误处理**：始终处理异步任务的错误，使用 `detach_and_log_err`
3. **取消任务**：不再需要的任务及时 drop 以释放资源
4. **测试覆盖**：使用测试执行器验证异步逻辑
