# 快捷键系统

> Action、Keymap 与键盘事件分发。

## 概述

rgpui 的快捷键系统由三个核心概念组成：
1. **Action** - 用户交互动作的抽象
2. **Keymap** - 按键与 Action 的绑定关系
3. **KeyDispatch** - 键盘事件的分发管线

## 定义 Action

### 简单 Action

使用 `actions!` 宏快速定义单元结构体 Action：

```rust
use rgpui::actions;

// 定义编辑器相关的 Action
actions!(editor, [
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Newline,
    Backspace,
    Delete,
]);

// 定义全局 Action（无命名空间）
actions!([Copy, Cut, Paste]);
```

### 带数据的 Action

使用 `#[derive(Action)]` 宏定义带参数的 Action：

```rust
use rgpui::Action;

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
#[action(namespace = editor)]
pub struct SelectNext {
    pub replace_newest: bool,
}

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
#[action(namespace = editor)]
pub struct InsertText {
    pub text: SharedString,
}
```

### Action 属性

`#[action(...)]` 支持以下参数：

| 参数 | 说明 |
|------|------|
| `namespace = "name"` | 设置命名空间 |
| `name = "CustomName"` | 覆盖 Action 名称 |
| `no_json` | 禁用 JSON 序列化 |
| `no_register` | 跳过注册 |
| `deprecated_aliases = [...]` | 弃用旧名称 |
| `deprecated = "msg"` | 弃用消息 |

## 注册 Action

### 在 App 中注册

```rust
App::new()
    .register_action::<editor::MoveUp>()
    .register_action::<editor::MoveDown>()
    .run(|cx| {
        // ...
    });
```

### 在渲染中注册监听器

```rust
impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("Editor")  // 设置键盘上下文
            .on_action(cx.listener(Editor::on_move_up))
            .on_action(cx.listener(Editor::on_move_down))
            .on_action(cx.listener(Editor::on_insert_text))
    }
}
```

### 实现 Action 处理器

```rust
impl Editor {
    fn on_move_up(&mut self, _: &editor::MoveUp, _window: &mut Window, cx: &mut Context<Self>) {
        self.move_cursor_up();
        cx.notify();
    }

    fn on_move_down(&mut self, _: &editor::MoveDown, _window: &mut Window, cx: &mut Context<Self>) {
        self.move_cursor_down();
        cx.notify();
    }

    fn on_insert_text(&mut self, action: &editor::InsertText, _window: &mut Window, cx: &mut Context<Self>) {
        self.insert_text(&action.text);
        cx.notify();
    }
}
```

## 绑定快捷键

### 基本绑定

```rust
use rgpui::KeyBinding;

cx.bind_keys([
    KeyBinding::new("up", editor::MoveUp, Some("Editor")),
    KeyBinding::new("down", editor::MoveDown, Some("Editor")),
    KeyBinding::new("cmd-z", editor::Undo, Some("Editor")),
    KeyBinding::new("cmd-shift-z", editor::Redo, Some("Editor")),
]);
```

### 按键序列

rgpui 支持按键序列（多键组合）：

```rust
// "cmd-k left" 表示先按 Cmd+K，再按 Left
KeyBinding::new("cmd-k left", pane::SplitLeft, Some("Pane"));
```

### 上下文限定

快捷键可以限定在特定上下文中生效：

```rust
// 仅在 "Editor" 上下文中生效
KeyBinding::new("cmd-a", editor::SelectAll, Some("Editor"));

// 在所有上下文中生效
KeyBinding::new("cmd-c", clipboard::Copy, None);
```

### 从 JSON 加载

快捷键映射也可以从 JSON 文件加载：

```json
{
  "bindings": {
    "cmd-z": "editor::Undo",
    "cmd-shift-z": "editor::Redo",
    "up": ["editor::MoveUp", { "line": true }],
    "cmd-k left": "pane::SplitLeft"
  }
}
```

## 键盘上下文（KeyContext）

键盘上下文用于限定快捷键的作用域：

```rust
impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("Editor")  // 设置上下文名称
            .on_action(cx.listener(Editor::on_action))
    }
}
```

上下文可以嵌套，子上下文的 Action 优先级高于父上下文。

## 事件分发管线

### 分发树（DispatchTree）

每一帧，GPUI 会根据 UI 树构建一棵分发树。每个节点对应一个可聚焦的 UI 元素。

### 分发路径

按键事件从焦点节点开始，沿着树向上冒泡，直到找到匹配的绑定：

```
焦点节点 (Editor)
    ↓
父节点 (Pane)
    ↓
根节点 (Workspace)
```

### 匹配优先级

当多个绑定匹配同一个按键时，最深层的上下文优先：

```
Editor 的 "cmd-z" > Pane 的 "cmd-z" > Workspace 的 "cmd-z"
```

## 修饰键变化

监听修饰键（Ctrl、Shift、Alt、Cmd）的变化：

```rust
impl Render for MyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .on_modifiers_changed(cx.listener(Self::on_modifiers_changed))
    }
}

impl MyView {
    fn on_modifiers_changed(&mut self, event: &ModifiersChangedEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if event.modifiers.control {
            // Ctrl 键被按下
        }
    }
}
```

## 最佳实践

1. **使用命名空间**：为 Action 添加命名空间避免冲突
2. **上下文分离**：为不同组件设置不同的键盘上下文
3. **文档化 Action**：为 Action 添加文档注释，自动生成快捷键帮助
4. **测试覆盖**：测试 Action 的触发和处理逻辑
