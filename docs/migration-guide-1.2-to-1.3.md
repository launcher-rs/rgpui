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

## 二、易混点详解（先看懂命名，再动手改）

总原则只有一条：**方法跟着组件的性质走，不跟着需求演变走**。
判断时问两问——（1）这个组件产出值吗？（2）值归谁所有？

### Button：先点击、后想加值变更，要换方法吗？

不用。`Button` 永远不产出值，所以永远是 `on_click`。
计数器、打开对话框、提交表单——这些都是**应用**的状态，
在点击回调里自己改就行，方法不变：

```rust
// 计数器归应用所有 → 保持 on_click，在里面改应用状态
Button::new("add").label("+1").on_click(cx.listener(
    |this: &mut MyView, _: &ClickEvent, _, cx| {
        this.count += 1;
        cx.notify();
    },
));
```

哪天你发现“这个按钮其实是个开关”，那说明**组件选错了**——
换成拥有值的组件，方法自然就换了：

```rust
// 开关拥有 checked 值 → on_change(bool)，新值按值给到你
Switch::new().checked(self.notify).on_change(cx.listener_value(
    |this: &mut MyView, checked: bool, _, cx| {
        this.notify = checked;
        cx.notify();
    },
));
```

### Tab vs TabBar：同一次点击，两种视角

这是理解整套规则最好的例子。用户点了一下“设置”页签，
同一时刻发生两件事：单个 `Tab` 被点了（事件），
`TabBar` 的选中下标变了（值）。所以：

```rust
TabBar::new("tabs")
    .selected_index(self.selected)
    // 栏拥有选中下标 → on_change(usize)
    .on_change(cx.listener_value(|this, ix: usize, _, cx| {
        this.selected = ix;
        cx.notify();
    }))
    .child(Tab::new().label("首页"))
    // 单个 Tab 只报告被点 → on_click(&ClickEvent)
    .child(Tab::new().label("设置").on_click(cx.listener(
        |_, _: &ClickEvent, _, _| { /* 偶尔需要单签特殊处理才用 */ },
    )));
```

日常只用 `TabBar::on_change` 即可；`Tab::on_click` 留给极少数
“某个签点击有额外动作”的场景。

### BreadcrumbItem / Link：看着像“选东西”，为什么是 on_click？

因为它们**不拥有值**：点完还是那个链接，组件自身没有任何状态变化。
它们是导航触发器，不是值生产者，所以是纯点击。1.3 只是给它们补上了
`&ClickEvent` 参数，改法是机械的——闭包首部加一个 `_,`：

```rust
// 旧：Link::new("帮助").on_click(move |_, cx| { /* … */ });
// 新：
Link::new("帮助").on_click(move |_, _, cx| { /* … */ });
```

### InteractiveText：明明是点击，为什么改成 on_change？

反例证明规则。文本里有多个可点范围（如链接），点击产出的是
“第几个范围”这个**索引值**，调用方靠它区分点中了哪个链接。
有值产出 → 值变更 → 1.3 从 `on_click(ranges, …)` 改名
`on_change(ranges, …)`（`ranges` 参数保留，它声明“哪些范围可点”）：

```rust
// 旧：InteractiveText::new(id, styled).on_click(ranges, move |idx, _, cx| { … });
// 新：
InteractiveText::new(id, styled).on_change(ranges, move |idx, _, cx| {
    if let Some(url) = urls.get(idx) {
        cx.open_url(url);
    }
});
```

### Command：on_select 为什么保留原名？

`Command::on_select` 看似值变更，实则是**执行语义**：选中即运行命令，
没有“新值”交给调用方（回调签名 `Fn(&mut Window, &mut App)` 连值都没有）。
改名反而误导，所以 breaking 大版本里特意保留原名。迁移时**不要动它**，
同理 `CommandPalette::on_close`。

### 小结：决策两问

1. 组件产出值吗？不产出（Button / Link / BreadcrumbItem / Tab / Command）→
   `on_click`（或执行语义原名），需求再变也不换。
