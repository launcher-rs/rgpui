# Element 最佳实践

**目录：** [状态管理](#状态管理) · [性能考虑](#性能考虑) · [交互处理](#交互处理) · [布局策略](#布局策略) · [错误处理](#错误处理) · [测试 Element 实现](#测试-element-实现) · [常见陷阱](#常见陷阱) · [性能检查清单](#性能检查清单)

## 状态管理

### 有效使用关联类型

**好：** 使用关联类型在阶段之间传递有意义的数据

```rust
// 好：带类型安全的结构化状态
type RequestLayoutState = (StyledText, Vec<ChildLayout>);
type PrepaintState = (Hitbox, Vec<ChildBounds>);
```

**坏：** 需要数据时使用空状态

```rust
// 坏：需要传递数据时没有状态
type RequestLayoutState = ();
type PrepaintState = ();
// 现在无法将布局信息传递到 paint 阶段！
```

### 管理复杂状态

对于具有复杂状态的元素，创建专用结构体：

```rust
// 好：复杂状态的专用结构体
pub struct TextElementState {
    pub styled_text: StyledText,
    pub text_layout: TextLayout,
    pub child_states: Vec<ChildState>,
}

type RequestLayoutState = TextElementState;
```

**好处：**
- 状态结构的清晰文档
- 易于扩展
- 类型安全的访问

### 状态生命周期

**黄金法则：** 状态通过阶段单向流动

```
request_layout → RequestLayoutState →
prepaint → PrepaintState →
paint
```

**不要：**
- 在元素结构体中存储应该在关联类型中的状态
- 尝试在 paint 阶段修改元素状态（使用 `cx.notify()` 调度重新渲染）
- 跨阶段边界传递可变引用

## 性能考虑

### 最小化 Paint 阶段的分配

**关键：** Paint 阶段在动画期间每帧调用。最小化分配。

**好：** 在 `request_layout` 或 `prepaint` 中预分配

```rust
impl Element for MyElement {
    fn request_layout(&mut self, .., window: &mut Window, cx: &mut App)
        -> (LayoutId, Vec<StyledText>)
    {
        // 在布局期间分配一次
        let styled_texts = self.children
            .iter()
            .map(|child| StyledText::new(child.text.clone()))
            .collect();

        (layout_id, styled_texts)
    }

    fn paint(&mut self, .., styled_texts: &mut Vec<StyledText>, ..) {
        // 只是使用预分配的 styled_texts
        for text in styled_texts {
            text.paint(..);
        }
    }
}
```

**坏：** 在 `paint` 阶段分配

```rust
fn paint(&mut self, ..) {
    // 坏：在 paint 阶段分配！
    let styled_texts: Vec<_> = self.children
        .iter()
        .map(|child| StyledText::new(child.text.clone()))
        .collect();
}
```

### 缓存昂贵的计算

对昂贵的操作使用记忆化：

```rust
pub struct CachedElement {
    // 缓存键
    last_text: Option<SharedString>,
    last_width: Option<Pixels>,

    // 缓存结果
    cached_layout: Option<TextLayout>,
}

impl Element for CachedElement {
    fn request_layout(&mut self, .., window: &mut Window, cx: &mut App)
        -> (LayoutId, TextLayout)
    {
        let current_width = window.bounds().width();

        // 检查缓存是否有效
        if self.last_text.as_ref() != Some(&self.text)
            || self.last_width != Some(current_width)
            || self.cached_layout.is_none()
        {
            // 重新计算昂贵的布局
            self.cached_layout = Some(self.compute_text_layout(current_width));
            self.last_text = Some(self.text.clone());
            self.last_width = Some(current_width);
        }

        // 使用缓存的布局
        let layout = self.cached_layout.as_ref().unwrap();
        (layout_id, layout.clone())
    }
}
```

### 延迟子元素渲染

在可滚动容器中只渲染可见的子元素：

```rust
fn paint(&mut self, .., bounds: Bounds<Pixels>, paint_state: &mut Self::PrepaintState, ..) {
    for (i, child) in self.children.iter_mut().enumerate() {
        let child_bounds = paint_state.child_bounds[i];

        // 只绘制可见的子元素
        if self.is_visible(&child_bounds, &bounds) {
            child.paint(..);
        }
    }
}

fn is_visible(&self, child_bounds: &Bounds<Pixels>, container_bounds: &Bounds<Pixels>) -> bool {
    child_bounds.bottom() >= container_bounds.top() &&
    child_bounds.top() <= container_bounds.bottom()
}
```

## 交互处理

### 正确的事件冒泡

始终在处理事件之前检查阶段和边界：

```rust
fn paint(&mut self, .., window: &mut Window, cx: &mut App) {
    window.on_mouse_event({
        let hitbox = self.hitbox.clone();
        move |event: &MouseDownEvent, phase, window, cx| {
            // 首先检查阶段
            if !phase.bubble() {
                return;
            }

            // 检查事件是否在边界内
            if !hitbox.is_hovered(window) {
                return;
            }

            // 处理事件
            self.handle_click(event);

            // 如果处理了则停止传播
            cx.stop_propagation();
        }
    });
}
```

**不要忘记：**
- 根据情况检查 `phase.bubble()` 或 `phase.capture()`
- 检查 hitbox 悬停状态或边界
- 如果你处理了事件则调用 `cx.stop_propagation()`

### Hitbox 管理

在 `prepaint` 阶段创建 hitbox，而不是 `paint`：

**好：**

```rust
fn prepaint(&mut self, .., bounds: Bounds<Pixels>, window: &mut Window, ..) -> Hitbox {
    // 在 prepaint 中创建 hitbox
    window.insert_hitbox(bounds, HitboxBehavior::Normal)
}

fn paint(&mut self, .., hitbox: &mut Hitbox, window: &mut Window, ..) {
    // 在 paint 中使用 hitbox
    window.set_cursor_style(CursorStyle::PointingHand, hitbox);
}
```

### 光标样式指南

为交互性线索设置适当的光标样式：

```rust
// 文本选择
window.set_cursor_style(CursorStyle::IBeam, &hitbox);

// 可点击元素（桌面约定：使用 default，而不是 pointing hand）
window.set_cursor_style(CursorStyle::Arrow, &hitbox);

// 链接（Web 约定：使用 pointing hand）
window.set_cursor_style(CursorStyle::PointingHand, &hitbox);

// 可调整大小的边缘
window.set_cursor_style(CursorStyle::ResizeLeftRight, &hitbox);
```

**桌面与 Web 约定：**
- 桌面应用：按钮使用 `Arrow`
- Web 应用：仅链接使用 `PointingHand`

## 布局策略

### 固定大小元素

对于已知、不变大小的元素：

```rust
fn request_layout(&mut self, .., window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
    let layout_id = window.request_layout(
        Style {
            size: size(px(200.), px(100.)),
            ..default()
        },
        vec![], // 没有子元素
        cx
    );
    (layout_id, ())
}
```

### 基于内容的大小

对于由内容决定大小的元素：

```rust
fn request_layout(&mut self, .., window: &mut Window, cx: &mut App)
    -> (LayoutId, Size<Pixels>)
{
    // 测量内容
    let text_bounds = self.measure_text(window);
    let padding = px(16.);

    let layout_id = window.request_layout(
        Style {
            size: size(
                text_bounds.width() + padding * 2.,
                text_bounds.height() + padding * 2.,
            ),
            ..default()
        },
        vec![],
        cx
    );

    (layout_id, text_bounds)
}
```

### 灵活布局

对于适应可用空间的元素：

```rust
fn request_layout(&mut self, .., window: &mut Window, cx: &mut App)
    -> (LayoutId, Vec<LayoutId>)
{
    let mut child_layout_ids = Vec::new();

    for child in &mut self.children {
        let (layout_id, _) = child.request_layout(window, cx);
        child_layout_ids.push(layout_id);
    }

    let layout_id = window.request_layout(
        Style {
            flex_direction: FlexDirection::Row,
            gap: px(8.),
            size: Size {
                width: relative(1.0),  // 填充父级宽度
                height: auto(),        // 自动高度
            },
            ..default()
        },
        child_layout_ids.clone(),
        cx
    );

    (layout_id, child_layout_ids)
}
```

## 错误处理

### 优雅降级

优雅地处理错误，不要 panic：

```rust
fn request_layout(&mut self, .., window: &mut Window, cx: &mut App)
    -> (LayoutId, Option<TextLayout>)
{
    // 尝试创建样式化文本
    match StyledText::new(self.text.clone()).request_layout(None, None, window, cx) {
        Ok((layout_id, text_layout)) => {
            (layout_id, Some(text_layout))
        }
        Err(e) => {
            // 记录错误
            eprintln!("Failed to layout text: {}", e);

            // 回退到简单文本
            let fallback_text = StyledText::new("(Error loading text)".into());
            let (layout_id, _) = fallback_text.request_layout(None, None, window, cx);
            (layout_id, None)
        }
    }
}
```

## 常见陷阱

### ❌ 在元素结构体中存储布局状态

**坏：**

```rust
pub struct MyElement {
    id: ElementId,
    // 坏：这应该在 RequestLayoutState 中
    cached_layout: Option<TextLayout>,
}
```

**好：**

```rust
pub struct MyElement {
    id: ElementId,
    text: SharedString,
}

type RequestLayoutState = TextLayout; // 好：状态在关联类型中
```

### ❌ 在 Paint 阶段修改元素

**坏：**

```rust
fn paint(&mut self, ..) {
    self.counter += 1; // 坏：在 paint 中修改元素
}
```

**好：**

```rust
fn paint(&mut self, .., window: &mut Window, cx: &mut App) {
    window.on_mouse_event(move |event, phase, window, cx| {
        if phase.bubble() {
            self.counter += 1;
            cx.notify(); // 调度重新渲染
        }
    });
}
```

### ❌ 在 Paint 阶段创建 Hitbox

**坏：**

```rust
fn paint(&mut self, .., bounds: Bounds<Pixels>, window: &mut Window, ..) {
    // 坏：在 paint 中创建 hitbox
    let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
}
```

**好：**

```rust
fn prepaint(&mut self, .., bounds: Bounds<Pixels>, window: &mut Window, ..) -> Hitbox {
    // 好：在 prepaint 中创建 hitbox
    window.insert_hitbox(bounds, HitboxBehavior::Normal)
}
```

### ❌ 忽略事件阶段

**坏：**

```rust
window.on_mouse_event(move |event, phase, window, cx| {
    // 坏：不检查阶段
    self.handle_click(event);
});
```

**好：**

```rust
window.on_mouse_event(move |event, phase, window, cx| {
    // 好：检查阶段
    if !phase.bubble() {
        return;
    }
    self.handle_click(event);
});
```

## 性能检查清单

在发布元素实现之前，验证：

- [ ] `paint` 阶段没有分配（事件处理程序除外）
- [ ] 昂贵的计算已缓存/记忆化
- [ ] 可滚动容器中只渲染可见的子元素
- [ ] Hitbox 在 `prepaint` 中创建，而不是 `paint`
- [ ] 事件处理程序检查阶段和边界
- [ ] 布局状态通过关联类型传递，而不是存储在元素中
- [ ] 元素实现了带后备的正确错误处理
- [ ] 测试覆盖布局计算和交互
