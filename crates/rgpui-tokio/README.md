# gpui_tokio

Tokio 异步运行时集成库，用于在 GPUI 应用中桥接 Tokio 异步生态。

## 描述

`gpui_tokio` 提供了 GPUI 与 Tokio 运行时之间的集成方案，允许在 GPUI 应用中方便地使用 Tokio 的异步能力，包括网络请求、文件 I/O 等需要异步运行时支持的操作。

## 主要功能与类型

### 初始化

- **`init(cx: &mut App)`** — 初始化 Tokio 包装器，创建一个新的多工作线程运行时（默认 2 个工作线程）
- **`init_from_handle(cx: &mut App, handle: Handle)`** — 使用已有的 Tokio runtime handle 进行初始化

### Tokio 任务生成

- **`Tokio::spawn()`** — 在 Tokio 线程池上生成 future，通过 GPUI Task 返回结果
  - 如果 GPUI Task 被 drop，Tokio 任务会被自动取消
  - 返回 `Task<Result<R, JoinError>>`

- **`Tokio::spawn_result()`** — 类似 `spawn`，但处理 `anyhow::Result` 类型的 future
  - 返回 `Task<anyhow::Result<R>>`

- **`Tokio::handle()`** — 获取当前 Tokio runtime 的 handle

## 使用示例

```rust
use rgpui_tokio::Tokio;

// 在 GPUI App 中初始化
fn setup(cx: &mut App) {
    gpui_tokio::init(cx);
}

// 生成 Tokio 任务
fn fetch_data(cx: &mut App) {
    let task = Tokio::spawn(cx, async {
        // 在 Tokio 运行时中执行的异步操作
        reqwest::get("https://example.com").await
    });
    
    // task 是一个 GPUI Task，可以在 GPUI 上下文中 await
}

// 使用已有的 runtime handle
fn setup_with_handle(cx: &mut App, handle: tokio::runtime::Handle) {
    gpui_tokio::init_from_handle(cx, handle);
}
```

## 依赖关系

- `gpui` — GPUI 框架
- `tokio` — Tokio 异步运行时（启用 `rt` 和 `rt-multi-thread` 特性）
- `anyhow` — 错误处理
- `util` — 通用工具库（用于 `defer` 功能）
