# ztracing_macro

ztracing 的过程宏，提供无操作版本的 `#[instrument]` 属性。

## 描述

`ztracing_macro` 是 `ztracing` crate 的配套过程宏库。当未启用 `ztracing` 编译配置时，它提供一个空操作的 `#[instrument]` 属性宏，确保 tracing 注解在编译时被完全消除，不产生任何运行时开销。

## 主要功能

### `#[instrument]` 属性宏

- **启用 `ztracing` 时**: 使用 `tracing::instrument` 的完整功能
- **未启用 `ztracing` 时**: 此宏直接返回原始函数，不添加任何代码

## 使用示例

```rust
use ztracing_macro::instrument;

// 当未启用 ztracing 时，此宏不产生任何代码
#[instrument]
fn my_function(x: i32) -> i32 {
    x * 2
}

// 编译后等同于：
fn my_function(x: i32) -> i32 {
    x * 2
}
```

## 设计目的

该 crate 的存在确保了在未启用性能分析时：
- tracing 相关代码在编译时被完全消除
- 零运行时开销
- 无需在业务代码中使用条件编译
