# rgpui-component 使用指南

**目录：** [设置](#设置) · [组件类型](#组件类型) · [常用组件](#常用组件)（Button、Input、Select、Checkbox、Icon、Dialog、Notification、Tabs、Tooltip、Form、List） · [主题](#主题) · [布局辅助](#布局辅助) · [覆盖层](#覆盖层对话框-sheets-通知) · [共享特征](#共享特征)

## 设置

### 1. Cargo.toml

```toml
[dependencies]
rgpui = { path = "../rgpui" }
rgpui_platform = { path = "../rgpui_platform", features = ["font-kit"] }
rgpui-component = { path = "../rgpui-component" }
rgpui-component-assets = { path = "../rgpui-component-assets" } # 可选图标
```

### 2. 初始化

```rust
fn main() {
    rgpui_platform::application()
        .with_assets(rgpui_component_assets::Assets)
        .run(move |cx| {
            rgpui_component::init(cx); // 必须首先调用

            cx.spawn(async move |cx| {
                cx.open_window(WindowOptions::default(), |window, cx| {
                    let view = cx.new(|_| MyApp);
                    cx.new(|cx| Root::new(view, window, cx)) // Root 包裹第一级视图
                }).expect("Failed to open window");
            }).detach();
        });
}
```

**`Root` 是必需的**，作为每个窗口的第一级子视图 — 它启用了对话框、sheets 和通知功能。

---

## 组件类型

### 无状态（大多数组件）

直接在 `render` 中使用，无需存储状态：

```rust
use rgpui_component::button::Button;

impl Render for MyView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Button::new("btn").primary().label("Submit")
            .on_click(|_, _, _| println!("clicked"))
    }
}
```

### 有状态（Input、Select、Combobox 等）

需要在视图中存储 `Entity<State>`：

```rust
use rgpui_component::input::{Input, InputState};

struct MyView {
    name: Entity<InputState>,
}

impl MyView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            name: cx.new(|cx| InputState::new(window, cx).placeholder("Your name")),
        }
    }
}

impl Render for MyView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        Input::new(&self.name)
    }
}
```

---

## 常用组件

### Button

```rust
use rgpui_component::button::{Button, ButtonGroup};

// 变体
Button::new("btn").label("Default")
Button::new("btn").primary().label("Primary")
Button::new("btn").danger().label("Delete")
Button::new("btn").warning().label("Warning")
Button::new("btn").success().label("Success")
Button::new("btn").ghost().label("Ghost")
Button::new("btn").link().label("Link")

// 状态
Button::new("btn").label("Text").disabled(true)
Button::new("btn").label("Text").loading(true)
Button::new("btn").label("Text").selected(true)

// 带图标
Button::new("btn").icon(IconName::Plus).label("Add")

// 尺寸
Button::new("btn").xsmall().label("XS")
Button::new("btn").small().label("S")
Button::new("btn").large().label("L")

// 按钮组
ButtonGroup::new("group")
    .child(Button::new("a").label("A"))
    .child(Button::new("b").label("B"))
    .on_click(|indices, _, _| { /* 选中的索引 */ })
```

### Input

```rust
use rgpui_component::input::{Input, InputState};

// 状态设置（在 new/init 中）
let input = cx.new(|cx| InputState::new(window, cx)
    .placeholder("Enter text...")
    .default_value("Hello")
);

// 渲染
Input::new(&input)
Input::new(&input).cleanable(true)           // 清除按钮
Input::new(&input).disabled(true)
Input::new(&input).prefix(Icon::new(IconName::Search).small())
Input::new(&input).suffix(Button::new("b").ghost().icon(IconName::X).xsmall())
Input::new(&input).mask_toggle()             // 密码显示切换
Input::new(&input).appearance(false)         // 移除默认边框/背景

// 读取值
let value = input.read(cx).value();

// 事件
cx.subscribe_in(&input, window, |view, state, event, window, cx| {
    match event {
        InputEvent::Change => { let v = state.read(cx).value(); }
        InputEvent::PressEnter { .. } => { /* 提交 */ }
        InputEvent::Focus | InputEvent::Blur => {}
    }
});
```

### Select

```rust
use rgpui_component::select::{Select, SelectState};

// 简单字符串列表
let state = cx.new(|cx| {
    SelectState::new(vec!["Apple", "Orange", "Banana"], Some(IndexPath::default()), window, cx)
});

// 渲染
Select::new(&state)
Select::new(&state).placeholder("Pick one")

// 读取选中项
let selected = state.read(cx).selected_item();
```

### Checkbox / Switch / Radio

```rust
use rgpui_component::{Checkbox, Switch};

// 无状态（受控）
Checkbox::new("cb").checked(self.checked)
    .on_click(|checked, _, cx| { /* &bool */ })

Switch::new("sw").checked(self.enabled)
    .on_click(|checked, _, cx| {})
```

### Icon

```rust
use rgpui_component::{Icon, IconName};

Icon::new(IconName::Check)
Icon::new(IconName::Search).small()
Icon::new(IconName::Plus).large().text_color(cx.theme().primary)
```

### Dialog

```rust
use rgpui_component::dialog::Dialog;

// 从窗口上下文打开
window.open_modal(cx, |modal, _, cx| {
    modal
        .title("Confirm")
        .child(div().child("Are you sure?"))
        .footer(|this, _, cx| {
            this.child(Button::new("cancel").label("Cancel"))
                .child(Button::new("ok").primary().label("OK")
                    .on_click(|_, window, cx| { window.close_modal(cx); }))
        })
});
```

### Notification

```rust
// 简单字符串消息
window.push_notification("Saved successfully!", cx);

// 带类型变体
window.push_notification(
    Notification::new("Upload complete").info().message("File uploaded"),
    cx,
);
```

### Tabs

```rust
use rgpui_component::tab::{Tab, TabBar};

TabBar::new("tabs")
    .child(Tab::new("tab1").child("Overview"))
    .child(Tab::new("tab2").child("Settings"))
    .child(Tab::new("tab3").child("Logs"))
```

### Tooltip

```rust
// 在任何带 .id() 的元素上添加 .tooltip()：
div()
    .id("my-btn")
    .tooltip(|window, cx| Tooltip::new("Delete item").build(window, cx))
    .child("Delete")

// 或直接在 Button 上：
Button::new("btn").icon(IconName::Trash).tooltip("Delete")
```

### Form

```rust
use rgpui_component::form::{v_form, h_form, field};

// 垂直表单
v_form()
    .child(field().label("Name").child(Input::new(&self.name)))
    .child(field().label("Email").child(Input::new(&self.email)))
    .child(Button::new("submit").primary().label("Submit"))

// 水平标签对齐
h_form()
    .child(field().label("Username").child(Input::new(&self.username)))
```

### List（可搜索、虚拟化）

```rust
use rgpui_component::list::{List, ListState, ListDelegate, ListItem, ListEvent};

// 为数据类型实现 ListDelegate，然后：
let list_state = cx.new(|cx| ListState::new(MyDelegate::new(), window, cx));

// 渲染
List::new(&list_state)
// 事件
cx.subscribe(&list_state, |this, _, event, cx| {
    if let ListEvent::Select(index_path) = event {
        // 处理选中
    }
});
```

---

## 主题

```rust
use rgpui_component::ActiveTheme as _;

// 访问颜色
cx.theme().primary
cx.theme().background
cx.theme().foreground
cx.theme().border
cx.theme().surface
cx.theme().muted
cx.theme().destructive

// 在样式中使用
div()
    .bg(cx.theme().surface)
    .text_color(cx.theme().foreground)
    .border_color(cx.theme().border)
```

### 切换主题

```rust
use rgpui_component::Theme;

// 切换亮/暗模式
cx.update_global::<Theme, _>(|theme, cx| {
    theme.toggle_mode(cx);
});

// 加载命名主题
Theme::global_mut(cx).apply_config(&theme_config);
```

---

## 布局辅助

rgpui-component 为 GPUI 扩展了便捷的布局方法：

```rust
h_flex()    // div().flex().flex_row().items_center()
v_flex()    // div().flex().flex_col()

// 常见模式
h_flex().gap_2().items_center()
    .child(Icon::new(IconName::User))
    .child(label("Username"))

v_flex().gap_4().p_4()
    .child(Input::new(&self.name))
    .child(Input::new(&self.email))
    .child(Button::new("submit").primary().label("Submit"))
```

---

## 覆盖层（对话框、Sheets、通知）

要渲染覆盖层，请在第一级视图的 render 中添加以下内容：

```rust
impl Render for MyApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(self.main_content(window, cx))
            .children(Root::render_dialog_layer(cx))
            .children(Root::render_sheet_layer(cx))
            .children(Root::render_notification_layer(cx))
    }
}
```

---

## 共享特征

所有组件遵循构建器模式 `Component::new("id").method().method()`：
- `Sizable`：`.xsmall()` / `.small()` / `.medium()`（默认）/ `.large()`
- `Disableable`：`.disabled(bool)`
- `Selectable`：`.selected(bool)`
- `Styled`：任何 GPUI 样式方法（`.w()`、`.bg()`、`.p_2()` 等）

对于此处未涵盖的任何组件，请从以下地址获取其文档：
`https://longbridge.github.io/gpui-component/docs/components/{name}.md`
