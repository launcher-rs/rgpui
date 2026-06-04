# 实体系统

> 理解 rgpui 的核心：Entity、Context 与生命周期管理。

## 概述

实体系统是 rgpui 的核心架构。每个视图（View）和组件都是一个实体（Entity），
它们存储在 App 的 Arena 分配器中，通过强引用（`Entity<T>`）和弱引用（`WeakEntity<T>`）进行访问。

## 核心类型

### Entity\<T\>

`Entity<T>` 是实体的强引用句柄，提供了对实体数据的安全访问。

```rust
use rgpui::{Entity, Context};

// 创建实体
let entity: Entity<MyView> = cx.new(|cx| {
    MyView { count: 0 }
});

// 读取实体数据
let count = entity.read(cx, |view, _cx| view.count);

// 更新实体数据
entity.update(cx, |view, cx| {
    view.count += 1;
    cx.notify(); // 通知视图需要重绘
});
```

### WeakEntity\<T\>

`WeakEntity<T>` 是实体的弱引用句柄，不会延长实体的生命周期。
当实体被销毁后，`upgrade()` 会返回 `None`。

```rust
use rgpui::WeakEntity;

// 在闭包中捕获弱引用，避免循环引用
let weak = entity.downgrade();
cx.observe(&entity, move |this, _entity, cx| {
    if let Some(entity) = weak.upgrade() {
        // 实体仍然存活，可以安全访问
        entity.update(cx, |view, _cx| {
            view.handle_observation();
        });
        true // 保持观察
    } else {
        false // 停止观察
    }
});
```

### Context\<T\>

`Context<T>` 是实体上下文，在创建和更新实体时提供。它封装了：
- 对 `App` 的可变引用（通过 `Deref`/`DerefMut` 自动解引用）
- 对实体的弱引用

```rust
impl Render for MyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // cx 可以直接调用 App 的方法
        cx.observe(&other_entity, |this, _entity, cx| {
            this.handle_observation();
        });
        
        // 也可以通过 entity() 获取实体句柄
        let entity = cx.entity();
        
        div()
    }
}
```

## 实体生命周期

### 创建实体

```rust
// 在 App 中创建
let entity = cx.new(|cx| {
    MyView::new(cx)
});

// 在窗口中创建
let entity = window.new_entity(cx, |window, cx| {
    MyView::new(window, cx)
});
```

### 更新实体

```rust
// 通过 Entity 句柄更新
entity.update(cx, |view, cx| {
    view.do_something();
});

// 通过 Context 更新（在 render 方法中）
cx.update(|view, cx| {
    view.do_something();
});
```

### 读取实体

```rust
// 只读访问
let value = entity.read(cx, |view, _cx| view.get_value());
```

### 销毁实体

当 `Entity<T>` 的所有强引用都被 drop 后，实体会被自动销毁。
销毁前会调用实体的 `Drop` 实现。

## 实体观察

观察者模式允许一个实体监听另一个实体的变化：

```rust
impl MyView {
    fn new(cx: &mut Context<Self>) -> Self {
        // 观察另一个实体
        cx.observe(&other_entity, |this, _other, cx| {
            this.handle_change();
            cx.notify();
        });
        
        Self { /* ... */ }
    }
}
```

当被观察的实体调用 `cx.notify()` 时，观察者的回调会被调用。

## 事件发射与订阅

实体可以发射事件，其他实体可以订阅这些事件：

```rust
// 定义事件
struct MyEvent {
    data: String,
}

// 实现 EventEmitter
impl EventEmitter<MyEvent> for MyView {}

// 发射事件
entity.update(cx, |view, cx| {
    cx.emit(MyEvent { data: "hello".into() });
});

// 订阅事件
cx.subscribe(&entity, |sender, event: MyEvent, cx| {
    println!("收到事件: {}", event.data);
});
```

## 预留实体 ID

在某些场景下，你需要在创建实体之前就获取其 ID：

```rust
// 预留一个槽位
let reservation = cx.reserve_entity::<MyView>();
let entity_id = reservation.entity_id();

// 使用预留的 ID 做一些操作
// ...

// 稍后创建实体
let entity = cx.insert_entity(reservation, |cx| {
    MyView::new(cx)
});
```

## 最佳实践

1. **优先使用弱引用**：在闭包中捕获 `WeakEntity<T>` 而不是 `Entity<T>`，避免循环引用
2. **及时通知**：修改实体状态后调用 `cx.notify()` 触发重绘
3. **观察者返回值**：在观察回调中返回 `true` 保持观察，返回 `false` 停止观察
4. **使用 `detach`**：如果需要永久观察，使用 `subscription.detach()` 防止自动取消
