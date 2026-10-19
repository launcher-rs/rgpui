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
| **富剪贴板 payload** | `system_clipboard.rs` | 只有 text/image/路径（已扩展，见 §二十） | 扩展 `ClipboardItem` |

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

**P1 特征控件**：KeyTips（已落地 §十）、QAT / Mini toolbar（已落地 §十一、§十二）、
Backstage（已落地 §十三）、公式栏（§十六）、电子表格网格（§十七）、文档页视图（§十八）、
画布对象工具（§十九）、富剪贴板（§二十）、富文本/文档编辑控件雏形（§二十一）、Ink（§二十二）。

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

> Ribbon 系控件已补齐：SplitButton / Gallery / ComboBox / 折叠态交互（见 §十五）。

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

## 十三、P1 落地：应用菜单 Backstage（已实现）

P1 第四件（应用菜单）已落地：覆盖整窗的「文件」面板，左侧页面导航 + 右侧内容面板，
把「新建 / 打开 / 打印 / 选项 / 关于」这类**不属于任何功能区选项卡**的入口收进一处。

- **模块**：`crates/rgpui/src/backstage.rs`（`rgpui::backstage::*`，不进根 glob）
  - `BackstageState`：`Entity` 状态，`open`/`close`/`toggle`/`is_open` + `active`/`set_active`
    + `focus_handle`。
  - `BackstageTab`：`new(id, label)` + `icon(IconName)`；两种互斥模式——
    `command(id)`（点击后经 `App::dispatch_command` 派发并关闭）与
    `content(Fn(&mut Window, &mut App) -> AnyElement)`（自定义内容页）。
  - `Backstage`：`new(&Entity<BackstageState>, Vec<BackstageTab>)` + `sidebar_width`；
    `RenderOnce`，关闭态渲染为空元素，打开态铺满父容器（父容器需 `relative()`）。
  - `Esc` 关闭：面板根节点 `track_focus` + `key_context("Backstage")`，
    打开时由调用方 `window.focus(...)` 交付焦点。
- **边界**：不做页面内容的业务实现（模板列表、设置表单由应用层提供），
  也不加全局快捷键绑定（避免与 KeyTips / 应用自身键位冲突），入口由调用方提供。
- **示例**：`examples/showcase/src/bin/backstage.rs`（`cargo run -p showcase --bin backstage`），
  顶栏「文件」按钮展开面板，六个页面覆盖内容页与命令页，模板点击回写主视图。
- **测试**：`backstage::tests`（2：状态开关与页面切换、关闭态为空 / 打开态可绘制）。

## 十四、结论

- storytold 已证明「纯 Rust、引擎优先、UI 可换」路线可行，**rgpui 完全可以作为其 UI 基座**；
- 真正的缺口不在渲染/布局（rgpui 有），而在 **套件特征 UI（Ribbon 系）** 与
  **文档/画布/网格类控件**，以及**命令注册表**这一贯穿全栈的骨架；
- 建议先落 P0「命令 + Ribbon + 视口 + 标尺/参考线」，它们被四个应用共享，且是套件应用的
  身份特征；引擎类能力一律留在应用层。

## 十五、P1 落地：Ribbon 扩展控件（SplitButton / Gallery / ComboBox / 折叠）

在既有 Ribbon 框架上补齐四类 Office 常用控件，仍以命令 id 为唯一绑定方式：

- **分裂按钮 SplitButton**（`RibbonItem::split(command)`）：
  - 主区域派发主命令，右侧下箭头展开附加项（`RibbonDropdownItem`）；
  - 支持 `.label/.icon/.large/.keytip/.item(..)`；大按钮时主区域复用大按钮样式。
- **库 Gallery**（`RibbonItem::gallery(label)`）：
  - 触发按钮展开图标网格（`.columns(n)`，默认 4）；点击单元格派发其命令并关闭弹层；
  - `RibbonGalleryItem::new(command).label(..).icon(..).selected(..)`，选中项带强调边框。
