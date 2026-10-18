# 键映射（Keymap）

**目录：** [概述](#概述) · [快速入门](#快速入门) · [KeyBinding](#keybinding) · [Keymap](#keymap) · [上下文谓词](#上下文谓词) · [从 keymap.json 加载](#从-keymapjson-加载) · [快捷键录制](#快捷键录制) · [全局动作 helper](#全局动作-helper) · [最佳实践](#最佳实践)

## 概述

`keymap` 模块管理按键绑定与上下文匹配：把「按键序列 + 上下文谓词」映射到
`Action`，由分发管线在每帧按键时求解最匹配的绑定。

**导出：** 模块私有（`mod keymap;`，`rgpui.rs:133`），全部经 `pub use keymap::*`
（`rgpui.rs:493`）在 crate 根导出：`Keymap`、`KeymapVersion`、`BindingIndex`、
`KeyBinding`、`KeyBindingMetaIndex`、`KeyContext`、`KeyBindingContextPredicate`、
`load_keymap_json`。

> 操作（`actions!`/`#[derive(Action)]`）与 `on_action`/`key_context` 的用法见
> [action.md](action.md)；本文件聚焦键映射的数据结构与动态改键。

## 快速入门

```rust
use rgpui::{actions, KeyBinding};

actions!(editor, [MoveUp, MoveDown, Save]);

const CONTEXT: &str = "Editor";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", MoveUp, Some(CONTEXT)),
        KeyBinding::new("down", MoveDown, Some(CONTEXT)),
        KeyBinding::new("cmd-s", Save, Some(CONTEXT)),
    ]);
}
```

`App` 侧 API（`app.rs`）：

| 方法 | 位置 | 说明 |
|------|------|------|
| `bind_keys(impl IntoIterator<Item = KeyBinding>)` | `app.rs:2256` | 追加绑定（用户层在后，优先级更高） |
| `clear_key_bindings()` | `app.rs:2262` | 清空全部绑定 |
| `key_bindings() -> Rc<RefCell<Keymap>>` | `app.rs:2268` | 取底层 `Keymap` |
| `on_action::<A>(listener)` | `app.rs:2275` | 全局动作监听（冒泡末尾） |
| `on_global_action(action, keystroke, handler)` | `app.rs:2302` | 绑定 + 打活动窗口（见下） |

运行示例：`cargo run -p showcase --bin keymap`（`examples/showcase/src/bin/keymap.rs`）。

## KeyBinding

`crates/rgpui/src/keymap/binding.rs:10`。

| 方法 | 签名 | 位置 |
|------|------|------|
| `new` | `<A: Action>(keystrokes: &str, action: A, context: Option<&str>) -> Self` | `:33` |
| `load` | `(keystrokes, action: Box<dyn Action>, predicate, use_key_equivalents, action_input, mapper) -> Result<Self, InvalidKeystrokeError>` | `:48` |
| `with_meta` / `set_meta` | `(KeyBindingMetaIndex)` | `:78`/`:84` |
| `match_keystrokes` | `(&self, typed: &[impl AsKeystroke]) -> Option<bool>`（`Some(true)` 表还有更长匹配） | `:89` |
| `keystrokes` | `(&self) -> &[KeybindingKeystroke]` | `:104` |
| `action` | `(&self) -> &dyn Action` | `:109` |
| `predicate` | `(&self) -> Option<Rc<KeyBindingContextPredicate>>` | `:114` |
| `action_input` | `(&self) -> Option<SharedString>` | `:124` |

**`KeyBinding::new` 的 `context`** 是上下文谓词字符串（如 `"Editor"`、
`"editor && mode==full"`），`None` 表示全局绑定。按键串可含多键序列，用空格分隔：
`"space w w"`、`"ctrl-x 0"`。

**来源优先级** 由 `KeyBindingMetaIndex(pub u32)`（`:143`）携带：
`User(0) > Vim(1) > Base(2) > Default(3)`。同上下文深度下，来源数字小者优先；
同来源下后添加者优先。用户绑定在内置绑定后添加，故默认覆盖内置。

## Keymap

`crates/rgpui/src/keymap.rs:23`，持有全部绑定。

| 方法 | 签名 | 位置 |
|------|------|------|
| `new` | `(Vec<KeyBinding>) -> Self` | `:56` |
| `add_bindings` | `<T: IntoIterator<Item = KeyBinding>>(&mut self, T)` | `:68` |
| `clear` | `(&mut self)` | `:86` |
| `version` | `(&self) -> KeymapVersion` | `:63` |
| `bindings` | `(&self) -> impl DoubleEndedIterator<Item = &KeyBinding>` | `:94` |
| `bindings_for_action` | `(&self, action: &dyn Action) -> impl DoubleEndedIterator` | `:100` |
| `all_bindings_for_input` | `(&self, &[Keystroke]) -> Vec<KeyBinding>` | `:143` |
| `bindings_for_input` | `(&self, &[impl AsKeystroke], &[KeyContext]) -> (SmallVec<[KeyBinding; 1]>, bool)` | `:167` |
| `possible_next_bindings_for_input` | `(&self, &[Keystroke], &[KeyContext]) -> Vec<KeyBinding>` | `:257` |

- `bindings_for_input` 返回 `(匹配绑定, 是否可能匹配更长输入)`。绑定按优先级排序：
  **上下文更深者优先**（`Editor` 上的绑定优先于 `Pane`/`Workspace`），
  同深度时后添加者优先。
- `bindings_for_action` 用于**回显某动作当前按键**（改键 UI 常用），
  过滤掉被更窄 `NoAction`/`Unbind` 禁用的绑定。

窗口侧回显（`window.rs`）：

```rust
let keys: Vec<KeyBinding> = window.bindings_for_action(&rgpui::input_ui::Undo);
```

- `Window::bindings_for_action(&dyn Action) -> Vec<KeyBinding>`（`window.rs:6128`）
- `Window::bindings_for_action_in_context(...)`（`:6146`）
- `Window::bindings_for_action_in(...)`（`:6169`）

## 上下文谓词

`KeyContext`（`crates/rgpui/src/keymap/context.rs:8`）是按键分发时压入的上下文栈元素；
`KeyBindingContextPredicate`（`:169`）是绑定上的谓词。

```rust
let ctx = KeyContext::parse("Editor mode=full").unwrap();
let predicate = KeyBindingContextPredicate::parse("editor && mode==full").unwrap();
predicate.eval(&[ctx]);            // bool
predicate.depth_of(&contexts);     // Option<usize>，最深匹配深度
```

- `KeyContext::parse(source) -> Result<Self>`（`:63`）、`add(id)`（`:115`）、
  `set(key, value)`（`:124`）、`contains(key)`（`:135`）、`get(key)`（`:140`）。
- 谓词 `eval(&[KeyContext])`（`:260`）、`depth_of(&[KeyContext])`（`:248`）、
  `is_superset(&Self)`（`:315`）。
- 元素上用 `.key_context("Editor")` 设置上下文（见 action.md）。

**禁用绑定：** 用 `NoAction`（全局禁用）或 `Unbind("namespace::Action")`（定向解除）：

```rust
use rgpui::{NoAction, Unbind};
cx.bind_keys([
    KeyBinding::new("ctrl-w", NoAction {}, Some("Editor")),
    KeyBinding::new("tab", Unbind("editor::AcceptEditPrediction".into()), Some("Editor")),
]);
```

## 从 keymap.json 加载

`load_keymap_json(cx: &mut App, json: &str) -> anyhow::Result<Vec<KeyBinding>>`
（`keymap/file.rs:40`）。用 `serde_json_lenient` 解析，**注释与尾逗号可写**：

```json
[
  { "key": "ctrl-s", "action": "input::Undo", "context": "Editor" },
  { "key": "ctrl-k", "action": null }
]
```

- `key`：按键串（同 `KeyBinding::load`；多键序列空格分隔）。
- `action`：注册动作名（`App::build_action` 解析）；`null` = 禁用（`NoAction`，仅此写法）。
- `context`：可选谓词字符串，**原样透传不校验**（写错只是匹配不上）。
- 未知动作名直接 `Err`，不静默跳过；返回的 `Vec` 即用户层全集，调用方
  `cx.bind_keys(bindings)` 一次装入；文件里没列出的键继续走默认键映射。

```rust
match rgpui::load_keymap_json(cx, &text) {
    Ok(bindings) => cx.bind_keys(bindings),
    Err(e) => eprintln!("加载失败：{e}"),
}
```

## 快捷键录制

组件 `rgpui::components::{HotkeyInput, HotkeyInputState}` 供用户录入按键：

```rust
use rgpui::components::{HotkeyInput, HotkeyInputState};

let state = cx.new(HotkeyInputState::new);
HotkeyInput::new(state.clone())
    .placeholder("点这里，然后按键")
    .on_change(|hotkey, _, cx| { /* hotkey: Option<HotkeyValue> */ });
```

读取录制值并拼成按键串（`HotkeyValue` 存已解析的 key）：

```rust
state.read_with(cx, |state, _| state.hotkey().map(|hk| {
    let mut parts = Vec::new();
    if hk.modifiers.control { parts.push("ctrl".to_string()); }
    if hk.modifiers.alt { parts.push("alt".to_string()); }
    if hk.modifiers.shift { parts.push("shift".to_string()); }
    if hk.modifiers.platform { parts.push("secondary".to_string()); }
    parts.push(hk.key.clone());
    parts.join("-")
}))
```

再经 `KeyBinding::load` + `cx.bind_keys([binding])` 增量绑定。完整演示见
`examples/showcase/src/bin/keymap.rs`（含冲突检测：同键不同动作名即提示）；
`HotkeyValue::format_display()` 可用于回显展示。

## 全局动作 helper

`App::on_global_action`（`app.rs:2302`）适合「按快捷键对活动窗口做点事」
（如 F12 开关检查器）：

```rust
cx.on_global_action(ToggleInspector, Some("f12"), |window, cx| {
    window.toggle_inspector(cx);
});
```

- `keystroke: Some("f12")` 时追加一条**无上下文**全局绑定；`None` 则只监听不绑定。
- 只打活动窗口（`active_window`），无窗口时静默跳过。
- 分发处于动作处理中，同步 `update` 活动窗口必失败，故经 `spawn` 延后执行 `handler`。
- `handler: Fn(&mut Window, &mut App) + 'static`，可 `Rc` 捕获。

## 最佳实践

- **用户绑定后加载**：`bind_keys` 追加语义，用户键映射在内置之后添加，天然覆盖内置。
- **用上下文限定范围**：全局绑定越少越好，避免抢走编辑器/输入框的按键。
- **改键 UI 用 `bindings_for_action` 回显**，用 `Unbind` 做定向解除而非 `NoAction`。
- **JSON 是改键语义**：文件列出的即用户层全集；未知动作名请当作错误提示给用户。
