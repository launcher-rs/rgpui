# ztracing

基于 zlog 的 tracing 集成，支持 Tracy 性能分析器。

## 描述

`ztracing` crate 将 `tracing` 生态系统与 zlog 日志系统集成，并提供可选的 Tracy 性能分析器支持。在启用 Tracy 时，所有 tracing 事件和 span 都会发送到 Tracy Profiler，用于性能分析和可视化。

## 主要功能

### Tracing 宏导出

根据编译配置条件性地导出 tracing 宏：

- **启用 `ztracing` 时**: 导出完整的 `tracing` 宏（`debug_span!`、`info_span!`、`event!`、`instrument` 等）
- **未启用 `ztracing` 时**: 导出空操作宏，所有 tracing 调用在编译时被消除

### Tracy 集成

- **`init()`**: 初始化 Tracy 订阅器，将 tracing 事件路由到 Tracy Profiler
- **内存分析**: 可选的 `tracy_client::ProfiledAllocator` 全局分配器，跟踪内存分配
- **调用栈深度**: 最大调用栈深度限制为 16 层

### TracyLayer 配置

- 使用 `DefaultFields` 格式化字段
- 在 zone 名称中格式化字段
- 错误消息以红色显示

## 使用示例

```rust
use ztracing::{debug_span, info_span, event, instrument, Level};

// 使用 instrument 宏自动创建 span
#[instrument]
fn my_function(x: i32) -> i32 {
    // 函数执行会被 Tracy 记录
    x * 2
}

// 手动创建 span
fn process_data() {
    let span = info_span!("process_data");
    let _enter = span.enter();
    
    // 处理数据...
    event!(Level::DEBUG, "data processed");
}

// 在应用启动时初始化
#[cfg(ztracing)]
fn init_tracing() {
    ztracing::init();
}
```

## Cargo.toml 配置

```toml
[dependencies]
ztracing = { workspace = true }

# 启用 Tracy 支持
[features]
profiling = ["ztracing/tracy"]
```

## 特性标志

- `tracy`: 启用 Tracy 性能分析器支持（依赖 `tracing-tracy` 和 `tracy-client`）

## 编译配置

- `ztracing`: 启用 tracing 宏（通过 build.rs 检测）
- `ztracing_with_memory`: 启用内存分配分析

## 依赖

- `zlog` — 日志系统
- `tracing` — 异步事件追踪
- `tracing-subscriber` — 订阅器实现
- `ztracing_macro` — 过程宏
- `tracing-tracy`（可选）— Tracy 集成
- `tracy-client`（可选）— Tracy 客户端