- **组合框 ComboBox**（`RibbonItem::combobox(id)`）：
  - 只读下拉：展示当前值（`.selected(index)` / `.placeholder(..)`），选中项派发其命令；
  - `.width(px)` 固定宽度，`.item(RibbonComboBoxItem::new(command))` 追加选项。
- **折叠态交互**：
  - `RibbonState::is_collapsed` / `set_collapsed` / `toggle_collapsed`；
  - 选项卡条右侧渲染折叠开关（展开朝上、折叠朝下）；
  - 折叠态下点击选项卡会 `activate_tab` 自动展开并切换。
- **按键提示**：三类新控件均纳入 `control_commands`——分裂按钮命中主命令、
  库命中首项、组合框命中当前选中项（无选中取首项）。

- **示例**：`examples/showcase/src/bin/ribbon.rs` 增补分裂粘贴、样式库、插入组合框。
- **测试**：新增 4 项（折叠切换与选项卡展开、新控件按键提示、库/组合框提示回退、
  分裂按钮构建），`ribbon::tests` 共 12 项。

## 十六、P1 落地：公式栏 FormulaBar（`spreadsheet` feature）

新增独立 feature `spreadsheet`，首个落地模块 `crates/rgpui/src/formula_bar.rs` ——
电子表格编辑器顶部公式栏（名称框 + 取消/提交 + `fx` + 公式输入 + 展开），
**只负责编辑与事件转发，不含单元格逻辑**（引擎留在应用层）：

- **`FormulaBarState`**：持有两个 [`InputState`] 实体（名称框单行、公式框多行
  `auto_grow(1,6)`，均 `submit_on_enter(true)`），`new(window, cx)` 内 `subscribe_in`
  转发回车事件；方法 `name/formula`（读）、`set_name/set_formula/clear_formula`
  （写）、`expanded/set_expanded/toggle_expanded`（展开态）。
- **`FormulaBarEvent`**：`NameSubmit(SharedString)`（名称框回车，值去首尾空白）、
  `FormulaSubmit(SharedString)`（公式框回车或点 ✓）、`Cancel`（`Esc` 或点 ✕）、
  `InsertFunction`（点 `fx`）。
- **`FormulaBar` 元素**：`new(&Entity<FormulaBarState>)`，可调 `name_width`、
  `show_name_box`/`show_controls`/`show_function`/`show_expand`；根挂
  `key_context("FormulaBar")` + `on_key_down`（`Esc` 冒泡转 `Cancel`）。
- **示例**：`examples/showcase/src/bin/formula_bar.rs`（绑定 4×5 迷你单元格模型，
  名称框输入引用跳转、公式框回车写入、`fx` 插入 `=SUM(`、`Esc` 回退）。
- **测试**：5 项（名称/公式读写、展开切换、回车转发名称/公式、`Esc` 取消）。

后续「网格 + 文档页 + 画布对象 + 富文本/墨迹」等文档套件能力继续在
`spreadsheet`（或独立 feature）下落，均遵循「框架出控件与状态、引擎留应用层」。

## 十七、P1 落地：电子表格网格 SheetGrid（`spreadsheet` feature）

`crates/rgpui/src/sheet_grid.rs` —— 面向电子表格/日历/甘特等「二维单元格」场景的
专用网格：左上角为 `A1` 原点，**行列双轴虚拟化**（海量行列只渲染可见范围），支持
矩形选区与填充柄。与 [`DataTable`] 的区别：DataTable 面向「列定义 + 数据行」，
本控件面向「A/B/C 列 + 1/2/3 行」的电子表格语义，并把数据/公式计算留在应用层。

- **坐标与选区**：`CellRef { row, col }`（0 起）；`CellRange { anchor, cursor }`
  归一化端点（`min/max/rows/cols/height/width/len/contains/clamp`）；
  `column_label(col)` 纯函数（`0 -> A`、`25 -> Z`、`26 -> AA`）。
- **`SheetDelegate`**：`rows_count`/`cols_count` + 可选 `column_label`/`row_label`/
  `column_width`/`row_height`（统一行高，默认 24px）/`text`（供默认渲染与公式栏同步）/
  `render_cell`（默认渲染 `text`）。
