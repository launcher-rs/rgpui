# rgpui 做文档/表格/演示套件的基础缺口分析

> 调研文档。参考 `temp/storytold` 下 4 个「用 egui 复刻生产力套件/Adobe」的 Rust 项目，
> 梳理 rgpui 作为 UI 基座要落地文档/表格/演示类应用的前置能力：哪些要**开发**（框架缺组件/能力）、
> 哪些要**借鉴**（工程范式）、哪些**不该进框架**（领域/引擎职责）。
> 目的：在动手做套件之前先把地基打牢，避免重复造轮子。

## 一、参考对象：storytold

4 个独立 workspace，纯 Rust + egui/eframe，UI 层在 `crates/ui-egui/`，对标微软/Adobe：

| 项目 | 对标 | 规模 | 层数 | 关键点 |
|---|---|---|---|---|
| `deckcraft` | PowerPoint | ~202k 行 / 24 crates | L0 geom/color/metafile → L6 ui | 形状/动画/媒体/母版 |
| `gridcraft` | Excel | engine-first | L0 core/numfmt → L2 calc/xlsx | 公式/函数/数值格式 |
| `pdfcraft` | Acrobat Pro | ~152k 行 / 31 crates | L0 geom/filters/crypt → L1 cos | 容错解析/xref 修复 |
| `wordcraft` | Word | ~85.6k 行 / 15 crates | L0 geom → L2 layout/docx | 分页/断行/表格/校对 |

### 共同范式（值得借鉴的部分）

1. **引擎优先 + 严格分层**：`cargo xtask layers` 强制「只依赖更低层」，**L6 之下不依赖
   `egui/eframe/winit/rfd`** → UI 层可整体替换。这正是 rgpui 可以顶替 egui 的前提。
2. **Everything is a command**：所有用户可见行为 = 一条 `CommandSpec`
   （`id / label / ribbon 位置 / 快捷键 / 参数文档 / enabled / run`）。
   功能区、快捷键、命令搜索、CLI、control channel、MCP **全部按 id 分发**。
3. **Control channel + MCP**：应用起一个 JSON-line TCP 端口（带 key），外部/agent 可
   `engine.execute`、`ui.screenshot`；另有 `wordcraft-cli mcp` 无头模式与
   `--connect` 驱动运行中实例。
4. **无头截图 `ui_shot`**：不抢焦点、锁屏也能跑，配合「看结果再改 UI」的开发闭环。
5. **never-crash**：`forbid unsafe`，生产 crate 根 `deny(unwrap_used, expect_used, panic,
   unimplemented, todo, unreachable)`；输入衍生数值按敌意处理（`get` 而非 `[]`、字符边界切片、
   checked/saturating、钳制尺寸、限递归）；逃逸 panic 有兜底恢复。
6. **WASM 必过**：`cargo xtask wasm` 是 CI 门槛。
7. **资产 clean-room**：禁止复制 MS/Adobe 图标/图片/字体；每份资产在 `ATTRIBUTION.md` 有行。
8. **parity 清单驱动**：`catalog.rs` 列目标软件的功能 + 命令 id，`cargo xtask parity` 生成清单，
   测试守住「只升不降」的下限。
9. **i18n catalog**：翻译目录 + 缺漏审计 + 伪本地化；RTL 接口镜像。

## 二、rgpui 现状（基线）

已具备（详见 `skills/rgpui/references/`）：基础元素 20+、`form`、`input_ui`
（含 `editor` feature：LSP/高亮/片段/minimap/vim）、`menu`、`dialog`、`list`/`table`/`tabs`、
`tree`、`title_bar`、`components`（DockArea/SplitPane/CommandPalette/ColorPicker/Sidebar/
Draggable/DropZone…，`charts`/`effects`/`qr-code` 门控）、`animation`/`mouse_gestures`/
`scroll_physics`、`canvas`、`Transformation`（`with_transformation`）、`i18n`、`theme`/
`theme_watcher`、`inspector`、`fps_hud`、`config_store`/`file_watcher`/`single_instance`/
`tray`/`webview`/`tokio`、平台层（accesskit 无障碍树、原生菜单 `set_menus`、多窗口
`open_window`、剪贴板 `ClipboardItem`/`ExternalPaths`、文件拖放 `FileDropEvent`）、WASM 后端。

