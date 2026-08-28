# 从 rgpui-component / rgpui-tokio 迁移到新版 rgpui

本文档指导依赖旧 `rgpui-component`、`rgpui-tokio` 等独立 crate 的项目，迁移到整合后的新版 rgpui 核心。

## 背景

旧版 rgpui 生态包含多个独立 crate：

- `rgpui-component`：UI 组件库（Button、Input、Select、Switch 等）
- `rgpui-tokio`：Tokio 异步运行时集成
- `rgpui-ui`：扩展组件库（动画、特效等）

这些 crate 已于 2026-08 全部合并进 `rgpui` 核心，旧 crate 已从 workspace 删除。

---

## 一、Cargo.toml 依赖变更

### 删除旧依赖

```toml
# 删除以下行：
rgpui-component = { git = "...", branch = "dev" }
rgpui-tokio = { git = "...", branch = "dev" }
rgpui-ui = { git = "...", branch = "dev" }
```

### 更新 rgpui 依赖

```toml
# 旧：
rgpui = { git = "...", branch = "dev" }

# 新（按需启用 feature）：
rgpui = { git = "...", branch = "dev", features = ["tokio"] }
```

可用 feature：

| Feature | 说明 |
|---------|------|
| `tokio` | 启用 `rgpui::tokio` 模块，提供 Tokio 运行时集成 |
| `charts` | 启用图表组件（bar、line、pie 等） |
| `effects` | 启用特效组件（aurora、confetti、particle_emitter 等） |
| `qr-code` | 启用二维码组件 |
| `dom-backend` | Web 平台 DOM 覆盖层（仅 WASM） |

### 添加 rgpui-markdown（如需要 Markdown 渲染）

```toml
rgpui-markdown = { git = "...", branch = "dev" }
```

---

## 二、import 路径变更总表

### 组件类型

| 旧路径 | 新路径 |
|--------|--------|
| `rgpui_component::button::Button` | `rgpui::Button` |
| `rgpui_component::button::ButtonVariants` | `rgpui::ButtonVariants` |
| `rgpui_component::input::Input` | `rgpui::input_ui::Input` |
| `rgpui_component::input::InputState` | `rgpui::input_ui::InputState` |
| `rgpui_component::switch::Switch` | `rgpui::Switch` |
| `rgpui_component::Root` | `rgpui::Root` |
| `rgpui_component::IndexPath` | `rgpui::IndexPath` |
| `rgpui_component::text::TextView::markdown(...)` | `rgpui_markdown::Markdown::new(...)` |

### Sizable / Size

| 旧路径 | 新路径 |
|--------|--------|
| `rgpui_component::Sizable` | `rgpui::Sizable`（也在 `prelude` 中） |
| `rgpui_component::Size` | `rgpui::StyleSized` |

### Tokio 异步运行时

| 旧路径 | 新路径 |
|--------|--------|
| `rgpui_tokio::init(cx)` | `rgpui::tokio::init(cx)` |
| `rgpui_tokio::Tokio::spawn(cx, f)` | `rgpui::tokio::Tokio::spawn(cx, f)` |
| `rgpui_tokio::Tokio::spawn_result(cx, f)` | `rgpui::tokio::Tokio::spawn_result(cx, f)` |
| `rgpui_tokio::Tokio::handle(cx)` | `rgpui::tokio::Tokio::handle(cx)` |

### 初始化调用

旧版只需一个统一的 `init()`：

```rust
rgpui_component::init(cx);  // 旧
```

新版需要按需调用各子系统的 `init()`：

```rust
rgpui::theme::init(cx);       // 主题（必须）
rgpui::input_ui::init(cx);    // 输入组件
rgpui::menu::init(cx);        // 菜单组件
rgpui::components::init(cx);  // 扩展组件（动画、内联编辑等）
rgpui::list::init(cx);        // 列表组件
rgpui::table::init(cx);       // 表格组件
```

通常至少需要 `theme::init` + `input_ui::init` + `menu::init`。

---

## 三、Root::new 签名变更

```rust
// 旧：
rgpui_component::Root::new(view, window, cx)

// 新（去掉了 window 参数）：
rgpui::Root::new(view, cx)
```

---

## 四、Select 组件（重大变更）

`rgpui_component::select::{Select, SelectState}` **未迁移到 rgpui 核心**，已被延后。

### 替代方案

#### 方案 A：Button + DropdownMenu（推荐，适合表单场景）

```rust
use rgpui::{Button, PopupMenuItem};

Button::new("my-select")
    .label(current_value.clone())
    .dropdown_menu(move |menu, _, _| {
        let mut menu = menu.label("选择选项");
        for (i, option) in options.iter().enumerate() {
            menu = menu.item(
                PopupMenuItem::new(option.clone())
                    .checked(i == selected_index)
                    .on_click(move |_, _, cx| {
                        // 处理选中
                    })
            );
        }
        menu
    })
```

#### 方案 B：箭头按钮（简单场景）

用左/右箭头 Button 循环切换选项，类似本项目 `settings.rs` 中的实现。

---

## 五、Markdown 渲染

旧版使用 `rgpui_component::text::TextView::markdown(id, source)`。