- **`SheetGridState<D>`**：持有委托、`selection`、垂直（`uniform_list`）与水平
  （`h_virtual_list`，共享同一 `VirtualListScrollHandle`，列头为水平滚动驱动）两套
  滚动句柄、焦点、边界缓存。方法：`delegate/delegate_mut`、`selection/active_cell`、
  `set_selection/set_active_cell/select_row/select_col/select_all/move_active`、
  `editing/set_editing`、`scroll_to_cell`、`refresh`（重新读取行列数并夹取选区）。
- **`SheetGridEvent`**：`SelectionChanged(CellRange)`、`EditRequested(CellRef)`、
  `Clear(CellRange)`、`Fill { source, target }`、`HeaderClicked { col, row }`。
- **交互**：鼠标拖拽扩展选区、`Shift` 点击扩展；方向键 / `Shift`+方向 /
  `PageUp/PageDown`（按可视行数翻页）/ `Tab` / `Home` / `End` / `Ctrl+A` /
  `Ctrl+Home·End` 移动或全选；双击 / `Enter` / `F2` 请求编辑；
  `Delete`/`Backspace` 请求清空；行列头点击整行/整列；右下角填充柄拖拽填充。
  键盘在根元素 `on_key_down` 处理（不注册 Action），指针释放用
  `capture_any_mouse_up` 结束拖拽/填充。
- **`SheetGrid<D>` 元素**：`new(&Entity<SheetGridState<D>>)`，薄包装
  `key_context("SheetGrid")` + `track_focus` + 键盘/鼠标释放处理。
- **示例**：`examples/showcase/src/bin/sheet_grid.rs`（`SheetGrid` + `FormulaBar`
  绑定 8×6 迷你表格：选区同步公式栏、公式回车写回、填充柄复制）。
- **测试**：7 项（列标签映射、选区归一化/包含/夹取、读取行列数、选区夹取与事件、
  行列头事件、移动/扩展/全选、编辑态更新）。

**本版暂缓**（后续增补）：可变行高（`uniform_list` 要求等高，需换布局策略）、
冻结行列（对应 `DataTable` 的固定列）、单元格内联编辑器（编辑器留在应用层，
框架仅以 `EditRequested`/`editing` 暴露意图）。

## 十八、P1 落地：文档页视图 PageView（`document` feature）

新增 feature `document`（蕴含 `viewport`），首个落地模块
`crates/rgpui/src/page_view.rs` —— 面向文字处理/排版应用的「分页视口」：
按纸张/边距/页眉页脚/页间距在**文档坐标**里纵向排布纸页，屏幕映射复用
[`Viewport`]，**分页/断行算法留在应用层**（见 §四）。

- **`PageSize`**：`width`/`height`（px，96 dpi），常量 `A4_PORTRAIT`（794×1123）、
  `A4_LANDSCAPE`、`LETTER_PORTRAIT`；`landscape`/`portrait` 互换宽高、`to_size`。
- **`PageMargins`**：四边像素，常量 `NORMAL`（1in）/`NARROW`（0.5in）/`WIDE`（1.5in）。
- **`PageSetup`**：`size` + `margins` + `header_height` + `footer_height` + `gap`（默认
  A4 纵向 + 常规边距 + 48/48 + 24）。
- **`PageLayout`**（纯几何、可比较）：`new(setup, page_count)`；导出
  `page_stride`、`page_rect(page)`（整页）、`header_rect`/`footer_rect`/`content_rect`
  （页眉/页脚/正文区，文档坐标）、`document_size()`（含页间距不含末页后间距）、
  `page_at(document)`（命中页，间距归上一页、越界为 `None`）。
- **`PageView<L>` 元素**：`new(layout, viewport, labels)`，`PageLabels` trait 提供
  `header`/`footer` 文本（`()` 为默认空实现）；`.content_guide(bool)` 描边正文区
  （默认开）、`.area_guide(bool)` 描边页眉页脚区。纸页为白底 + 边框的绝对定位 div，
  子元素按 `viewport` 换算到屏幕后落位，故缩放时纸与页眉页脚同步。
