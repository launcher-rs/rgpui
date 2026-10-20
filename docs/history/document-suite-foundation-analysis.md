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
| **缩略图 / 排序器** | `thumbs.rs`/`sorter.rs` | 无 | P2（已落地，见 §二十三） |
| **动画时间线** | `animtimeline.rs` | 无 | P2（已落地，见 §二十四） |
| **取色器增强**（吸管/最近颜色/主题色板） | `eyedropper.rs`/`fillui.rs` | 有 `ColorPicker`（已增强，见 §二十五；吸管缓做） | 增强 |
| **多文档界面 MDI**（子文档窗/层叠/平铺/最小化） | `doc_window.rs`/`dock.rs` | 有 `DockArea`（面板树） | P2（已落地，见 §二十六） |
| **任务窗格**（可停靠 / 可浮动侧边面板） | `dock.rs` | 有 `SplitPane`（分割） | P2（已落地，见 §二十七） |
| **富剪贴板 payload** | `system_clipboard.rs` | 只有 text/image/路径（已扩展，见 §二十） | 扩展 `ClipboardItem` |

### B. 现有能力需增强（有但不敷套件用）

| 维度 | rgpui 现状 | 缺口 |
|---|---|---|
| 文本/排版 | `text_system`（wrap/truncate、font-fallbacks/features、font-kit）+ `bidi`（段落基准方向 / start·end 对齐）+ `line_break`（CJK 禁则） | 缺核心层 UBA 字形重排（依赖平台 shaping）、拼音指南/纵横排版/文档网格、段落/分页布局、inline 域/对象 |
| 富文本编辑 | `input_ui` + `editor`（面向代码） | 缺文档模型（段落/节/表/批注/修订） |
| 表格 | `table::DataTable`（数据网格） | 缺电子表格的单元格/公式/循环引用模型 |
| 变换/缩放 | 元素级 `with_transformation` | 缺「可缩放平移视口」控件与屏幕↔文档坐标映射 |
| 无障碍 | accesskit 树 + action | 缺把画布/网格内容暴露为语义节点的工具（已落地，见 §二十八） |
| i18n | `i18n`（文本） | 缺 RTL 布局镜像（文本层已具备基准方向与 start/end 对齐，见 §二十九） |
| 命令系统 | `Action`（序列化+分发） | 缺**命令注册表 + ribbon 位置/快捷键/参数元数据 + 动态 enabled** |
| 多窗格 | `DockArea`/`SplitPane` | 缺文档窗口(MDI)/任务窗格模式（均已落地，见 §二十六、§二十七） |

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

**P2 深度**：缩略图/排序（§二十三）、动画时间线（§二十四）、取色器增强（§二十五）、
MDI 文档多窗（§二十六）、任务窗格（§二十七）、画布语义（§二十八）、国际文本
（bidi + 东亚禁则，§二十九）。

**横向工程范式**
- ✅ control channel（JSON-RPC 2.0 over TCP + key）+ MCP server 集成（§三十）；
- 无头渲染/截图 API（不抢焦点的场景捕获）—— 已有 `HeadlessAppContext` +
  `Window::render_to_image`（`test-support` 门控）：Linux 走 wgpu 离屏纹理，Windows 走
  D3D11 离屏渲染目标（临时换 target → 重画 → `CopyResource` 到 staging → Map 读回 →
  换回，BGRA→RGBA），macOS 尚未实现；
- ✅ 命令/功能 parity 清单 + gate 测试（`CommandRegistry::validate`，§三十一）；
- ⏳ 生产 crate 加 `deny(clippy::unwrap_used, expect_used, panic, todo, unimplemented)` ——
  按模块收紧，已覆盖 24 个套件模块（§三十一），全 crate 迁移未完成；
- ⏳ i18n catalog 审计 + RTL 镜像 —— catalog 审计与语言方向已落地（§三十一），
  外壳/图标镜像未做。

## 六、P0 落地：命令注册表（已实现）

P0 第一件（命令注册表）已落地，作为其余 P0 件（Ribbon/视口/标尺）的共同底座。

- **模块**：`crates/rgpui/src/commands.rs`
  - `CommandDescriptor`：命令 UI 元数据（`id`/`label`/`category`/`description`/`default_keystrokes`），
    构建方法 `with_category` / `with_description` / `with_keystroke(s)`，读取用同名 getter。
  - `CommandRegistry`：inventory 自动收集的全局单例（`CommandRegistry::global()`），
    按「分类 → 标签」排序，提供 `get`/`contains`/`iter`/`categories`/`len`，
    以及 `validate()`（清单自洽校验：id 唯一、标签/分类非空、快捷键可解析）。
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
> `ribbon` feature 门控 `ribbon.rs` / `keytips.rs` / `qat.rs` / `mini_toolbar.rs` / `backstage.rs`
> 与命令面板的注册表接线；`viewport` feature 门控 `viewport.rs` / `guides.rs` / `object_toolkit.rs`；
> `spreadsheet` / `document` feature 门控对应套件。命令注册表 `commands.rs` 是**核心**
> （UI 与外部自动化的共同底座，不随 feature 开关）。前两组默认关闭，示例 crate `showcase`
> 默认开启（保证 workspace check/test 覆盖），CI 另跑 feature 组合检查。

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
按纸张/边距/页眉页脚/页间距在**文档坐标**里排布纸页（默认单列纵向，可切对开 /
封面对开），屏幕映射复用
[`Viewport`]，**分页/断行算法留在应用层**（见 §四）。

- **`PageSize`**：`width`/`height`（px，96 dpi），常量 `A4_PORTRAIT`（794×1123）、
  `A4_LANDSCAPE`、`LETTER_PORTRAIT`；`landscape`/`portrait` 互换宽高、`to_size`。
- **`PageMargins`**：四边像素，常量 `NORMAL`（1in）/`NARROW`（0.5in）/`WIDE`（1.5in）。
- **`PageSetup`**：`size` + `margins` + `header_height` + `footer_height` + `gap`（默认
  A4 纵向 + 常规边距 + 48/48 + 24）。
- **`PageArrangement`**（本版增补）：`Single`（单列纵向，默认）/`TwoUp`（对开，每行
  两页）/`TwoUpCover`（封面对开，封面单独占首行右列，其后 1-2、3-4 ……）；
  `columns()` 给每行页数。`PageLayout::with_arrangement(arrangement)` 切换，
  `arrangement()` 读回。
- **`PageLayout`**（纯几何、可比较）：`new(setup, page_count)`；导出
  `page_stride`、`column_stride`（页宽 + 页间距）、`page_slot(page)`（网格行/列）、
  `row_count`、`page_rect(page)`（整页）、`header_rect`/`footer_rect`/`content_rect`
  （页眉/页脚/正文区，文档坐标）、`document_size()`（含页间距不含末页后间距，
  宽 = 列数 × 页宽 + 间距）、`page_at(document)`（命中页，间距归上一页/左页、
  空白格与越界为 `None`）。