新版使用独立 crate `rgpui-markdown`：

```rust
// 旧：
rgpui_component::text::TextView::markdown("msg_1".into(), content)
    .selectable(true)

// 新：
rgpui_markdown::Markdown::new(content)
```

---

## 六、完整迁移示例

参考 `ru_pet` 项目的迁移实践：

### Cargo.toml

```toml
[dependencies]
rgpui = { git = "...", branch = "dev", features = ["tokio"] }
rgpui-3d = { git = "...", branch = "dev" }
rgpui-platform = { git = "...", branch = "dev" }
rgpui-markdown = { git = "...", branch = "dev" }
```

### main.rs 初始化

```rust
// 旧：
rgpui_component::init(cx);
rgpui_tokio::init(cx);

// 新：
rgpui::theme::init(cx);
rgpui::input_ui::init(cx);
rgpui::menu::init(cx);
rgpui::tokio::init(cx);
```

### 组件 import

```rust
// 旧：
use rgpui_component::button::{Button, ButtonVariants};
use rgpui_component::input::{Input, InputState};
use rgpui_component::switch::Switch;
use rgpui_component::Root;

// 新：
use rgpui::{Button, ButtonVariants, Switch, Root};
use rgpui::input_ui::{Input, InputState};
```

### WindowHandle 类型

```rust
// 旧：
WindowHandle<rgpui_component::Root>

// 新：
WindowHandle<rgpui::Root>
```

---

## 七、可用组件一览（rgpui 核心自带）

| 模块 | 组件 |
|------|------|
| `rgpui::*`（根） | Button, Checkbox, Radio, Switch, Slider, Spinner, Skeleton, Badge, Tag, Separator, Kbd, Tooltip, Icon, Label, Img |
| `rgpui::input_ui` | Input, InputState, NumberInput, PasswordInput, MaskedInput, TextArea |
| `rgpui::dialog` | Dialog, AlertDialog, DialogHeader, DialogContent, DialogFooter |
| `rgpui::menu` | PopupMenu, ContextMenu, DropdownMenu, MenuBar, MenuItem, HoverCard, Notification, Toast |
| `rgpui::list` | List, VirtualList, ListDelegate, ListState |
| `rgpui::table` | Table, DataTable, Column, TableState |
| `rgpui::tabs` | Tab, TabBar, Accordion, Collapsible |
| `rgpui::form` | Form, Field, FieldBuilder |
| `rgpui::title_bar` | TitleBar, WindowBorder |
| `rgpui::components` | animated_*, drag_drop, resizable, split_pane, sortable_list, spotlight, command_palette 等 |
| `rgpui::animation` | Spring, AnimationPreset, KeyframeAnimation |
| `rgpui::mouse_gestures` | GestureDetector, TapGesture, SwipeGesture, PanGesture |
| `rgpui_markdown` | Markdown（独立 crate） |

---

## 八、注意事项

1. **init 顺序**：`theme::init` 必须最先调用，否则组件渲染会 panic
2. **Tokio feature**：需要 Tokio 异步运行时时，Cargo.toml 中必须启用 `features = ["tokio"]`
3. **Root::new**：新签名去掉了 `window` 参数，只需 `Root::new(view, cx)`
4. **Input/InputState**：在 `rgpui::input_ui` 模块中，不在 crate 根
5. **Select**：未迁移，需自行用 Button + DropdownMenu 或其他方案替代
6. **Markdown**：使用独立 crate `rgpui-markdown`，API 为 `Markdown::new(source)`

---

## 九、ru_editor 迁移补充（较复杂差异）

以下差异在 `ru_pet` 等简单项目中未暴露，在 `ru_editor` 这类大型项目迁移时必须处理。

### 9.1 Tree 组件已并入核心

`rgpui-component::tree` 已移植进 rgpui 核心（见 `crates/rgpui/src/tree.rs`），`init` 已暴露为 `pub`：

```rust
rgpui::tree::init(cx);  // 在 main.rs 的 init 序列中调用
use rgpui::tree::{TreeItem, TreeState, TreeEvent};
```

注意点：
- `AppContext` 在 rgpui 中是 **trait**，`App` 是具体类型。回调闭包统一接收 `&mut App`。
- `TreeItem.state` 为 `Rc<RefCell<TreeItemState>>`，用 `RefCell::borrow_mut(&self.state)` 取可变引用（不要用 `.borrow_mut()`，会与 `Rc` 的 `BorrowMut` 冲突）。
- `Tree::render(cx: &mut App)`（RenderOnce）；`TreeState::render(cx: &mut Context<Self>)`（Render）。
- `Confirm` 位于 `rgpui::menu::{Confirm, ...}`（`menu::actions` 为私有，不要直接引用）。
- `ScrollableElement` 从 `rgpui` 根路径导出（`use rgpui::ScrollableElement;`），不在 `rgpui::scroll`。

### 9.2 通知系统（无全局 push_notification）

rgpui 的 `Root` 不再提供 `window.push_notification(...)`，也没有 `Root::render_notification_layer(...)`。
`NotificationList` 是视图组件，需自行持有并渲染：

