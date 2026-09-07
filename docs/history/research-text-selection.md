# rgpui-component 文本选择功能研究报告

## 1. 功能概述

rgpui-component 实现了一套**窗口级协调文本选择系统**，允许用户通过鼠标拖拽在 `TextView` 组件中选择文本。该系统支持：

- 拖拽选择（单击拖拽）
- 双击选词
- 三击选段落
- Ctrl+A 全选
- 跨多个 TextView 的选择
- 滚动时自动滚动（拖拽到视口边缘时）
- 空白区域代理锚定（在空白区域开始拖拽也能选择下方的文本）

## 2. 架构设计

### 2.1 核心架构

```
┌─────────────────────────────────────────────────────────────────┐
│ Root（根容器，拥有所有选择状态）                                    │
│  ├─ selectable_text_views: HashMap<EntityId, (Weak, Hitbox)>   │
│  └─ text_selection: WindowTextSelection { anchor, cursor }     │
│                                                                 │
│  ├─ TextSelectionController（零大小 Element，第一个子元素）       │
│  │   注册 window.on_mouse_event：                               │
│  │     • MouseDown（capture 阶段：重置 suppress + 清除选择）      │
│  │     • MouseDown（bubble 阶段：如果未被抑制则开始选择）          │
│  │     • MouseMove（bubble 阶段：更新选择）                       │
│  │     • MouseUp（bubble 阶段：结束选择）                         │
│  │     • ScrollWheel（bubble 阶段：拖拽时重新解析光标位置）        │
│  └─────────────────────────────────────────────────────────────│
│                                                                 │
│  └─ 用户视图树                                                   │
│      └─ TextView.paint() → 如果 selectable：                    │
│           Root::register_selectable_text_view(state, hitbox)    │
│                                                                 │
│         └─ Inline.paint() → layout_selections()                │
│              从 WindowTextSelection 读取选择状态                  │
│              渲染高亮矩形                                       │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 数据流

1. **配置阶段**：`TextView::selectable(true)` 设置标志
2. **注册阶段**：`TextView::paint()` 调用 `Root::register_selectable_text_view()`
3. **事件处理**：`TextSelectionController` 捕获鼠标事件
4. **状态更新**：`WindowTextSelection` 更新 anchor/cursor
5. **渲染阶段**：`Inline::layout_selections()` 读取选择状态并渲染高亮

## 3. 关键文件和类型

### 3.1 核心文件

| 文件 | 职责 |
|------|------|
| `text/text_view.rs` | 配置 selectable 标志，传播到 state，paint 时注册到 Root |
| `text/state.rs` | `TextViewState` 实体：存储选择状态，管理视图本地选择 |
| `text/window_selection.rs` | `WindowTextSelection`（anchor/cursor 端点）、`TextSelectionController`（全局鼠标处理）|
| `text/inline.rs` | `Inline` 元素：渲染文本，计算逐字符选择命中测试，绘制高亮 |
| `text/selection.rs` | 纯文本工具：`CharType`、`word_range_at()` 等 |
| `root.rs` | `Root` 结构体：拥有 `text_selection` 和 `selectable_text_views` |
| `global_state.rs` | `GlobalState`：`text_view_state_stack`、`suppress_text_selection` 标志 |

### 3.2 核心类型

#### WindowTextSelection

```rust
pub(crate) struct WindowTextSelection {
    pub(crate) anchor: Option<SelectionEndpoint>,
    pub(crate) cursor: Option<SelectionEndpoint>,
    pub(crate) is_selecting: bool,
}
```

#### SelectionEndpoint

```rust
pub(crate) struct SelectionEndpoint {
    pub(crate) view: Option<WeakEntity<TextViewState>>,
    pub(crate) point: Point<Pixels>,  // 内容坐标（相对于视图）
    pub(crate) inside: bool,          // true=点击命中视图；false=空白区域代理锚定
}
```

- `point` 始终存储在视图的**内容坐标**中（相对于 `bounds.origin + scroll_offset`）
- `resolve()` 转换为窗口坐标：`point + scroll_offset + bounds.origin`

#### TextSelectionController

零大小 Element，必须是 Root 容器 div 的**第一个子元素**。原因：GPUI 按注册的逆序触发 bubble-phase 监听器，最早注册的控制器在所有交互组件（按钮、输入框等）之后执行。

## 4. 选择模式

| 模式 | 触发方式 | 存储位置 | 渲染方式 |
|------|---------|---------|---------|
| 全选 | Ctrl+A | `TextViewState.select_all` | 全文本范围 |
| 双击选词 | 双击 | `TextViewState.multi_click_selection` + `selected_text_override` | `word_range_at()` |
| 三击选段落 | 三击 | 同上，kind=Paragraph | 全 Inline 文本范围 |
| 拖拽选择 | 鼠标拖拽 | `WindowTextSelection`（anchor/cursor） | 逐字符几何命中测试 |
| 跨视图拖拽 | 拖拽跨越多个 TextView | `WindowTextSelection`（端点在不同视图） | 所有注册视图绘制高亮 |

## 5. 使用示例

```rust
// 方式1：Markdown 文本（支持选择）
TextView::markdown("md-view", "**粗体** 普通文本")
    .selectable(true)
    .scrollable(true)