- **`PageView<L>` 元素**：`new(layout, viewport, labels)`，`PageLabels` trait 提供
  `header`/`footer` 文本（`()` 为默认空实现）；`.content_guide(bool)` 描边正文区
  （默认开）、`.area_guide(bool)` 描边页眉页脚区。纸页为白底 + 边框的绝对定位 div，
  子元素按 `viewport` 换算到屏幕后落位，故缩放时纸与页眉页脚同步；纸页位置全部
  取自 `page_rect`，切换排布后自动跟随。
- **示例**：`examples/showcase/src/bin/page_view.rs`（三页 A4 文档，正文按
  `content_rect` 叠加、字号随缩放同步；滚轮 Ctrl/⌘ 缩放、拖拽平移、适应窗口；
  工具栏「排布」按钮在 单页 / 对开 / 封面对开 之间循环）。
- **测试**：8 项（默认设置、横纵互换、整页排布、内容区内缩、文档总尺寸、命中判定、
  对开排布、封面对开）。

**本版暂缓**（后续增补）：真实分页（需应用侧断行）、页面缩略图。

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
  - `handle_point_rotated(rect, angle, handle)` / `hit_handle_rotated(rect, angle, point, tol)`
    旋转后的手柄位置与命中（把点反向旋转回局部坐标再判定，看到哪点得到哪）；
  - `marquee_bounds(start, current)` 框选矩形（两端点归一化）、
    `marquee_hits(objects, marquee, MarqueeMode)` 框选命中（`Intersect` 相交即选 /
    `Contain` 完全包住），保持输入顺序；
  - `align(rects, AlignMode)` 六种对齐（左/水平居中/右/顶/垂直居中/底）；
  - `distribute(rects, axis)` 沿轴等间距分布（≥3 个，两端不动）。
- **`SelectionHandles` 元素**：`new(selection, viewport)`，`handle_size(px)`、`rotate(bool)`；
  选区描边 + 手柄方块，缩放/平移时同步（旋转态请用上面的 `*_rotated` 几何自绘）。
- **示例**：`examples/showcase/src/bin/canvas_objects.rs`（三张卡片：点选/拖动/手柄缩放/
  旋转手柄旋转 + 左·居中·右对齐与水平分布）。
- **测试**：12 项（手柄位置、命中容差、缩放锚点与最小尺寸、对侧手柄、绕点旋转、
  角度差、旋转手柄跟随、外接矩形、框选归一化与两种口径、对齐、分布）。

**本版暂缓**（后续增补）：吸附/参考线联动（已有 `guides`）、图层顺序调整。

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
  + `.lasso(polygon)`（虚线套索框）+ `.opacity(f32)`（整层不透明度，夹取 `0..=1`），逐笔画
  `window.paint_path`，铺满父容器。
- **多图层文档**：`InkDocumentLayer { name, layer, visible, locked, opacity }`
  （`is_editable` = 可见且未锁定、`fade(color)` 把不透明度乘到颜色上）；`InkDocument`
  按数组自底向上叠放，编辑落在激活图层：`layers`/`layer(idx)`/`visible_layers`/
  `layer_count`/`active_index`/`active`、`set_active`/`add_layer`/`remove_layer`（至少留一层）/
  `move_layer`（激活下标跟随）、`set_visible`/`set_locked`/`set_opacity`；
  绘制 `begin_stroke`/`begin_stroke_with_pressure`/`extend_stroke`/`end_stroke`/`cancel_stroke`、
  `undo`/`clear`/`simplify`（限激活层且可编辑），`erase_at`/`remove`（跨全部可编辑层）、
  `hit_test`（自顶向下首个可见层）、`lasso`（返回 `(图层, 笔画)`）、`bounds`（可见层并集）。
  应用逐层创建 `InkLayerElement::new(layer.layer.clone()).opacity(layer.opacity)` 叠加，
  隐藏层跳过即可。
- **示例**：`examples/showcase/src/bin/ink.rs`（画布层 + 工具条：钢笔/荧光笔/橡皮/套索、
  撤销、删除选中、清空；新建 / 切换 / 隐藏 / 锁定图层）。
- **测试**：`ink::tests` 17 项（原有 11 项：落笔-提交-撤销、最小间距过滤、取消落笔、按半径
  擦除、套索选择与删除、命中判定、包围盒并集、抽稀保留共线/拐点、几何工具、路径构建与
  荧光笔透明度、压感夹取；图层 6 项：默认单层、编辑跟随激活层且锁定/隐藏不可落笔、
  增删移动后的激活下标、隐藏/锁定层不参与命中擦除包围盒、不透明度夹取与淡化、套索与跨层
  删除、隐藏层跳过）。

**本版暂缓**（后续增补）：逐段压感变宽（现为等宽描边）、压感/倾角曲线拟合、
橡皮的局部切割、触摸/数位板事件接入（现由指针事件驱动）。

## 二十三、P2 落地：缩略图排序器（核心，无 feature 门控）

`crates/rgpui/src/sorter.rs` —— 面向幻灯片 / 页面 / 图片的**可拖拽重排缩略图网格**：等尺寸
网格自动换行、单选 / 范围选择 / 多选、缩放、键盘导航，以及拖拽重排（拖拽中在插入位置画一条
指示线）。框架只负责控件与交互，缩略图内容与项目顺序由应用通过 `SorterDelegate` 提供；重排
只抛出事件，真正移动数据由应用完成。

- **委托**：`SorterDelegate { count, aspect_ratio（默认 16:9）, render_item }`，`render_item`
  渲染填充单元格的任意内容，获得 `selected` 选中态与 `&mut Context<SorterState<Self>>`。
- **状态与元素**：`SorterState::new(delegate, cx)`（`Entity` 承载）+ `Sorter::new(&entity)`
  薄包装元素（`key_context("Sorter")` + 焦点 + 键盘 / 鼠标释放）。
- **选择**：`select`/`toggle`/`select_range`/`select_all`/`clear_selection`、`selected`/
  `primary`/`is_selected`/`selection_len`，全部经 `SorterEvent::SelectionChanged` 上报。
- **拖拽重排**：`begin_drag`/`set_drop_slot`/`update_drag(pointer)`/`finish_drag`/`cancel_drag`；
  鼠标按下进入拖拽并顺带完成选择（`Shift` 范围选、`Ctrl/Cmd` 切换多选），移动时按缓存的单元格
  边界求插入槽，释放时经 `SorterEvent::Reorder { from, to }` 上报。
