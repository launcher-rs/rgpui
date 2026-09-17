# 1.2 → 1.3 迁移指南（回调签名统一为主）

1.3 是 dev 分支上的 breaking 大版本，无兼容层，直接改。影响最大的是
C1 + C2（H1）两轮回调签名统一：约 30 个组件的 `on_click` / `on_change` /
`on_select` 改名、改参、换存储，外加 `StatusBar` 数据驱动重写与
`chat_ui` 别名删除。本指南按“必改 → 易踩 → 未改”组织，可照单迁移。

约定速览（三条规则，详见 `rgpui-book/08-components.md` 回调章节）：

1. **纯点击一律 `on_click`**：`Fn(&ClickEvent, &mut Window, &mut App)`。
2. **值变更一律 `on_change`**：`Fn(Value, &mut Window, &mut App)`，
   值一律按值传递（`bool` / `usize` / `f32` / `Hsla` / `NaiveDate` /
   `SharedString` / `Vec<T>` / 自定义值类型）。
3. **存储统一 `Arc + Send + Sync + 'static`**（向严格方向统一）。

---

## 一、改名（方法名变，签名多半也变）

| 组件 | 1.2 | 1.3 | 说明 |
|------|-----|-----|------|
| `Checkbox` / `Switch` / `Radio` / `RadioGroup` / `TabBar` | `on_click` | `on_change` | 值语义，见第二表 |
| `Sidebar` / `Upload` / `NavigationMenu` | `on_select` | `on_change` | 值语义，见第二表 |
| `InteractiveText` | `on_click(ranges, …)` | `on_change(ranges, …)` | 范围索引 `usize` 按值 |
| `CompletionPopup` | `on_select` | `on_change` | 补全索引 `usize` 按值 |
| `Command` / `CommandPalette` | `on_select` / `on_close` | **保留原名** | 执行语义（选中即运行），不是值变更 |

---

## 二、签名变更（按值传递 + 补参数）

### 按值传递（去引用）

| 组件 | 1.2 | 1.3 |
|------|-----|-----|
| `Checkbox` / `Switch` / `Radio(bool)` / `Toggle(bool)` | `Fn(&bool, …)` | `Fn(bool, …)` |
| `RadioGroup` / `TabBar` / `Pagination` | `Fn(&usize, …)` | `Fn(usize, …)` |
| `Carousel` | `Fn(usize, usize, …)` | 不变（双值保留，仅存储转 `Arc`） |
| `Select` | `Fn(usize, &SharedString, …)` | `Fn(usize, SharedString, …)` |
| `SegmentedNav` / `Sidebar` / `Tabs` | — | `Fn(SharedString, …)`（已按值） |
| `Combobox` | `Fn(&[usize], …)` | `Fn(Vec<usize>, …)` |
| `TagInput` | `Fn(&[SharedString], …)` | `Fn(Vec<SharedString>, …)` |
| `Upload` | `Fn(&[…], …)` | `Fn(Vec<PathBuf>, …)` |
| `Rate` / `ColorPicker` / `DatePicker` | `Fn(f32 / Hsla / NaiveDate, …)` | 不变（本来就按值，仅存储转 `Arc`） |
| `HotkeyInput` | `Fn(Option<&HotkeyValue>, …)` | `Fn(Option<HotkeyValue>, …)` |
| `HotkeyListInput` | `Fn(&[HotkeyValue], …)` | `Fn(Vec<HotkeyValue>, …)` |

`SharedString` / `Vec` 均为 Arc-backed 廉价克隆，回调同步执行无生命周期问题，
放心按值收。典型改法：

```rust
// 旧：
Checkbox::new("notify").on_click(cx.listener(|this, checked: &bool, _, cx| {
    this.notify = *checked;
    cx.notify();
}));

// 新（改名 + 按值 + listener_value，见第五节）：
Checkbox::new("notify").on_change(cx.listener_value(
    |this, checked: bool, _, cx| {
        this.notify = checked;
        cx.notify();
    },
));
```

```rust
// 旧：
Select::new(options).on_change(move |ix, label: &SharedString, _, cx| {
    demo.update(cx, |this, _| {
        this.fruit = label.to_string();
    })
});

// 新（双值回调直接实体捕获，无需 listener）：
Select::new(options).on_change(move |ix: usize, label: SharedString, _, cx| {
    demo.update(cx, |this, _| {
        this.fruit = label.to_string();
    })
});
```