2. 产出值？看类型：单值（bool / usize / SharedString / …）→ `on_change` 配
   `listener_value`；双值（`Select(usize, SharedString)`、
   `Carousel(旧下标, 新下标)`）→ `on_change` 配实体直接捕获（没有配套
   listener 变体，见第六节）。

### 附：在 Checkbox 上写 on_click 会怎样（静默坑）

分两种情况：

1. **迁旧代码**（闭包还是 `|checked: &bool, …|` 形状）：**直接编译报错**
   （期望 `&ClickEvent`，拿到 `&bool`）。这是好事，照本指南改即可。
2. **新代码手滑写成事件形状**（`move |_, _, cx| …`）：**能编译，但行为是错的**。
   因为 `Checkbox` 实现了 `StatefulInteractiveElement`（prelude 常驻），
   trait 自带的 `on_click` 仍可调用，它只是往点击监听链里追加一个普通监听：
   你的回调会执行，但复选框的翻转逻辑（`handle_change` → `on_change` →
   父组件回写 `checked`）完全绕过去了——**框永远打不上勾**；
   且 `disabled` 时内部翻转不挂载，你的监听却照样触发。

```rust
// 错：能编译，框打不上勾（且 disabled 照样触发）
Checkbox::new("x").label("通知").on_click(move |_, _, cx| {
    demo.update(cx, |this, _| { this.notify = true; });
});

// 对：走 on_change，父组件回写 checked
Checkbox::new("x").label("通知").checked(self.notify).on_change(
    cx.listener_value(|this, checked: bool, _, cx| {
        this.notify = checked;
        cx.notify();
    }),
);
```

同理适用于 `Switch` / `Radio` / `TabBar` 等一切实现
`StatefulInteractiveElement` 的值组件：**值组件上只用 `on_change`**，
看到 `on_click` 能编译通过不要信，那是底层元素的通用监听。

---

## 三、签名变更（按值传递 + 补参数）

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

// 新（改名 + 按值 + listener_value，见第六节）：
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

## 四、存储与线程界限（最易大面积报错的一节）

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

## 五、未改动的回调（仍 `Rc` / `Box`，非本次范围）

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

## 六、`listener` vs `listener_value`：看签名选

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

## 七、行为注记（签名之外的语义变化）

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

## 八、检查器：从手写面板到两行启用（新增为主，附带清理）

1.2 每个应用手写 `set_inspector_renderer` + 状态展示约 200 行；
1.3 框架给了默认面板，接入只剩两行：

```rust
// 应用入口（窗口创建前调用）：
cx.enable_default_inspector(); // 注册默认面板 + Div 布局展示

// 开关面板（F12 等快捷键；正式 UI 不放调试按钮，检查器只走快捷键）：
window.toggle_inspector(cx);
```

默认面板内容：状态徽标、拾取按钮（悬停蓝框、点击选中、滚轮穿透层级）、
选中元素源码位置与实例号、完整树（逐节点折叠，点击行选中画布对应区域）、
`Div` 布局边界与内容尺寸。顶栏固定（标题/拾取按钮滚不走），
展开集只增不重置（点树不塌浏览进度）。

### 自定义面板（三档，由浅入深）

1. **插槽**（只换皮）：`cx.set_inspector_panel_slots` 覆盖顶栏
   （`InspectorHeaderSlot`）或信息卡外皮（`InspectorSectionSlot`），
   `None` 即默认。
2. **按类型扩展**（加一块）：`cx.register_inspector_element` 为自有元素状态
   加展示卡；同类型后注册覆盖先注册（可覆盖默认 Div 展示）。
3. **整板替换**（全自写）：`cx.set_inspector_renderer`，行点击经
   `window.select_inspector_element` 选中，树数据经
   `inspector_tree_roots/children/parent` 拿，选中经
   `Inspector::select` / `select_ancestor` 切换。

 living 范例：`examples/inspector/`（默认面板）与
 `examples/inspector_custom/`（全自写 + 覆盖 Div 展示），演示内容各自独立。

### F12 全局开关（必抄的接线，三个坑）