- **视图配置**：`set_zoom`/`zoom_by`（夹取 `MIN_ZOOM..=MAX_ZOOM`）、`set_show_index`（序号角标）、
  `set_base_width`/`set_gap`、`scroll_to_item`（键盘导航自动滚入视野）；内部 `ScrollHandle` +
  `Scrollbar`。
- **键盘**：方向键按列数移动（`columns` 由可用宽度换算）、`Shift` 扩展、`Home`/`End`、
  `Ctrl/Cmd+A` 全选、`Enter`/`Space` 抛 `Activate`、`Delete`/`Backspace` 抛 `Delete`。
- **纯几何**：`clamp_zoom`/`normalize_aspect_ratio`、`cell_size`、`grid_columns`、`drop_slot`、
  `move_dest`（源位置自身或紧随其后视为无位移）、`move_group_dest`（整组落位，抽走选中项后
  索引前移）、`autoscroll_velocity`（贴边距离 → 每帧滚动量）。
- **边缘自动滚动**：`SorterState::drag_autoscroll` 按指针位置滚动 `ScrollHandle` 并重算插入槽，
  返回本帧增量（`0` 表示可停掉前台定时器）；组件本身不持有定时器。
- **多选整组拖动**：拖的是选中项之一且选中多于一项时，`finish_drag` 抛
  `SorterEvent::ReorderMany { from, to }`（`from` 升序，`to` 是抽走后的落位索引）。
- **示例**：`examples/showcase/src/bin/sorter.rs`（12 张彩色幻灯片卡 + 工具条：缩小/放大/新建/
  删除选中；重排后按事件真正调整 `slides` 顺序）。
- **测试**：`sorter::tests` 10 项（缩放夹取与宽高比归一化、单元格尺寸、换行列数、插入槽中线判定、
  移动目标换算、边缘自动滚动、整组落位、选择三种变体与越界忽略、拖拽重排 / 无位移 / 取消、
  视图配置）。

**本版暂缓**（后续增补）：拖拽实时预览（现为原格半透明 + 指示线）、虚拟化（大数量缩略图
全量渲染）、章节分组折叠。

## 二十四、P2 落地：动画时间线（核心，无 feature 门控）

`crates/rgpui/src/anim_timeline.rs` —— 面向动效 / 关键帧动画的**多轨时间线编辑器**：时间标尺 +
多条轨道 + 关键帧 + 可拖拽播放头。纯数据模型负责关键帧存储与求值，控件负责标尺刻度、命中测试、
拖拽与播放头；轨道可直接转成运行时的进度式 `KeyframeAnimation`，编辑完即交给元素动画播放。

- **数据模型**：`Keyframe { time, value }`（时间非负）；`Interpolation`（`Linear`/`Hold`/`Smooth`）；
  `Track { name, keyframes, interpolation }`；`Timeline { tracks }`。
- **轨道运算**：`normalize`（排序 + 同时间合并，保留最后一个）、`set_keyframe`（同时间覆盖并返回
  是否新增）、`remove_keyframe`、`keyframe_index_at(time, tolerance)`、`move_keyframe`（移动后重新
  排序并返回新下标）、`duration`、`value_at`（首帧前 / 末帧后保持端点值）；`Timeline::add_track`/
  `remove_track`/`move_track`（目标下标越界夹取）/`duration`（各轨最大值）/`values_at`。
- **运行时桥接**：`Track::to_keyframe_animation(id, duration)` 把绝对秒映射到进度 0~1（端点值显式
  写入，覆盖 `KeyframeAnimation::new` 自带的首尾帧）。
- **控件**：`TimelineState`（`Entity` 承载）+ `TimelineView`（`key_context("AnimTimeline")` + 焦点 +
  指针 / 滚轮 / 键盘事件）。播放头 `set_time`（吸附后夹取非负）/`advance(dt)`（循环回绕或停在末尾
  并停止）；`play`/`pause`/`toggle_play`/`set_looping`；`set_px_per_second`/`zoom_by`/`set_scroll`/
  `set_fps`/`snapped`；`select_keyframe`/`clear_selection`/`remove_selected_keyframe`；
  纵向 `vscroll`/`set_vscroll`（夹取到 `0..=max_vscroll()`）/`max_vscroll`/`scroll_to_track`；
  轨道 `move_track`（选中项与拖动中的关键帧跟随移动）。
- **交互**：`hit_test` 把局部坐标映射为 `TimelineHit`（`Ruler`/`Keyframe`/`Track`/`None`，行号按
  `vscroll` 换算成内容坐标）；标尺拖拽移动播放头、关键帧拖拽改时间（吸附）、轨道空白双击抛
  `KeyframeRequested`、`Ctrl/Cmd + 滚轮`以指针为锚缩放；`Shift + 滚轮`或轨道溢出时的滚轮纵向
  滚动，其余滚轮水平平移；键盘 `Space` 播放/暂停、方向键按帧步进（`Shift` 1s）、`Home`/`End`、
  `Delete` 删除关键帧、`+`/`-` 缩放。轨道与标签放进以标尺下沿为顶的裁剪容器，滚动时不会盖住
  标尺；`on_prepaint` 在窗口缩小时把 `vscroll` 夹回可滚动范围。
- **事件**：`TimelineEvent::{TimeChanged, SelectionChanged, KeyframeMoved, KeyframeRequested,
  KeyframeRemoved, PlaybackChanged, TrackMoved}`。
- **纯几何**：`time_to_x`/`x_to_time`、`snap_time`、`ruler_step`（1/2/5×10ⁿ 整齐步长）、
  `format_time`（`1.20s` / `1:02.5`）、`clamp_px_per_second`。
- **示例**：`examples/showcase/src/bin/timeline.rs`（六轨：位置 X / 缩放 / 透明度 / 旋转 / 模糊 /
  颜色，后三轨把轨道区撑到需要纵向滚动；工具条含播放、回到开头、加/删关键帧、缩放、吸附开关、
  轨道上移 / 下移；预览方块由前三条轨道的当前值驱动，播放时后台按 1/60s 推进）。
- **测试**：`anim_timeline::tests` 12 项（归一化与去重、三种插值求值、命中 / 移动 / 删除关键帧、
  时间映射与吸附 / 缩放夹取 / 标尺步长 / 时间格式化、总时长与当前值、进度式动画转换、播放头推进与
  循环 / 播放状态、缩放滚动吸附与选中删除、命中测试坐标映射、模型层轨道重排（目标夹取 / 原地不动）、
  状态层轨道重排（选中跟随 + `TrackMoved`）、纵向滚动（内容坐标命中 / 夹取 / 滚入视野）。

**本版暂缓**（后续增补）：轨道折叠、关键帧值曲线编辑器（当前只编辑时间）、多选关键帧与框选、
轨道静音 / 独奏、音视频波形与对齐吸附、播放时钟内置到控件（现由应用推进 `advance`）。

## 二十五、P2 落地：色板 / 取色器增强（核心，无 feature 门控）

