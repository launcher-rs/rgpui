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
            focus_handle: cx.focus_handle(),
        }
    }
}
```

### 使元素可聚焦

```rust
impl Render for FocusableComponent {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
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
        let focus_handle = cx.focus_handle();
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

### Tab 顺序

带有 `track_focus()` 的元素自动参与 Tab 导航。

```rust
div()
    .child(
        input1.track_focus(&focus1)  // Tab 顺序：1
    )
    .child(
        input2.track_focus(&focus2)  // Tab 顺序：2
    )
    .child(
        input3.track_focus(&focus3)  // Tab 顺序：3
    )
```

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
// ❌ 坏：没有 track_focus，键盘导航将不起作用
div()
    .on_action(cx.listener(Self::on_enter))
```