### 补事件 / 补窗口参数

| 组件 | 1.2 | 1.3 | 改法 |
|------|-----|-----|------|
| `BreadcrumbItem::on_click` | `Fn(&mut Window, &mut App)` | `Fn(&ClickEvent, &mut Window, &mut App)` | 闭包首参加 `_,` |
| `typography::Link::on_click` | `Fn(&mut Window, &mut App)` | `Fn(&ClickEvent, &mut Window, &mut App)` | 同上 |
| `StatusBarItem::on_click` | `Fn(&mut Window, &mut App)` | `Fn(&ClickEvent, &mut Window, &mut App)` | 同上 |
| `OTPInput::on_change` / `on_complete` | `Fn(String, &mut App)` | `Fn(String, &mut Window, &mut App)` | 闭包加 `_,`（中参） |

```rust
// 旧：
Link::new("帮助").on_click(move |_, cx| { /* … */ });

// 新（补事件参数）：
Link::new("帮助").on_click(move |_, _, cx| { /* … */ });
```

---

## 三、存储与线程界限（最易大面积报错的一节）

所有 `on_click` / `on_change`（及 `Command` 系）存储统一为
`Arc<dyn Fn(…) + Send + Sync + 'static>`。**闭包若捕获了 `Rc` / `RefCell` /
`Cell` 等非 `Send` 类型，编译直接失败**——这是本次迁移体感最大的 breaking，
但修法统一：

```rust
// 旧（1.2 能过，1.3 报错：closure is not Send）：
let seen = Rc::new(Cell::new(false));
Checkbox::new("x").on_change(move |checked: &bool, _, _| {
    seen.set(true);
});

// 新：换线程安全容器（测试/单线程代码同理）：
let seen = Arc::new(AtomicBool::new(false));
Checkbox::new("x").on_change(move |checked: bool, _, _| {
    seen.store(checked, Ordering::SeqCst);
});
```

另外两条出路（按场景选）：

- 捕获 `Entity<T>`：`Entity` 本身 `Send + Sync`，沿用旧写法即可
  （`demo.update(cx, …)` 模式不受影响）。
- 需要回写视图状态：用 `cx.listener_value`（下一节），别手写 `Arc` 包装。

---

## 四、未改动的回调（仍 `Rc` / `Box`，非本次范围）

以下**没变**，不要顺手“统一”它们（类型不同，改了反而编译失败）：

| 回调 | 现状 | 说明 |
|------|------|------|
| `BottomSheet::on_close` / `ImageViewer::on_close` / `DrawerNavigation::on_close` / `SearchPanel::on_close` | `Rc<Fn(&mut Window, &mut App)>` | 关闭语义，不在统一范围 |
| `Countdown::on_complete` | `Rc<Fn(&mut Window, &mut App)>` | 完成语义 |
| `NotificationCenter::on_notification_click` / `NotificationCard::on_click` | `Rc<…>` | 注意与已 Arc 化的 `menu::Notification` 区分 |
| `InlineEdit::on_cancel` | `Rc<Fn(&mut Window, &mut App)>` | |
| `MenuItemElement::on_click` | `Box<Fn(&ClickEvent, …)>` | 内部元素 |
| `TypeWriter::on_complete` | `Box<Fn(&mut App)>` | 无 Window 参数 |

---

## 五、`listener` vs `listener_value`：看签名选

```rust
use rgpui::{Checkbox, prelude::*};

// 引用型事件（&ClickEvent 等纯点击类）→ listener：
Button::new("ok").label("确定").on_click(cx.listener(
    |this: &mut MyView, _: &ClickEvent, _, cx| {
        this.confirmed = true;
        cx.notify();
    },
));

// 按值型事件（所有 on_change）→ listener_value：
Checkbox::new("notify").checked(true).on_change(cx.listener_value(
    |this: &mut MyView, checked: bool, _, cx| {
        this.notify = checked;
        cx.notify();
    },
));
```