`crates/rgpui/src/components/color_palette.rs` —— 面向「选一个预设色」的**色块网格**元素，并
用它给既有 `ColorPicker` 补上**主题色板**与**最近颜色**两组快捷色块。原取色器只有三滑块 + hex，
改一个由主题驱动的颜色要手抄十六进制；本版本把「当前主题的语义色」和「刚用过的颜色」铺成一格一格，
点一下即应用。

- **元素**：`ColorPalette`（`#[derive(IntoElement)]`，自动换行的等尺寸色块网格）。
  `new(id)`（id 需父级内唯一，用作子元素标识）/`colors`/`selected`/`swatch_size`/`gap`；
  点击回调 `on_select(F)` 或共享回调 `on_select_arc(Arc)`（同一份回调可复用给多个色板）。
  选中色块加描边、悬停变描边色，全部走主题令牌（`border`/`ring`）。
- **色板来源**：
  - `theme_palette(cx)`：当前主题的语义色（`primary`/`primary_hover`/`secondary`/`accent`/
    `danger`/`success`/`warning`/`info`/`link`/`red`/`green`/`blue`/`yellow`/`cyan`/
    `foreground`/`muted`/`border`/`ring`），换主题即换色板；
  - `color_name_palette(scale)`：`ColorName::all()`（19 个命名色）在指定色阶
    （`50`…`950`，越界回退 500）下的色板，供「调色盘」式取色。
- **最近颜色**：纯函数 `push_recent(&mut Vec<Hsla>, color, limit) -> bool`（去重、置顶、截断到
  `limit`，已在首位且未超限时返回 `false` 供调用方决定是否重绘），上限常量 `RECENT_COLORS_LIMIT`（12）。
- **取色器增强**（`components/color_picker.rs`）：
  - 新增字段与 API：`show_palette(bool)`（builder，默认显示）、`recent_limit(usize)`（最小 1）、
    `recents()`、`push_recent(color, cx)`、`clear_recents(cx)`、`set_color(color, window, cx)`
    （同步三滑块 + hex + 触发 `on_change`；滑块步长 1 只作近似指示，颜色以传入值为准）。
  - 渲染：预览块 / 三滑块 / hex 输入之后追加「主题色」与「最近」两段色板；点击色块即应用并记入
    最近颜色；hex 输入框回车视为提交，把当前颜色记入最近颜色；应用也可显式 `push_recent`（如对话框
    点「确定」时）。最近颜色为空时不渲染该段。
- **示例**：`examples/showcase/src/bin/color_picker.rs`（左：带色板的取色器 + 「记录当前颜色 /
  清空最近」按钮；右：预览卡（按亮度自动选前景色）+ 独立命名色阶色板，点击直接应用到取色器）。
- **测试**：`color_palette::tests` 3 项（最近颜色去重置顶 / 截断 / 重复无变化；命名色阶色板与
  `ColorName::all()` 一一对应且色阶回退；主题色板含语义色并完成一次真实绘制）+ `color_picker::tests`
  1 项（最近颜色去重置顶、`set_color` 同步当前颜色、`clear_recents`、`recent_limit` 下限与
  `show_palette` builder）。

**本版暂缓**（后续增补）：吸管取色（屏幕捕获）：`scap` / `screen-capture` feature 与
`zed-scap`、`windows-capture` API 不兼容，编译失败，故整块延后；取色器色域（RGB/HSV 面板 +
色环）与多色板自定义收藏。

## 二十六、P2 落地：多文档界面 MDI（核心，无 feature 门控）

`crates/rgpui/src/mdi.rs` —— 经典**多文档界面**：一块 MDI 区域内并列多个**子文档窗口**，
每个子窗口带标题栏（最小化 / 最大化 / 关闭）、八向缩放边框与可交互正文；支持层叠 / 平铺排布、
最小化图标条、z 序激活与键盘循环切换。与既有 `DockArea`（停靠面板树）/ `SplitPane`（分割）
互补：那两个解决「面板布局」，本模块解决「同一区域内多个文档窗」。

状态 [`MdiState`] 只持有**几何与元数据**（不持有视图），正文由 [`MdiArea::body`] 回调按窗口
id 现渲染，因此应用可随时增删文档、换主题、重排而无需重建 MDI 区域。

- **数据**：`MdiWindow { id, title, bounds, state, restore_bounds, closable/minimizable/maximizable }`
  （`windows` 按 z 序后 → 前存放，末尾在最上层，激活即「移到末尾」，无需单独 z 字段）；
  `WindowState { Normal, Minimized, Maximized }`；`MdiWindowFlags`（能力开关，默认全开）；
  `MdiFrame`（渲染快照：id / 标题 / 矩形 / 是否最小化 / 最大化 / 激活 / 三个能力位）。
- **操作**：`open`/`open_with`（返回 id，自动激活并抛 `MdiEvent::Opened`）、`close`/`close_active`/
  `close_all`、`activate`（提到最前 + `Activated`）、`cycle(forward)`（沿 z 序遍历，`Ctrl+Tab` 用）、
  `minimize`/`maximize`/`restore`/`toggle_maximize`（均抛 `StateChanged`）、`move_by`/`set_bounds`
  （夹取到可见范围）、`arrange(layout)`、`hit_test`、`rect_of`/`layout`/`get`/`windows`/`active`。
  builder：`with_min_size`/`with_cascade_offset`/`with_tile_gap`。
- **排布**（纯函数，可单测）：`cascade_rects`（错开层叠，尺寸取区域 60%×70%，越出右下角后回绕
  成第二圈）、`tile_rects`（`horizontal` 真为左右并排、假为上下堆叠，带间隔等分）、
  `clamp_rect`（不小于最小尺寸，且始终保留 [`MIN_VISIBLE`] 宽度与整条标题栏可见）、
  `icon_rect`（最小化图标沿底部自左向右排列）。区域未知时 `arrange` 先挂起（`pending_layout`），
  待视图预绘制拿到区域后一次性铺开；首次拿到区域还会把「区域未知期间开出的窗口」自动层叠，
  并随区域变化把普通窗口夹回可见范围。
- **命中与拖拽**：`MdiHit { None, TitleBar, Body, Icon, Button{id,button}, Resize{id,edge}, Tab(id) }`；
  `resize_edge_at`（角优先于边，6px 命中带）、`title_button_rects`（自右向左关闭 / 最大化 /
  最小化，28px 宽）、`resize_bounds`（按边 / 角缩放，拖左 / 上边时以右 / 下边为锚保证最小尺寸）。
  指针按下 → 激活并起拖拽会话（移动 / 缩放 / 按住按钮三类）；移动 / 缩放实时写回几何；
  释放时若仍命中同一按钮才执行（避免「按下后滑走仍触发」）。拖拽中每次变更只 `notify`，
  不做布局重排。
