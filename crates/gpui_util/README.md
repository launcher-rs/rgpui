# gpui_util

通用工具库，提供 GPUI 项目中常用的辅助工具和实用程序。

## 描述

`gpui_util` 是一个基础工具 crate，提供了错误处理扩展、延迟执行、性能测量、调试宏等通用功能，为 GPUI 生态系统提供底层支撑。

## 主要功能与类型

### 错误处理扩展

- **`ResultExt`** — Result 类型的扩展 trait，提供便捷的错误处理方法
  - `log_err()` — 记录错误并返回 `Option`
  - `log_err_with_backtrace()` — 带完整回溯信息的错误日志
  - `warn_on_err()` — 以警告级别记录错误
  - `debug_assert_ok()` — 在调试模式下断言结果必须成功
  - `anyhow()` — 将错误转换为 `anyhow::Error`

- **`TryFutureExt`** — Future 类型的错误处理扩展
  - `log_err()` — 异步完成后记录错误
  - `warn_on_err()` — 异步完成后以警告级别记录
  - `unwrap()` — 异步完成后解包值

- **`TryFutureExtBacktrace`** — 带回溯信息的 Future 错误处理

### 延迟执行

- **`defer()`** — 返回一个 `Deferred` 守卫，在 drop 时执行指定函数
  - `Deferred::abort()` — 取消延迟执行

### 性能测量

- **`measure()`** — 测量代码块执行时间（通过 `ZED_MEASUREMENTS` 环境变量控制）
- **`post_inc()`** — 后置自增操作

### 宏工具

- **`debug_panic!`** — 仅在调试模式下 panic，发布模式下记录错误日志
- **`maybe!`** — 立即执行的函数表达式，支持在非 fallible 函数中使用 `?` 运算符

### ArcCow

- **`ArcCow<'a, T>`** — 结合 `Arc` 和借用的写时克隆类型，支持 `Borrowed(&'a T)` 和 `Owned(Arc<T>)` 两种变体

## 使用示例

```rust
use gpui_util::{ResultExt, TryFutureExt, defer};

// 错误处理
let result: Result<(), &str> = Err("something went wrong");
result.log_err(); // 记录错误并返回 None

// 延迟执行
let _guard = defer(|| {
    println!("作用域结束时执行");
});

// 异步错误处理
async fn example() {
    let future = async { Ok::<_, String>(42) };
    let result = future.log_err().await;
}

// 性能测量
let value = measure("expensive operation", || {
    // 耗时操作
    42
});
```

## 依赖关系

- `log` — 日志记录
- `anyhow` — 错误处理
