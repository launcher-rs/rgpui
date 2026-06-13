# 事件与订阅

**目录：** [概述](#概述) · [快速入门](#快速入门) · [常见模式](#常见模式) · [subscribe_in](#subscribe_in--带窗口访问的订阅) · [observe_window_activation](#observe_window_activation) · [observe_global](#observe_global) · [订阅生命周期](#订阅生命周期) · [最佳实践](#最佳实践)

## 概述

rgpui 提供事件系统用于组件协调：

**事件机制：**
- **自定义事件**：定义和发送类型安全的事件
- **观察**：响应实体状态变化
- **订阅**：监听其他实体的事件
- **全局事件**：应用级事件处理

## 快速入门

### 定义和发送事件

```rust
#[derive(Clone)]
enum MyEvent {
    DataUpdated(String),
    ActionTriggered,
}

impl MyComponent {
    fn update_data(&mut self, data: String, cx: &mut Context<Self>) {
        self.data = data.clone();

        // 发送事件
        cx.emit(MyEvent::DataUpdated(data));
        cx.notify();
    }
}
```

### 订阅事件

```rust
impl Listener {
    fn new(source: Entity<MyComponent>, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            // 订阅事件
            cx.subscribe(&source, |this, emitter, event: &MyEvent, cx| {
                match event {
                    MyEvent::DataUpdated(data) => {
                        this.handle_update(data.clone(), cx);
                    }
                    MyEvent::ActionTriggered => {
                        this.handle_action(cx);
                    }
                }
            }).detach();

            Self { source }
        })
    }
}
```

### 观察实体变化

```rust
impl Observer {
    fn new(target: Entity<Target>, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            // 观察实体的任何变化
            cx.observe(&target, |this, observed, cx| {
                // 当 observed.update() 调用 cx.notify() 时调用
                println!("Target changed");
                cx.notify();
            }).detach();

            Self { target }
        })
    }
}
```

## 常见模式

### 1. 父子通信

```rust
// 父级发送事件
impl Parent {
    fn notify_children(&mut self, cx: &mut Context<Self>) {
        cx.emit(ParentEvent::Updated);
        cx.notify();
    }
}

// 子级订阅
impl Child {
    fn new(parent: Entity<Parent>, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            cx.subscribe(&parent, |this, parent, event, cx| {
                this.handle_parent_event(event, cx);
            }).detach();

            Self { parent }
        })
    }
}
```

### 2. 全局事件广播

```rust
struct EventBus {
    listeners: Vec<WeakEntity<dyn Listener>>,
}

impl EventBus {
    fn broadcast(&mut self, event: GlobalEvent, cx: &mut Context<Self>) {
        self.listeners.retain(|weak| {
            weak.update(cx, |listener, cx| {
                listener.on_event(&event, cx);
            }).is_ok()
        });
    }
}
```

### 3. 观察者模式

```rust
cx.observe(&entity, |this, observed, cx| {
    // 响应任何状态变化
    let state = observed.read(cx);
    this.sync_with_state(state, cx);
}).detach();
```

## subscribe_in — 带窗口访问的订阅

当订阅回调需要 `&mut Window` 时使用：

```rust
// 存储订阅以保持存活
struct MyComponent {
    _subscriptions: Vec<Subscription>,
}

impl MyComponent {
    fn new(input: &Entity<InputState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _subscriptions = vec![
            cx.subscribe_in(input, window, |this, state, event, window, cx| {
                match event {
                    InputEvent::PressEnter { .. } => this.on_submit(window, cx),
                    InputEvent::Change => {
                        let val = state.read(cx).value();
                        this.on_change(val, cx);
                    }
                    _ => {}
                }
            }),
        ];
        Self { _subscriptions }
    }
}
```

`subscribe` 与 `subscribe_in` 的区别：
- `cx.subscribe(&entity, |this, source, event, cx|)` — 没有窗口
- `cx.subscribe_in(&entity, window, |this, source, event, window, cx|)` — 有窗口访问

## observe_window_activation

```rust
let _sub = cx.observe_window_activation(window, |this, window, cx| {
    if window.is_window_active() {
        this.start_polling(cx);
    } else {
        this.stop_polling(cx);
    }
});
```

## observe_global

```rust
cx.observe_global::<Theme>(|cx| {
    cx.notify(); // 主题变化时重新渲染
});
```

## 订阅生命周期

订阅在 drop 时取消。两种保持存活的方式：

```rust
// 1. .detach() — 存活到实体被 drop
cx.subscribe(&entity, |this, _, event, cx| {
    // ...
}).detach();

// 2. 存储在结构体中 — 结构体 drop 时取消
struct MyView {
    _subscriptions: Vec<Subscription>,
}
// _subscriptions.push(cx.subscribe(...));
```

使用 `.detach()` 进行永久订阅；存储在结构体中用于在组件卸载时停止的订阅。

## 最佳实践

### ✅ 分离订阅

```rust
// ✅ 分离以保持存活
cx.subscribe(&entity, |this, source, event, cx| {
    // 处理事件
}).detach();
```

### ✅ 清晰的事件类型

```rust
#[derive(Clone)]
enum AppEvent {
    DataChanged { id: usize, value: String },
    ActionPerformed(ActionType),
    Error(String),
}
```

### ❌ 避免事件循环

```rust
// ❌ 不要创建相互订阅
entity1.subscribe(entity2) → 发送事件
entity2.subscribe(entity1) → 发送事件 → 无限循环！
```

## 参考文档
