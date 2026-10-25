# 命令注册表 / Ribbon / 视口 / 电子表格 / 文档 / 控制通道

为文档/表格/演示类应用补齐的套件基座。除 `commands` 外均由 feature 门控，均不进根 glob，
按 `rgpui::<模块>::*` 使用：

- `rgpui::commands` — 命令注册表（UI 元数据 + 按 id 派发，**核心，不随 feature 开关**）
- `rgpui::ribbon` — 数据驱动 Ribbon 功能区（feature `ribbon`）
- `rgpui::keytips` — 按键提示解析与徽标（feature `ribbon`）
- `rgpui::qat` — 快速访问工具栏（feature `ribbon`）
- `rgpui::mini_toolbar` — 选中后就近浮出的迷你工具栏（feature `ribbon`，需应用自行接选区）
- `rgpui::backstage` — 覆盖整窗的应用菜单/文件面板（feature `ribbon`）
- `rgpui::viewport` — 屏幕 ↔ 文档坐标映射与缩放/平移（feature `viewport`）
- `rgpui::guides` — 标尺刻度 + 对齐参考线/吸附（feature `viewport`）
- `rgpui::object_toolkit` — 画布对象的选择把手/缩放旋转/对齐分布（feature `viewport`）
- `rgpui::formula_bar` — 公式栏：名称框 + `fx` + 公式输入（feature `spreadsheet`）
- `rgpui::sheet_grid` — 电子表格网格：行列头/双轴虚拟化/选区/填充柄（feature `spreadsheet`，**不含公式求值**，计算由宿主实现 `SheetDelegate`）
- `rgpui::page_view` — 分页画布 + 页眉页脚 + 页面设置（feature `document`，蕴含 `viewport`）
- `rgpui::control_channel` — 本机 TCP 上的 JSON-RPC 控制通道（feature `control-channel`，蕴含 `tokio`/`tokio/net`；wasm 目标下不编译）
- `rgpui::mcp` — stdio MCP 工具面，与外部 AI 客户端共用同一批命令（feature `control-channel`）

启用：`rgpui = { features = ["ribbon", "viewport", "spreadsheet", "document", "control-channel"] }`
（默认全关，普通应用不必开启；`document` 已蕴含 `viewport`）。
示例：`cargo run -p showcase --bin {command_registry,ribbon,qat,mini_toolbar,backstage,viewport,ruler,canvas_objects,formula_bar,sheet_grid,page_view,automation}`，
整合演示：`cargo run -p showcase --bin app_suite`。

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
let state = cx.new(RibbonState::new);
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

- `RibbonState`：`new(cx)`/`active_tab()`/`set_active_tab(index, cx)`（默认第 0 个）；
  另持 `FocusHandle` 与按键提示状态，并提供 `show_keytips`/`hide_keytips`/
  `toggle_keytips`/`handle_key`。
- `RibbonGroup`：底部显示组名；连续小按钮自动聚成每列 ≤3 的竖列。
- `RibbonItem` 枚举 `Button`/`Toggle`/`Dropdown`/`Separator`，构造器
  `RibbonItem::{button,toggle,dropdown,separator}`（返回描述符，`impl Into<RibbonItem>`）。
- `RibbonButton`（`label`/`icon`/`large`/`keytip`）、`RibbonToggle`（`label`/`icon`/
  `checked`/`keytip`，checked 状态由调用方维护）、`RibbonDropdown`（`label`/`icon`/
  `item`/`large`/`keytip`）+ `RibbonDropdownItem`（`command`/`label`）。
- `RibbonTab::{keytip}` 设置选项卡按键提示。
- `Ribbon::new(&Entity<RibbonState>, Vec<RibbonTab>).collapsed(bool)`，`RenderOnce` 元素。

### 按键提示（KeyTips）

