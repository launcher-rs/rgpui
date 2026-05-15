# perf

性能测量工具，用于基准测试和性能回归检测。

## 描述

`perf` crate 提供了一套性能基准测试的基础设施和数据结构，与 `util_macros` 中的 `#[perf]` 属性宏配合使用。它定义了性能测试的元数据格式、重要性分级和权重系统，用于在持续集成中自动检测和追踪性能变化。

## 主要功能

### 重要性分级

性能测试分为 5 个重要性级别：

- **`Critical`**: 关键性能路径，任何退化都需要关注
- **`Important`**: 重要性能路径，显著影响用户体验
- **`Average`**: 一般性能测试（默认级别）
- **`Iffy`**: 不太可靠的性能指标，可能受环境影响
- **`Fluff`**: 边缘路径或冷启动场景，退化可接受

### 权重系统

每个测试有一个权重值（默认 50），用于在同级别内比较不同测试的相对重要性。权重仅在相同重要性类别内有效。

### 迭代控制

- 支持自动确定迭代次数
- 可通过 `iterations = n` 手动指定
- 每次迭代后重启进程以重置全局状态

### 元数据输出

性能测试会生成一个辅助的"元数据测试"，输出：
- 迭代次数（`ITER_COUNT`）
- 权重（`WEIGHT`）
- 重要性（`IMPORTANCE`）
- 元数据版本（`VERSION`）

## 使用示例

```rust
use util_macros::perf;

// 默认重要性（Average）
#[perf]
fn generic_test() {
    // 性能测试代码
}

// 指定重要性和权重
#[perf(critical, weight = 80)]
fn critical_path_test() {
    // 关键路径测试
}

// 边缘路径，低权重
#[perf(fluff, weight = 30)]
fn cold_path_test() {
    // 冷路径测试
}

// 固定迭代次数
#[perf(iterations = 100, important)]
fn fixed_iterations_test() {
    // 固定迭代测试
}
```

## 特性标志

- `perf-enabled`: 启用性能测量功能（由构建系统控制）

## 依赖

- `collections` — 集合类型
- `serde` — 序列化
- `serde_json` — JSON 序列化

## 代码风格

该 crate 应用了严格的 Clippy lint 规则：
- 启用 `pedantic` 和 `style` 警告
- 禁止 `as_` 前缀（使用 `as_underscore`）
- 禁止 `allow` 属性（要求使用 `expect` 并附带说明）
- 禁止未记录的不安全代码块
- 禁止缺少安全文档
