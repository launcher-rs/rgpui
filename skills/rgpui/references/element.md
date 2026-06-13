
## 何时使用

在以下情况下使用底层 `Element` trait：
- 需要对布局计算进行细粒度控制
- 构建复杂、性能关键的组件
- 实现自定义布局算法（瀑布流、圆形等）
- 高层 `Render`/`RenderOnce` API 不够用时

**对于以下情况优先使用 `Render`/`RenderOnce`：** 简单组件、标准布局、声明式 UI

## 快速入门

`Element` trait 提供对三个渲染阶段的直接控制：

```rust
impl Element for MyElement {
    type RequestLayoutState = MyLayoutState;  // 传递到后续阶段的数据
    type PrepaintState = MyPaintState;        // 绘制数据

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    // 阶段 1：计算大小和位置
    fn request_layout(&mut self, .., window: &mut Window, cx: &mut App)
        -> (LayoutId, Self::RequestLayoutState)
    {
        let layout_id = window.request_layout(
            Style { size: size(px(200.), px(100.)), ..default() },
            vec![],
            cx
        );
        (layout_id, MyLayoutState { /* ... */ })
    }

    // 阶段 2：创建 hitbox，为绘制做准备
    fn prepaint(&mut self, .., bounds: Bounds<Pixels>, layout: &mut Self::RequestLayoutState,
                window: &mut Window, cx: &mut App) -> Self::PrepaintState
    {
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        MyPaintState { hitbox }
    }

    // 阶段 3：渲染并处理交互
    fn paint(&mut self, .., bounds: Bounds<Pixels>, layout: &mut Self::RequestLayoutState,
             paint_state: &mut Self::PrepaintState, window: &mut Window, cx: &mut App)
    {
        window.paint_quad(paint_quad(bounds, Anchor::all(px(4.)), cx.theme().background));

        window.on_mouse_event({
            let hitbox = paint_state.hitbox.clone();
            move |event: &MouseDownEvent, phase, window, cx| {
                if hitbox.is_hovered(window) && phase.bubble() {
                    // 处理交互
                    cx.stop_propagation();
                }
            }
        });
    }
}

// 允许元素用作子元素
impl IntoElement for MyElement {
    type Element = Self;
    fn into_element(self) -> Self::Element { self }
}
```

## 核心概念

### 三阶段渲染

1. **request_layout**：计算大小和位置，返回布局 ID 和状态
2. **prepaint**：创建 hitbox，计算最终边界，为绘制做准备
3. **paint**：渲染元素，设置交互（鼠标事件、光标样式）

### 状态流

```
RequestLayoutState → PrepaintState → paint
```

状态通过关联类型单向流动，在阶段之间作为可变引用传递。

### 关键操作

- **布局**：`window.request_layout(style, children, cx)` - 创建布局节点
- **Hitbox**：`window.insert_hitbox(bounds, behavior)` - 创建交互区域
- **绘制**：`window.paint_quad(...)` - 渲染视觉内容
- **事件**：`window.on_mouse_event(handler)` - 处理用户输入

## 参考文档

### 完整 API 文档
- **API**：参见 [element-api.md](element-api.md) — 关联类型、hitbox 系统、事件处理、光标样式
- **示例**：参见 [element-examples.md](element-examples.md) — 文本、交互、复杂元素
- **模式**：参见 [element-patterns.md](element-patterns.md) — 文本、容器、交互、组合、可滚动
- **最佳实践**：参见 [element-best-practices.md](element-best-practices.md) — 性能、状态、常见陷阱
- **高级**：参见 [element-advanced.md](element-advanced.md) — 瀑布流/圆形布局、记忆化、虚拟列表
