# 异步与后台任务

**目录：** [概述](#概述) · [快速入门](#快速入门) · [核心模式](#核心模式) · [常见陷阱](#常见陷阱)

## 概述

rgpui 提供集成的异步运行时，用于前台 UI 更新和后台计算。

**核心概念：**

- **前台任务**：UI 线程，可以更新实体（`cx.spawn`）
- **后台任务**：工作线程，CPU 密集型工作（`cx.background_spawn`）
- 所有实体更新都在前台线程上进行

## 快速入门

### 前台任务（UI 更新）

从 `Context<Self>` 生成时，闭包接收 `(WeakEntity<Self>, &mut AsyncApp)`：

```rust
impl MyComponent {
    fn fetch_data(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            // 在 UI 线程上运行，可以 await 并更新实体
            let data = fetch_from_api().await;

            this.update(cx, |state, cx| {
                state.data = Some(data);
                cx.notify();
            }).ok();
        }).detach();
    }
}
```

从 `&mut App` 生成时（不在实体内部），闭包仅接收 `(cx: &mut AsyncApp)`：

```rust
cx.spawn(async move |cx: &mut AsyncApp| {
    // 没有实体引用
}).detach();
```

### 带窗口上下文的生成（spawn_in）

当任务还需要窗口访问时使用 `spawn_in`（`update_in`）：

```rust
impl MyComponent {
    fn animate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            // 这里的 cx 是 AsyncWindowContext
            this.update_in(cx, |state, window, cx| {
                // 可以在这里访问窗口
                state.frame += 1;
                cx.notify();
            }).ok();
        }).detach();
    }
}
```

### 后台任务（繁重工作）

```rust
impl MyComponent {
    fn process_file(&mut self, cx: &mut Context<Self>) {
        let entity = cx.entity().downgrade();

        cx.background_spawn(async move {
            // 在后台线程上运行，CPU 密集型
            let result = heavy_computation().await;
            result
        })
        .then(cx.spawn(move |result, cx| {
            // 回到前台更新 UI
            entity.update(cx, |state, cx| {
                state.result = result;
                cx.notify();
            }).ok();
        }))
        .detach();
    }
}
```

### 任务管理

```rust
struct MyView {
    _task: Task<()>,  // 如果存储但不访问，使用 _ 前缀
}

impl MyView {
    fn new(cx: &mut Context<Self>) -> Self {
        let _task = cx.spawn(async move |this, cx: &mut AsyncApp| {
            // 任务在 drop 时自动取消
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                this.update(cx, |state, cx| {
                    state.tick();
                    cx.notify();
                }).ok();
            }
        });

        Self { _task }
    }
}
```

## 核心模式

### 1. 异步数据获取（从 Context<Self>）

```rust
cx.spawn(async move |this, cx: &mut AsyncApp| {
    let data = fetch_data().await?;
    this.update(cx, |state, cx| {
        state.data = Some(data);
        cx.notify();
    })?;
    Ok::<_, anyhow::Error>(())
}).detach();
```

### 2. 后台计算 + UI 更新

```rust
cx.background_spawn(async move {
    heavy_work()
})
.then(cx.spawn(move |this, cx: &mut AsyncApp| {
    this.update(cx, |state, cx| {
        state.result = result;
        cx.notify();
    }).ok();
}))
.detach();
```

### 3. 周期性任务

```rust
cx.spawn(async move |this, cx: &mut AsyncApp| {
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;
        this.update(cx, |state, cx| {
            state.tick();
            cx.notify();
        }).ok();
    }
}).detach();
```

### 4. 任务取消

任务在 drop 时自动取消。存储在结构体中以保持存活。

## 常见陷阱

### ❌ 不要：使用 `defer_in` 然后通过句柄更新同一实体

`cx.defer_in(window, callback)` 调度 `callback` 在**当前实体**上运行 — rgpui 重新获取该实体的锁来执行它。在延迟回调中调用 `entity.update(cx, …)` 会重新进入锁并 panic：

```
cannot update … while it is already being updated
```

```rust
// ❌ Panic：list 实体被 defer_in 锁定；调用 list.update 重新进入
fn confirm(&mut self, _: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
    cx.defer_in(window, |list_state, window, cx| {
        parent.update(cx, |this, cx| {
            this.inner_list.update(cx, |_, _| {}); // PANIC 如果 inner_list == 延迟的实体
        });
    });
}
```

```rust
// ✅ 正确：使用直接的 &mut 引用 — 不需要锁
fn confirm(&mut self, _: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
    cx.defer_in(window, |list_state, window, cx| {
        // 通过 &mut 引用直接访问列表数据
        list_state.delegate_mut().some_method();

        // 更新 *不同的* 实体 — 可以，不同的锁
        parent.update(cx, |this, cx| { /* … */ });

        // 在父级更新后直接同步列表状态 — 不需要锁
        list_state.delegate_mut().update_snapshot(new_val);
    });
}
```

规则：在 `defer_in` 回调内部，**永远不要对 `defer_in` 调度的实体调用 `entity.update(cx, …)` 或 `entity.read(cx)`**。使用回调提供的 `&mut Entity` 直接引用。

### ❌ 不要：从后台任务更新实体

```rust
// ❌ 错误：不能从后台线程更新实体
cx.background_spawn(async move {
    entity.update(cx, |state, cx| { // 编译错误！
        state.data = data;
    });
});
```

### ✅ 应该：使用前台任务或链式调用

```rust
// ✅ 正确：与前台任务链式调用
cx.background_spawn(async move { data })
    .then(cx.spawn(move |data, cx| {
        entity.update(cx, |state, cx| {
            state.data = data;
            cx.notify();
        }).ok();
    }))
    .detach();
```
