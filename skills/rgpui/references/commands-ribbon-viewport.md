# 命令注册表 / Ribbon / 视口 / 标尺参考线

为 Office 类应用（文档/表格/演示）补齐的四个共享基座。均不进根 glob，
按 `rgpui::<模块>::*` 使用：

- `rgpui::commands` — 命令注册表（UI 元数据 + 按 id 派发）
- `rgpui::ribbon` — 数据驱动 Ribbon 功能区
- `rgpui::viewport` — 屏幕 ↔ 文档坐标映射与缩放/平移
- `rgpui::guides` — 标尺刻度 + 对齐参考线/吸附

示例：`cargo run -p showcase --bin {command_registry,ribbon,viewport,ruler}`。

---

## 1. 命令注册表（`rgpui::commands`）

在既有 Action 之上附加 UI 元数据。**命令 id 即动作名称**（如 `"editor::MoveUp"`），
因此与 Action 系统零冲突、零迁移。

### 登记命令

`register_command!` 只接受 `fn() -> CommandDescriptor` 的**函数项**（inventory 的
const 约束不接受闭包）：

```rust
use rgpui::{CommandDescriptor, register_command};

fn open_file() -> CommandDescriptor {
    CommandDescriptor::new("app::Open", "打开")
        .with_category("文件")
        .with_description("打开一个文件")
        .with_keystrokes(["ctrl-o"])
}

register_command!(open_file);
```

### 查询与派发

```rust
use rgpui::CommandRegistry;

let registry = CommandRegistry::global();       // inventory 全局单例，首次访问加载
registry.contains("app::Open");                 // 存在性
registry.get("app::Open").map(|d| d.label());   // 元数据
registry.iter();                                // 已按「分类 → 标签」排序
registry.categories();                          // 去重分类

// 按 id 构建并派发动作（走常规焦点路由；无窗口走全局分发）
cx.dispatch_command("app::Open", None)?;        // App::dispatch_command(id, params)
```

`CommandDescriptor`：`id`/`label`/`category`/`description`/`default_keystrokes`，
构建用 `with_category` / `with_description` / `with_keystroke` / `with_keystrokes`，
读取用同名 getter。

### 命令面板接入

```rust
let state = CommandPaletteState::from_registry();
let palette = CommandPalette::from_registry(window, cx);
```

---

## 2. Ribbon 功能区（`rgpui::ribbon`）

命令项按**命令 id** 绑定；标签未显式指定时从 `CommandRegistry` 读取，点击统一经
`App::dispatch_command` 派发。

结构与用法：

```rust
use rgpui::ribbon::{Ribbon, RibbonContext, RibbonGroup, RibbonItem, RibbonState, RibbonTab};
use rgpui::{IconName, rgb};

// 状态是 Entity；父视图需 observe 以便切换选项卡时重绘。
let state = cx.new(|_| RibbonState::new());
cx.observe(&state, |_, _, cx| cx.notify()).detach();

Ribbon::new(&state, vec![
    RibbonTab::new("home", "开始")
        .group(
            RibbonGroup::new("剪贴板")
                .item(RibbonItem::button("app::Paste").label("粘贴").icon(IconName::Copy).large())
                .item(RibbonItem::button("app::Cut").label("剪切"))
                .item(RibbonItem::separator()),
        )
        .group(RibbonGroup::new("插入").item(
            RibbonItem::button("app::InsertTable").icon(IconName::Plus).large(),
        )),
    RibbonTab::new("image", "图片")
        .context(RibbonContext::new("图片工具", rgb(0xc2410c))), // 上下文选项卡强调色
])
```

要点：

- `RibbonState`：`new()`/`active_tab()`/`set_active_tab(index, cx)`，`Default` = 第 0 个。
- `RibbonGroup`：底部显示组名；连续小按钮自动聚成每列 ≤3 的竖列。
- `RibbonItem` 枚举 `Button`/`Toggle`/`Dropdown`/`Separator`，构造器
  `RibbonItem::{button,toggle,dropdown,separator}`（返回描述符，`impl Into<RibbonItem>`）。