**结论**：rgpui 是「通用 UI 框架」，套件类应用所需的**特征组件（Ribbon 系）与文档/画布类能力基本缺席**；
文本层缺 RTL/CJK 深度；这些正是要补的地基。

## 三、UI 原语缺口矩阵

### A. 需新开发（rgpui 完全没有）—— 4 个应用高频共用，优先级最高

| 能力 | 参考实现（storytold ui-egui） | rgpui 现状 | 建议 |
|---|---|---|---|
| **Ribbon 功能区**（tabs/groups/大按钮/small/split button/dropdown/gallery/checkbox/combobox/上下文选项卡/折叠态） | 各项目 `ribbon.rs` | 无 | 新增 `components/ribbon/`（数据驱动，绑定命令 id） |
| **KeyTips**（Alt/F10 逐层按键提示） | `keytips.rs` | 无 | 新增覆盖层组件，联动命令注册表 |
| **QAT / 迷你工具栏**（快速访问工具栏、选中浮动格式条） | `qat.rs`、`mini_toolbar.rs` | 无 | 新增 |
| **Backstage / File 页**（左侧导航 + 页面） | `backstage.rs` | 无 | 新增 |
| **公式栏 Formula Bar**（名称框 + fx + 富文本编辑） | gridcraft `formula_bar.rs` | 无 | 新增 |
| **电子表格网格**（冻结窗格、行列头、选区、填充柄、公式编辑、海量虚拟化） | gridcraft `grid.rs` | `DataTable` 面向数据表 | 新增专用 Grid 子系统 |
| **文档页视图**（分页画布、页边距、标尺、页眉页脚、缩放/平移） | wordcraft `canvas.rs`/`ruler_tabs.rs` | 无页模型 | 新增分页视口 |
| **标尺 Ruler** | `ruler_tabs.rs`、`table_ruler.rs` | 无 | 新增可复用 Ruler 组件 |
| **对齐参考线 Guides/吸附** | `guides.rs` | 无 | 新增对齐/吸附工具库 |
| **画布对象工具**（选择/多选框/缩放手柄/旋转手柄/对齐） | `objects.rs`/`crop.rs` | 无 | 新增对象操作 toolkit |
| **Ink / 笔迹层**（pen/highlighter/eraser/lasso） | `ink.rs` | 无 | P1 |
| **缩略图 / 排序器** | `thumbs.rs`/`sorter.rs` | 无 | P2 |
| **动画时间线** | `animtimeline.rs` | 无 | P2 |
| **取色器增强**（吸管/最近颜色/主题色板） | `eyedropper.rs`/`fillui.rs` | 有 `ColorPicker` | 增强 |
| **富剪贴板 payload** | `system_clipboard.rs` | 只有 text/image/路径 | 扩展 `ClipboardItem` |

### B. 现有能力需增强（有但不敷套件用）

| 维度 | rgpui 现状 | 缺口 |
|---|---|---|
| 文本/排版 | `text_system`（wrap/truncate、font-fallbacks/features、font-kit） | **无 bidi/RTL**（未依赖 `unicode-bidi`）、无 CJK 深度（拼音指南/纵横/文档网格）、无段落/分页布局、无 inline 域/对象 |
| 富文本编辑 | `input_ui` + `editor`（面向代码） | 缺文档模型（段落/节/表/批注/修订） |
| 表格 | `table::DataTable`（数据网格） | 缺电子表格的单元格/公式/循环引用模型 |
| 变换/缩放 | 元素级 `with_transformation` | 缺「可缩放平移视口」控件与屏幕↔文档坐标映射 |
| 无障碍 | accesskit 树 + action | 缺把画布/网格内容暴露为语义节点的工具 |
| i18n | `i18n`（文本） | 缺 RTL 布局镜像 |
| 命令系统 | `Action`（序列化+分发） | 缺**命令注册表 + ribbon 位置/快捷键/参数元数据 + 动态 enabled** |
| 多窗格 | `DockArea`/`SplitPane` | 缺文档窗口(MDI)/任务窗格模式 |

