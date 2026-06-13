# ElementId

`ElementId` 是 rgpui 元素的唯一标识符。以下元素需要它：
- 鼠标事件处理（`on_click`、`on_hover` 等）
- 通过 `window.use_keyed_state` 存储状态
- 交互跟踪

## 使元素有状态

在 `div()` 上调用 `.id()` 创建 `Stateful<Div>`：

```rust
div().id("my-element")          // ElementId 来自 &str
div().id(42usize)               // ElementId 来自 usize
div().id(ElementId::from(idx))  // 显式
```

没有 `.id()`，div 无法接收鼠标事件或存储状态。

## 接受的类型

```rust
impl Into<ElementId> for &str      // "my-id"
impl Into<ElementId> for String    // String::from("my-id")
impl Into<ElementId> for usize     // 0, 1, 2, ...
impl Into<ElementId> for u64
impl Into<ElementId> for SharedString
```

## 唯一性规则

ID 必须在同**有状态父级的作用域**内唯一 — 而不是全局。rgpui 通过链接父级 ID 构建 `GlobalElementId`：

```rust
div().id("app").child(
    div().id("list1").children(vec![
        div().id(1usize).child("Item 1"),  // GlobalId: ["app", "list1", 1]
        div().id(2usize).child("Item 2"),  // GlobalId: ["app", "list1", 2]
    ])
).child(
    div().id("list2").children(vec![
        div().id(1usize).child("Item 1"),  // GlobalId: ["app", "list2", 1] — 没有冲突
    ])
)
```

不同父级作用域中的项目可以重用简单 ID（整数、短字符串）。

## 在组件结构体中

组件始终存储 `id: ElementId` 并在 `new()` 中传递：

```rust
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    base: Stateful<Div>,
    // ...
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();
        Self {
            id: id.clone(),
            base: div().id(id),  // id 应用到 base
            // ...
        }
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.base  // 已经应用了 .id()
            .on_click(/* ... */)
    }
}
```

## 在调用点使用

```rust
// 使用唯一字符串 ID 用于命名组件
Button::new("save-btn").label("Save")
Button::new("cancel-btn").label("Cancel")

// 在列表中使用基于索引的 ID
for (i, item) in items.iter().enumerate() {
    div().id(i)  // 在此父级内唯一
}

// 使用描述性 ID 用于调试
Input::new("search-input")
Select::new("country-select")
```
