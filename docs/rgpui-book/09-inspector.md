# 元素检查器（Inspector）

> 调试用 UI 检查工具：拾取画布元素、查看源码位置与布局边界、浏览元素树。
> 仅 `debug_assertions` 或 feature `inspector` 下可用；关闭时零额外开销。

## 两行启用（开箱即用）

```rust
// 应用入口（二选一位置均可，窗口创建前调用）：
cx.enable_default_inspector(); // 注册默认面板 + Div 布局展示

// 开关面板（按钮点击 / F12 等）：
window.toggle_inspector(cx);
```

默认面板内容：状态徽标（空闲/拾取中/已选中）、拾取按钮（悬停蓝框、点击选中、
滚轮穿透重叠层级）、选中元素源码位置与实例号、祖先链树、完整树（逐节点折叠）、
`Div` 布局边界与内容尺寸。选中区域在画布上以橙框常驻高亮。

完整可运行示例见 `examples/inspector/`（`cargo run -p inspector`）。

## 自定义：两个接口的分工

| 接口 | 作用域 | 何时用 |
|------|--------|--------|
| `App::set_inspector_renderer` | 整板替换 | 面板整体风格/结构都要改时 |
| `App::register_inspector_element` | 按状态类型扩展 | 只想为某种元素状态加一块展示时（如自定义元素的布局信息） |

### recipe 1：整板替换（复用默认面板 + 自定义横幅）

```rust
use rgpui::{AnyElement, Context, Inspector, IntoElement, Window, div, px, rgb, v_flex};

fn custom_panel(
    inspector: &mut Inspector,
    window: &mut Window,
    cx: &mut Context<Inspector>,
) -> AnyElement {
    div()
        .id("my-inspector-panel")
        .size_full()
        .bg(rgb(0xf7f7f7))
        .child(
            v_flex()
                .size_full()
                .child(my_banner())
                .child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        // 默认面板自带滚动，包在 flex_1 + overflow_hidden 容器里铺满剩余空间。
                        .child(rgpui::default_inspector_panel(inspector, window, cx)),
                ),
        )
        .into_any_element()
}

// 注意：先 enable（注册 Div 展示），再替换渲染器，Div 布局展示不受影响。
cx.enable_default_inspector();
cx.set_inspector_renderer(Box::new(custom_panel));
```

`examples/inspector/` 即此 recipe 的 living 范例：默认面板 + 顶部自定义横幅。

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
