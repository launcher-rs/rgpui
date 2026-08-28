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