- **元素**：`MdiArea::new(&state).body(|id, window, cx| …)` —— 渲染每个窗口的标题栏（激活态
  用主题 `secondary` + `primary` 描边）、正文、标题栏按钮与 8 个缩放命中块（仅设光标，点击统一
  由容器命中测试处理，避免按钮吞掉拖拽）；最小化窗口渲染为底部图标（点击还原）；
  `on_prepaint` 同步区域、`on_mouse_down`/`on_mouse_move`/`capture_any_mouse_up` 转发指针、
  `track_focus` + `key_context("Mdi")` + `on_key_down` 处理键盘。`Arc<dyn Fn(…)>` 承载正文回调，
  元素本身不带泛型参数，便于 `#[derive(IntoElement)]`。
- **键盘**：`Ctrl+Tab` / `Ctrl+Shift+Tab`、`Ctrl+F6` / `Ctrl+Shift+F6` 循环切换；`Ctrl+F4` 关闭
  激活窗口；`Ctrl+M` 最小化激活窗口；`Esc` 取消当前拖拽。
- **示例**：`examples/showcase/src/bin/mdi.rs`（新建 / 只读视图 / 层叠 / 水平平铺 / 垂直平铺 /
  页签 / 全部最小化 / 关闭全部；窗口正文内嵌按钮验证「正文可交互 + 点击自动激活」）。
- **测试**：16 项 —— 纯几何 7 项（层叠与平铺落在区域内且铺满、夹取最小尺寸与标题栏可见、
  边 / 角判定与最小尺寸回退、标题栏按钮右对齐与图标条从左排列、吸附阈值内吸到边/中线、
  页签条等分与命中）；状态 9 项（打开 / 激活 / 关闭的 z 序与事件、最小化 / 最大化 / 还原矩形语义、
  能力开关禁用按钮、指针拖拽移动 + 缩放 + 按钮关闭、平铺 / 层叠排布且最小化不参与、无区域时
  排布挂起、键盘循环 / 最小化 / 还原 / 关闭、`move_by`/`set_bounds` 夹取、活动窗口吸附到边缘、
  页签式只渲染激活窗口且点击标签切换）。
- **吸附**：`snap_rect(rect, targets, threshold)` 把边/中线吸到工作区或邻窗的边/中线（每轴取
  最近一条，严格小于阈值才生效）；`MdiState::snap_active(threshold, cx)` 在松手时对活动窗口
  套用一次，`MdiState::with_snap(threshold)` 打开后由 `on_pointer_up` 在拖动移动结束自动调用
  （示例用 6px）。
- **页签式（Tabbed MDI）**：`MdiLayout::Tabbed` —— 顶部 `TAB_BAR_HEIGHT`（28px）标签条等分，
  其下正文区铺满；`MdiState::is_tabbed`/`tabs()`（`MdiTab { id, title, rect, active, closable }`，
  按创建顺序）、`tab_bar_rect`/`tabbed_content_rect`，几何 `tab_rects(bar, count)`/`tab_at`。
  `rect_in` 让普通窗口统一落在正文区，`layout()` 只下发激活窗口（最小化图标仍照常），
  `hit_test` 先判标签条（关闭区 `TAB_CLOSE_WIDTH` 命中为 `MdiHit::Button{Close}`，否则
  `MdiHit::Tab`），页签式下不产生缩放 / 拖动命中、`maximize` 为空操作；关闭或最小化激活窗口
  后自动切到下一个未最小化窗口。`MdiArea` 在窗口之上绘制标签条（激活标签用主题 `primary` 描边）。

**本版暂缓**（后续增补）：拖拽最小化图标、多显示器跨屏拖拽。

## 二十七、P2 落地：任务窗格（核心，无 feature 门控）

`crates/rgpui/src/task_pane.rs` —— 可停靠 / 可浮动的**侧边任务窗格**：默认贴在父容器左 /
右缘、占据固定宽度（内缘一条拖拽把手可改宽）；切到浮动态变成一块可拖动标题栏、八向缩放的
窗格。与既有 `DockArea`（面板树）/ `SplitPane`（分割）互补：那两个是容器级布局，本模块是
「单个可停靠 / 可浮动的属性面板」，正是 WordCraft 的 task pane 需求。

- **数据**：`TaskPaneSide { Left, Right }`（`grip_edge`：左停靠把手在右缘、右停靠把手在左缘）；
  `TaskPaneState`（侧 / 宽度 / 最小最大宽度 / 可见 / 浮动 / 浮动矩形 / `closable` / 窗格窗口矩形 /
  拖拽会话 / `FocusHandle`）；`TaskPaneHit { None, TitleBar, Close, Grip, Resize(edge), Body }`；
  `TaskPaneEvent { VisibilityChanged, SideChanged, WidthChanged, FloatingChanged }`。
- **操作**：`show`/`hide`/`toggle`/`set_visible`、`set_side`、`set_width`（夹取）、`set_closable`、
  `float(bounds)` / `dock` / `toggle_float(parent)`（浮动时贴父容器右下角内侧）、`set_rect`
  （预绘制回写）、`hit_test`、`on_pointer_down/move/up`、`on_key_down`（`Esc` 取消拖拽）。
  builder：`with_side`/`with_width`/`with_width_range`/`with_visible`/`with_closable`。
- **纯几何**：`clamp_width`（非有限值回落最小宽）、`resize_width`（拖左缘取负号 → 向左变宽）、
  `grip_rect`、`close_button_rect`、`floating_default_bounds`；浮动态的八向缩放直接复用
  `mdi` 的 `resize_edge_at`/`resize_edge_rects`/`resize_bounds`（`resize_edge_rects` 因此提为 pub）。
- **元素**：`TaskPane::new(&state).title(…).body(|window, cx| …)` —— 停靠态是流式子元素
  （`flex_none` + 固定宽 + `h_full` + 内缘边框 + 内缘把手），浮动态是 `absolute` 子元素
  （**父容器须为定位祖先** `relative()`）+ 八向缩放块；把手 / 按钮只设光标，命中与拖拽统一由
  容器层处理（与 MDI 同构，避免子元素吞掉拖拽）。`on_prepaint` 回写窗格矩形，指针 / 键盘事件
  经 `window.listener_for` 转发给状态；命中优先顺序：缩放边 / 角 → 内缘把手 → 关闭按钮 →
  标题栏 → 正文。
- **键盘**：`Esc` 取消当前拖拽。
- **示例**：`examples/showcase/src/bin/task_pane.rs`（显示 / 隐藏、停靠左 / 右、浮动 / 回停靠、
  加宽 / 收窄、关闭按钮开关；正文实时读取窗格宽度与停靠侧）。