`rgpui::init_all`（或 `rgpui::ribbon::init`）会绑定 `Alt`/`F10`。功能区根节点带
`key_context("Ribbon")`；获得焦点后按键提示模式先显示选项卡提示、再显示命令提示。
提示键来源：显式覆盖 > `CommandDescriptor::with_keytip` 登记 > 标签首个 ASCII 字母派生。
`CommandDescriptor::keytip()` 可查询。纯解析工具在 `rgpui::keytips`
（`KeyTipScope`/`KeyTipMatch`/`resolve_key`/`KeyTip`）。

```rust
CommandDescriptor::new("app::Paste", "粘贴").with_keytip("V");
RibbonTab::new("home", "开始").keytip("H").group(
    RibbonGroup::new("剪贴板").item(RibbonItem::button("app::Paste")),
);
```

> 尚未实现：SplitButton、Gallery、ComboBox、折叠态交互（应用菜单见 2d）。

---

## 2b. 快速访问工具栏（`rgpui::qat`）

一排按**命令 id** 绑定的紧凑图标按钮，通常置于标题栏或 Ribbon 之上；点击统一经
`App::dispatch_command` 派发。

```rust
use rgpui::qat::{QatItem, QuickAccessToolbar};
use rgpui::IconName;

QuickAccessToolbar::new()
    .item(QatItem::button("app::Save").icon(IconName::Check))
    .item(QatItem::button("app::Undo").icon(IconName::Undo))
    .item(QatItem::separator())
    .item(QatItem::button("app::Search").label("查找"))   // 覆盖注册表标签
```

- `QuickAccessToolbar::new()` + `item(impl Into<QatItem>)` / `items(..)`。
- `QatItem`：`Button(QatButton)` / `Separator`（`QatItem::button` / `QatItem::separator`）。
- `QatButton`：`new(command)` + `label(..)` / `icon(IconName)`。
- 标签来源：显式覆盖 > `CommandRegistry::label_for(id)` > 命令 id 本身。

---

## 2c. 迷你工具栏（`rgpui::mini_toolbar`）

选中文本/对象时就近浮出的紧凑命令条，项与 QAT 完全同源（`QatItem`）。

```rust
use rgpui::mini_toolbar::MiniToolbar;
use rgpui::qat::QatItem;
use rgpui::{IconName, point, px};

// 父容器需 relative()；at 用父容器坐标绝对定位。
div().relative().size_full().child(
    MiniToolbar::new()
        .at(point(px(120.0), px(64.0)))
        .item(QatItem::button("app::Bold").label("B"))
        .item(QatItem::separator())
        .item(QatItem::button("app::Copy").icon(IconName::Copy)),
)
```

- `MiniToolbar::new()` + `item(impl Into<QatItem>)` / `items(..)` / `at(Point<Pixels>)`
  （不调用 `at` 则按流式元素渲染）。
- 表面为 `popover_style`（popover 底 + 边框 + 阴影 + 圆角）；何时/在哪出现由调用方决定。
- QAT/迷你工具栏按钮按下会 `stop_propagation`，不会打断父级拖拽选择。

---

## 2d. 应用菜单（`rgpui::backstage`）

覆盖整窗的「文件」面板：左侧页面导航 + 右侧内容，页面既可是自定义内容，
也可是**一条命令**（点击即派发并关闭）。

```rust
use rgpui::backstage::{Backstage, BackstageState, BackstageTab};
use rgpui::{IconName, div};

// 状态与页面列表通常建在视图的 `new` 里；`Backstage` 每帧渲染时构造。
let state = cx.new(BackstageState::new);

// 关闭态渲染为空元素；父容器需 relative()。
div().relative().size_full().child(
    Backstage::new(&state, vec![
        BackstageTab::new("new", "新建").icon(IconName::Plus)
            .content(|_window, _cx| div().child("模板列表").into_any_element()),
        BackstageTab::new("save", "保存").icon(IconName::Check)
            .command("app::Save"),
    ]),
)
```

- `BackstageState`（`Entity`）：`open` / `close` / `toggle` / `is_open`、
  `active` / `set_active(index)`、`focus_handle`。
- `BackstageTab`：`new(id, label)` + `icon(IconName)`；`command(id)` 与
  `content(Fn(&mut Window, &mut App) -> AnyElement)` **互斥**（后设置者生效）。
