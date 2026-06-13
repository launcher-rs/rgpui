# 常见 Element 模式

**目录：** [文本渲染元素](#文本渲染元素) · [容器元素](#容器元素) · [交互元素](#交互元素) · [组合元素](#组合元素) · [可滚动元素](#可滚动元素) · [模式选择指南](#模式选择指南)

## 文本渲染元素

显示和操作文本内容的元素。

### 模式特点

- 使用 `StyledText` 进行文本布局和渲染
- 在 `paint` 阶段通过 hitbox 交互处理文本选择
- 在 `prepaint` 中为文本交互创建 hitbox
- 支持通过 runs 进行文本高亮和自定义样式

### 实现模板

```rust
pub struct TextElement {
    id: ElementId,
    text: SharedString,
    style: TextStyle,
}

impl Element for TextElement {
    type RequestLayoutState = StyledText;
    type PrepaintState = Hitbox;

    fn request_layout(&mut self, .., window: &mut Window, cx: &mut App)
        -> (LayoutId, StyledText)
    {
        let styled_text = StyledText::new(self.text.clone())
            .with_style(self.style);
        let (layout_id, _) = styled_text.request_layout(None, None, window, cx);
        (layout_id, styled_text)
    }

    fn prepaint(&mut self, .., bounds: Bounds<Pixels>, styled_text: &mut StyledText,
                window: &mut Window, cx: &mut App) -> Hitbox
    {
        styled_text.prepaint(None, None, bounds, &mut (), window, cx);
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(&mut self, .., bounds: Bounds<Pixels>, styled_text: &mut StyledText,
             hitbox: &mut Hitbox, window: &mut Window, cx: &mut App)
    {
        styled_text.paint(None, None, bounds, &mut (), &mut (), window, cx);
        window.set_cursor_style(CursorStyle::IBeam, hitbox);
    }
}
```

### 用例

- 带语法高亮的代码编辑器
- 富文本显示
- 带自定义格式的标签
- 可选择的文本区域

## 容器元素

管理和布局子元素的元素。

### 模式特点

- 管理子元素的布局和位置
- 需要时处理滚动和裁剪
- 实现 flex/grid 类布局
- 协调子元素交互和事件委托

## 交互元素

响应用户输入（鼠标、键盘、触摸）的元素。

### 模式特点

- 为交互区域创建适当的 hitbox
- 正确处理鼠标/键盘/触摸事件
- 管理焦点和光标样式
- 支持悬停、激活和禁用状态

## 组合元素

组合多个子元素并进行复杂协调的元素。

### 模式特点

- 组合不同类型的多个子元素
- 跨子元素管理复杂状态
- 协调动画和过渡
- 处理子元素之间的焦点委托

## 可滚动元素

具有可滚动内容区域的元素。

### 模式特点

- 管理滚动状态（偏移量、速度）
- 处理滚动事件（滚轮、拖动、触摸）
- 绘制滚动条（轨道和滑块）
- 将内容裁剪到可见区域

## 模式选择指南

| 需求 | 模式 | 复杂度 |
|------|------|--------|
| 显示样式化文本 | 文本渲染 | 低 |
| 布局多个子元素 | 容器 | 低-中 |
| 处理点击/悬停 | 交互 | 中 |
| 复杂多部分 UI | 组合 | 中-高 |
| 带滚动的大内容 | 可滚动 | 高 |

选择满足需求的最简单模式，然后根据需要扩展。