- **测试**：8 项 —— 纯几何 2 项（宽度夹取与内缘拖拽换算、把手 / 关闭按钮 / 浮动默认矩形）；
  状态 6 项（显隐 / 换侧 / 改宽事件、浮动 / 停靠切换与最小浮动尺寸、停靠态两向把手拖拽改宽、
  浮动态标题栏平移 + 右下角缩放、关闭按钮「按下后滑走不触发」、真实绘制回写矩形且隐藏 / 浮动
  两态均可渲染）。

**本版暂缓**（后续增补）：多窗格并列（一屏多个任务窗格）、窗格吸附 / 自动隐藏（鼠标滑出收起）、
页签式窗格组、窗格内滚动位置记忆。

## 二十八、P2 落地：画布语义（核心，无 feature 门控）

`crates/rgpui/src/canvas_a11y.rs` —— 把自绘（canvas）内容暴露给辅助技术。自绘组件用
`Window::paint_*` 直接画像素，元素树里只有**一个**叶子节点，屏幕阅读器看不到「一格单元格」
「一笔笔迹」；本模块补上这段，并顺带让上游遗留的
`Element::a11y_synthetic_children`（此前**没有生产调用点**）有了真正的消费方。

- **模型**：`CanvasNodeId`（u64，应用自定义的稳定 id）、`CanvasNode`（`role`/`label`/
  `bounds`（画布局部坐标）/`value`/`description`/`disabled`/`selected`/`focused`/`actions`，
  builder 式构造 + 就地 setter）、`CanvasSemantics`（按绘制顺序存放的节点集合，含
  `with_row_tolerance`）。
- **纯几何 / 顺序**：`reading_order_indices`（**两趟**：先整体按 y，再逐行按 x —— 避免用带
  容差的比较函数做排序，那种比较不满足全序、标准库可能 panic）、`node_at_point`
  （**逆绘制序** = 后画者在上）、`next_in_order`（两端环绕）、`window_bounds`
  （局部 → 窗口坐标）；`CanvasSemantics::hit_test`/`reading_order`/`first_in_reading_order`/
  `next_in_reading_order`/`index_of`/`get_mut`。
- **无障碍桥**：`accesskit_node`（名称 / 值 / 说明 / `selected` / `disabled` / 动作位集 /
  矩形 —— 矩形按**缩放因子**换算成 AccessKit 要求的物理像素）；
  `push_canvas_a11y(builder, semantics, canvas_origin)`（在 `a11y_synthetic_children` 里把每个
  节点发布为合成子节点；标记为焦点的节点同时写父节点的 `active_descendant`）；
  `register_canvas_a11y_actions(window, global_id, semantics, handler)`（在 `paint` 里为每个
  「节点 × 动作」登记监听器，把辅助技术的 `Click`/`Focus`/… 路由回应用）。
- **框架补齐**：`A11ySubtreeBuilder::push_child_with_bounds`（合成节点也进本帧的
  「节点 → 矩形」表，于是内置的 `Click` 回退——向节点中心合成一次鼠标点击——对画布对象同样
  生效）、`A11ySubtreeBuilder::scale_factor`、自由函数 `synthetic_child_node_id`
  （在 `paint` 里复算与发布侧一致的合成子节点 id）、`A11y::scale_factor` 每帧同步。
- **元素接线**：`elements/canvas.rs` 的 [`Canvas`] 增加 `.id(..)` / `.a11y(role, semantics)` /
  `.a11y_label(..)` / `.on_canvas_a11y_action(..)`；`Element::id`/`a11y_role`/`write_a11y_info`/
  `a11y_synthetic_children` 全部接上，预绘制记下画布原点供发布时换算。
- **示例**：`examples/showcase/src/bin/canvas_a11y.rs`（六个自绘卡片 / 圆形；指针点击、键盘
  ← → ↑ ↓ / `Home` / `End` / `Enter` / `Esc`、无障碍动作三条路径等价；工具栏显示阅读顺序）。
- **测试**：7 项 —— 阅读顺序分行与导航环绕、命中测试（后画者在上 / 半开边界）、局部 → 窗口
  矩形、AccessKit 节点映射（状态 / 动作 / 物理像素矩形）、集合维护，以及 `window::a11y` 侧
  的桥接测试（合成子节点带矩形发布 + 焦点对象成为容器 active descendant）。

**本版暂缓**（后续增补）：把 `InkLayer` / `PageView` / `SheetGrid` 等既有自绘组件接到本模块
（当前只有示例消费）、行 / 列（表格）语义与 `Role::Cell` 的 `row_index`/`column_index` 扩展、
虚拟滚动下的语义节点裁剪与 `ScrollIntoView`。

## 二十九、P2 落地：国际文本 —— bidi 与东亚禁则（核心，无 feature 门控）

`crates/rgpui/src/bidi.rs` + `crates/rgpui/src/line_break.rs` —— 中西文混排最要紧的两件事：
**方向**与**断行**。此前核心层两者都没有：没有「这段文字是 LTR 还是 RTL」的概念（`TextAlign`
只有绝对方向的 `Left`/`Center`/`Right`），CJK 换行点也不遵守禁则，于是句号会跑到行首、
开引号会留在行末。

- **段落基准方向（`bidi.rs`）**：`TextDirection`（`Ltr` / `Rtl`）+ `TextDirection::resolve`
  （HTML `dir=auto` 口径 = UBA 的 P2 / P3 规则：取**首个强方向字符**，全段没有强方向字符时
  按 LTR 处理 —— 所以「纯阿拉伯数字」与浏览器一致地回退为 LTR，而不是让人误以为它是 RTL）；
  `TextDirection::is_rtl`；`strong_direction` / `is_rtl_char`（UBA 的 `L` / `R` / `AL` 三类，
  基于 `unicode-bidi` 的字符表，方向标记 `LRM` / `RLM` / `ALM` 自然落在其中）。纯 ASCII
  文本走快速路径直接判 LTR。
- **`start` / `end` 对齐**：`TextAlign` 新增 `Start` / `End` 变体 + `TextAlign::resolve(TextDirection)`
  （`Start` 在 LTR 段落解析为 `Left`、RTL 段落为 `Right`，`End` 相反；绝对方向原样返回）。
  解析点在**绘制入口**（`ShapedLine::paint` / `paint_background` 与 `WrappedLine::paint` /
  `paint_background`），方向取自**该行文本自身** —— 同一份布局在 LTR / RTL 段落上呈现相反的
  对齐，应用不必自己判断该用左还是右。其他消费方各自补臂：`rich_text` 导出 HTML 透传
  `text-align:start|end`，DOM 后端映射为 CSS `start` / `end`（浏览器按文字方向解析，RTL 文本
  自动右对齐），输入框内部按 LTR 处理（不做段落重排），`aligned_origin_x` 兜底按 LTR。