```rust
use rgpui::menu::{Notification, NotificationList};
use rgpui::Global;

/// Entity<NotificationList> 未实现 Global，需用 newtype 包裹
pub struct NotificationListHandle(pub Entity<NotificationList>);
impl Global for NotificationListHandle {}

// App::new 中：
let notification_list = NotificationList::new(window, cx);
cx.set_global(NotificationListHandle(notification_list.clone()));

// 全局推送辅助函数（cx 为 &mut Context<App>）：
pub fn push_notification(
    window: &mut Window,
    notification: impl Into<Notification>,
    cx: &mut Context<App>,
) {
    let list = cx.global::<NotificationListHandle>().0.clone();
    list.update(cx, |list, cx| list.push(notification, window, cx));
}

// App::render 中手动挂载（绝对定位到右上角）：
.children({
    let list = cx.global::<NotificationListHandle>().0.clone();
    div().absolute().top_0().right_0().child(list)
})
```

原 `window.push_notification(notification, cx)` 调用统一替换为
`crate::core::app::push_notification(notification, window, cx)`。

> 注意：导入通知类型时用 `rgpui::menu::{Notification, NotificationList, NotificationType}`；
> `rgpui::notification` 是私有模块，直接引用会报 "module notification is private"。

### 9.3 Resizable 面板（API 变更最大）

- **错误写法**：`cx.new(|cx| ResizableState::new(cx))` 会双重包裹成 `Entity<Entity<ResizableState>>`。
  **正确写法**：`ResizableState::new` 本身返回 `Entity<ResizableState>`，直接调用即可：
  ```rust
  let state = ResizableState::new(std::borrow::BorrowMut::borrow_mut(cx));
  ```
  （`Context<App>` 实现了 `BorrowMut<App>`，不能用 `cx.borrow_mut()` 以免与 `RefCell::borrow_mut` 混淆；
  若 `use std::borrow::BorrowMut` 全局导入，会与 `RefCell` 的方法冲突，建议用完全限定 `std::borrow::BorrowMut::borrow_mut(cx)`。）
- `h_resizable` / `v_resizable` 现在需要 `(id, state: Entity<ResizableState>)` 两个参数，
  旧版的 `with_state(...)` 已移除，改为构造时传入：
  ```rust
  let layout = h_resizable("multi-panel", file_tree_panel_state.clone());
  ```
- 旧版 `ResizablePanelGroup::on_resize(cb)` 已移除。rgpui 改为 `ResizableState` 发出
  `ResizablePanelEvent`，通过 `cx.subscribe` 监听：
  ```rust
  cx.subscribe(&file_tree_panel_state, move |this: &mut App, _state, _event, cx| {
      // 注意闭包是 4 参数：(&mut App, Entity<T2>, &Evt, &mut Context<App>)
      if let Some(size) = _state.read(cx).sizes().first() { /* ... */ }
  }).detach();
  ```
- `resizable_panel()` 仍无需参数，链式 `child` / `size` / `size_range` 保持不变。

### 9.4 InputState 无 searchable

`InputState` 不再有 `.searchable(true/false)` 方法，直接删除对应链式调用即可。

### 9.5 Markdown 预览

`rgpui_component::text::TextView::markdown(id, src)` 替换为 `rgpui_markdown::Markdown::new(src)`。
`Markdown` 仅实现 `Styled` 与 `RenderOnce`，**没有** `.selectable()` / `.scrollable()`：

```rust
div()
    .size_full()
    .overflow_y_scrollbar()   // 用 scrollbar 而非 scrollable
    .p_2()
    .child(Markdown::new(content.clone()))
```

### 9.6 ThemeRegistry 无 watch_dir

旧版 `ThemeRegistry::watch_dir(dir, cx, cb)` 不存在。改为启动时直接调用加载逻辑：

```rust
apply_themes_from_registry(cx);
```

### 9.7 其它零散变更

- `TitleBar` 位于 `rgpui::title_bar::TitleBar`（不在 crate 根，旧 `use rgpui::TitleBar` 失效）。
- `gpui_component::set_locale(...)` 已删除，仅保留 `rust_i18n::set_locale(...)`。
- `Root::new(view, cx)`（2 参数）保持不变；`Root::render_dialog_layer(window, cx)` 仍存在可用。

### 9.8 Cargo.toml 本地依赖（ru_editor 当前用法）

ru_editor 直接引用本地 rgpui git 仓库（而非 crates.io）：

```toml
rgpui          = { git = "file:///C:/code/rgpui_workspace/rgpui", branch = "dev", features = ["tokio"] }
rgpui-platform = { git = "file:///C:/code/rgpui_workspace/rgpui", branch = "dev" }
rgpui-term     = { git = "file:///C:/code/rgpui_workspace/rgpui", branch = "dev" }
rgpui-markdown = { git = "file:///C:/code/rgpui_workspace/rgpui", branch = "dev" }
```

> 由于 rgpui 依赖的 `rayon`/`indexmap` 等版本可能与上游不同，若 Cargo.lock 解析冲突，
> 删除 `Cargo.lock` 后重新 `cargo generate-lockfile` / `cargo build` 即可。