不要用带上下文绑定 + 视图 `on_action`：无焦点时分发路径只有 root，
上下文匹配不上、冒泡也到不了视图 handler；面板聚焦时同样到不了
（面板是独立 prepaint 根）。正确接法——全局绑定（无上下文）+
全局监听打到活动窗口，**监听内必须 spawn 延后更新**
（分发中窗口已被 take，同步 `update_window` 必失败，勿用 `_ =` 吞 Result）：

```rust
cx.bind_keys([KeyBinding::new("f12", ToggleInspector, None)]);
cx.on_action(|_: &ToggleInspector, cx: &mut App| {
    if let Some(window) = cx.active_window() {
        cx.spawn(async move |cx| {
            _ = window.update(cx, |_, window, cx| {
                window.toggle_inspector(cx);
            });
        })
        .detach();
    }
});
```

回归测试 `f12_toggles_inspector_without_focus` 覆盖无焦点路径。
（注意测试平台的 `App::activate` 是空实现，单测里用
`window.activate_window()` 设置活动窗口。）

### 发布剥离（推荐的上线姿势）

库侧检查器 API 全是 `#[cfg(any(feature = "inspector", debug_assertions))]`，
release 默认零代码。应用侧三条：Cargo 里**不要**开 `inspector` feature
（dev 靠 `debug_assertions` 自动生效）；装配线同条件 `#[cfg]` 包起来；
面板里不放按钮。release 想保留（如内部工具）：
`--release --features inspector`（两示例即此布局）。

---

## 九、其余新增（无需迁移，顺带掌握）

- **值驱动 `rgpui::tabs::{Tabs, TabsItem}`**：`items/active/on_change`，
  `active` 按 id 比对，与新 `StatusBar` 同模式，无实体；
  两三个静态页签零同步代码接入（`rgpui_story` tabs 页有演示）。
- **`Root::open_dialog` 返回 `DialogId`** + `close_dialog_by(id)` 按标识关闭
  栈中任意位置（上层不受影响，未知 id 返回 `false`）。
- **`TreeEvent::Confirmed(id)`**：Enter 在文件行触发（目录行仅切换展开，
  与 `Selected` 仅点击触发对称），键盘开文件凭此实现，无需另绑动作。
- **i18n**：`impl Global for I18nManager`（`cx.set_global` / `cx.global` 共享）+
  `load_locale_dir`（`<locale>.json` 按文件名加载，原生平台）+
  `I18nSnapshot` 快照/回退（`snapshot` / `restore`）+
  `I18nText::translate_global(cx)`（读全局 manager，一行取文）。
- **`App::on_global_action`**：全局动作 helper（F12 沉淀），封装
  “全局绑定 + 打到活动窗口 + spawn 延后更新”三件套。
- **检查器默认面板**（见第八节）与面板插槽。
- 集中演示见 `examples/v1_3_showcase`。

---

## 十、迁移步骤与验证

1. 全局替换方法名（注意同名不同义，逐处确认）：
   `Checkbox|Switch|Radio|RadioGroup|TabBar` 的 `.on_click(` →
   `.on_change(`；`Sidebar|Upload|NavigationMenu` 的 `.on_select(` →
   `.on_change(`；`InteractiveText::…on_click(` → `.on_change(`；
   `CompletionPopup…on_select(` → `.on_change(`。
   `Command::on_select` / `CommandPalette::on_close` **不要动**。
2. 按第二表改闭包参数（去 `&` / 补事件参数 / 补 `Window`）。
3. 修 `Send` 报错：`Rc`/`Cell`/`RefCell` 捕获换 `Arc<Mutex/Atomic>` 或
   `Entity`，视图回写换 `listener_value`。
4. `StatusBarState` 相关代码按第七节重写；`chat_ui` 全局替换为 `chat`。
5. 验证（与 CI 同口径）：

```text
cargo check --workspace --all-targets
cargo fmt --all
cargo clippy --workspace --lib --bins -- -D warnings
cargo test --workspace --lib
cargo run -p v1_3_showcase   # 对照新 API 行为
```
