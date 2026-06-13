# Entity API 参考

**目录：** [实体类型](#实体类型) · [实体创建](#实体创建) · [实体操作](#实体操作) · [Context 的实体方法](#context-的实体方法) · [异步操作](#异步操作) · [实体生命周期](#实体生命周期) · [EntityId](#entityid) · [错误处理](#错误处理) · [类型转换](#类型转换)

## 实体类型

### Entity<T>

类型 `T` 状态的强引用。

**方法：**
- `entity_id()` → `EntityId` - 返回唯一标识符
- `downgrade()` → `WeakEntity<T>` - 创建弱引用
- `read(cx)` → `&T` - 不可变访问状态
- `read_with(cx, |state, cx| ...)` → `R` - 带闭包读取，返回闭包结果
- `update(cx, |state, cx| ...)` → `R` - 带 `Context<T>` 的可变更新，返回闭包结果
- `update_in(cx, |state, window, cx| ...)` → `R` - 带 `Window` 访问的更新（需要 `AsyncWindowContext` 或 `VisualTestContext`）

**重要说明：**
- 尝试在实体已被更新时更新它会 panic
- 在闭包内使用提供的内部 `cx` 以避免多次借用问题
- 在异步上下文中，返回值包装在 `anyhow::Result` 中

### WeakEntity<T>

类型 `T` 状态的弱引用。

**方法：**
- `upgrade()` → `Option<Entity<T>>` - 如果仍存活则转换为强引用
- `read_with(cx, |state, cx| ...)` → `Result<R>` - 如果实体存在则读取
- `update(cx, |state, cx| ...)` → `Result<R>` - 如果实体存在则更新
- `update_in(cx, |state, window, cx| ...)` → `Result<R>` - 如果实体存在则带窗口更新

**用例：**
- 避免实体之间的循环依赖
- 在闭包/回调中存储引用而不阻止清理
- 组件之间的可选关系

**重要：** 所有操作返回 `Result`，因为实体可能不再存在。

## 实体创建

### cx.new()

用初始状态创建新实体。

```rust
let entity = cx.new(|cx| MyState {
    count: 0,
    name: "Default".to_string(),
});
```

**参数：**
- `cx: &mut App` 或其他上下文类型
- 接收 `&mut Context<T>` 返回初始状态 `T` 的闭包

**返回：** `Entity<T>`

## 实体操作

### 读取状态

#### read()

直接只读访问状态。

```rust
let count = my_entity.read(cx).count;
```

**何时使用：** 简单字段访问，不需要上下文操作。

#### read_with()

在闭包中带上下文访问的读取。

```rust
let count = my_entity.read_with(cx, |state, cx| {
    // 可以同时访问状态和上下文
    state.count
});

// 返回多个值
let (count, theme) = my_entity.read_with(cx, |state, cx| {
    (state.count, cx.theme().clone())
});
```

**何时使用：** 需要上下文操作、多个返回值、复杂逻辑。

### 更新状态

#### update()

带 `Context<T>` 的可变更新。

```rust
my_entity.update(cx, |state, cx| {
    state.count += 1;
    cx.notify(); // 触发重新渲染
});
```

**可用操作：**
- `cx.notify()` - 触发重新渲染
- `cx.entity()` - 获取当前实体
- `cx.emit(event)` - 发送事件
- `cx.spawn(task)` - 生成异步任务
- 其他 `Context<T>` 方法

#### update_in()

同时带 `Window` 和 `Context<T>` 访问的更新。

```rust
my_entity.update_in(cx, |state, window, cx| {
    state.focused = window.is_window_focused();
    cx.notify();
});
```

**需要：** `AsyncWindowContext` 或 `VisualTestContext`

**何时使用：** 需要窗口特定操作，如焦点状态、窗口边界等。

## Context 的实体方法

### cx.entity()

获取当前正在更新的实体。

```rust
impl MyComponent {
    fn some_method(&mut self, cx: &mut Context<Self>) {
        let current_entity = cx.entity();  // Entity<MyComponent>
        let weak = current_entity.downgrade();
    }
}
```

### cx.observe()

观察实体的变化。

```rust
cx.observe(&entity, |this, observed_entity, cx| {
    // 当 observed_entity.update() 调用 cx.notify() 时调用
    println!("Entity changed");
}).detach();
```

**返回：** `Subscription` - 调用 `.detach()` 使其永久

### cx.subscribe()

订阅实体的事件。

```rust
cx.subscribe(&entity, |this, emitter, event: &SomeEvent, cx| {
    // 当 emitter 发送 SomeEvent 时调用
    match event {
        SomeEvent::DataChanged => {
            cx.notify();
        }
    }
}).detach();
```

**返回：** `Subscription` - 调用 `.detach()` 使其永久

## 异步操作

### cx.spawn()

生成前台任务（UI 线程）。

```rust
cx.spawn(async move |this, cx| {
    // `this`: WeakEntity<T>
    // `cx`: &mut AsyncApp

    let result = some_async_work().await;

    // 安全更新实体
    let _ = this.update(cx, |state, cx| {
        state.data = result;
        cx.notify();
    });
}).detach();
```

**注意：** 始终在生成的任务中使用弱实体引用以防止循环引用。

### cx.background_spawn()

生成后台任务（后台线程）。

```rust
cx.background_spawn(async move {
    // 长时间运行的计算
    let result = heavy_computation().await;
    // 不能直接在这里更新实体
    // 使用通道或生成前台任务来更新
}).detach();
```

## 实体生命周期

### 创建

实体通过 `cx.new()` 创建并立即在应用中注册。

### 引用计数

- `Entity<T>` 是强引用（增加引用计数）
- `WeakEntity<T>` 是弱引用（不增加引用计数）
- 克隆 `Entity<T>` 增加引用计数

### 销毁

当所有强引用被 drop 时，实体自动销毁。

```rust
{
    let entity = cx.new(|cx| MyState::default());
    // entity 存在
} // 如果没有其他强引用，entity 在这里被 drop
```

**内存泄漏预防：**
- 在闭包/回调中使用 `WeakEntity`
- 对父子关系使用 `WeakEntity`
- 避免循环强引用

## EntityId

每个实体都有唯一标识符。

```rust
let id: EntityId = entity.entity_id();

// EntityIds 可以比较
if entity1.entity_id() == entity2.entity_id() {
    // 同一实体
}
```

## 错误处理

### WeakEntity 操作

所有 `WeakEntity` 操作返回 `Result`：

```rust
let weak = entity.downgrade();

// 处理潜在失败
match weak.read_with(cx, |state, cx| state.count) {
    Ok(count) => println!("Count: {}", count),
    Err(e) => eprintln!("Entity no longer exists: {}", e),
}

// 或使用 Result 组合器
let _ = weak.update(cx, |state, cx| {
    state.count += 1;
    cx.notify();
}).ok(); // 忽略错误
```

### Update Panic

同一实体上的嵌套更新会 panic：

```rust
// ❌ 会 panic
entity.update(cx, |state1, cx| {
    entity.update(cx, |state2, cx| {
        // Panic：entity 已被借用
    });
});
```

**解决方案：** 顺序执行更新或使用不同的实体。

## 类型转换

### Entity → WeakEntity

```rust
let entity: Entity<T> = cx.new(|cx| T::default());
let weak: WeakEntity<T> = entity.downgrade();
```

### WeakEntity → Entity

```rust
let weak: WeakEntity<T> = entity.downgrade();
let strong: Option<Entity<T>> = weak.upgrade();
```

## 最佳实践指南

### 始终使用内部 cx

```rust
// ✅ 好：使用内部 cx
entity.update(cx, |state, inner_cx| {
    inner_cx.notify(); // 使用 inner_cx，而不是外部 cx
});

// ❌ 坏：使用外部 cx
entity.update(cx, |state, inner_cx| {
    cx.notify(); // 错误！多次借用错误
});
```

### 闭包中的弱引用

```rust
// ✅ 好：弱引用
let weak = cx.entity().downgrade();
callback(move || {
    let _ = weak.update(cx, |state, cx| {
        cx.notify();
    });
});

// ❌ 坏：强引用（循环引用）
let strong = cx.entity();
callback(move || {
    strong.update(cx, |state, cx| {
        // 可能永远不会被 drop
        cx.notify();
    });
});
```

### 顺序更新

```rust
// ✅ 好：顺序更新
entity1.update(cx, |state, cx| { /* ... */ });
entity2.update(cx, |state, cx| { /* ... */ });

// ❌ 坏：嵌套更新
entity1.update(cx, |_, cx| {
    entity2.update(cx, |_, cx| {
        // 如果实体相关可能会 panic
    });
});
```