> `wordcraft` 的 `ui-parity.md` 明示了目标 UI 需求：Ribbon tabs/groups/split/gallery/
> contextual、key tips(Alt/F10)、backstage/File tab、命令面板、各式对话框、任务窗格、标尺、
> 表格列行标记、选择栏等 —— 与上表一一对应。对话框深度也是大头
>（WordCraft 54 个模态 vs Word ~100 个）。

## 四、不该进框架（领域/引擎职责）

公式计算引擎、OOXML/ODF/RTF/PDF 解析、字体度量替代（metric-compatible substitution）、
拼写/语法/断词、分页/断行算法、色彩管理、打印/PDF 导出、各格式 I/O —— 都属于应用/领域层，
**不要塞进 rgpui 核心**。rgpui 只需提供能被这些引擎复用的底层：几何（EMU/pt）、字体与 shaping、
纹理/场景合成、无头捕获、命令分发。

## 五、建议路线图

**P0 地基（共用且完全缺失，先做这四件套）**
1. **命令注册表 + 多前端分发**：在 `Action` 之上加 registry（id/label/分类/快捷键/参数/启用条件），
   经统一入口分发到 UI、快捷键、命令面板、CLI、control channel、MCP。
2. **Ribbon 框架**：数据驱动渲染，命令 id 绑定，支持 tabs/groups/split/gallery/上下文/折叠。
3. **视口 Viewport**：可缩放/平移的画布容器 + 屏幕↔文档坐标映射（供页视图与网格共用）。
4. **标尺 + 对齐参考线/吸附**：通用工具库。

**P1 特征控件**：KeyTips（已落地 §十）、QAT / Mini toolbar（已落地 §十一、§十二）、Backstage、
公式栏、电子表格网格、文档页视图、画布对象工具、富剪贴板、富文本/文档编辑控件雏形、Ink。

**P2 深度**：缩略图/排序、动画时间线、取色器增强、任务窗格/MDI、a11y 画布语义、
bidi/RTL 与 CJK 排版。

**横向工程范式（可立即引入）**
- control channel（JSON-line TCP + key）+ MCP server 集成；
- 无头渲染/截图 API（不抢焦点的场景捕获）；
- 命令/功能 parity 清单 + gate 测试；
- 生产 crate 加 `deny(clippy::unwrap_used, expect_used, panic, todo, unimplemented)`；
- i18n catalog 审计 + RTL 镜像。

## 六、P0 落地：命令注册表（已实现）

P0 第一件（命令注册表）已落地，作为其余 P0 件（Ribbon/视口/标尺）的共同底座。

- **模块**：`crates/rgpui/src/commands.rs`
  - `CommandDescriptor`：命令 UI 元数据（`id`/`label`/`category`/`description`/`default_keystrokes`），
    构建方法 `with_category` / `with_description` / `with_keystroke(s)`，读取用同名 getter。
  - `CommandRegistry`：inventory 自动收集的全局单例（`CommandRegistry::global()`），
    按「分类 → 标签」排序，提供 `get`/`contains`/`iter`/`categories`/`len`。
  - `register_command!(fn_name)`：登记一个 `fn() -> CommandDescriptor` 函数项
    （inventory const 约束，不接受闭包）。
  - `App::dispatch_command(id, params)`：按 id 用 `App::build_action` 构建动作再
    `dispatch_action`，走常规焦点路由；UI、快捷键、命令面板、CLI、MCP 共用此入口。
- **命令面板接入**：`CommandPaletteState::from_registry()` /
  `CommandPalette::from_registry(window, cx)` / `Command::from_descriptor(&descriptor)`
  （选中时按 id 派发）。