- **示例**：`examples/showcase/src/bin/page_view.rs`（三页 A4 文档，正文按
  `content_rect` 叠加、字号随缩放同步；滚轮 Ctrl/⌘ 缩放、拖拽平移、适应窗口）。
- **测试**：6 项（默认设置、横纵互换、整页排布、内容区内缩、文档总尺寸、命中判定）。

**本版暂缓**（后续增补）：多列/对开页排布、真实分页（需应用侧断行）、页面缩略图。

## 十九、P1 落地：画布对象工具 object_toolkit（`viewport` feature）

`crates/rgpui/src/object_toolkit.rs` —— 面向绘图/画布编辑器的「对象操作 toolkit」：
以**文档坐标**矩形为对象边界，提供选区手柄、缩放、旋转、对齐与分布的纯几何函数，
屏幕映射复用 [`Viewport`]；`SelectionHandles` 元素把选区描边与 9 个手柄画到画布上。

- **`ResizeHandle`**：8 向缩放手柄 + `Rotate`；常量 `RESIZE`（8）/`ALL`（9），
  `is_rotate()`、`opposite()`。
- **几何函数**（均以文档坐标）：
  - `handle_point(rect, handle)` 手柄位置（旋转手柄在顶边上方 `ROTATE_HANDLE_DISTANCE`）；
  - `hit_handle(rect, point, tolerance)` 命中判定（旋转手柄最后判定）；
  - `resize_bounds(rect, handle, pointer, min_size)` 按手柄缩放，保留对侧锚点、尺寸不小于
    `min_size`（旋转手柄原样返回）；
  - `rotate_point(p, center, angle)` / `angle_between(center, from, to)` 绕点旋转与角度差；
  - `union_bounds(rects)` 外接矩形；
  - `align(rects, AlignMode)` 六种对齐（左/水平居中/右/顶/垂直居中/底）；
  - `distribute(rects, axis)` 沿轴等间距分布（≥3 个，两端不动）。
- **`SelectionHandles` 元素**：`new(selection, viewport)`，`handle_size(px)`、`rotate(bool)`；
  选区描边 + 手柄方块，缩放/平移时同步。
- **示例**：`examples/showcase/src/bin/canvas_objects.rs`（三张卡片：点选/拖动/手柄缩放/
  旋转手柄旋转 + 左·居中·右对齐与水平分布）。
- **测试**：9 项（手柄位置、命中容差、缩放锚点与最小尺寸、对侧手柄、绕点旋转、
  角度差、外接矩形、对齐、分布）。

**本版暂缓**（后续增补）：多选框选（marquee）、旋转后手柄跟随、吸附/参考线联动
（已有 `guides`）、图层顺序调整。

## 二十、P1 落地：富剪贴板 payload（核心，无 feature 门控）

`crates/rgpui/src/platform.rs` —— 在既有 `ClipboardItem`（纯文本/图像/文件路径）之上，
新增**带 MIME 类型的富负载**，让「一次复制、多种表示」成为一等能力：应用复制一段
带格式内容时同时写入纯文本与 HTML/RTF，不同目标程序各取所需；粘贴时能读回剪贴板上
所有可用格式。

- **`ClipboardFormat`**：`mime_type: String` + `bytes: Vec<u8>`，构造
  `new(mime, bytes)` / `new_text(mime, text)`，读取 `text()`（非 UTF-8 返回 `None`）。
  常用 MIME 常量：`PLAIN_TEXT`/`HTML`/`RTF`/`MARKDOWN`/`CSV`/`TSV`/`URI_LIST`。
- **`ClipboardEntry::Format(ClipboardFormat)`**：与既有 `String`/`Image`/`ExternalPaths`
  并列的第四种条目；新增 `From<ClipboardFormat>` 转换。
