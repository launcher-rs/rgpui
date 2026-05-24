# 鼠标穿透（Passthrough）行为分析

## 核心概念

两个独立的穿透控制层：

| 层级 | 名称 | 控制方式 | 效果 |
|------|------|----------|------|
| 渲染层 | `state.passthrough` | `PetSharedState` 字段 | `!passthrough` 时显示控制面板 |
| 平台层 | `mouse_passthrough` | `window.set_mouse_passthrough()` → `WM_NCHITTEST` | `true` 时返回 `HTTRANSPARENT`（穿透） |

**两者必须同步**：面板可见 = 平台层关闭穿透；面板隐藏 = 平台层开启穿透。

## 期望行为矩阵

| # | 场景 | running | interactive | 活动 | 面板 | 平台穿透 | 说明 |
|---|------|:-------:|:-----------:|------|:----:|:--------:|------|
| 1 | 启动（散步） | true | false | Walk | 隐藏 | ON | 初始状态 |
| 2 | 散步中自动转进食 | true | false | Eat | 显示 | OFF | 面板弹出自交互 |
| 3 | 非散步时点击"散步"按钮 | true | false | Walk | 隐藏 | ON | 回归散步 |
| 4 | 按 Ctrl 进入交互 | false | true | Sleep | 显示 | OFF | 面板可点击 |
| 5 | 交互中点非散步按钮 | true | true | Eat/Spin/Yawn | 显示 | OFF | 保持交互 |
| 6 | 交互中点"散步" | true | false | Walk | 隐藏 | ON | 退出交互 |
| 7 | **按 Ctrl 退出交互** | true | false | Walk | 隐藏 | **ON** | **必须强制散步** |
| 8 | Ctrl+Shift+P 暂停 | false | true | Sleep | 显示 | OFF | 暂停状态 |
| 9 | Ctrl+Shift+P 恢复 | true | false | Walk | 隐藏 | ON | 恢复散步 |

**核心原则**：鼠标穿透由"是否正在散步"决定。散步时穿透 ON，其他活动穿透 OFF（面板现身让用户交互）。

## 当前代码分析

### 状态成员变量

```rust
running: bool,       // 是否自动运行
interactive: bool,   // 是否交互模式
passthrough: bool,   // 渲染层穿透标记
activity: CatActivity, // 当前活动
previous_activity: Option<CatActivity>, // 进入交互前保存的活动
requested_activity: Option<CatActivity>, // 用户/系统请求的下个活动
```

### 状态修改入口

| 函数 | 调用时机 | running | interactive | passthrough | activity |
|------|---------|:-------:|:-----------:|:-----------:|----------|
| `new()` | 启动 | true | false | true | Walk |
| `toggle_running()` | Ctrl+Shift+P 暂停 | false | true | false | Sleep |
| `toggle_running()` | Ctrl+Shift+P 恢复 | true | false | true | Walk |
| `enter_interactive()` | 按 Ctrl 进入 | false | true | false | Sleep |
| `exit_interactive()` | 按 Ctrl 退出 | true | false | true | **恢复 previous_activity** |
| `request_activity(Walk)` | 点"散步" | true | false | true | Walk |
| `request_activity(Eat/Spin/Yawn)` | 点非散步 | true | true | false | Eat/Spin/Yawn |
| `request_activity(Sleep)` | 点"睡觉" | false | true | false | Sleep |
| `tick()` | 每帧 | — | — | 非交互时: `activity==Walk` | 更新为活动 |

## 发现的 Bug

### Bug 1（严重）：`exit_interactive()` 恢复 previous_activity 导致穿透失效

**问题**：进入交互后点击"吃东西"，`previous_activity = Some(Eat)`。按 Ctrl 退出时：

```
exit_interactive()
  → resume = previous_activity.take() = Some(Eat)
  → requested_activity = Some(Eat)
  → passthrough = true     // 临时设为 true

tick()
  → behavior.force(Eat)
  → activity = Eat
  → passthrough = (Eat == Walk) = false  ← 被 tick 覆盖回 false

window.set_mouse_passthrough(false)  ← 平台层关闭穿透
render: passthrough=false → 面板出现!  ← 用户刚按Ctrl退出，面板又弹出来
```

**根本原因**：`exit_interactive()` 恢复上次活动，但只有 Walk 能开启穿透。恢复 Eat/Spin/Yawn 时穿透被 tick() 关掉。

**影响**：用户按 Ctrl 退出交互，期望面板消失、穿透开启。但如果之前点了非散步按钮，面板会立刻重新出现。

**修复**：`exit_interactive()` 强制回到 Walk（与 `toggle_running()` 恢复时一致）。

### Bug 2（次要）：按钮点击后未立即触发重绘

**问题**：`action_button` 的 `on_click` 调用 `request_activity` 修改状态，但没有调用 `cx.notify()`，状态更新延迟 1 帧（16ms）。

**影响**：轻微可感知延迟，非关键。

### Bug 3（已验证修复）：WM_NCHITTEST 已被 WS_EX_LAYERED 覆盖

**状态**：已修复。不再使用 WS_EX_LAYERED，穿透完全由 WM_NCHITTEST 处理器控制。

## 修复方案

1. **`exit_interactive()` 强制 Walk**：删除 `previous_activity` 恢复逻辑，退出交互时始终回到散步。与 `toggle_running()` 恢复行为一致。
2. **按钮点击触发重绘**：在 `action_button` 的 `on_click` 中调用 `cx.notify()`。

## 修改后行为验证（修复 Bug 1 后）

### 场景 A：散步 → 自动进食 → 面板弹出 → 点"散步"

```
初始: R=true, I=false, P=true, A=Walk
行为决定进食: tick → A=Eat, P=false → 面板显示
用户点"散步": request_activity(Walk) → R=true, I=false, P=true
tick: force(Walk), A=Walk, P=true → 面板隐藏, 穿透开启 ✓
```

### 场景 B：散步 → 按 Ctrl 进入交互 → 点"吃东西" → 按 Ctrl 退出

```
散步: R=true, I=false, P=true, A=Walk
按 Ctrl: enter_interactive() → R=false, I=true, P=false, A=Sleep
点"吃东西": request_activity(Eat) → R=true, I=true, P=false, prev=Eat
按 Ctrl: exit_interactive() → R=true, I=false, P=true, request=Walk (强制)
tick: force(Walk), A=Walk, P=true → 面板隐藏, 穿透开启 ✓
```

### 场景 C：散步 → 按 Ctrl 进入交互 → 直接按 Ctrl 退出

```
按 Ctrl: enter_interactive() → R=false, I=true, P=false, A=Sleep
再按 Ctrl: exit_interactive() → R=true, I=false, P=true, request=Walk
tick: force(Walk), A=Walk, P=true → 面板隐藏, 穿透开启 ✓
```

### 场景 D：散步 → 自动进食 → 面板弹 → 点"吃东西" → 按 Ctrl 退出

```
自动进食: tick → A=Eat, P=false → 面板显示, 穿透关闭
点"吃东西": request_activity(Eat) → R=true, I=true, P=false
按 Ctrl: exit_interactive() → R=true, I=false, P=true, request=Walk
tick: force(Walk), A=Walk, P=true → 面板隐藏, 穿透开启 ✓
```

所有场景修复后一致：退出交互 = 强制散步 = 屏幕穿透开启。
