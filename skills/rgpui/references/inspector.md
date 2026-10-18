# 检查器（Inspector）

**目录：** [概述](#概述) · [两行启用](#两行启用) · [面板行为](#面板行为) · [AI 可读导出](#ai-可读导出) · [崩溃快照](#崩溃快照) · [自定义面板](#自定义面板) · [调试帧覆盖层](#调试帧覆盖层) · [API 速查](#api-速查)

## 概述

Inspector 是开发期调试工具：逐节点浏览元素树、点击选中画布区域并高亮、
查看盒模型与样式、监控帧率/CPU/内存，并在崩溃时落盘快照。

**门控：** 除 `mod inspector;`（`rgpui.rs:120`，内含 `InspectorElementId` 等基础类型）
外，全部检查器能力标 `#[cfg(any(feature = "inspector", debug_assertions))]`。
默认 **debug 构建即可用**，release 未开 `inspector` feature 时代码整体剥离、零开销。

**导出：** `pub use inspector::*`（`rgpui.rs:482`）；`inspector_panel` 模块与
类型经 `rgpui.rs:485-488` 重导出。**不在 prelude**。

可运行示例：`examples/showcase/src/bin/inspector.rs`（默认面板）、
`examples/showcase/src/bin/inspector_custom.rs`（全自写面板）。
运行 `cargo run -p showcase --bin inspector`。

## 两行启用

```rust
use rgpui::actions;

// 1. 定义开关动作（框架不内置，需自备）
actions!(my_app, [ToggleInspector]);

fn main() {
    application().run(|cx: &mut App| {
        cx.enable_default_inspector();                       // 第 1 行：注册默认面板

        // F12 用全局动作 helper：无焦点也能触发（见下方「为何用 on_global_action」）
        cx.on_global_action(ToggleInspector, Some("f12"), |window, cx| {
            window.toggle_inspector(cx);                     // 第 2 行：开关面板
        });

        cx.open_window(/* ... */);
    });
}
```

> **为何用 `on_global_action` 而不是 `key_context` + `on_action`？**
> 检查器开关必须无焦点、面板聚焦时也能用。普通上下文绑定在无焦点时分发路径
> 只有 root，匹配不上上下文、冒泡到不了视图 handler（`inspector.rs` 测试
> `f12_toggles_inspector_without_focus` 即为该回归而设）。
> `App::on_global_action`（`app.rs:2302`）只打活动窗口并经 `spawn` 延后执行，
> 正适合「按快捷键对活动窗口做点事」。

## 面板行为

`enable_default_inspector`（`app.rs:3016`）注册的面板提供：

- **运行卡**：帧率 / CPU / 内存实时监控。
- **报错卡**：读取 `App::report_error` 上报的错误环（最多 50 条，
  `app.rs:3025`/`3036`）。
- **拾取模式**：点「拾取元素」后悬停画布出蓝框、点击选中、滚轮穿透重叠层级
  （`Inspector::start_picking` / `is_picking`，`inspector.rs:258`/`263`）。
- **盒模型 + 样式**：选中元素的 margin/border/padding 四边值 + 已指定样式列表
  （由 `render_div_inspector_state` 提供，`inspector_panel.rs:302`）。
- **完整树**：逐节点折叠/展开，展开集只增不重置；单帧最多渲染 800 行
  （`FULL_TREE_ROW_CAP`，`inspector_panel.rs:42`）。
- **面板宽度**：默认 30rem，可拖拽调整（`window.inspector_width()` /
  `set_inspector_width()`，`window.rs:6519`/`6525`）。

## AI 可读导出

两个程序化入口，检查器**打开时**才返回 `Some`，未打开返回 `None`：

```rust
// Markdown 缩进树：每行一节点，含名称/实例号/源码位置/实测边界，选中标 [*]。
// 全量 DFS（不受面板折叠影响），超 max_nodes 截断并注总数。
if let Some(text) = window.inspector_tree_text(cx, 2000) {
    cx.write_to_clipboard(ClipboardItem::new_string(text));
}

// 结构化快照：JSON 可序列化，适合喂给工具消费。
if let Some(snapshot) = window.capture_inspector_snapshot(cx) {
    let json = serde_json::to_string_pretty(&snapshot).unwrap();
}
```

- `Window::inspector_tree_text(&self, cx: &App, max_nodes: usize) -> Option<String>`（`window.rs:6850`）
- `Window::capture_inspector_snapshot(&self, cx: &App) -> Option<InspectorSnapshot>`（`window.rs:6621`）
- `InspectorSnapshot` 字段（`inspector.rs:373`）：`version`、`timestamp_millis`、
  `active`、`ancestors`、`tree_total`、`tree`（截断 2000）、`errors`、`viewport`。
- 树遍历：`inspector_tree_roots()`（`window.rs:6806`）、
  `inspector_tree_children(id)`（`:6812`）、`inspector_tree_parent(id)`（`:6825`）、
  `select_inspector_element(id, cx) -> bool`（`:6937`）。
- `InspectorElementId`（`inspector.rs:5`）方法：`short_label()`（行标签）、
  `source_label()`（`文件:行`）、`tree_key()`（跨帧稳定键）。

面板自带的「复制树文本 / 复制快照 JSON」按钮即这两个调用，见
`examples/showcase/src/bin/inspector.rs:252`。

## 崩溃快照

```rust
cx.enable_crash_recorder(".rgpui-crash");               // last.json 约 2 秒一写（原子替换）
rgpui::runtime_stats::install_crash_hook(".rgpui-crash"); // panic-*.log 记录死因
```

- `App::enable_crash_recorder(dir)`（`app.rs:3045`）：目录不存在自动创建，
  仅检查器打开的窗口写入，重启后不持久化。
- 快照随检查器门控：release 未开 `inspector` feature 时无此数据，此时仅 panic 日志可用。
- 程序死后直接读 `.rgpui-crash/last.json`（死前状态）+ `panic-*.log`（死因）。

## 自定义面板

三种粒度，从粗到细：

```rust
// 1) 整板替换（复用 default_inspector_panel 包裹扩展）
cx.set_inspector_renderer(Box::new(|inspector, window, cx| {
    v_flex()
        .child(div().child("自定义横幅"))
        .child(rgpui::default_inspector_panel(inspector, window, cx))
        .into_any_element()
}));

// 2) 只换顶栏 / 段落外皮（不整板替换）
cx.set_inspector_panel_slots(InspectorPanelSlots {
    header: Some(Arc::new(|inspector, window, cx| {
        v_flex()
            .child(div().child("定制横幅"))
            .child(default_inspector_header(inspector, window, cx))
            .into_any_element()
    })),
    section: Some(Arc::new(|title, body| default_inspector_section(title, body))),
});

// 3) 按状态类型追加展示（默认已注册 Div 布局展示）
cx.register_inspector_element::<MyState, _>(|id, state, window, cx| {
    div().child(format!("{} = {}", id.short_label(), state.value))
});
```

- `InspectorRenderer = Box<dyn Fn(&mut Inspector, &mut Window, &mut Context<Inspector>) -> AnyElement>`（`inspector.rs:88`）
- `default_inspector_panel`（`inspector_panel.rs:61`）、`default_inspector_header`（`:226`）、`default_inspector_section`（`:282`）
- `InspectorPanelSlots { header, section }`（`inspector_panel.rs:32`），
  `InspectorHeaderSlot` / `InspectorSectionSlot`（`:17`/`:24`）均为 `Arc<dyn Fn ... + Send + Sync>`
- `App::register_inspector_element::<T, R>(f)`（`app.rs:3001`），
  `f: Fn(InspectorElementId, &T, &mut Window, &mut App) -> R`
- `Inspector` 实体方法：`select(id, window)`（`inspector.rs:124`）、
  `select_ancestor(levels_up, window) -> bool`（`:136`）、`active_element_id()`（`:227`）、
  `start_picking()`（`:258`）、`is_picking()`（`:263`）、
  `render_inspector_states(window, cx)`（`:268`）

## 调试帧覆盖层

与检查器解耦、release 也保留（未启用时不画任何图元，零开销）：

```rust
cx.on_global_action(CycleDebugOverlay, Some("f11"), |window, _cx| {
    window.cycle_debug_overlay();
});
```

- `window.cycle_debug_overlay()`（`window.rs:6470`）：在 `Hidden → Minimal → Full` 间循环
- `window.debug_overlay_mode() -> DebugFrameOverlayMode`（`:6456`），
  枚举 `DebugFrameOverlayMode::{Hidden, Minimal, Full}`（`rgpui.rs:478` 重导出）
- `window.is_debug_overlay_enabled()`（`:6483`）、`window.reset_debug_overlay_stats()`（`:6477`）

## API 速查

| 入口 | 位置 | 说明 |
|------|------|------|
| `App::enable_default_inspector()` | `app.rs:3016` | 注册默认面板 + Div 展示 |
| `App::set_inspector_renderer(f)` | `app.rs:2995` | 整板替换 |
| `App::register_inspector_element::<T,R>(f)` | `app.rs:3001` | 按类型追加展示 |
| `App::set_inspector_panel_slots(slots)` | `app.rs:3054` | 换顶栏/段落外皮 |
| `App::enable_crash_recorder(dir)` | `app.rs:3045` | 崩溃滚动快照 |
| `App::report_error(msg)` / `recent_errors()` | `app.rs:3025`/`3036` | 应用错误环（面板“报错”卡） |
| `Window::toggle_inspector(cx)` | `window.rs:6489` | 开/关面板 |
| `Window::is_inspector_picking(cx)` / `is_inspector_open()` | `window.rs:6498`/`6513` | 状态查询 |
| `Window::inspector_tree_text(cx, max)` | `window.rs:6850` | 导出 Markdown 树 |
| `Window::capture_inspector_snapshot(cx)` | `window.rs:6621` | 导出 JSON 快照 |

## 最佳实践

- **只在调试构建启用**：用 `#[cfg(any(feature = "inspector", debug_assertions))]` 包裹
  `enable_default_inspector` 与 F12 绑定，release 自动剥离。
- **用 `on_global_action` 而非上下文绑定**：保证无焦点/面板聚焦时也能开关。
- **检查器打开才导出**：`inspector_tree_text` / `capture_inspector_snapshot`
  在面板关闭时返回 `None`，调用方需处理。
- **应用错误走 `report_error`**：框架不拦截 `log`，需进面板的错误请显式上报。