- **`ClipboardItem` 构造/读取**：`new_html(html, plain)`、`new_rtf(rtf, plain)`、
  `new_with_format`、`with_string`/`with_format`；`format(mime)`、`formats()`、`html()`、
  `rtf()`、`text()`。
- **平台映射**：
  - Windows（`rgpui-windows/src/clipboard.rs`）：`text/html` ↔ `CF_HTML`（"HTML Format"，
    写入时按规范拼 `Version/StartHTML/EndHTML/StartFragment/EndFragment` 头，读取时按偏移量
    解出片段）；`text/rtf` ↔ "Rich Text Format"；其余 MIME 以同名注册格式读写。
  - macOS（`rgpui-macos/src/pasteboard.rs`）：`text/html` ↔ `public.html`、
    `text/rtf` ↔ `public.rtf`，未知 MIME 按字符串注册 UTI；`write` 改为逐条目写入
    （原先多条目只写纯文本）。
  - Linux X11（`linux/x11/clipboard.rs`）：`set_item`/`get_item` 把整份项目写成多个
    TARGETS（纯文本、HTML/RTF、图像），自定义 MIME 运行时 `intern_atom` 并缓存；
    Wayland（`linux/wayland/clipboard.rs`）：数据源按内容 offer 富 MIME，`send` 按请求的
    MIME 回送对应字节。
- **示例**：`examples/showcase/src/bin/rich_clipboard.rs`（复制 HTML/RTF/CSV 三种项目，
  粘贴后列出剪贴板上每种格式与预览）。
- **测试**：`platform::clipboard_tests` 5 项（HTML/RTF/仅格式/非 UTF-8/构建器顺序）；
  `rgpui-windows::clipboard` 3 项（`CF_HTML` 往返、偏移量一致、缺标记返回 `None`）；
  `rgpui-windows::platform::tests::test_clipboard` 增加 HTML/RTF 经真实系统剪贴板往返。

**本版暂缓**（后续增补）：多条目 HTML 解析（表格/样式 → 文档模型，属应用层）、
剪贴板变化监听、Linux 主选择缓冲的富负载。

## 二十一、P1 落地：富文本 / 文档编辑控件雏形（核心，无 feature 门控）

`crates/rgpui/src/rich_text.rs` —— 面向文档/文字处理应用的**最小编排外壳**：把「段落
（`Paragraph`）+ 行内富文本运行（`Run`）」建模成一等文档类型，并提供字符坐标下的编辑
操作与 HTML 往返，让 WordCraft/DeckCraft 这类应用不必各自造一套模型。渲染侧提供只读
元素 `RichText`，编辑控件（光标/选区/IME）仍由应用用既有输入组件拼接。

- **文档模型**：`RichTextDoc { paragraphs }`；`Paragraph { style: ParagraphStyle, runs }`；
  `Run { text, props: RunProps }`。`RunProps`（`Copy`）含 `bold`/`italic`/`underline`/
  `strikethrough`/`color: Option<Hsla>`，构造器 `new`/`bold`/`italic`/`underline`/
  `strikethrough`/`color`/`is_plain`。`ParagraphStyle` 含 `align: TextAlign`、`indent`
  （0..=8 级）、`list: ListKind`（`None`/`Bullet`/`Number`）。
- **字符坐标编辑**：`locate(index)` 把平面字符索引映射到 `(段落, 段内偏移)`（段落间分隔符
  各计一个字符）；`insert_text`（含 `\n` 自动拆段）、`delete`（跨段自动合并）、
  `split_paragraph`/`merge_paragraph`、`apply_props(range, f)`（跨运行/跨段按选区拆分后逐段
  应用）、`normalize`（合并同属性相邻运行）、`char_len`/`plain_text`/`is_empty`。