- `Backstage::new(&state, tabs)` + `sidebar_width(Pixels)`。
- **打开时要自己交付焦点**：`window.focus(&state.read(cx).focus_handle(), cx)`，
  否则 `Esc` 关不掉（面板靠 `track_focus` + `key_context("Backstage")` 接收按键）。
- 不提供全局快捷键（避免与 KeyTips / 应用键位冲突），入口按钮由调用方给。

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

---

## 5. 画布对象把手（`rgpui::object_toolkit`，feature `viewport`）

在 `Viewport` 之上叠一层对象交互：选中框、把手命中、拖拽改尺寸/旋转、多选对齐分布。
纯几何 + 元素，不绑具体业务对象——宿主把「被选中的东西」映射成矩形即可。

- 元素：`ResizeHandle`（单个把手）、`SelectionHandles`（选中框 + 八向把手 + 旋转柄）
- 几何：`handle_point` / `handle_point_rotated`（把手位置）、`hit_handle` /
  `hit_handle_rotated`（命中判定）、`resize_bounds`（按把手拖动出新矩形）、
  `rotate_point` / `angle_between`、`marquee_bounds` / `marquee_hits`（框选）、
  `union_bounds`、`align` / `distribute`（多选对齐与等分，配合 `AlignMode`）
- 示例：`cargo run -p showcase --bin canvas_objects`

## 6. 电子表格基座（`rgpui::formula_bar` / `rgpui::sheet_grid`，feature `spreadsheet`）

**口径提醒**：这两个模块只做「表格 UI 基座 + 单元格模型」，**仓库内没有公式求值引擎**。
`=SUM(A1:A3)` 之类的计算由宿主实现 `SheetDelegate` 自行完成，框架不预测、不求值。

- `FormulaBar` / `FormulaBarState` / `FormulaBarEvent`：名称框 + `fx` + 公式输入行，
  `set_formula` / `formula` / `clear_formula` 读写的是**原文**
- `SheetGrid` / `SheetGridState` / `SheetGridEvent`：行列头、双轴虚拟化、矩形选区、
  填充柄拖拽填充；`CellRef` / `CellRange` 表示单元格与区域，`column_label` 生成 A/AA 列头
- `SheetDelegate`：宿主侧数据与写入协议（读写单元格、由宿主负责重算）
- 示例：`cargo run -p showcase --bin {formula_bar,sheet_grid}`

## 7. 文档分页（`rgpui::page_view`，feature `document`）

把 `Viewport` 的文档坐标铺成纸页：分页排列、页眉页脚、页边距与内容参考线。

- 元素：`PageView`（`layout` + 视口 + 标签），`content_guide` / `area_guide` 开内容区/整页虚线框
- 模型：`PageLayout`、`PageSetup` / `PageSize` / `PageMargins`、`PageArrangement`
  （单页 / 对开 / 连续）、`PageLabels`
- 示例：`cargo run -p showcase --bin page_view`

## 8. 控制通道与 MCP（`rgpui::control_channel` / `rgpui::mcp`，feature `control-channel`）

让外部进程（脚本、AI 客户端）调用应用已注册的命令——与 UI 走同一批 `CommandRegistry`
条目，因此清单体检（`CommandRegistry::validate`）天然覆盖它们。

- `App::start_control_channel(ControlChannelOptions)`：本机 TCP 上的 JSON-RPC，
  `ControlChannelOptions::with_key` 设密钥（不设则 `generate_key` 随机生成，
  经 `ControlChannel::key()` 读出交给调用方）；请求顶层 `key` 字段不匹配即拒绝
  （`authorized`）
- `McpServer` / `tool_name`：stdio 上的 MCP 工具面，一条命令映射一个工具
- wasm 目标下这两个模块不编译（`not(target_family = "wasm")`），开了 feature 也没有服务端
- 示例：`cargo run -p showcase --bin automation`（加 `--mcp` 走 stdio MCP）；
  回归：`cargo test -p showcase --bin automation`
