# 上下文管理

**目录：** [概述](#概述) · [快速入门](#快速入门) · [常用操作](#常用操作) · [上下文层次](#上下文层次) · [cx.listener](#cxlistener--绑定回调到自身) · [subscribe_in](#subscribe_in--带窗口访问的订阅) · [observe_window_activation](#observe_window_activation) · [observe_global](#observe_global) · [defer / defer_in](#defer-和-defer_in) · [命名约定](#上下文命名约定)

## 概述

rgpui 使用不同的上下文类型来处理不同的场景：

**上下文类型：**
- **`App`**：全局应用状态，实体创建
- **`Window`**：窗口特定操作，绘制，布局
- **`Context<T>`**：组件 `T` 的实体特定上下文
- **`AsyncApp`**：前台任务的异步上下文
- **`AsyncWindowContext`**：带窗口访问的异步上下文

## 快速入门

### Context<T> - 组件上下文

```rust
impl MyComponent {
    fn update_state(&mut self, cx: &mut Context<Self>) {
        self.value = 42;
        cx.notify(); // 触发重新渲染

        // 生成异步任务
        cx.spawn(async move |cx| {
            // 异步工作
        }).detach();

        // 获取当前实体
        let entity = cx.entity();
    }
}
```

### App - 全局上下文

```rust
fn main() {
    let app = Application::new();
    app.run(|cx: &mut App| {
        // 创建实体
        let entity = cx.new(|cx| MyState::default());

        // 打开窗口
        cx.open_window(WindowOptions::default(), |window, cx| {
            cx.new(|cx| Root::new(view, window, cx))
        });
    });
}
```

### Window - 窗口上下文

```rust
impl Render for MyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 窗口操作
        let is_focused = window.is_window_focused();
        let bounds = window.bounds();

        div().child("Content")
    }
}
```

### AsyncApp - 异步上下文

```rust
cx.spawn(async move |cx: &mut AsyncApp| {
    let data = fetch_data().await;

    entity.update(cx, |state, inner_cx| {
        state.data = data;
        inner_cx.notify();
    }).ok();
}).detach();
```

## 常用操作

### 实体操作

```rust
// 创建实体
let entity = cx.new(|cx| MyState::default());

// 更新实体
entity.update(cx, |state, cx| {
    state.value = 42;
    cx.notify();
});

// 读取实体
let value = entity.read(cx).value;
```

### 通知和事件

```rust
// 触发重新渲染
cx.notify();

// 发送事件
cx.emit(MyEvent::Updated);

// 观察实体
cx.observe(&entity, |this, observed, cx| {
    // 响应变化
}).detach();

// 订阅事件
cx.subscribe(&entity, |this, source, event, cx| {
    // 处理事件
}).detach();
```

### 窗口操作

```rust
// 窗口状态
let focused = window.is_window_focused();
let bounds = window.bounds();
let scale = window.scale_factor();

// 关闭窗口
window.remove_window();
```

### 异步操作

```rust
// 生成前台任务
cx.spawn(async move |cx| {
    // 带实体访问的异步工作
}).detach();

// 生成后台任务
cx.background_spawn(async move {
    // 繁重计算
}).detach();
```

## 上下文层次

```
App (全局)
  └─ Window (每窗口)
       └─ Context<T> (每组件)
            └─ AsyncApp (异步任务中)
                 └─ AsyncWindowContext (异步 + 窗口)
```

## cx.listener — 绑定回调到自身

`cx.listener` 创建一个借用 `&mut self`（当前实体）的回调。用于 `on_click`、`on_action` 和其他元素事件处理：

```rust
impl Render for MyView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .on_action(cx.listener(Self::on_save))
            .child(
                Button::new("btn")
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.count += 1;
                        cx.notify();
                    }))
            )
    }
}

impl MyView {
    fn on_save(&mut self, _: &Save, _window: &mut Window, cx: &mut Context<Self>) {
        cx.notify();
    }
}
```

`cx.listener(Self::method)` 等同于创建调用 `self.method(...)` 的闭包。

## subscribe_in — 带窗口访问的订阅

当回调需要 `&mut Window` 时使用 `subscribe_in`（而不是 `subscribe`）：

```rust
let _subscription = cx.subscribe_in(&input, window, |this, state, event, window, cx| {
    match event {
        InputEvent::Change => {
            let val = state.read(cx).value();
            this.on_input_change(val, window, cx);
        }
        _ => {}
    }
});
// 将 _subscription 存储在结构体中以保持存活
```

`subscribe` 与 `subscribe_in` 的区别：
- `subscribe(&entity, |this, source, event, cx|)` — 没有窗口访问
- `subscribe_in(&entity, window, |this, source, event, window, cx|)` — 有窗口访问

## observe_window_activation

当窗口获得或失去焦点时响应：

```rust
let _sub = cx.observe_window_activation(window, |this, window, cx| {
    if window.is_window_active() {
        this.resume(cx);
    } else {
        this.pause(cx);
    }
});
```

## observe_global

当全局值变化时响应：

```rust
cx.observe_global::<Theme>(|cx| {
    // 主题变化 — 响应
    cx.notify();
});
```

## defer 和 defer_in

在当前更新完成后调度工作：

```rust
// defer：在当前 App 更新后运行，没有窗口访问
cx.defer(|cx| {
    // 在当前实体更新完成后运行
});

// defer_in：在更新后运行，有窗口访问
cx.defer_in(window, |this, window, cx| {
    // 可以在这里访问窗口
    // 注意：永远不要在 defer_in 内部对 *同一个实体* 调用 entity.update(cx)
    // — 它会重新进入锁并 panic。直接使用 &mut self 引用。
    this.some_method(window, cx);
});
```

## 上下文命名约定

无论类型如何，始终将上下文命名为 `cx`：

```rust
fn new(window: &mut Window, cx: &mut App) {}             // cx = App
impl Render for View {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) {}  // cx = Context<Self>
}
cx.spawn(async move |this, cx: &mut AsyncApp| {})         // cx = AsyncApp
```