- **示例**：`examples/showcase/src/bin/command_registry.rs`（`cargo run -p showcase --bin command_registry`）。
- **测试**：`commands::tests`（4）+ `command_palette::tests`（1）。

> 说明：命令 id 即动作名称，因此注册表与既有 Action 系统零冲突、零迁移成本；
> 图标/放置位置（Ribbon tab/group/QAT）字段待 Ribbon 落地时再加。

> **feature 门控**：套件基座整体是可选能力，普通应用不需要时不必编译。
> `ribbon` feature 门控 `commands.rs` / `ribbon.rs` / `keytips.rs` / `qat.rs` 与命令面板的
> 注册表接线；`viewport` feature 门控 `viewport.rs` / `guides.rs`。二者默认关闭，
> 示例 crate `showcase` 默认开启（保证 workspace check/test 覆盖），CI 另跑 feature 组合检查。

## 七、P0 落地：Ribbon 框架（已实现）

P0 第二件（Ribbon 框架）已落地，直接建立在命令注册表之上：命令项按**命令 id**绑定，
标签/快捷键未显式指定时从注册表读取，点击统一经 `App::dispatch_command` 派发。

- **模块**：`crates/rgpui/src/ribbon.rs`（`rgpui::ribbon::*`，不进根 glob，避免与用户命名冲突）
  - `RibbonState`：激活选项卡索引（`new`/`active_tab`/`set_active_tab(index, cx)`），
    为 `Entity`，父视图应 `observe` 它以在切换时重绘。
  - `RibbonTab`：`new(id, label).group(..).context(..)`。
  - `RibbonGroup`：`new(label).item(impl Into<RibbonItem>)`，底部显示组名。
  - `RibbonItem`：枚举 `Button`/`Toggle`/`Dropdown`/`Separator`，构造器
    `RibbonItem::button/toggle/dropdown/separator`；`RibbonButton`（`label`/`icon`/`large`）、
    `RibbonToggle`（`label`/`icon`/`checked`）、`RibbonDropdown`（`label`/`icon`/`item`/`large`）。
  - `Ribbon`：`new(&Entity<RibbonState>, Vec<RibbonTab>).collapsed(bool)`，`RenderOnce` 元素；
    选项卡条 + 激活 tab 内容；连续小按钮自动聚成每列 ≤3 的竖列。
  - `RibbonContext`：上下文选项卡的强调色（`new(label, color: impl Into<Hsla>)`）。
- **示例**：`examples/showcase/src/bin/ribbon.rs`（`cargo run -p showcase --bin ribbon`），
  含两个选项卡、大/小按钮、切换按钮、下拉按钮与上下文选项卡。
- **测试**：`ribbon::tests`（5：状态默认值、标签回退×3、绘制与切换选项卡冒烟）。

> 尚未实现（待后续 P1）：SplitButton、Gallery、ComboBox、折叠态交互、Backstage
> 等 Ribbon 系控件（按键提示见 §十，快速访问工具栏见 §十一，迷你工具栏见 §十二）。

## 八、P0 落地：视口 Viewport（已实现）

P0 第三件（视口）已落地，提供所有「画布/页视图/网格」共用的坐标系统与交互辅助。

- **模块**：`crates/rgpui/src/viewport.rs`（`rgpui::viewport::*`，不进根 glob）
  - `Viewport`：纯数据（`Copy`），持有 `zoom` + 文档原点偏移 `offset` 与缩放上下限。
  - **坐标映射**：`screen_to_document` / `document_to_screen` / `document_size_to_screen`
    （约定 `screen = document * zoom + offset`）。
  - **缩放**：`set_zoom_at(zoom, anchor)`（锚点对应的文档坐标不变）、`zoom_by_at`、
    `zoom_in_at` / `zoom_out_at`（步进 `ZOOM_STEP`）；`with_zoom_limits` 夹取上下限。
  - **平移/安置**：`pan_by(delta)`、`center(document, view)`、`fit(document, view, padding)`。
  - **滚轮**：`apply_wheel(&ScrollWheelEvent, line_height)`（`Ctrl/⌘` 以光标为中心缩放，
    否则平移）。