- **东亚禁则（`line_break.rs`）**：`BreakClass` + `break_class`（汉字 / 假名 / 小写假名 /
  长音符 / 句读闭标点 / 开括号开引号 / 谚文 / 单词 / 空白 / 其他）、`is_line_start_prohibited`
  （行首禁则）、`is_line_end_prohibited`（行末禁则）、`is_no_break`（胶连字符：不换行空格、
  词连接符、零宽连字…）、`can_break_between`（判定入口）、`is_ideographic`。规则**只覆盖
  CJK 与全角标点**，不改半角 ASCII 标点的既有行为（`can_break_between('a', ')') == true`），
  避免动到英文排版。
- **接入换行**：`LineWrapper::wrap_line`（字符级候选断点）与
  `LineLayout::compute_wrap_boundaries`（字形级候选断点）在设置候选点时调用
  `can_break_between(prev, cur)` **否决**不合规的断点 —— 于是 `。` 不再跑到行首、`「` 不再
  留在行末；当禁则导致无处可断时仍回退成硬断（与 CSS 溢出行为一致，宁可溢出也不违禁则）。
- **字形重排**（UBA 的完整实现：逐 run 方向 + 视觉序）仍属平台 shaping 的职责：Web /
  cosmic-text 后端内部已用 `unicode-bidi`，Windows DirectWrite 后端把整行交给
  `IDWriteTextLayout` 自行重排。核心层负责的是「方向是谁」与「断在哪」。
- **示例**：`examples/showcase/src/bin/intl_text.rs`（方向判定表 + 每项的首个强方向字符、
  `Left`/`Start`/`End`/`Right` 在 RTL 文本上的对照、窄容器里的禁则换行、`can_break_between`
  与 `break_class` 速查表）。
- **测试**：11 项 —— `bidi` 5 项（首个强方向字符 / 弱方向字符不决定方向 / 方向标记为强方向 /
  `start`·`end` 随方向翻转 / RTL 字符判定）、`line_break` 5 项（字符分类 / 禁则集合不动 ASCII /
  断行判定（禁则 + 胶连）/ 胶连字符集合 / 表意文字范围），另在 `line_wrapper` 增加 1 项
  换行集成测试（句号不起行、开引号不收行、汉字正常断行）。

**本版暂缓**（后续增补）：核心层的 UBA 重排与视觉序（需要逐 run 方向贯通 shaping）、
`TextStyle::text_direction` / 元素 `dir` 属性的**显式**方向覆盖（当前只有 `dir=auto` 式自动
解析）、镜像字符表（`unicode-bidi-mirroring`）、行首标点悬挂（hanging punctuation / 追い出し）、
纵向排版（`writing-mode: vertical-rl`）、CJK 的标点挤压与文档网格，以及**应用外壳的 RTL 镜像**
（滚动条侧、图标方向、整体布局方向）。

> 语言层面的方向与目录审计见 §三十一：`TextDirection::for_locale`（语言标签 → 默认方向）与
> `I18nManager::direction/validate`。外壳镜像仍未做。

## 三十、横向工程范式落地：控制通道 + MCP（`control-channel` feature）

`crates/rgpui/src/control_channel.rs` + `crates/rgpui/src/mcp.rs` —— 命令注册表的**外部前端**。
此前「命令 id 统一分发」只覆盖 UI / 快捷键 / 命令面板，CLI、脚本、AI agent 无路可走。

- **传输**：本机回环 TCP + 换行分隔的 JSON-RPC 2.0；每条请求带 `key`（默认随机 32 位十六进制，
  也可自定）。`ControlChannelOptions` 可配 `bind_addr` / `key` / `max_connections`（默认 4）。
- **线程模型**：TCP 读写在 Tokio 线程池（`rgpui::tokio`）上跑，动作派发必须回主线程 ——
  请求经 `RpcDispatcher` 投递到前台执行器的任务，由它调用 `App::dispatch_command`，
  结果再经 oneshot 送回连接。主线程无需任何加锁。
- **方法表**：`ping`、`app.info`（版本/平台/命令数/窗口数/焦点窗口）、`commands.list`、
  `commands.get`、`command.dispatch`（`{id, params}`）、`window.list`，以及 MCP 的
  `initialize` / `tools/list` / `tools/call`。错误码沿用 JSON-RPC（`-32700` 解析失败、
  `-32601` 未知方法、`-32602` 参数错误、`-32000` 派发失败、`-32001` 密钥不匹配）。
- **MCP 服务端**：stdio 传输最小子集（`initialize` / `tools/list` / `tools/call`）。
  一个命令 = 一个工具：工具名 = 命令 id 把 `[A-Za-z0-9_-]` 之外字符换成 `_` 并截断 64 字符
  （`editor::Save` → `editor__Save`），`inputSchema` 取自动作 JSON Schema
  （`App::action_schemas`），转义撞名时按 `_2` / `_3` 消歧（`tools/list` 与 `tools/call`
  共用同一套结果）；执行失败按 MCP 约定以 `isError: true` 回报，而不是 JSON-RPC 错误。
  协议版本支持 `2024-11-05` / `2025-03-26` / `2025-06-18`（回显客户端版本，未知则回最新）。
- **入口**：`ControlChannel::start(cx, options)` / `App::start_control_channel(options)`、
  `McpServer::start_stdio(cx)`；句柄丢弃即停止（accept 任务与前台风泵任务一起取消）。
  未初始化 Tokio 时自动补一次 `rgpui::tokio::init`。
- **示例**：`examples/showcase/src/bin/automation.rs` —— 控制台打印 `RGPUI_CONTROL_CHANNEL` /
  `RGPUI_CONTROL_KEY`（`--mcp` 时改打 stderr，保证 stdout 只有 JSON-RPC），窗口里显示地址、
  密钥与计数器；外部 `command.dispatch` 或 MCP `tools/call` 都能实时改计数。
- **测试**：19 项 —— 控制通道 9 项（密钥 / ping / 通知无回复 / 未知方法 / `commands.list` /
  `commands.get` 未知 id / 派发未知命令 / `app.info` / `window.list`），MCP 10 项
  （版本回显与回退 / 工具名转义与消歧 / 工具清单 / 未知工具 / 工具执行失败 / 通知 /
  非 MCP 方法透传 / 工具名唯一）。另用真实进程做过端到端验证：TCP 七种请求 + MCP stdio 三步握手。

**本版暂缓**：TLS / 远程绑定（当前只建议回环）、订阅式推送（事件流 / `resources` 能力）、
MCP 的 `prompts` / `resources` / `sampling`、控制通道的请求限流与审计日志。

## 三十一、横向工程范式落地：命令清单 gate + 目录审计 + lint 收紧

三件「让缺口不再复发」的小工程：