- **序列化**：`from_plain_text`/`plain_text`（`\n` 连接）；`to_html`/`from_html`
  （`<p>`/`<ul>`/`<ol>`/`<li>` + `<b>`/`<i>`/`<u>`/`<s>` + `style` 中的
  `color`/`text-align`/`margin-left`/`font-weight`/`font-style`/`text-decoration`）；
  解析有 `MAX_HTML`/`MAX_PARAS`/`MAX_RUNS` 上限，非规范实体按原样保留；可直接对接富剪贴板
  `ClipboardItem::new_html(doc.to_html(), Some(doc.plain_text()))`。
- **渲染元素**：`RichText::new(doc)` 逐段落编译成 `StyledText`（行内属性映射到 `TextRun` 的
  字体粗细/斜体/下划线/删除线/颜色），按段落 `text_align` 与缩进排版，列表段以
  「• 」/「N. 」标记开头；`RenderOnce` + `From<RichTextDoc>`。
- **示例**：`examples/showcase/src/bin/rich_text.rs`（样例文档 + 按词命中套用样式 + 对齐/
  列表切换 + 复制为 HTML / 从剪贴板反解析）。
- **测试**：`rich_text::tests` 12 项（纯文本往返、段中插字拆运行、插换行拆段、跨段删除合并、
  拆/并段、跨段应用样式、HTML 往返含转义/下划线/删除线/颜色/列表/对齐/浏览器片段、空与超长
  HTML 拒绝、`normalize` 合并与保底一段）。

**本版暂缓**（后续增补）：节（section）/分栏、表格、批注、修订、脚注尾注、项目符号字体与
层级样式、可编辑光标与选区（现为只读渲染）。

## 二十二、P1 落地：Ink 笔迹层（核心，无 feature 门控）

`crates/rgpui/src/ink.rs` —— 面向白板 / 手写批注 / 手绘应用的**墨迹层**：以「采样点序列 +
画笔样式」为笔画，按工具编辑整个墨迹层；提供纯几何工具与把笔画平滑成路径的构建器，绘制由
`InkLayerElement` 承担。

- **模型**：`InkTool`（`Pen`/`Highlighter`/`Eraser`/`Lasso`）；`InkStyle { tool, color,
  width }`（`pen`/`highlighter`/`effective_color`，荧光笔叠加 `HIGHLIGHTER_ALPHA`）；
  `InkPoint { position, pressure }`；`Stroke { points, style }`；
  `InkLayer { strokes, pending }`。
- **落笔编辑**：`begin_stroke`/`begin_stroke_with_pressure`、`extend_stroke`（移动不足
  `MIN_SAMPLE_DISTANCE` 的点丢弃）、`end_stroke`/`cancel_stroke`、`undo`/`clear`/`is_drawing`。
- **选择与擦除**：`erase_at(center, radius)`（删除与圆相交的笔画）、`hit_test(position,
  tolerance)`（最近笔画）、`lasso(polygon)`（返回框选笔画索引）、`remove(&[indices])`、
  `bounds()`（全部笔画并集）、`simplify(tolerance)`（Douglas–Peucker 抽稀）。
- **纯几何**：`point_segment_distance`、`point_in_polygon`（射线法）、`polygon_bounds`、
  `simplify_points`；`stroke_path(points, width)` 用**中点二次贝塞尔**平滑折线（圆头/圆角），
  单点渲染成圆点。
- **绘制元素**：`InkLayerElement::new(layer)` + `.highlight(indices)`（套索选中以主题色重绘）
  + `.lasso(polygon)`（虚线套索框），逐笔画 `window.paint_path`，铺满父容器。
- **示例**：`examples/showcase/src/bin/ink.rs`（画布层 + 工具条：钢笔/荧光笔/橡皮/套索、
  撤销、删除选中、清空）。
- **测试**：`ink::tests` 11 项（落笔-提交-撤销、最小间距过滤、取消落笔、按半径擦除、套索
  选择与删除、命中判定、包围盒并集、抽稀保留共线/拐点、几何工具、路径构建与荧光笔透明度、
  压感夹取）。

**本版暂缓**（后续增补）：逐段压感变宽（现为等宽描边）、压感/倾角曲线拟合、笔迹分图层与
橡皮的局部切割、触摸/数位板事件接入（现由指针事件驱动）。