- **示例**：`examples/showcase/src/bin/viewport.rs`（`cargo run -p showcase --bin viewport`），
  A4 页视图：`canvas` 自绘，滚轮缩放/平移 + 左键拖拽平移 + 缩小/放大/100%/适应窗口。
- **测试**：`viewport::tests`（7：单位映射、缩放往返、平移、锚点缩放、上下限、fit/center）。

> 说明：rgpui 元素层不支持对任意子树施加仿射变换（仅 `Svg` 有 `Transformation`），
> 因此视口按「坐标映射 + 调用方自绘」设计：整页放缩建议用 `canvas` 绘制，
> 需要真正缩放控件子树的应用自行按 `document_size_to_screen` 换算尺寸。

## 九、P0 落地：标尺 + 对齐参考线/吸附（已实现）

P0 第四件（标尺/参考线）已落地，为纯计算工具库（无状态、无渲染），
页面/网格/画布控件共用同一套刻度与吸附规则。

- **模块**：`crates/rgpui/src/guides.rs`（`rgpui::guides::*`，不进根 glob）
  - **标尺**：`nice_step(min)` 取「好看」步长（1/2/5 × 10ⁿ）；
    `ruler_ticks(range, zoom, min_spacing_px)` 返回 `RulerTicks { step, ticks }`。
  - **吸附**：`align(moving, others, extra_x, extra_y, threshold) -> SnapResult`，
    按左/中/右与上/中/下六个锚点吸附到其它矩形或自由参考线，每轴取偏移最小的候选；
    返回位移修正 `offset` 与需绘制的 `Guide { axis, position, anchor }`。
  - 类型：`GuideAxis`（Vertical/Horizontal）、`Anchor`（Start/Center/End）、`Guide`、`SnapResult`。
- **示例**：`examples/showcase/src/bin/ruler.rs`（`cargo run -p showcase --bin ruler`），
  顶部标尺刻度 + 拖拽两个方块互相吸附并显示红色参考线（`Ctrl/⌘` + 滚轮缩放标尺）。
- **测试**：`guides::tests`（8：步长、刻度、零缩放、左边缘/最小偏移/超阈值/额外参考线/双轴吸附）。

## 十、P1 落地：按键提示 KeyTips（已实现）

P1 第一件（按键提示）已落地，补齐 Ribbon 系的键盘可达性：`Alt`/`F10` 后
先选选项卡、再选命令，全部走命令注册表派发。

- **模块**：
  - `crates/rgpui/src/keytips.rs`（`rgpui::keytips::*`，不进根 glob）：
    - `KeyTipScope`（Hidden/Tabs/Controls）与 `KeyTipMatch`（Match/Pending/None）；
    - `resolve_key(prefix, key, keytips)`：逐层解析，支持 `H` → `O` 这类多字符提示；
    - `keytip_visible`、`derive_keytip`（取标签首个 ASCII 字母/数字，中文标签不派生）；
    - `KeyTip`：提示小徽标元素（`RenderOnce`）。
  - `crates/rgpui/src/ribbon.rs`：`RibbonState` 增持 `FocusHandle` 与提示状态，
    提供 `show_keytips` / `hide_keytips` / `toggle_keytips` / `handle_key`；
    `Ribbon` 根节点 `track_focus` + `key_context("Ribbon")` + `on_action(ToggleKeyTips)`
    + `on_key_down`，并在选项卡/按钮/下拉/切换上渲染徽标；
    `ribbon::init` 绑定 `Alt`/`F10`（已并入 `init_all`）。
  - `crates/rgpui/src/commands.rs`：`CommandDescriptor::with_keytip` / `keytip()`，
    让提示键与标签/快捷键同源登记。
- **提示键来源**：显式覆盖 > 注册表登记 > 从标签首个 ASCII 字母派生。
- **示例**：`examples/showcase/src/bin/ribbon.rs`（`cargo run -p showcase --bin ribbon`），
  已为命令/选项卡登记提示键，按 `Alt`/`F10` 演示逐层键盘导航。
