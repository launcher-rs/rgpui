# Element 实现示例

各种场景下自定义元素的完整示例。

## 目录

1. [简单文本元素](#简单文本元素)
2. [带选择的交互元素](#带选择的交互元素)
3. [带子元素管理的复杂元素](#带子元素管理的复杂元素)

## 简单文本元素

支持语法高亮的基本文本元素。

```rust
pub struct SimpleText {
    id: ElementId,
    text: SharedString,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
}

impl IntoElement for SimpleText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SimpleText {
    type RequestLayoutState = StyledText;
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App
    ) -> (LayoutId, Self::RequestLayoutState) {
        // 创建带高亮的样式化文本
        let mut runs = Vec::new();
        let mut ix = 0;

        for (range, highlight) in &self.highlights {
            // 在高亮前添加无样式文本
            if ix < range.start {
                runs.push(window.text_style().to_run(range.start - ix));
            }

            // 添加高亮文本
            runs.push(
                window.text_style()
                    .highlight(*highlight)
                    .to_run(range.len())
            );
            ix = range.end;
        }

        // 添加剩余的无样式文本
        if ix < self.text.len() {
            runs.push(window.text_style().to_run(self.text.len() - ix));
        }

        let styled_text = StyledText::new(self.text.clone()).with_runs(runs);
        let (layout_id, _) = styled_text.request_layout(
            global_id,
            inspector_id,
            window,
            cx
        );

        (layout_id, styled_text)
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        styled_text: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App
    ) -> Self::PrepaintState {
        // Prepaint 样式化文本
        styled_text.prepaint(
            global_id,
            inspector_id,
            bounds,
            &mut (),
            window,
            cx
        );

        // 为交互创建 hitbox
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        hitbox
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        styled_text: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App
    ) {
        // 绘制样式化文本
        styled_text.paint(
            global_id,
            inspector_id,
            bounds,
            &mut (),
            &mut (),
            window,
            cx
        );

        // 为文本设置光标样式
        window.set_cursor_style(CursorStyle::IBeam, hitbox);
    }
}
```

## 带选择的交互元素

支持通过鼠标交互进行文本选择的文本元素。

```rust
#[derive(Clone)]
pub struct Selection {
    pub start: usize,
    pub end: usize,
}

pub struct SelectableText {
    id: ElementId,
    text: SharedString,
    selectable: bool,
    selection: Option<Selection>,
}

impl IntoElement for SelectableText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SelectableText {
    type RequestLayoutState = TextLayout;
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App
    ) -> (LayoutId, Self::RequestLayoutState) {
        let styled_text = StyledText::new(self.text.clone());
        let (layout_id, _) = styled_text.request_layout(
            global_id,
            inspector_id,
            window,
            cx
        );

        // 提取文本布局用于选择绘制
        let text_layout = styled_text.layout().clone();

        (layout_id, text_layout)
    }

    fn prepaint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _text_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App
    ) -> Self::PrepaintState {
        // 仅在可选择时创建 hitbox
        if self.selectable {
            Some(window.insert_hitbox(bounds, HitboxBehavior::Normal))
        } else {
            None
        }
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        text_layout: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App
    ) {
        // 绘制文本
        let styled_text = StyledText::new(self.text.clone());
        styled_text.paint(
            global_id,
            inspector_id,
            bounds,
            &mut (),
            &mut (),
            window,
            cx
        );

        // 如果有选择则绘制选择
        if let Some(selection) = &self.selection {
            Self::paint_selection(selection, text_layout, &bounds, window, cx);
        }

        // 处理鼠标事件用于选择
        if let Some(hitbox) = hitbox {
            window.set_cursor_style(CursorStyle::IBeam, hitbox);

            // 鼠标按下开始选择
            window.on_mouse_event({
                let bounds = bounds.clone();
                move |event: &MouseDownEvent, phase, window, cx| {
                    if bounds.contains(&event.position) && phase.bubble() {
                        // 在鼠标位置开始选择
                        let char_index = Self::position_to_index(
                            event.position,
                            &bounds,
                            text_layout
                        );
                        self.selection = Some(Selection {
                            start: char_index,
                            end: char_index,
                        });
                        cx.notify();
                        cx.stop_propagation();
                    }
                }
            });

            // 鼠标拖动扩展选择
            window.on_mouse_event({
                let bounds = bounds.clone();
                move |event: &MouseMoveEvent, phase, window, cx| {
                    if let Some(selection) = &mut self.selection {
                        if phase.bubble() {
                            let char_index = Self::position_to_index(
                                event.position,
                                &bounds,
                                text_layout
                            );
                            selection.end = char_index;
                            cx.notify();
                        }
                    }
                }
            });
        }
    }
}

impl SelectableText {
    fn paint_selection(
        selection: &Selection,
        text_layout: &TextLayout,
        bounds: &Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App
    ) {
        // 从文本布局计算选择边界
        let selection_rects = text_layout.rects_for_range(
            selection.start..selection.end
        );

        // 绘制选择背景
        for rect in selection_rects {
            window.paint_quad(paint_quad(
                Bounds::new(
                    point(bounds.left() + rect.origin.x, bounds.top() + rect.origin.y),
                    rect.size
                ),
                Anchor::default(),
                cx.theme().selection_background,
            ));
        }
    }

    fn position_to_index(
        position: Point<Pixels>,
        bounds: &Bounds<Pixels>,
        text_layout: &TextLayout
    ) -> usize {
        // 将屏幕位置转换为字符索引
        let relative_pos = point(
            position.x - bounds.left(),
            position.y - bounds.top()
        );
        text_layout.index_for_position(relative_pos)
    }
}
```

## 带子元素管理的复杂元素

管理多个子元素并支持滚动的容器元素。

```rust
pub struct ComplexElement {
    id: ElementId,
    children: Vec<Box<dyn Element<RequestLayoutState = (), PrepaintState = ()>>>,
    scrollable: bool,
    scroll_offset: Point<Pixels>,
}

struct ComplexLayoutState {
    child_layouts: Vec<LayoutId>,
    total_height: Pixels,
}

struct ComplexPaintState {
    child_bounds: Vec<Bounds<Pixels>>,
    hitbox: Hitbox,
}

impl IntoElement for ComplexElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ComplexElement {
    type RequestLayoutState = ComplexLayoutState;
    type PrepaintState = ComplexPaintState;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut child_layouts = Vec::new();
        let mut total_height = px(0.);

        // 为所有子元素请求布局
        for child in &mut self.children {
            let (child_layout_id, _) = child.request_layout(
                global_id,
                inspector_id,
                window,
                cx
            );
            child_layouts.push(child_layout_id);

            // 从布局获取子元素大小
            let child_size = window.layout_bounds(child_layout_id).size();
            total_height += child_size.height;
        }

        // 创建容器布局
        let layout_id = window.request_layout(
            Style {
                flex_direction: FlexDirection::Column,
                gap: px(8.),
                size: Size {
                    width: relative(1.0),
                    height: if self.scrollable {
                        // 可滚动的固定高度
                        px(400.)
                    } else {
                        // 不可滚动的自动高度
                        total_height
                    },
                },
                ..default()
            },
            child_layouts.clone(),
            cx
        );

        (layout_id, ComplexLayoutState {
            child_layouts,
            total_height,
        })
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout_state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App
    ) -> Self::PrepaintState {
        let mut child_bounds = Vec::new();
        let mut y_offset = self.scroll_offset.y;

        // 计算子元素边界并 prepaint 子元素
        for (child, layout_id) in self.children.iter_mut()
            .zip(&layout_state.child_layouts)
        {
            let child_size = window.layout_bounds(*layout_id).size();
            let child_bound = Bounds::new(
                point(bounds.left(), bounds.top() + y_offset),
                child_size
            );

            // 仅 prepaint 可见的子元素
            if self.is_visible(&child_bound, &bounds) {
                child.prepaint(
                    global_id,
                    inspector_id,
                    child_bound,
                    &mut (),
                    window,
                    cx
                );
            }

            child_bounds.push(child_bound);
            y_offset += child_size.height + px(8.); // 间距
        }

        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);

        ComplexPaintState {
            child_bounds,
            hitbox,
        }
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout_state: &mut Self::RequestLayoutState,
        paint_state: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App
    ) {
        // 绘制背景
        window.paint_quad(paint_quad(
            bounds,
            Anchor::all(px(4.)),
            cx.theme().background,
        ));

        // 仅绘制可见的子元素
        for (i, child) in self.children.iter_mut().enumerate() {
            let child_bounds = paint_state.child_bounds[i];

            if self.is_visible(&child_bounds, &bounds) {
                child.paint(
                    global_id,
                    inspector_id,
                    child_bounds,
                    &mut (),
                    &mut (),
                    window,
                    cx
                );
            }
        }

        // 如果可滚动则绘制滚动条
        if self.scrollable {
            self.paint_scrollbar(bounds, layout_state, window, cx);
        }

        // 处理滚动事件
        if self.scrollable {
            window.on_mouse_event({
                let hitbox = paint_state.hitbox.clone();
                let total_height = layout_state.total_height;
                let visible_height = bounds.size.height;

                move |event: &ScrollWheelEvent, phase, window, cx| {
                    if hitbox.is_hovered(window) && phase.bubble() {
                        // 更新滚动偏移
                        self.scroll_offset.y -= event.delta.y;

                        // 限制滚动偏移
                        let max_scroll = (total_height - visible_height).max(px(0.));
                        self.scroll_offset.y = self.scroll_offset.y
                            .max(px(0.))
                            .min(max_scroll);

                        cx.notify();
                        cx.stop_propagation();
                    }
                }
            });
        }
    }
}

impl ComplexElement {
    fn is_visible(&self, child_bounds: &Bounds<Pixels>, container_bounds: &Bounds<Pixels>) -> bool {
        // 检查子元素是否在可见区域内
        child_bounds.bottom() >= container_bounds.top() &&
        child_bounds.top() <= container_bounds.bottom()
    }

    fn paint_scrollbar(
        &self,
        bounds: Bounds<Pixels>,
        layout_state: &ComplexLayoutState,
        window: &mut Window,
        cx: &mut App
    ) {
        let scrollbar_width = px(8.);
        let visible_height = bounds.size.height;
        let total_height = layout_state.total_height;

        if total_height <= visible_height {
            return; // 不需要滚动条
        }

        // 计算滚动条位置和大小
        let scroll_ratio = self.scroll_offset.y / (total_height - visible_height);
        let thumb_height = (visible_height / total_height) * visible_height;
        let thumb_y = scroll_ratio * (visible_height - thumb_height);

        // 绘制滚动条轨道
        let track_bounds = Bounds::new(
            point(bounds.right() - scrollbar_width, bounds.top()),
            size(scrollbar_width, visible_height)
        );
        window.paint_quad(paint_quad(
            track_bounds,
            Anchor::default(),
            cx.theme().scrollbar_track,
        ));

        // 绘制滚动条滑块
        let thumb_bounds = Bounds::new(
            point(bounds.right() - scrollbar_width, bounds.top() + thumb_y),
            size(scrollbar_width, thumb_height)
        );
        window.paint_quad(paint_quad(
            thumb_bounds,
            Anchor::all(px(4.)),
            cx.theme().scrollbar_thumb,
        ));
    }
}
```

## 使用示例

### 使用 SimpleText

```rust
fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
        .child(SimpleText {
            id: ElementId::Name("code-text".into()),
            text: "fn main() { println!(\"Hello\"); }".into(),
            highlights: vec![
                (0..2, HighlightStyle::keyword()),
                (3..7, HighlightStyle::function()),
            ],
        })
}
```

### 使用 SelectableText

```rust
fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
        .child(SelectableText {
            id: ElementId::Name("selectable-text".into()),
            text: "用鼠标选择此文本".into(),
            selectable: true,
            selection: self.current_selection.clone(),
        })
}
```

### 使用 ComplexElement

```rust
fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let children: Vec<Box<dyn Element<_, _>>> = self.items
        .iter()
        .map(|item| Box::new(div().child(item.name.clone())) as Box<_>)
        .collect();

    div()
        .child(ComplexElement {
            id: ElementId::Name("scrollable-list".into()),
            children,
            scrollable: true,
            scroll_offset: self.scroll_offset,
        })
}
```
