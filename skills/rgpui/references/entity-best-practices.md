# Entity 最佳实践

**目录：** [避免常见陷阱](#避免常见陷阱) · [性能优化](#性能优化) · [实体生命周期管理](#实体生命周期管理) · [实体观察最佳实践](#实体观察最佳实践) · [异步最佳实践](#异步最佳实践) · [测试最佳实践](#测试最佳实践) · [性能检查清单](#性能检查清单)

## 避免常见陷阱

### 避免重入实体访问（同一实体）

**问题：** GPUI 在渲染或更新传递期间锁定实体。在锁持有期间尝试 `read` 或 `update` *同一* 实体会 panic：

```
cannot update … while it is already being updated
```

```rust
// ❌ Panic：entity_a 从自身更新中更新
entity_a.update(cx, |_, cx| {
    entity_a.update(cx, |_, _| {}); // PANIC
});

// ✅ 可以：从更新中更新 *不同的* 实体
entity_a.update(cx, |_, cx| {
    entity_b.update(cx, |_, _| {}); // OK — 不同的锁
});
```

### `defer_in` 重新锁定实体 — 相同规则适用

`cx.defer_in(window, callback)` 调度 `callback` 在*上下文引用的实体*上运行。GPUI 重新获取该实体的锁来执行延迟回调，因此重入规则同样适用：

```rust
// ❌ Panic：defer_in 在 ListState 锁定时运行；调用 list.update 重新进入
fn confirm(&mut self, _: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
    cx.defer_in(window, |list_state, window, cx| {
        // list_state 在整个回调期间被锁定！
        parent.update(cx, |this, cx| {
            this.list.update(cx, |_, _| {}); // PANIC — 重新进入 ListState 锁
        });
    });
}
```

**修复：** 使用回调提供的直接 `&mut` 引用，而不是通过实体句柄：

```rust
// ✅ 正确：直接可变访问 — 不需要锁
fn confirm(&mut self, _: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
    cx.defer_in(window, |list_state, window, cx| {
        // 步骤 1：直接通过 list_state 调用钩子（不需要实体锁）
        list_state.delegate_mut().on_will_change(&mut op, &snapshot);

        // 步骤 2：更新父级实体 — 不同的锁，安全
        let new_sel = parent.update(cx, |this, cx| {
            this.state.apply_change(op);
            cx.notify();
            this.state.selection.clone() // 返回步骤 3 需要的数据
        });

        // 步骤 3：直接同步列表状态 — 不需要实体锁
        if let Ok(sel) = new_sel {
            list_state.delegate_mut().update_snapshot(sel.clone());
            list_state.delegate_mut().on_confirm(&sel);
        }
    });
}
```

### 渲染回调不得访问外部实体

`render_item` 和任何其他渲染钩子在实体的渲染传递内运行。对外部实体调用 `entity.read(cx)` 或 `entity.update(cx, …)` 会以相同的重入错误 panic。

**修复：** 从渲染外部的每次突变后急切地更新一个普通的 `snapshot` 字段：

```rust
// ❌ Panic — 在 ListState 渲染期间调用；外部实体访问重新进入
fn render_item(&mut self, ix: IndexPath, …) -> … {
    let checked = parent.read(cx).selection.contains(&ix); // PANIC
}

// ✅ 从快照字段读取 — 完全没有实体访问
fn render_item(&mut self, ix: IndexPath, …) -> … {
    let checked = self.selection_snapshot.iter().any(|(sel_ix, _)| sel_ix == &ix);
}
```

### 在闭包中使用弱引用

**问题：** 闭包中的强引用可能导致循环引用和内存泄漏。

**解决方案：** 在闭包中使用弱引用。

### 在闭包中使用内部上下文

**问题：** 使用外部上下文导致多次借用错误。

**解决方案：** 始终使用闭包提供的内部上下文。

## 性能优化

### 最小化 cx.notify() 调用

每次 `cx.notify()` 触发重新渲染。尽可能批量更新。

### 条件更新

仅在状态实际变化时通知。

### 对复杂操作使用 read_with

优先使用 `read_with` 而不是单独的 `read` 调用。

### 避免过度创建实体

创建实体有开销。适当时重用。

## 实体生命周期管理

### 清理弱引用

定期从集合中清理无效的弱引用。

### 实体克隆和共享

理解克隆 `Entity<T>` 会增加引用计数。

### 适当的资源清理

在 `Drop` 或显式清理方法中实现清理。

## 实体观察最佳实践

### 适当地分离订阅

对你希望保持存活的订阅调用 `.detach()`。

### 避免观察循环

不要在实体之间创建相互观察。

## 异步最佳实践

### 在异步任务中始终使用弱引用

### 优雅地处理异步错误

### 取消模式

为长时间运行的任务实现取消。

## 测试最佳实践

### 对实体测试使用 TestAppContext

### 测试实体观察

## 性能检查清单

在发布基于实体的代码之前，验证：

- [ ] 闭包/回调中没有强引用（使用 `WeakEntity`）
- [ ] 没有嵌套实体更新（使用顺序更新）
- [ ] 在更新闭包中使用内部 `cx`
- [ ] 在调用 `cx.notify()` 之前批量更新
- [ ] 定期清理无效的弱引用
- [ ] 对复杂读取操作使用 `read_with`
- [ ] 适当地分离订阅和观察者
- [ ] 在异步任务中使用弱引用
- [ ] 实体之间没有观察循环
- [ ] 异步操作中的适当错误处理
- [ ] 在 `Drop` 或显式方法中进行资源清理
- [ ] 测试覆盖实体生命周期和交互
