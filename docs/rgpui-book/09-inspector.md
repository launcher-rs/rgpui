# 元素检查器（Inspector）

> 调试用 UI 检查工具：拾取画布元素、查看源码位置与布局边界、浏览元素树。
> 仅 `debug_assertions` 或 feature `inspector` 下可用；关闭时零额外开销。

## 两行启用（开箱即用）

```rust
// 应用入口（二选一位置均可，窗口创建前调用）：
cx.enable_default_inspector(); // 注册默认面板 + Div 布局展示

// 开关面板（F12 等快捷键；正式 UI 不放调试按钮，检查器只走快捷键）：
window.toggle_inspector(cx);
```

F12 这类全局开关不要用带上下文绑定 + 视图 `on_action`：
无焦点时分发路径只有 root，上下文匹配不上、冒泡也到不了视图 handler，
面板聚焦时同样到不了（面板是独立根）。正确接法是全局动作 helper
（全局绑定 + 打活动窗口 + spawn 延后更新三件套；分发中窗口已被 take，
同步 `update_window` 必失败，故监听内 spawn 延后）：

```rust
cx.on_global_action(ToggleInspector, Some("f12"), |window, cx| {
    window.toggle_inspector(cx);
});
```

`App::on_global_action(action, keystroke, handler)`：`keystroke` 为 `None`
则只监听不绑定；监听只打活动窗口（无窗口静默跳过）；`handler` 只要求
`'static`（允许 `Rc` 捕获，分发经本地 `spawn`，不需要 `Send + Sync`）。

## 发布剥离（推荐的上线姿势）

库侧检查器 API 全是 `#[cfg(any(feature = "inspector", debug_assertions))]`，
release 默认零代码。应用侧照抄三条：

1. Cargo 里**不要**开 `inspector` feature（dev 靠 `debug_assertions` 自动生效）；
2. 装配线（`enable_default_inspector` / `set_inspector_renderer` / F12 绑定）
   用同条件 `#[cfg]` 包起来；
3. 面板里不放“打开检查器”按钮，只留快捷键入口。

如此 `cargo run` 有完整检查器，`cargo run --release` 自动剥离。
release 想保留（如内部工具）：`--release --features inspector`
（示例 `inspector` / `inspector_custom` 即此布局，可直接验证三种模式）。

默认面板内容：状态徽标（空闲/拾取中/已选中）、拾取按钮（悬停蓝框、点击选中、
滚轮穿透重叠层级）、选中元素源码位置与实例号、完整树（逐节点折叠，点击行选中，
展开集只增不重置）、`Div` 布局边界与内容尺寸。选中区域在画布上以橙框常驻高亮。

完整可运行示例见 `examples/inspector/`（默认面板）与
`examples/inspector_custom/`（全自写面板：自有顶栏 + 选中卡 + 单层子节点，
并覆盖默认 Div 布局展示），两示例演示内容各自独立
（`cargo run -p inspector_custom`）。

## 多窗口与面板位置

- 检查器状态按窗口隔离：`toggle_inspector` / 拾取 / 选中 / 展开集都是
  `Window` 级别，多窗口互不干扰（`inspector` 与 `inspector_custom` 分属两进程，
  同进程多窗口同理）。
- 默认面板为右侧停靠：打开时画布视口让出 30rem，内容会重排，这是预期行为。
- 独立窗口面板暂不做：`Inspector` 实体、`inspector_hitboxes` 注册表、
  焦点恢复链都是按窗口挂载的，独立面板窗要跨窗口实体通信 + 焦点协同，
  代价高、收益仅是“画布不重排”，1.3.0 不纳入。

## 自定义：两个接口的分工

| 接口 | 作用域 | 何时用 |
|------|--------|--------|
| `App::set_inspector_renderer` | 整板替换 | 面板整体风格/结构都要改时 |
| `App::register_inspector_element` | 按状态类型扩展 | 只想为某种元素状态加一块展示时（如自定义元素的布局信息） |
| `App::set_inspector_panel_slots` | 顶栏/段落换皮 | 默认面板够用、只想换顶栏或信息卡外皮时（不必整板替换） |

### recipe 0：插槽换皮（顶栏/段落，不整板替换）

```rust
use rgpui::{InspectorPanelSlots, default_inspector_header, default_inspector_section};
use std::sync::Arc;

cx.enable_default_inspector();
cx.set_inspector_panel_slots(InspectorPanelSlots {
    // 包裹扩展：默认顶栏前加横幅；
    // 完全自写顶栏也行（签名见 InspectorHeaderSlot）。
    header: Some(Arc::new(|inspector, window, cx| {
        rgpui::v_flex()
            .child(rgpui::div().child("定制横幅"))
            .child(default_inspector_header(inspector, window, cx))
            .into_any_element()
    })),
    // 段落外皮：标题 + 正文；注册表状态展示不经过它，保持全自定义。
    section: Some(Arc::new(|title, body| {
        default_inspector_section(title, body)
    })),
});
```

不设插槽即默认外皮；`examples/inspector/` 的 `panel_slots_render_without_panic`
即此 recipe 的回归测试。

### recipe 1：整板替换（全自写面板）

`examples/inspector_custom/` 即此 recipe 的 living 范例：顶栏、选中卡、
子节点列表全部自写，不复用默认面板；另以 `register_inspector_element`
覆盖默认 Div 布局展示（同类型后注册覆盖先注册）。最小骨架：

```rust
// 自有顶栏（固定）+ 内容滚动区 + 选中卡 + 子节点列表，全自写；
// 行点击经 window.select_inspector_element 选中画布区域，
// Div 布局经 register_inspector_element 另行覆盖。
// 完整实现见 examples/inspector_custom/src/main.rs。
cx.enable_default_inspector(); // 先 enable，拿 Div 注册与全局装配
cx.set_inspector_renderer(Box::new(custom_panel));
cx.register_inspector_element(custom_div_state);
```

偷懒变体：整板只包一层横幅、内部复用 `rgpui::default_inspector_panel`
（注意默认面板根带 id，直接嵌套即可，无需处理滚动）。

### recipe 2：按状态类型扩展

元素侧以 `window.with_inspector_state::<MyState>(inspector_id, cx, |slot, _| { ... })`
上报状态（仅当前选中元素的上报会被保留），面板侧注册展示：

```rust
cx.register_inspector_element(
    |_id: rgpui::InspectorElementId,
     state: &MyState,
     _window: &mut rgpui::Window,
     _cx: &mut rgpui::App| {
        // 返回任意 IntoElement：该选中元素的专属展示卡片
        my_state_card(state)
    },
);
```

库默认已用此机制注册 `Div` 布局展示（见 `rgpui::render_div_inspector_state`，
自定义面板可直接复用该函数）。

## 树与选中 API（面板/工具代码用）

- `Inspector::select(id, window)`：选中指定元素（id 须来自当前帧树/注册表）。
- `Inspector::select_ancestor(levels_up, window)`：从当前选中上移 `levels_up` 层
  （`0` 保持，`1` 为父级），按全局路径前缀反查 + hitbox 包含消歧同路径多实例。
- `Window::select_inspector_ancestor(levels_up, cx)` /
  `Window::select_inspector_element(id, cx)`：面板 `on_click` 中的便捷入口。
- `Window::inspector_tree_roots()` / `inspector_tree_children(id)` /
  `inspector_tree_parent(id)`：完整树（prepaint 期记录，仅打开时保留）。
- `InspectorElementId::{short_label, source_label, tree_key}`：行标签与展开状态键。
