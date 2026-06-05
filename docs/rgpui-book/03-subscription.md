# 订阅系统

> rgpui 的事件订阅与通知机制。

## 概述

订阅系统基于观察者模式，提供了组件间解耦通信的能力。
核心组件包括：
- [`Subscription`](#subscription) - 订阅句柄，Drop 时自动取消
- [`SubscriberSet`](#subscriberset) - 管理订阅者的容器
- [`EventEmitter`](#eventemitter) - 事件发射器 trait

## Subscription

`Subscription` 是订阅的 RAII 管理器。当它被 Drop 时，对应的订阅会自动取消。

```rust
// 创建订阅
let subscription = cx.subscribe(&entity, |this, event, cx| {
    // 处理事件
});

// 取消订阅（手动）
drop(subscription);

// 保持订阅活跃（即使句柄被丢弃）
subscription.detach();

// 合并多个订阅
let joined = Subscription::join(sub_a, sub_b);
```

### 关键方法

| 方法 | 说明 |
|------|------|
| `new(unsubscribe)` | 创建新订阅，Drop 时调用取消闭包 |
| `detach(self)` | 将订阅从句柄中分离，保持活跃 |
| `join(a, b)` | 合并两个订阅为一个 |

## 订阅模式

### 1. 观察实体变化

```rust
// 当 other_entity 调用 notify 时触发
let sub = cx.observe(&other_entity, |this, _other, cx| {
    this.handle_observation();
    cx.notify();
});
```

### 2. 订阅事件

```rust
// 当 entity 发射 MyEvent 时触发
let sub = cx.subscribe(&entity, |this, event: MyEvent, cx| {
    match event {
        MyEvent::Changed { data } => this.update_data(data),
        MyEvent::Closed => this.handle_close(),
    }
});
```

### 3. 订阅全局变化

```rust
// 当全局状态变化时触发
let sub = cx.observe_global::<MyGlobal>(|cx| {
    // 全局状态已变化
});
```

### 4. 监听焦点变化

```rust
// 当焦点状态变化时触发
let sub = cx.on_focus(&focus_handle, |cx| {
    // 获得焦点
});
```

## 事件发射

### 定义事件

```rust
/// 编辑器内容变化事件
#[derive(Clone, Debug)]
pub struct BufferChanged {
    pub range: Range<usize>,
    pub text: String,
}
```

### 实现 EventEmitter

```rust
impl EventEmitter<BufferChanged> for Editor {}
```

### 发射事件

```rust
entity.update(cx, |editor, cx| {
    // 修改内容后发射事件
    cx.emit(BufferChanged {
        range: 0..10,
        text: "new content".into(),
    });
});
```

### 订阅事件

```rust
// 订阅特定类型的事件
cx.subscribe(&editor, |this, event: BufferChanged, cx| {
    this.on_buffer_changed(event);
});
```

## SubscriberSet

`SubscriberSet` 是订阅系统的底层容器，管理一组以键为索引的订阅者。

```rust
use rgpui::collections::BTreeMap;

// 创建订阅者集合
let set = SubscriberSet::new();

// 插入订阅
let (subscription, activate) = set.insert(entity_id, callback);
activate(); // 激活订阅

// 移除某个 emitter 的所有订阅
let callbacks: Vec<_> = set.remove(&entity_id).into_iter().collect();

// 遍历并过滤订阅
set.retain(&entity_id, |callback| {
    // 返回 true 保留，false 移除
    true
});
```

### 延迟激活机制

新插入的订阅默认处于**非活跃状态**，需要通过返回的闭包手动激活。

这个设计解决了以下问题：
- 在回调执行期间添加新订阅时，新订阅不应该立即生效
- 避免在迭代期间修改集合导致的问题

## 安全性

### 回调期间取消订阅

在回调执行期间取消订阅是安全的：

```rust
cx.subscribe(&entity, |this, event, cx| {
    // 在回调中取消自身订阅
    if should_stop {
        this.subscription.take(); // 取消订阅
    }
});
```

### 回调期间添加订阅

在回调执行期间添加新订阅也是安全的：

```rust
cx.subscribe(&entity, |this, event, cx| {
    // 添加新订阅（会在当前回调完成后生效）
    let new_sub = cx.subscribe(&other_entity, |this, event, cx| {
        // ...
    });
    this.other_subscription = Some(new_sub);
});
```

## 最佳实践

1. **存储 Subscription**：将 `Subscription` 存储在实体中，以便在需要时取消
2. **使用 detach**：如果需要永久监听，使用 `subscription.detach()`
3. **弱引用**：在订阅回调中使用弱引用避免循环引用
4. **及时清理**：组件销毁时自动清理订阅（通过 RAII）
