# Element API 参考

**目录：** [Element Trait 结构](#element-trait-结构) · [关联类型](#关联类型) · [方法](#方法) · [IntoElement 集成](#intoelement-集成) · [布局系统集成](#布局系统集成) · [Hitbox 系统](#hitbox-系统) · [事件处理](#事件处理) · [光标样式](#光标样式)

## Element Trait 结构

`Element` trait 需要实现三个关联类型和五个方法：

```rust
pub trait Element: 'static + IntoElement {
    type RequestLayoutState: 'static;
    type PrepaintState: 'static;

    fn id(&self) -> Option<ElementId>;
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>>;
    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState);
    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState;
    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    );
}
```

## 关联类型

### RequestLayoutState

从 `request_layout` 传递到 `prepaint` 和 `paint` 阶段的数据。

**用途：**
- 存储布局计算（样式化文本、子布局 ID）
- 缓存昂贵的计算
- 在阶段之间传递子状态

**示例：**
```rust
// 简单：不需要状态
type RequestLayoutState = ();

// 单个值
type RequestLayoutState = StyledText;

// 多个值
type RequestLayoutState = (StyledText, Vec<ChildLayout>);

// 复杂结构体
pub struct MyLayoutState {
    pub styled_text: StyledText,
    pub child_layouts: Vec<(LayoutId, ChildState)>,
    pub computed_bounds: Bounds<Pixels>,
}
type RequestLayoutState = MyLayoutState;
```

### PrepaintState

从 `prepaint` 传递到 `paint` 阶段的数据。

**用途：**
- 存储用于交互的 hitbox
- 缓存视觉边界
- 存储 prepaint 结果

**示例：**
```rust
// 简单：只是一个 hitbox
type PrepaintState = Hitbox;

// 可选 hitbox
type PrepaintState = Option<Hitbox>;

// 多个值
type PrepaintState = (Hitbox, Vec<Bounds<Pixels>>);

// 复杂结构体
pub struct MyPaintState {
    pub hitbox: Hitbox,
    pub child_bounds: Vec<Bounds<Pixels>>,
    pub visible_range: Range<usize>,
}
type PrepaintState = MyPaintState;
```

## 方法

### id()

返回可选的唯一标识符，用于调试和检查。

```rust
fn id(&self) -> Option<ElementId> {
    Some(self.id.clone())
}

// 或者如果不需要 ID
fn id(&self) -> Option<ElementId> {
    None
}
```

### source_location()

返回用于调试的源位置。通常返回 `None`，除非需要调试。

```rust
fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
    None
}
```

### request_layout()

计算元素树的大小和位置。

**参数：**
- `global_id`：全局元素标识符（可选）
- `inspector_id`：检查器元素标识符（可选）
- `window`：可变窗口引用
- `cx`：可变应用上下文

**返回：**
- `(LayoutId, Self::RequestLayoutState)`：布局 ID 和下一阶段的状态

**职责：**
1. 通过调用 `child.request_layout()` 计算子布局
2. 使用 `window.request_layout()` 创建自己的布局
3. 返回布局 ID 和状态传递到下一阶段

### prepaint()

通过创建 hitbox 和计算最终边界来准备绘制。

**参数：**
- `global_id`：全局元素标识符（可选）
- `inspector_id`：检查器元素标识符（可选）
- `bounds`：布局引擎计算的最终边界
- `request_layout`：布局状态的可变引用
- `window`：可变窗口引用
- `cx`：可变应用上下文

**返回：**
- `Self::PrepaintState`：paint 阶段的状态

**职责：**
1. 根据布局边界计算最终子边界
2. 为所有子元素调用 `child.prepaint()`
3. 使用 `window.insert_hitbox()` 创建 hitbox
4. 返回 paint 阶段的状态

### paint()

渲染元素并处理交互。

**参数：**
- `global_id`：全局元素标识符（可选）
- `inspector_id`：检查器元素标识符（可选）
- `bounds`：渲染的最终边界
- `request_layout`：布局状态的可变引用
- `prepaint`：prepaint 状态的可变引用
- `window`：可变窗口引用
- `cx`：可变应用上下文

**职责：**
1. 首先绘制子元素（从下到上）
2. 绘制自己的内容（背景、边框等）
3. 设置交互（鼠标事件、光标样式）

## IntoElement 集成

元素还必须实现 `IntoElement` 才能用作子元素：

```rust
impl IntoElement for MyElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
```

这允许你的自定义元素直接在元素树中使用：

```rust
div()
    .child(MyElement::new()) // 因为 IntoElement 所以可以工作
```

## 常用参数

### Global 和 Inspector ID

两者都是可选标识符，用于调试和检查：
- `global_id`：整个应用的唯一标识符
- `inspector_id`：开发工具/检查器的标识符

通常直接传递给子元素，不做修改。

### Window 和 Context

- `window: &mut Window`：窗口特定操作（绘制、hitbox、事件）
- `cx: &mut App`：应用级操作（生成任务、访问全局变量）

## 布局系统集成

### window.request_layout()

创建具有指定样式和子元素的布局节点：

```rust
let layout_id = window.request_layout(
    Style {
        size: size(px(200.), px(100.)),
        flex: Flex::Column,
        gap: px(8.),
        ..default()
    },
    vec![child1_layout_id, child2_layout_id],
    cx
);
```

### Bounds<Pixels>

表示矩形区域：

```rust
pub struct Bounds<T> {
    pub origin: Point<T>,
    pub size: Size<T>,
}

// 创建 bounds
let bounds = Bounds::new(
    point(px(10.), px(20.)),
    size(px(100.), px(50.))
);

// 访问属性
bounds.left()    // origin.x
bounds.top()     // origin.y
bounds.right()   // origin.x + size.width
bounds.bottom()  // origin.y + size.height
bounds.center()  // 中心点
```

## Hitbox 系统

### 创建 Hitbox

```rust
// 普通 hitbox（阻止事件传递）
let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);

// 透明 hitbox（允许事件传递到下面的元素）
let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Transparent);
```

### 使用 Hitbox

```rust
// 检查是否悬停
if hitbox.is_hovered(window) {
    // ...
}

// 设置光标样式
window.set_cursor_style(CursorStyle::PointingHand, &hitbox);

// 在事件处理程序中使用
window.on_mouse_event(move |event, phase, window, cx| {
    if hitbox.is_hovered(window) && phase.bubble() {
        // 处理事件
    }
});
```

## 事件处理

### 鼠标事件

```rust
// 鼠标按下
window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
    if phase.bubble() && bounds.contains(&event.position) {
        // 处理鼠标按下
        cx.stop_propagation(); // 阻止冒泡
    }
});

// 鼠标释放
window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
    // 处理鼠标释放
});

// 鼠标移动
window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
    // 处理鼠标移动
});

// 滚动
window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
    // 处理滚动
});
```

### 事件阶段

事件经历两个阶段：
- **捕获**：自顶向下（父 → 子）
- **冒泡**：自底向上（子 → 父）

```rust
move |event, phase, window, cx| {
    if phase.capture() {
        // 在捕获阶段处理
    } else if phase.bubble() {
        // 在冒泡阶段处理
    }

    cx.stop_propagation(); // 停止事件继续
}
```

## 光标样式

可用的光标样式：

```rust
CursorStyle::Arrow
CursorStyle::IBeam           // 文本选择
CursorStyle::PointingHand    // 可点击
CursorStyle::ResizeLeft
CursorStyle::ResizeRight
CursorStyle::ResizeUp
CursorStyle::ResizeDown
CursorStyle::ResizeLeftRight
CursorStyle::ResizeUpDown
CursorStyle::Crosshair
CursorStyle::OperationNotAllowed
```

用法：

```rust
window.set_cursor_style(CursorStyle::PointingHand, &hitbox);
```
