## 测试最佳实践

### 测试组织

将相关测试分组到模块中：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    mod entity_tests {
        use super::*;

        #[rgpui::test]
        fn test_creation() { /* ... */ }

        #[rgpui::test]
        fn test_updates() { /* ... */ }
    }

    mod async_tests {
        use super::*;

        #[rgpui::test]
        async fn test_async_ops() { /* ... */ }
    }

    mod distributed_tests {
        use super::*;

        #[rgpui::test]
        fn test_multi_app() { /* ... */ }
    }
}
```

### 设置和拆卸

使用辅助函数进行常见设置：

```rust
fn create_test_counter(cx: &mut TestAppContext) -> Entity<Counter> {
    cx.new(|cx| Counter::new(cx))
}

#[rgpui::test]
fn test_counter_operations(cx: &mut TestAppContext) {
    let counter = create_test_counter(cx);

    // 测试操作
}
```

### 断言

使用描述性断言：

```rust
#[rgpui::test]
fn test_counter_bounds(cx: &mut TestAppContext) {
    let counter = create_test_counter(cx);

    // 测试上界
    for _ in 0..100 {
        counter.update(cx, |counter, cx| {
            counter.increment(cx);
        });
    }

    let count = counter.read_with(cx, |counter, _| counter.count);
    assert!(count <= 100, "Counter should not exceed maximum");

    // 测试下界
    for _ in 0..200 {
        counter.update(cx, |counter, cx| {
            counter.decrement(cx);
        });
    }

    let count = counter.read_with(cx, |counter, _| counter.count);
    assert!(count >= 0, "Counter should not go below minimum");
}
```

### 性能测试

测试性能特性：

```rust
#[rgpui::test]
fn test_operation_performance(cx: &mut TestAppContext) {
    let component = cx.new(|cx| MyComponent::new(cx));

    let start = std::time::Instant::now();

    // 执行多次操作
    for i in 0..1000 {
        component.update(cx, |comp, cx| {
            comp.perform_operation(i, cx);
        });
    }

    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_millis(100), "Operations should complete quickly");
}
```

## 运行测试

### 基本测试执行

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test test_counter_operations

# 运行特定模块中的测试
cargo test entity_tests::

# 带输出运行
cargo test -- --nocapture
```

### 测试配置

为 rgpui 测试启用 test-support feature：

```toml
[features]
test-support = ["rgpui/test-support"]
```

```bash
cargo test --features test-support
```

### 高级测试执行

```bash
# 使用迭代次数运行属性测试
cargo test -- --test-threads=1

# 运行匹配模式的测试
cargo test test_async

# 失败时带 backtrace 运行测试
RUST_BACKTRACE=1 cargo test
```

### CI/CD 集成

用于持续集成：

```yaml
# .github/workflows/test.yml
name: Tests
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - name: Run tests
        run: cargo test --features test-support
```

rgpui 的测试框架提供确定性、快速和全面的测试能力，反映真实应用行为，同时提供对复杂 UI 和异步场景进行彻底测试所需的控制。
