
## 概述

rgpui 提供全面的测试框架，允许你测试 UI 组件、异步操作和分布式系统。测试运行在单线程执行器上，提供确定性执行和测试复杂异步场景的能力。rgpui 测试使用 `#[rgpui::test]` 属性，与 `TestAppContext` 一起用于基本测试，与 `VisualTestContext` 一起用于窗口相关测试。

### 规则

- 如果测试不需要窗口或渲染，可以避免使用 `[rgpui::test]` 和 `TestAppContext`，只需编写简单的 rust 测试。

## 核心测试基础设施

### 测试属性

#### 基本测试

```rust
#[rgpui::test]
fn my_test(cx: &mut TestAppContext) {
    // 测试实现
}
```

#### 异步测试

```rust
#[rgpui::test]
async fn my_async_test(cx: &mut TestAppContext) {
    // 异步测试实现
}
```

#### 带迭代次数的属性测试

```rust
#[rgpui::test(iterations = 10)]
fn my_property_test(cx: &mut TestAppContext, mut rng: StdRng) {
    // 使用随机数据的属性测试
}
```

### 测试上下文

#### TestAppContext

`TestAppContext` 提供对 rgpui 核心功能的访问，无需窗口：

```rust
#[rgpui::test]
fn test_entity_operations(cx: &mut TestAppContext) {
    // 创建实体
    let entity = cx.new(|cx| MyComponent::new(cx));

    // 更新实体
    entity.update(cx, |component, cx| {
        component.value = 42;
        cx.notify();
    });

    // 读取实体
    let value = entity.read_with(cx, |component, _| component.value);
    assert_eq!(value, 42);
}
```

#### VisualTestContext

`VisualTestContext` 扩展 `TestAppContext` 并添加窗口支持：

```rust
#[rgpui::test]
fn test_with_window(cx: &mut TestAppContext) {
    // 创建带组件的窗口
    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |_, cx| {
            cx.new(|cx| MyComponent::new(cx))
        }).unwrap()
    });

    // 转换为视觉上下文
    let mut cx = VisualTestContext::from_window(window.into(), cx);

    // 访问窗口和组件
    let component = window.root(&mut cx).unwrap();
}
```

## 其他资源

- 有关详细模式（包括重入无崩溃测试），请参见 [test-reference.md](test-reference.md)
- 有关示例和最佳实践，请参见 [test-examples.md](test-examples.md)