- `RibbonButton`（`label`/`icon`/`large`）、`RibbonToggle`（`label`/`icon`/`checked`，
  checked 状态由调用方维护）、`RibbonDropdown`（`label`/`icon`/`item`/`large`）
  + `RibbonDropdownItem`（`command`/`label`）。
- `Ribbon::new(&Entity<RibbonState>, Vec<RibbonTab>).collapsed(bool)`，`RenderOnce` 元素。

> 尚未实现：SplitButton、Gallery、ComboBox、折叠态交互、QAT/Mini toolbar、KeyTips、Backstage。

---

## 3. 视口（`rgpui::viewport`）

纯数据（`Copy`）的坐标系统，约定 `screen = document * zoom + offset`。

```rust
use rgpui::viewport::Viewport;
use rgpui::{point, px};

let mut viewport = Viewport::new()
    .with_zoom(0.6)
    .with_zoom_limits(0.1, 32.0)
    .with_offset(point(px(80.0), px(24.0)));

viewport.screen_to_document(event.position);     // 屏幕 → 文档
viewport.document_to_screen(point(px(0.0), px(0.0)));
viewport.document_size_to_screen(size(px(794.0), px(1123.0)));

viewport.set_zoom_at(1.0, anchor);               // 锚点对应的文档坐标不变
viewport.zoom_by_at(1.25, anchor);
viewport.zoom_in_at(anchor);
viewport.zoom_out_at(anchor);

viewport.pan_by(delta);
viewport.center(document_size, view_size);
viewport.fit(document_size, view_size, px(32.0)); // 等比铺满并居中

viewport.apply_wheel(&event, px(50.0));           // Ctrl/⌘ 缩放，否则平移
```

注意：rgpui 元素层**不支持对任意子树施加仿射变换**（仅 `Svg` 有 `Transformation`）。
整页放缩建议用 `canvas` 自绘；需要真正缩放控件子树时，应用自行按
`document_size_to_screen` 换算尺寸。

---

## 4. 标尺 + 对齐参考线/吸附（`rgpui::guides`）

纯计算工具库（无状态、无渲染）。

```rust
use rgpui::guides::{align, nice_step, ruler_ticks};

// 标尺刻度：按可见区间与缩放挑选「好看」步长（1/2/5 × 10ⁿ）。
let ticks = ruler_ticks(0.0..visible_doc_width, zoom, 60.0);
// ticks.step / ticks.ticks: Vec<f32>

// 吸附：把 moving 的六锚点吸附到其它矩形或自由参考线。
let result = align(moving_bounds, &others, &extra_x, &extra_y, px(6.0));
moving_bounds.origin += result.offset;   // 应用位移修正
for guide in &result.guides {            // 绘制参考线
    match guide.axis {
        rgpui::guides::GuideAxis::Vertical => { /* 在 guide.position 画竖线 */ }
        rgpui::guides::GuideAxis::Horizontal => { /* 在 guide.position 画横线 */ }
    }
}
```

- `nice_step(min) -> f32`：取 1/2/5 × 10ⁿ 中不小于 `min` 的最小步长。
- `ruler_ticks(range: Range<f32>, zoom: f32, min_spacing_px: f32) -> RulerTicks`。
- `align(moving: Bounds<Pixels>, others: &[Bounds<Pixels>], extra_x: &[Pixels], extra_y: &[Pixels], threshold: Pixels) -> SnapResult`；
  每轴取偏移量最小且不超过阈值的候选，返回 `offset` 与 `guides`。
- 类型：`GuideAxis`（Vertical/Horizontal）、`Anchor`（Start/Center/End）、
  `Guide { axis, position, anchor }`、`SnapResult { offset, guides }`。