- **命令清单 gate**：`CommandRegistry::validate()` 返回问题清单（空 = 通过）：id 非空且唯一、
  标签非空、分类非空、默认快捷键可被 `Keystroke::parse` 解析。重复 id 在 `load()` 时只保留
  第一条（`by_id`）并记 `log::error!`，重复项仍留在清单里由 `validate()` 报出 ——
  应用在自己的测试里 `assert!(CommandRegistry::global().validate().is_empty())` 即可把
  「漏填分类 / 快捷键拼错 / 重复登记」挡在 CI 上（示例见 `automation` bin 的 `#[cfg(test)]`）。
- **i18n 目录审计与语言方向**：`I18nManager::validate()` 以所有语言 key 的并集为基准逐语言
  报缺失项（并检查当前语言 / 回退语言是否已加载，输出顺序稳定）；`TextDirection::for_locale`
  + `primary_language` + `is_rtl_locale` 提供「语言标签 → 默认方向」（CLDR 常用 RTL 语言，
  主语言子标签、大小写不敏感、认 `iw` / `ji` 历史代码），`I18nManager::direction/is_rtl`
  据此给出基准方向。示例 `intl_text` 增加「语言标签 → 方向 + 目录审计」小节。
- **生产代码 lint 收紧（按模块）**：新增 `clippy.toml`（测试内允许 `unwrap` / `expect` /
  `panic!`），并对 24 个套件模块在 `rgpui.rs` 的模块声明处加
  `#[deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]`
  （`commands` / `control_channel` / `mcp` / `bidi` / `line_break` / `i18n` / `rich_text` /
  `ink` / `sorter` / `anim_timeline` / `mdi` / `task_pane` / `canvas_a11y` / `viewport` /
  `guides` / `object_toolkit` / `formula_bar` / `sheet_grid` / `page_view` / `ribbon` /
  `keytips` / `qat` / `mini_toolbar` / `backstage`）。收紧只进不退：这些模块新增的
  `unwrap` 会直接编译失败；其余历史模块按同一口径逐步迁移。

**本版暂缓**：全 crate 的 `unwrap` / `expect` 迁移（约 1200 处，需按子系统分批）、
`unimplemented` 纳入 workspace deny、`TextStyle::text_direction` 显式方向与外壳 RTL 镜像。

## 三十二、收尾：暂缓功能补齐与完整性审计

### 补齐的暂缓项

| 模块 | 本轮新增 |
|------|----------|
| 缩略图排序器 `sorter` | `autoscroll_velocity`（指针贴边/越界时按距离给速度）、`drag_autoscroll`（拖拽中按速度推进滚动偏移）、`move_group_dest`（多选整组移动的目标槽位）、`SorterEvent::ReorderMany` |
| 画布对象工具 `object_toolkit` | `handle_point_rotated` / `hit_handle_rotated`（旋转后控制点）、`marquee_bounds` / `marquee_hits` + `MarqueeMode`（框选/交叉框选） |
| MDI `mdi` | `snap_rect` / `snap_active` / `with_snap`（拖动松手吸附到工作区边/中线）；`MdiLayout::Tabbed` 页签式（见 §二十六） |
| 页视图 `page_view` | `PageArrangement`（单页 / 双页 / 双页封面）与 `page_slot` 等栅格几何 |
| 动画时间线 `anim_timeline` | 纵向滚动（`vscroll`/`set_vscroll`/`max_vscroll`/`scroll_to_track` + 轨道区裁剪容器）、轨道重排（`Timeline::move_track` / `TimelineState::move_track` / `TimelineEvent::TrackMoved`） |
| Ink `ink` | `InkDocument` + `InkDocumentLayer` 多图层、`InkLayerElement::opacity`（见 §二十二） |

仍未做（保持暂缓）：时间线轨道折叠 / 值曲线、任务窗格多栏与自动隐藏、
富文本分节 / 表格 / 批注 / 脚注、笔迹逐段压感变宽、画布语义与自绘组件接线、bidi 视觉重排、
取色器吸管（屏幕捕获，`scap` feature 与 `windows-capture` API 不兼容）。

### AGENTS 完整性检查清单（12 项）逐条结果

1. `cargo check --workspace` 通过（含全部示例）。
2. `mouse_passthrough`、`WindowKind::Overlay`、Mica 变体、`tray.rs`、
   `single_instance/`（目录模块）都在。
3. `Platform` / `PlatformWindow` 自有方法本轮只增未删（新增 `render_to_image`、
   Linux `activate`/`hide`、终端 IME 等）。
4. `tray`、`desktop_pet`、`desktop_pet_3d` 随 workspace check 编译通过。
5. 中文注释保留；`#[allow(dead_code)]` **清零**：
   - macOS `display_link.rs` 删掉未使用的 `CVTimeStampFlags` / `CVSMPTETimeType` /
     `CVSMPTETimeFlags` 常量组；
   - macOS `global_hotkey.rs` 删掉未使用的 `NEXT_HOTKEY_ID` / `next_id` /
     `modifiers_to_nsevent` / `key_to_cgkeycode` 与配套 bitflags；
   - macOS `notifications.rs` 删掉未使用的 `request_permission`（权限统一走 `Platform`）；
   - web `WebEventListeners` 用 `Drop` 显式清空闭包，替代 `allow`（字段确实「只持有不读取」）；
   - wgpu 两个恢复字段只在非 WASM 使用，改为 `#[cfg_attr(target_family = "wasm", allow(dead_code))]`。
   AGENTS 规则同步为「平台专属字段用 `cfg_attr` 精确限定，不得整块放宽」。
6. 组件子系统目录齐全（`form` / `input_ui` / `menu` / `dialog` / `list` / `table` /
   `tabs` / `title_bar` / `elements`（含 `scroll`））。
7. 应用套件基座模块齐全，`ribbon` / `viewport` / `spreadsheet` / `document` /
   `control-channel` 开关两侧都能编译。
8. 未复活 `rgpui-ui` / `rgpui-tokio`；`rgpui-markdown` 在，`pulldown-cmark = "0.13"`。
9. feature 组合逐个 `cargo check -p rgpui --features …` 通过：
   `charts,effects,qr-code` / `tokio,control-channel` / `ribbon,viewport` /
   `spreadsheet` / `document` / `webview`；`cargo test -p rgpui-dom` 15 项通过。
10. `cargo publish -p rgpui --dry-run --registry crates-io` 通过。
11. 无 dev 循环依赖（测试放在消费方 `tests/`）。
12. 命令清单 gate：`CommandRegistry::validate()` 为空；`cargo test -p showcase --bin automation`
    通过。

已知波动（与本轮改动无关）：`util::scheduler::tests::test_nondeterministic_wake_allowed_with_parking`
单跑约 45 秒，`cargo test --workspace` 并行满载时会触发其内部 15 秒 park 超时断言，
重跑即过；属既有 flaky 用例。


