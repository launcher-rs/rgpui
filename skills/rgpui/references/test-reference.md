# 测试参考

**目录：** [测试模式](#测试模式) · [测试无崩溃状态管理（重入）](#测试无崩溃状态管理重入) · [属性测试](#属性测试) · [分布式系统测试](#分布式系统测试) · [模拟和隔离](#模拟和隔离)

## 测试模式

### 基本实体测试

测试实体的创建、更新和读取：

```rust
#[rgpui::test]
fn test_counter_entity(cx: &mut TestAppContext) {
    let counter = cx.new(|cx| Counter::new(cx));

    // 测试初始状态
    let initial_count = counter.read_with(cx, |counter, _| counter.count);
    assert_eq!(initial_count, 0);

    // 测试更新
    counter.update(cx, |counter, cx| {
        counter.count = 42;
        cx.notify();
    });

    let updated_count = counter.read_with(cx, |counter, _| counter.count);
    assert_eq!(updated_count, 42);
}
```

### 事件测试

测试事件发送和处理：

```rust
#[derive(Clone)]
struct ValueChanged {
    new_value: i32,
}

impl EventEmitter<ValueChanged> for MyComponent {}

#[rgpui::test]
fn test_event_emission(cx: &mut TestAppContext) {
    let component = cx.new(|cx| {
        let mut comp = MyComponent::default();

        // 订阅自身
        cx.subscribe_self(|this, event: &ValueChanged, cx| {
            this.received_value = event.new_value;
            cx.notify();
        });

        comp
    });

    // 发送事件
    component.update(cx, |_, cx| {
        cx.emit(ValueChanged { new_value: 123 });
    });

    // 验证事件已处理
    let received = component.read_with(cx, |comp, _| comp.received_value);
    assert_eq!(received, 123);
}
```

### 操作测试

测试操作分发和处理：

```rust
actions!(my_app, [Increment, Decrement]);

#[rgpui::test]
fn test_action_dispatch(cx: &mut TestAppContext) {
    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |_, cx| {
            cx.new(|cx| MyComponent::new(cx))
        }).unwrap()
    });

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let counter = window.root(&mut cx).unwrap();

    // 通过焦点句柄分发操作
    let focus_handle = counter.read_with(&cx, |counter, _| counter.focus_handle.clone());
    cx.update(|window, cx| {
        focus_handle.dispatch_action(&Increment, window, cx);
    });

    let count = counter.read_with(&cx, |counter, _| counter.count);
    assert_eq!(count, 1);
}
```

### 异步测试

测试异步操作和后台任务：

```rust
impl MyComponent {
    fn load_data(&self, cx: &mut Context<Self>) -> Task<i32> {
        cx.spawn(async move |this, cx| {
            // 模拟异步工作
            this.update(cx, |comp, _| comp.loading = true).await;
            // 返回结果
            42
        })
    }

    fn background_update(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            // 后台工作
            this.update(cx, |comp, _| {
                comp.value += 10;
            }).await;
        }).detach();
    }
}

#[rgpui::test]
async fn test_async_operations(cx: &mut TestAppContext) {
    let component = cx.new(|cx| MyComponent::new(cx));

    // 测试 await 任务
    let result = component.update(cx, |comp, cx| comp.load_data(cx)).await;
    assert_eq!(result, 42);

    // 测试分离任务
    component.update(cx, |comp, cx| comp.background_update(cx));

    // 分离任务在 yield 之前不会运行
    let value_before = component.read_with(cx, |comp, _| comp.value);
    assert_eq!(value_before, 0);

    // 运行待处理的任务
    cx.run_until_parked();

    let value_after = component.read_with(cx, |comp, _| comp.value);
    assert_eq!(value_after, 10);
}
```

### 定时器测试

测试基于定时器的操作：

```rust
impl MyComponent {
    fn delayed_action(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;

            this.update(cx, |comp, cx| {
                comp.action_performed = true;
                cx.notify();
            }).await;
        }).detach();
    }
}

#[rgpui::test]
async fn test_timers(cx: &mut TestAppContext) {
    let component = cx.new(|cx| MyComponent::new(cx));

    component.update(cx, |comp, cx| comp.delayed_action(cx));

    // 操作应该还没有完成
    let performed = component.read_with(cx, |comp, _| comp.action_performed);
    assert!(!performed);

    // 运行直到暂停（定时器完成）
    cx.run_until_parked();

    let performed = component.read_with(cx, |comp, _| comp.action_performed);
    assert!(performed);
}
```

### 外部 I/O 测试

对于涉及外部系统的测试，使用 `allow_parking()`：

```rust
#[rgpui::test]
async fn test_external_io(cx: &mut TestAppContext) {
    // 允许外部 I/O 的 parking
    cx.executor().allow_parking();

    // 模拟外部操作
    let (tx, rx) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(10));
        tx.send(42).ok();
    });

    let result = rx.await.unwrap();
    assert_eq!(result, 42);
}
```

## 测试无崩溃状态管理（重入）

最危险的 rgpui bug 类别是**实体重入**：代码尝试在实体已被锁定在渲染/更新传递中时更新或读取它。这些 bug 在编译时不可见 — 它们只在运行时 panic，通常在用户点击列表或下拉菜单中的某项时。

**重入 panic 的关键特性：**
- 由用户交互（点击、按键）触发，而不是在静态渲染期间。
- `#[should_panic]` 只确认 bug 存在 — 测试必须*不* panic 通过。
- 触发操作后需要 `cx.run_until_parked()` 来让延迟回调（`defer_in`）执行。

### 模式：通过真实委托驱动 `confirm` / `cancel`

### 模式：验证 `on_will_change` 和 `on_confirm` 钩子被正确调用

### 模式：快速多次 confirm（快照一致性）

### 检查清单：每个使用 `defer_in` 的组件要测试什么

- [ ] `confirm` 路径：没有 panic，正确的最终选择，快照与选择匹配
- [ ] `cancel` 路径：没有 panic，选择不变，弹出框关闭
- [ ] `on_will_change` 否决：选择未改变，`on_confirm` 未调用
- [ ] `on_will_change` 修改更改：最终选择反映委托的修改
- [ ] 快速连续 confirm：每个都使快照与选择一致
- [ ] `render_item` 即使在突变后立即调用也不会 panic

## 属性测试

使用随机数据测试边界情况：

```rust
#[rgpui::test(iterations = 10)]
fn test_counter_random_operations(cx: &mut TestAppContext, mut rng: StdRng) {
    let counter = cx.new(|cx| Counter::new(cx));

    let mut expected = 0i32;
    for _ in 0..100 {
        let delta = rng.random_range(-10..=10);
        expected += delta;

        counter.update(cx, |counter, cx| {
            counter.count += delta;
            cx.notify();
        });
    }

    let actual = counter.read_with(cx, |counter, _| counter.count);
    assert_eq!(actual, expected);
}
```

## 分布式系统测试

测试多个应用上下文之间的通信：

```rust
#[derive(Clone)]
struct NetworkMessage {
    from: String,
    to: String,
    data: i32,
}

#[rgpui::test]
fn test_distributed_apps(cx_a: &mut TestAppContext, cx_b: &mut TestAppContext) {
    // 在不同的应用上下文中创建组件
    let comp_a = cx_a.new(|_| MyComponent::new("A".to_string()));
    let comp_b = cx_b.new(|_| MyComponent::new("B".to_string()));

    // 模拟消息传递
    comp_a.update(cx_a, |comp, cx| {
        comp.send_message("B", 42, cx);
    });

    // 运行异步操作
    cx_a.run_until_parked();

    // 验证在其他上下文中收到消息
    comp_b.update(cx_b, |comp, _| {
        comp.receive_messages();
    });

    let messages = comp_b.read_with(cx_b, |comp, _| comp.messages.clone());
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].data, 42);
}
```

## 模拟和隔离

### 网络模拟

为测试分布式功能创建模拟网络：

```rust
struct MockNetwork {
    messages: Arc<Mutex<Vec<NetworkMessage>>>,
}

impl MockNetwork {
    fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn send(&self, message: NetworkMessage) {
        self.messages.lock().unwrap().push(message);
    }

    fn receive_all(&self) -> Vec<NetworkMessage> {
        self.messages.lock().unwrap().drain(..).collect()
    }
}

#[rgpui::test]
fn test_networked_components(cx: &mut TestAppContext) {
    let network = Arc::new(MockNetwork::new());

    let sender = cx.new(|_| MessageSender::new(network.clone()));
    let receiver = cx.new(|_| MessageReceiver::new(network));

    // 发送消息
    sender.update(cx, |sender, _| {
        sender.send("Hello");
    });

    // 接收消息
    receiver.update(cx, |receiver, _| {
        receiver.receive_all();
    });

    let received = receiver.read_with(cx, |receiver, _| receiver.messages.clone());
    assert_eq!(received, vec!["Hello"]);
}
```