// 方式2：纯文本（支持选择）
TextView::plain("plain-view", "这是纯文本内容\n支持多行选择")
    .selectable(true)

// 方式3：使用便捷函数
text::plain("这是可选择的纯文本")
    .selectable(true)

// 方式4：在对话框中使用可选择的文本
div().child(
    text::plain("请复制以上错误信息")
        .selectable(true)
        .scrollable(false)
)
```

## 6. 内置到 rgpui 的可能性分析

### 6.1 可移植的组件（低难度）

| 组件 | 说明 |
|------|------|
| `WindowTextSelection` + `SelectionEndpoint` | 替换 `WeakEntity<TextViewState>` 为 trait 对象 |
| `TextSelectionController` | 替换 `Root::update()` 调用为通用注册表 |
| 词/段落选择算法 | 已经无依赖 |
| Auto-scroll | 已经自包含 |
| 跨视图文本合并 | 算法通用，仅类型引用需抽象 |

### 6.2 需要抽象的组件（中等难度）

| 组件 | 说明 |
|------|------|
| `Inline::layout_selections` 几何逻辑 | 需要文本布局访问 trait |
| `Inline::paint_selection` | 需要绘制抽象 |
| `suppress_text_selection` 标志 | 需要窗口级事件过滤器 |

### 6.3 硬依赖（需要重构）

| 类型 | 依赖方 | 重构方案 |
|------|--------|---------|
| `Root` | `window_selection.rs` | 将选择状态移入 `Window` 或创建独立的 `TextSelectionState` |
| `TextViewState` | `text_view.rs`, `window_selection.rs`, `inline.rs` | 引入 `SelectableText` trait |
| `GlobalState` | `inline.rs`, `window_selection.rs` | 使用通用 paint-context 机制 |

### 6.4 建议的提取策略

**第一阶段：核心算法提取**
- 将 `WindowTextSelection`、`SelectionEndpoint`、`TextSelectionController` 和选择算法移入 rgpui core
- 引入 `SelectableText` trait：
  ```rust
  trait SelectableText {
      fn bounds(&self) -> Bounds<Pixels>;
      fn scroll_offset(&self) -> Point<Pixels>;
      fn is_selectable(&self) -> bool;
      fn selected_text(&self) -> String;
      fn has_view_selection(&self) -> bool;
      fn clear_selection(&mut self);
  }
  ```

**第二阶段：窗口级 API**
- 在 `Window` 上添加 `TextSelection` 字段
- 创建 `Window::register_selectable()`、`Window::selected_text()` API

**第三阶段：渲染层保持在组件层**
- `Inline` 渲染保持在 rgpui-component
- 从核心 `Window::text_selection()` API 读取

**第四阶段：事件过滤**
- 将 `suppress_text_selection` 标志变为窗口级事件过滤器

## 7. 潜在问题

### 7.1 架构耦合

- **Root 紧耦合**：选择状态存储在 `Root` 中，所有选择生命周期方法都是 `impl Root` 方法。提取需要将状态移入 `Window` 或独立结构。
- **GlobalState 依赖**：`text_view_state_stack` 用于 paint-time 上下文传递，Inline 通过它找到父 TextViewState。需要通用 paint-context 机制。

### 7.2 性能考虑

- **逐字符命中测试**：`Inline::layout_selections()` 对每个字符进行几何命中测试，O(n) 复杂度。对于长文本可能有性能问题。
- **每帧注册**：`register_selectable_text_view` 在每个 paint 帧调用，需要清理死视图（O(N²) 每帧）。
- **通知范围**：`notify_selection_band` 只通知受影响的视图，但跨视图选择时仍需遍历所有注册视图。

### 7.3 虚拟化支持

- 当前使用 `ListState` 进行虚拟化渲染，但选择状态与虚拟化列表的交互可能有问题
- 滚动时内容坐标需要重新计算

### 7.4 平台差异

- 文本布局依赖 `TextLayout`，不同平台（Windows/macOS/Linux）的文本渲染可能有差异
- 光标样式和鼠标行为可能需要平台特定处理

### 7.5 与 Input 组件的冲突

- Input 组件有自己的选择机制，需要通过 `suppress_text_selection` 标志避免冲突
- 需要确保 Input 的选择不会被窗口级选择系统干扰

## 8. 结论

rgpui-component 的文本选择功能是一个设计良好的窗口级协调系统，支持复杂的跨视图选择场景。将其内置到 rgpui core 是可行的，但需要：

1. 引入 `SelectableText` trait 抽象
2. 将选择状态从 `Root` 移入 `Window` 或独立结构
3. 创建窗口级选择 API
4. 保持渲染层在组件层

建议分阶段实施，第一阶段先提取核心算法和类型，第二阶段创建窗口级 API，这样可以保持向后兼容性。
