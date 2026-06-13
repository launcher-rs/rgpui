# 实体状态管理

**目录：** [概述](#概述) · [快速入门](#快速入门) · [核心原则](#核心原则) · [常见用例](#常见用例) · [参考文档](#参考文档)

## 概述

`Entity<T>` 是类型 `T` 状态的句柄，提供安全的访问和更新。

**关键方法：**
- `entity.read(cx)` → `&T` - 只读访问
- `entity.read_with(cx, |state, cx| ...)` → `R` - 带闭包的读取
- `entity.update(cx, |state, cx| ...)` → `R` - 可变更新
- `entity.downgrade()` → `WeakEntity<T>` - 创建弱引用
- `entity.entity_id()` → `EntityId` - 唯一标识符

**实体类型：**
- **`Entity<T>`**：强引用（增加引用计数）
- **`WeakEntity<T>`**：弱引用（不阻止清理，返回 `Result`）

## 快速入门

### 创建和使用实体

```rust
// 创建实体
let counter = cx.new(|cx| Counter { count: 0 });

// 读取状态
let count = counter.read(cx).count;

// 更新状态
counter.update(cx, |state, cx| {
    state.count += 1;
    cx.notify(); // 触发重新渲染
});

// 弱引用（用于闭包/回调）
let weak = counter.downgrade();
let _ = weak.update(cx, |state, cx| {
    state.count += 1;
    cx.notify();
});
```

### 在组件中

```rust
struct MyComponent {
    shared_state: Entity<SharedData>,
}

impl MyComponent {
    fn new(cx: &mut App) -> Entity<Self> {
        let shared = cx.new(|_| SharedData::default());

        cx.new(|cx| Self {
            shared_state: shared,
        })
    }

    fn update_shared(&mut self, cx: &mut Context<Self>) {
        self.shared_state.update(cx, |state, cx| {
            state.value = 42;
            cx.notify();
        });
    }
}
```

### 异步操作

从 `Context<Self>` 调用 `cx.spawn` 时，闭包接收 `(WeakEntity<Self>, &mut AsyncApp)`：

```rust
impl MyComponent {
    fn fetch_data(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let data = fetch_from_api().await;

            // 通过弱引用安全更新实体
            let _ = this.update(cx, |state, cx| {
                state.data = Some(data);
                cx.notify();
            });
        }).detach();
    }
}
```

## 核心原则

### 在闭包中始终使用弱引用

```rust
// ✅ 好：弱引用防止循环引用
let weak = cx.entity().downgrade();
callback(move || {
    let _ = weak.update(cx, |state, cx| cx.notify());
});

// ❌ 坏：强引用可能导致内存泄漏
let strong = cx.entity();
callback(move || {
    strong.update(cx, |state, cx| cx.notify());
});
```

### 使用内部上下文

```rust
// ✅ 好：使用闭包中的内部 cx
entity.update(cx, |state, inner_cx| {
    inner_cx.notify(); // 正确
});

// ❌ 坏：使用外部 cx（多次借用错误）
entity.update(cx, |state, inner_cx| {
    cx.notify(); // 错误！
});
```

### 避免嵌套实体更新

嵌套的 `entity.update(cx, …)` 调用是危险的。默认做法是：**不要嵌套它们**。

**同一实体 → 始终 panic。**  
GPUI 在更新或渲染传递期间锁定实体。重新进入同一锁会立即 panic：

```
cannot update … while it is already being updated
```

```rust
// ❌ Panic：从 entity_a 自身的更新中更新 entity_a
entity_a.update(cx, |state, cx| {
    entity_a.update(cx, |_, _| {}); // PANIC — 同一锁
});
```

**不同实体 → 通常安全，但间接循环仍会 panic。**  
每个实体有自己的锁，因此从 `entity_a` 的更新中更新 `entity_b` 通常成功。但是，如果 `entity_b` 的回调直接或通过链回到 `entity_a`，GPUI 将尝试重新获取 `entity_a` 的锁并 panic。

```rust
// ✅ 通常可以：不同实体，没有循环
entity_a.update(cx, |_, cx| {
    entity_b.update(cx, |_, _| {}); // OK — 不同锁
});

// ❌ Panic：间接循环回到 entity_a
entity_a.update(cx, |_, cx| {
    entity_b.update(cx, |_, cx| {
        entity_a.update(cx, |_, _| {}); // PANIC — entity_a 仍被锁定
    });
});
```

如有疑问，扁平化调用序列而不是嵌套：完成外部更新，然后从外部更新第二个实体。

**`defer_in` 不绕过锁。** `cx.defer_in(window, callback)` 调度 `callback` 在当前实体上运行 — 意味着 GPUI 重新获取实体的锁来执行它。重入规则同样适用于延迟回调：

```rust
// ❌ Panic：defer_in 重新锁定 entity_a；在内部调用 entity_a.update 重新进入
impl SomeDelegate for MyAdapter {
    fn confirm(&mut self, _: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        cx.defer_in(window, |list_state, window, cx| {
            // list_state 在此回调期间被锁定！
            parent.update(cx, |this, cx| {
                this.list.update(cx, |_, _| {}); // PANIC — list 已在上面被锁定
            });
        });
    }
}

// ✅ 修复：使用回调提供的直接 &mut 引用
impl SomeDelegate for MyAdapter {
    fn confirm(&mut self, _: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        cx.defer_in(window, |list_state, window, cx| {
            // 直接访问列表数据 — 不需要实体锁
            list_state.delegate_mut().some_hook();

            // 更新 *父级* 实体 — 不同锁，安全
            parent.update(cx, |this, cx| { /* … */ });

            // 在父级更新后直接同步列表状态
            list_state.delegate_mut().update_snapshot(new_val);
        });
    }
}
```

**渲染回调的快照模式。** `render_item`（和任何其他渲染钩子）在实体的渲染传递内运行。它绝不能对外部实体调用 `entity.read(cx)` 或 `entity.update(cx, …)`。相反，从渲染外部的每次突变后急切地更新一个普通的 `snapshot` 字段：

```rust
// ❌ Panic in render_item — ListState 已被锁定
fn render_item(&mut self, ix: IndexPath, window: &mut Window, cx: &mut Context<ListState<Self>>) -> … {
    let checked = parent_entity.read(cx).selection.contains(&ix); // PANIC
}

// ✅ 从普通快照字段读取 — 没有实体访问
fn render_item(&mut self, ix: IndexPath, window: &mut Window, cx: &mut Context<ListState<Self>>) -> … {
    let checked = self.selection_snapshot.iter().any(|(sel_ix, _)| sel_ix == &ix);
}
```

## 常见用例

1. **组件状态**：需要响应性的内部状态
2. **共享状态**：多个组件之间共享的状态
3. **父子关系**：协调相关组件（使用弱引用）
4. **异步状态**：管理从异步操作变化的状态
5. **观察**：响应其他实体的变化

## 参考文档

- **API**：参见 [entity-api.md](entity-api.md) — 实体类型、方法、生命周期、错误处理
- **模式**：参见 [entity-patterns.md](entity-patterns.md) — 模型-视图、跨实体通信、观察者
- **最佳实践**：参见 [entity-best-practices.md](entity-best-practices.md) — 陷阱、内存、性能、异步
- **高级**：参见 [entity-advanced.md](entity-advanced.md) — 集合、注册表、防抖、状态机
