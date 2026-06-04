# 元素系统

> Element trait、布局与绘制管线。

## 概述

元素（Element）是 rgpui 的核心构建块，负责窗口中所有内容的布局和绘制。
元素形成一棵树，并按照 Taffy 实现的 Flexbox 布局标准进行布局。

## 内置元素

### div

最常用的容器元素：

```rust
use rgpui::{div, h_flex, v_flex};

// 基本 div
div()
    .w_full()           // 宽度 100%
    .h_full()           // 高度 100%
    .bg(gpui::blue())   // 背景色

// 水平 Flex 容器
h_flex()
    .gap_2()            // 子元素间距
    .items_center()     // 垂直居中
    .child(text("Hello"))
    .child(div().w_4().h_4().bg(gpui::red()))

// 垂直 Flex 容器
v_flex()
    .gap_4()
    .justify_between()  // 两端对齐
    .child(div())
    .child(div())
```

### text

文本元素：

```rust
use rgpui::text;

text("Hello, World!")
    .text_lg()          // 大号文本
    .text_color(gpui::white())
    .font_weight(gpui::FontWeight::BOLD)
```

### img

图片元素：

```rust
use rgpui::img;

img("assets/image.png")
    .w_64()
    .h_64()
    .object_fit(gpui::ObjectFit::Contain)
```

### canvas

自定义绘制元素：

```rust
use rgpui::canvas;

canvas(|window, cx, bounds| {
    // 在 bounds 区域内自定义绘制
    let mut scene = Scene::new();
    scene.push(gpui::Primitive::Rect {
        bounds,
        background: Some(gpui::green().into()),
    });
    scene
})
.w_full()
.h_full()
```

## 样式系统

### 尺寸

```rust
div()
    .w(px(200.0))       // 像素宽度
    .h(px(100.0))       // 像素高度
    .w_full()           // 100% 宽度
    .h_pct(50.0)        // 50% 高度
    .min_w(px(100.0))   // 最小宽度
    .max_w(px(500.0))   // 最大宽度
```

### 间距

```rust
div()
    .p(px(8.0))         // 内边距
    .px(px(16.0))       // 水平内边距
    .py(px(8.0))        // 垂直内边距
    .m(px(4.0))         // 外边距
    .mx_auto()          // 水平外边距居中
```

### 背景和边框

```rust
div()
    .bg(gpui::blue())           // 背景色
    .border_1()                 // 1px 边框
    .border_color(gpui::red())  // 边框颜色
    .rounded_lg()               // 圆角
    .shadow_md()                // 阴影
```

### 定位

```rust
div()
    .relative()         // 相对定位
    .absolute()         // 绝对定位
    .top(px(10.0))
    .left(px(20.0))
    .z_index(10)        // 层级
```

### 交互

```rust
div()
    .hover(|style| style.bg(gpui::red()))      // 悬停状态
    .active(|style| style.bg(gpui::blue()))    // 按下状态
    .cursor(gpui::CursorStyle::PointingHand)   // 鼠标样式
```

## 自定义元素

### 实现 Element trait

```rust
use rgpui::{Element, LayoutId, Window, App};

struct MyElement {
    value: i32,
}

impl Element for MyElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        // 向 Taffy 请求布局
        let layout_id = window.with_element_arena(|arena| {
            // 配置布局属性
        });
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // 提交边界信息
        ()
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // 绘制内容
        window.paint_scene(|scene| {
            scene.push(Primitive::Rect {
                bounds,
                background: Some(gpui::green().into()),
            });
        });
    }
}
```

### IntoElement trait

使用 `IntoElement` 将元素转换为 `AnyElement`：

```rust
impl IntoElement for MyElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
```

## 布局缓存

元素可以在帧间缓存，避免不必要的重新布局和绘制：

```rust
impl Render for MyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 当视图状态未变化时，复用上一帧的布局
        AnyView::from(self.entity.clone())
            .cached(StyleRefinement::default())
    }
}
```

## 最佳实践

1. **优先使用内置元素**：`div`、`text`、`img` 等覆盖大多数场景
2. **组合优于继承**：通过嵌套组合构建复杂 UI
3. **样式集中管理**：使用 `styled` 模块管理可复用的样式
4. **性能优化**：合理使用缓存避免不必要的重绘
