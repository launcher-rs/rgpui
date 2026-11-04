# 焦点与键盘导航

**目录：** [概述](#概述) · [快速入门](#快速入门) · [焦点事件](#焦点事件) · [键盘导航](#键盘导航) · [常见模式](#常见模式) · [最佳实践](#最佳实践)

## 概述

rgpui 的焦点系统支持键盘导航和焦点管理。

**核心概念：**
- **FocusHandle**：可聚焦元素的引用
- **焦点跟踪**：当前聚焦的元素
- **键盘导航**：Tab/Shift-Tab 在元素间导航
- **焦点事件**：on_focus、on_blur

## 快速入门

### 创建焦点句柄

```rust
struct FocusableComponent {
    focus_handle: FocusHandle,
}

impl FocusableComponent {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            // 需要键盘可达的组件在创建句柄时就声明停靠点（见「键盘导航」）。
            focus_handle: cx.focus_handle().tab_stop(true),
        }
    }
}
```

### 使元素可聚焦

```rust
impl Render for FocusableComponent {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            // 句柄在创建时就已 `.tab_stop(true)`（见上一节），
            // 这里只负责跟踪焦点状态与样式。
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_enter))
            .child("可聚焦内容")
    }

    fn on_enter(&mut self, _: &Enter, _window: &mut Window, cx: &mut Context<Self>) {
        // 聚焦时处理 Enter 键
        cx.notify();
    }
}
```

### 焦点管理

```rust
impl MyComponent {
    fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus_handle.focus(window, cx);
    }

    fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    fn blur(&self, window: &mut Window) {
        window.blur();
    }
}
```

## 焦点事件

### 处理焦点变化

元素构建器上**没有** `.on_focus()` / `.on_blur()`；改用 `Context::on_focus` /
`Context::on_blur` 注册回调，并把返回的 `Subscription` 保存下来：

```rust
struct MyInput {
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl MyInput {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 输入框必须是 Tab 停靠点，否则键盘用户走不到它。
        let focus_handle = cx.focus_handle().tab_stop(true);
        let _subscriptions = vec![
            cx.on_focus(&focus_handle, window, Self::handle_focus),
            cx.on_blur(&focus_handle, window, Self::handle_blur),
        ];
        Self { focus_handle, _subscriptions }
    }

    fn handle_focus(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        // 处理获得焦点
        cx.notify();
    }

    fn handle_blur(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        // 处理失去焦点
        cx.notify();
    }
}

impl Render for MyInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_focused = self.focus_handle.is_focused(window);

        div()
            .track_focus(&self.focus_handle)
            .when(is_focused, |el| {
                el.bg(cx.theme().list_active)
            })
            .child(self.render_content())
    }
}
```

## 键盘导航

### Tab 停靠点：`track_focus()` 本身不够

`FocusHandle::new` / `Context::focus_handle()` 建的句柄默认 `tab_stop = false`
（`window.rs:514-527`），`track_focus()`（`div.rs:748`）只登记句柄、不改这个标志，
所以**光有 `track_focus()` 的元素按 Tab 是走不到的**。要成为键盘停靠点，在建句柄时
显式声明（`FocusHandle::tab_stop`，`window.rs:555`）：

```rust
// ✅ 好：一次声明，全程是停靠点
let focus_handle = cx.focus_handle().tab_stop(true);
```

组件里的真实写法与此一致，可作参照：`input/state.rs:482`、`table/state.rs:280`、
`split_pane.rs:92`、`otp_input.rs:590`、`hotkey_input.rs:355`；
`Button`/`Checkbox`/`Radio`/`Switch` 则把 `tab_stop(bool)` / `tab_index(isize)`
透传成自己的构建器（`button.rs:394`/`:402`、`checkbox.rs:92`/`:98`、
`radio.rs:90`/`:96`、`switch.rs:97`/`:103`）。

反过来，**容器**要「自己不被 Tab 停到、但 `focus_next` 能进到内部第一个停靠点」时，
用 div 上的 `.tab_stop(false)`（`div.rs:760`）——它作用在元素自建的句柄上
（`div.rs:2246-2261`），须与 `.tab_index(..)` 配合；`.tab_index()`（`div.rs:769`）
会顺带把 `tab_stop` 置真，`.tab_group()`（`div.rs:779`）让组内索引从 0 重新计数。

### Tab 顺序

按「句柄是停靠点」的元素以文档顺序参与 Tab 导航；负 `tab_index` 排在前面，
`0` 保持文档顺序，正数按索引升序（`window.rs:544`）。

```rust
div()
    .child(
        input1.track_focus(&focus1.tab_stop(true))  // Tab 顺序：1
    )
    .child(
        input2.track_focus(&focus2.tab_stop(true))  // Tab 顺序：2
    )
    .child(
        input3.track_focus(&focus3.tab_stop(true))  // Tab 顺序：3
    )
```

`FocusHandle` 的 `tab_stop`/`tab_index` 都按值消费自己，所以句柄字段要被再次命名时
先克隆：`&self.focus_handle.clone().tab_stop(true)`。

### 键盘激活与提示

- **Enter / 空格 = 点击**：聚焦元素收到 Enter 或空格时，框架合成一次
  `ClickEvent::Keyboard`（`elements/div.rs:3001-3017`）。因此可点击组件必须用
  `.on_click(..)` 注册行为；只用 `.on_mouse_down(..)` 的组件鼠标能点、键盘不能。
- **`.tooltip(..)` 随焦点展开**：div 内置提示（`elements/div.rs:1562`）除了鼠标悬停，
  现在也响应键盘聚焦——聚焦即在元素下沿锚点处显示提示，失焦立即收起
  （`show_or_hide_tooltip_on_focus`）。组件级 `Tooltip` / `TooltipOverlay`
  （`elements/tooltip.rs:24`/`:370`）仍只由悬停驱动，需要键盘可达的提示就挂 div 的 `.tooltip(..)`。
- **焦点可见**：`FocusableExt::focus_ring(is_focused, px(2.), window, cx)` 或
  `StyledExt::focused_border(cx)`；无可见焦点指示的停靠点等于把键盘用户丢在黑暗里。

### 容器内的焦点

```rust
impl Container {
    fn focus_first(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(first) = self.children.first() {
            first.update(cx, |child, cx| {
                child.focus_handle.focus(window, cx);
            });
        }
    }

    fn focus_next(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        // 自定义焦点导航逻辑
    }
}
```

## 常见模式

### 1. 挂载时自动聚焦

```rust
impl MyDialog {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        // 创建时聚焦
        focus_handle.focus(window, cx);

        Self { focus_handle }
    }
}
```

### 2. 焦点陷阱（模态框）

```rust
impl Modal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                if event.key == Key::Tab {
                    // 保持焦点在模态框内
                    this.focus_next_in_modal(cx);
                    cx.stop_propagation();
                }
            }))
            .child(self.render_content())
    }
}
```

### 3. 条件焦点

```rust
impl Searchable {
    fn activate_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_active = true;
        // 直接聚焦；不在 render 中改焦点
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle)
            .child(self.search_input())
    }
}
```

## 最佳实践

### ✅ 在交互元素上跟踪焦点

```rust
// ✅ 好：为键盘交互跟踪焦点
input()
    .track_focus(&self.focus_handle)
    .on_action(cx.listener(Self::on_enter))
```

### ✅ 提供视觉焦点指示器

```rust
let is_focused = self.focus_handle.is_focused(window);

div()
    .when(is_focused, |el| {
        // StyledExt::focused_border 用主题 ring 色绘制 1px 焦点边框
        el.focused_border(cx)
    })
```

### ❌ 不要：忘记跟踪焦点

```rust
// ❌ 坏：没有 track_focus，键盘导航与焦点样式都不起作用
div()
    .on_action(cx.listener(Self::on_enter))

// ❌ 坏：跟踪了焦点，但句柄没声明停靠点，Tab 依然走不到
div()
    .track_focus(&cx.focus_handle())
```