- **测试**：`keytips::tests`（7：单键/多字符前缀/精确优先/跳过空/大小写/可见性/派生）
  + `ribbon` 提示流程测试（选项卡选择、命令派发、Esc 退出、注册表提示优先）。

## 十一、P1 落地：快速访问工具栏 QAT（已实现）

P1 第二件（快速访问工具栏）已落地：一排按**命令 id** 绑定的紧凑图标按钮，
可放在标题栏、Ribbon 之上或状态栏，点击统一经 `App::dispatch_command` 派发。

- **模块**：`crates/rgpui/src/qat.rs`（`rgpui::qat::*`，不进根 glob）
  - `QuickAccessToolbar`：`new()` + `item(impl Into<QatItem>)` / `items(..)` 构建，
    横向 `h_flex` 排列，图标按钮 + 分组分隔符。
  - `QatItem`：`Button(QatButton)` / `Separator`，构造器 `QatItem::button(id)` /
    `QatItem::separator()`。
  - `QatButton`：`new(command)`（命令 id）+ `label(..)`（覆盖注册表标签）/ `icon(IconName)`。
  - 标签来源：显式覆盖 > 注册表 `CommandRegistry::label_for(id)` > 命令 id 本身。
- **注册表配套**：`CommandRegistry::label_for(id)`（未登记时回退为 id），
  Ribbon / QAT / 菜单等按 id 绑定的 UI 共用。
- **示例**：`examples/showcase/src/bin/qat.rs`（`cargo run -p showcase --bin qat`），
  顶部 QAT（保存/撤销/重做/分隔/查找/设置）+ 等价普通按钮 + 最近派发回显。
- **测试**：`qat::tests`（3：标签覆盖优先、注册表/ id 回退、零交互绘制）。

## 十二、P1 落地：迷你工具栏 MiniToolbar（已实现）

P1 第三件（迷你工具栏）已落地：选中文本/对象时就近浮出的紧凑命令条，与 QAT
共用 [`QatItem`] 与命令派发链路，仅多出「表面样式 + 绝对定位落点」。

- **模块**：`crates/rgpui/src/mini_toolbar.rs`（`rgpui::mini_toolbar::*`，不进根 glob）
  - `MiniToolbar`：`new()` + `item(impl Into<QatItem>)` / `items(..)`；`RenderOnce` 元素。
  - `at(Point<Pixels>)`：以绝对定位钉在父容器坐标（父容器需 `relative()`）；
    不调用则按普通流式元素渲染，由调用方自行布局。
  - 表面样式复用 `StyledExt::popover_style`（popover 底 + 边框 + 阴影 + 圆角）。
  - 项渲染与 QAT 共享 `qat::render_item`（按钮 / 分隔符）。
- **配套**：QAT 按钮按下时终止冒泡（`cx.stop_propagation()`），
  使浮动工具条上的点击不会打断父级的拖拽选择手势。
- **示例**：`examples/showcase/src/bin/mini_toolbar.rs`（`cargo run -p showcase --bin mini_toolbar`），
  在「文档」上拖动选择 → 就近浮出 B/I/U/高亮/复制，按钮切换状态并在状态条回显；`Esc` 取消选择。
- **测试**：`mini_toolbar::tests`（2：流式渲染、绝对定位渲染）。

## 十三、结论

- storytold 已证明「纯 Rust、引擎优先、UI 可换」路线可行，**rgpui 完全可以作为其 UI 基座**；
- 真正的缺口不在渲染/布局（rgpui 有），而在 **套件特征 UI（Ribbon 系）** 与
  **文档/画布/网格类控件**，以及**命令注册表**这一贯穿全栈的骨架；
- 建议先落 P0「命令 + Ribbon + 视口 + 标尺/参考线」，它们被四个应用共享，且是套件应用的
  身份特征；引擎类能力一律留在应用层。