- `cx.listener(|this, e: &E, _, cx| …)`：配 `Fn(&E, …)`（纯点击类）。
- `cx.listener_value(|this, e: E, _, cx| …)`：配 `Fn(E, …)`（所有 `on_change`）。
  C1 之前值回调全是引用，`listener` 就够了；改按值后才加了它。
- `processor` 也是按值的，但会透出内部返回值类型，不适配返回 `()` 的
  `on_change`，别混用。
- 双值回调（`Select(usize, SharedString)`、`Carousel(usize, usize)`）没有配套
  listener 变体，直接实体捕获（见第二节第二例）。

---

## 六、行为注记（签名之外的语义变化）

- **OTP 回调经 `spawn` 延后**：`OTPInput::on_change` / `on_complete`
  的触发常在按键分发中（窗口正被 take），同步 `update` 窗口必失败
  （F12 同款教训），故实现经 `spawn` 延后分发、无活动窗口时跳过。
  后果：回调不再与 `set_value` / 按键**同步**执行（微任务延迟），
  依赖“回调内立即读到后续状态”的代码需改为状态驱动；
  回归测试 `otp_callbacks_fire_when_emitted_during_dispatch` 覆盖此路径。
- **`Combobox` 延后到 render 触发**：toggle 时无 Window，置 `pending_emit`
  延后到 render 期触发（C1 已有行为，1.3 未变，顺带一提以免误判时序）。
- **`Carousel::on_change` 保留 `(旧下标, 新下标)` 双值**，别按单值迁移。
- **`StatusBar` 数据驱动重写**（breaking）：旧 `StatusBarState` 实体
  （`line` / `column` / `language` / `encoding` 等编辑器专有字段）+
  `StatusBar::new(status实体)` 已删除；新 `StatusBar::new().left(…).right(…)`
  纯值驱动，条目带稳定 `id`：

```rust
// 旧：
let status = cx.new(|_| StatusBarState { line: 1, column: 1, ..Default::default() });
StatusBar::new(status)

// 新：
StatusBar::new()
    .left(vec![StatusBarItem::new("main").id("branch")])
    .right(vec![StatusBarItem::new("Ln 42, Col 15").id("cursor").muted(true)])
```

`rgpui_story` 与 `v1_1_showcase` 的状态栏页已同步改吃新组件，可对照。

---

## 七、其他 breaking 与新增（回调之外）

- **删除 `chat_ui` 别名**（Z1）：1.2.0 已迁移到 `chat`，1.3.0 按计划删除。
  全局替换 `chat_ui` → `chat` 即可（内部已确认零引用旧路径）。
- 新增（无需迁移，顺带掌握）：值驱动 `rgpui::tabs::{Tabs, TabsItem}`；
  `Root::open_dialog` 返回 `DialogId` + `close_dialog_by(id)` 按标识关闭；
  `TreeEvent::{Selected, Confirmed}`；`I18nManager` Global +
  `I18nText::translate_global(cx)`；`App::on_global_action` 全局动作 helper；
  检查器默认面板（`enable_default_inspector` 两行启用）与面板插槽。
  集中演示见 `examples/v1_3_showcase`。

---

## 八、迁移步骤与验证

1. 全局替换方法名（注意同名不同义，逐处确认）：
   `Checkbox|Switch|Radio|RadioGroup|TabBar` 的 `.on_click(` →
   `.on_change(`；`Sidebar|Upload|NavigationMenu` 的 `.on_select(` →
   `.on_change(`；`InteractiveText::…on_click(` → `.on_change(`；
   `CompletionPopup…on_select(` → `.on_change(`。
   `Command::on_select` / `CommandPalette::on_close` **不要动**。
2. 按第二表改闭包参数（去 `&` / 补事件参数 / 补 `Window`）。
3. 修 `Send` 报错：`Rc`/`Cell`/`RefCell` 捕获换 `Arc<Mutex/Atomic>` 或
   `Entity`，视图回写换 `listener_value`。
4. `StatusBarState` 相关代码按第六节重写；`chat_ui` 全局替换为 `chat`。
5. 验证（与 CI 同口径）：

```text
cargo check --workspace --all-targets
cargo fmt --all
cargo clippy --workspace --lib --bins -- -D warnings
cargo test --workspace --lib
cargo run -p v1_3_showcase   # 对照新 API 行为
```
