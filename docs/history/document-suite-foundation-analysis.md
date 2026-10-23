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
| **取色器增强**（吸管/最近颜色/主题色板） | `eyedropper.rs`/`fillui.rs` | 有 `ColorPicker`（已增强，见 §二十五；吸管已落地，见 §三十三） | 增强 |
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
- ✅ 核心 crate 生产代码 lint 收紧（第十六批，§七十九）—— `rgpui` 生产代码零
  `unwrap` / `expect` / `panic!`，由 `rgpui.rs` 顶部一条 crate 根
  `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` 封死；
  ⏳ 兄弟 crate（`rgpui-{windows,wgpu,macros,term,3d,platform}` 及需 CI 度量的
  `rgpui-{macos,linux}`）按同一口径继续，站点分布见 §七十九 的剩余候选表；
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

**本版暂缓**（后续增补）：剪贴板变化监听、Linux 主选择缓冲的富负载。

> 更新：核心富文本的 HTML 解析已能还原表格 / 节（见 §五十五），应用层据此把富剪贴板
> 载荷转成文档模型。

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

**本版暂缓**（后续增补）：脚注尾注（endnote）、项目符号字体与层级样式、
可编辑光标与选区（现为只读渲染）。

> 更新：节 / 分栏（`RichSection`）、表格（`RichTable`）、批注（`Annotation`）、脚注
> （`Footnote`）、修订标记（`RunProps.inserted` / `deleted`）均已落地，见 §五十二～§五十四。

## 二十二、P1 落地：Ink 笔迹层（核心，无 feature 门控）

`crates/rgpui/src/ink.rs` —— 面向白板 / 手写批注 / 手绘应用的**墨迹层**：以「采样点序列 +
画笔样式」为笔画，按工具编辑整个墨迹层；提供纯几何工具与把笔画平滑成路径的构建器，绘制由
`InkLayerElement` 承担。

- **模型**：`InkTool`（`Pen`/`Highlighter`/`Eraser`/`Lasso`）；`InkStyle { tool, color,
  width }`（`pen`/`highlighter`/`effective_color`，荧光笔叠加 `HIGHLIGHTER_ALPHA`）；
  `InkPoint { position, pressure }`；`Stroke { points, style }`；
  `InkLayer { strokes, pending }`。
- **落笔编辑**：`begin_stroke`/`begin_stroke_with_pressure`、`extend_stroke`/
  `extend_stroke_with_pressure`（移动不足 `MIN_SAMPLE_DISTANCE` 的点丢弃）、`end_stroke`/
  `cancel_stroke`、`undo`/`clear`/`is_drawing`。
- **选择与擦除**：`erase_at(center, radius)`（删除与圆相交的笔画）、`hit_test(position,
  tolerance)`（最近笔画）、`lasso(polygon)`（返回框选笔画索引）、`remove(&[indices])`、
  `bounds()`（全部笔画并集）、`simplify(tolerance)`（Douglas–Peucker 抽稀）。
- **纯几何**：`point_segment_distance`、`point_in_polygon`（射线法）、`polygon_bounds`、
  `simplify_points`；`stroke_path(points, width)` 用**中点二次贝塞尔**平滑折线（圆头/圆角），
  单点渲染成圆点。
- **压感变宽**：`pressure_width(base, pressure)` 把压感 0..=1 映射到基础宽度的
  `MIN_PRESSURE_SCALE`（0.35）..=1 倍；`stroke_paths(points, width)` 在压感非全满时按线段拆分，
  每段宽度取两端压感均值（圆头圆角自然衔接），压感全为 1（鼠标）或点数 ≤ 2 时退化为单条平滑
  `stroke_path`，不增加绘制开销。`InkLayerElement` 内部按 `stroke_paths` 逐条绘制；半透明笔画
  （荧光笔 / 整层不透明度）仍走等宽单路径，避免逐段圆头在接缝处叠加出「串珠」加深。
- **绘制元素**：`InkLayerElement::new(layer)` + `.highlight(indices)`（套索选中以主题色重绘）
  + `.lasso(polygon)`（虚线套索框）+ `.opacity(f32)`（整层不透明度，夹取 `0..=1`）
  + `.a11y(id, label)`（把笔画暴露给辅助技术），逐笔画 `window.paint_path`，铺满父容器。
- **无障碍**：`InkLayer::semantics()` 每笔生成一个 `Role::GraphicsObject` 节点（节点 id =
  笔画下标 + 1，值 = 点数，说明 = 工具与笔宽，包围盒按笔宽一半外扩以便单点笔画也能命中与
  排序）；绘制元素 `.a11y(..)` 后经由 `canvas_a11y` 发布为画布的合成子节点，高亮笔画标记为
  选中，阅读顺序按行 / x 排列。
- **多图层文档**：`InkDocumentLayer { name, layer, visible, locked, opacity }`
  （`is_editable` = 可见且未锁定、`fade(color)` 把不透明度乘到颜色上）；`InkDocument`
  按数组自底向上叠放，编辑落在激活图层：`layers`/`layer(idx)`/`visible_layers`/
  `layer_count`/`active_index`/`active`、`set_active`/`add_layer`/`remove_layer`（至少留一层）/
  `move_layer`（激活下标跟随）、`set_visible`/`set_locked`/`set_opacity`；
  绘制 `begin_stroke`/`begin_stroke_with_pressure`/`extend_stroke`/`extend_stroke_with_pressure`/
  `end_stroke`/`cancel_stroke`、
  `undo`/`clear`/`simplify`（限激活层且可编辑），`erase_at`/`remove`（跨全部可编辑层）、
  `hit_test`（自顶向下首个可见层）、`lasso`（返回 `(图层, 笔画)`）、`bounds`（可见层并集）。
  应用逐层创建 `InkLayerElement::new(layer.layer.clone()).opacity(layer.opacity)` 叠加，
  隐藏层跳过即可。
- **示例**：`examples/showcase/src/bin/ink.rs`（画布层 + 工具条：钢笔/荧光笔/橡皮/套索、
  撤销、删除选中、清空、压感笔开关（鼠标输入按轨迹长度合成压感）；新建 / 切换 / 隐藏 /
  锁定图层）。
- **测试**：`ink::tests` 19 项（原有 11 项：落笔-提交-撤销、最小间距过滤、取消落笔、按半径
  擦除、套索选择与删除、命中判定、包围盒并集、抽稀保留共线/拐点、几何工具、路径构建与
  荧光笔透明度、压感夹取；压感 1 项：宽度映射夹取 + 逐段拆分 + 压感采样保留；图层 6 项：
  默认单层、编辑跟随激活层且锁定/隐藏不可落笔、增删移动后的激活下标、隐藏/锁定层不参与
  命中擦除包围盒、不透明度夹取与淡化、套索与跨层删除、隐藏层跳过；无障碍 1 项：节点 id /
  点数 / 工具 / 外扩包围盒 / 阅读顺序）。

**本版暂缓**（后续增补）：倾角（tilt）拟合（模型尚无倾角字段）、橡皮的局部切割、
触摸 / 数位板事件接入（现由指针事件驱动）。

> 更新：压感曲线拟合（`smooth_pressure`）与 Catmull-Rom 曲线平滑（`catmull_rom_path`）
> 已落地，见 §五十八。

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

**本版暂缓**（后续增补）：虚拟化（大数量缩略图全量渲染）、章节分组折叠。

> 更新：拖拽实时预览（跟随指针的半透明副本）已落地，见 §六十。

## 二十四、P2 落地：动画时间线（核心，无 feature 门控）

`crates/rgpui/src/anim_timeline.rs` —— 面向动效 / 关键帧动画的**多轨时间线编辑器**：时间标尺 +
多条轨道 + 关键帧 + 可拖拽播放头。纯数据模型负责关键帧存储与求值，控件负责标尺刻度、命中测试、
拖拽与播放头；轨道可直接转成运行时的进度式 `KeyframeAnimation`，编辑完即交给元素动画播放。

- **数据模型**：`Keyframe { time, value }`（时间非负）；`Interpolation`（`Linear`/`Hold`/`Smooth`）；
  `Track { name, keyframes, interpolation, collapsed }`（`collapsed` 折叠时行高缩为
  `COLLAPSED_TRACK_HEIGHT`，`row_height` 取当前行高，`with_collapsed`/`set_collapsed` 设置）；
  `Timeline { tracks }`。
- **轨道运算**：`normalize`（排序 + 同时间合并，保留最后一个）、`set_keyframe`（同时间覆盖并返回
  是否新增）、`set_keyframe_value`（只改值、不重排，非有限值忽略）、`remove_keyframe`、
  `keyframe_index_at(time, tolerance)`、`move_keyframe`（移动后重新排序并返回新下标）、
  `value_range`（曲线值域：上下各留 `VALUE_RANGE_PADDING`=10%，空轨道 `(0, 1)`，全等值上下各留
  0.5）、`duration`、`value_at`（首帧前 / 末帧后保持端点值）；`Timeline::add_track`/
  `remove_track`/`move_track`（目标下标越界夹取）/`duration`（各轨最大值）/`values_at`。
- **运行时桥接**：`Track::to_keyframe_animation(id, duration)` 把绝对秒映射到进度 0~1（端点值显式
  写入，覆盖 `KeyframeAnimation::new` 自带的首尾帧）。
- **控件**：`TimelineState`（`Entity` 承载）+ `TimelineView`（`key_context("AnimTimeline")` + 焦点 +
  指针 / 滚轮 / 键盘事件）。播放头 `set_time`（吸附后夹取非负）/`advance(dt)`（循环回绕或停在末尾
  并停止）；`play`/`pause`/`toggle_play`/`set_looping`；`set_px_per_second`/`zoom_by`/`set_scroll`/
  `set_fps`/`snapped`；`select_keyframe`/`clear_selection`/`remove_selected_keyframe`；
  纵向 `vscroll`/`set_vscroll`（夹取到 `0..=max_vscroll()`）/`max_vscroll`/`scroll_to_track`；
  轨道 `move_track`（选中项与拖动中的关键帧跟随移动）、`toggle_track_collapsed`（行高变化后夹回
  纵向偏移）、`tracks_height`/`track_top`（按各行实际高度累加的轨道区几何）。
- **交互**：`hit_test` 把局部坐标映射为 `TimelineHit`（`Ruler`/`Keyframe`/`Track`/`None`，行号按
  `vscroll` 换算成内容坐标后逐行累加行高定位，折叠行整行算轨道空白）；标尺拖拽移动播放头、
  关键帧拖拽水平改时间（吸附）、垂直改值（值按行内纵坐标换算，按下时冻结值域并记住
  「指针值 → 关键帧值」偏移，拖动中曲线不随值域重算抖动、抓取不跳变）、轨道空白双击抛
  `KeyframeRequested`、`Ctrl/Cmd + 滚轮`以指针为锚缩放；`Shift + 滚轮`纵向滚动轨道，其余滚轮
  水平平移时间轴；点轨道名前的箭头折叠 / 展开轨道（点标签列不清空关键帧选择）；键盘 `Space`
  播放/暂停、方向键按帧步进（`Shift` 1s）、`Home`/`End`、`Delete` 删除关键帧、`+`/`-` 缩放。
  轨道与标签放进以标尺下沿为顶的裁剪容器，滚动时不会盖住标尺；`on_prepaint` 在窗口缩小时把
  `vscroll` 夹回可滚动范围。
- **事件**：`TimelineEvent::{TimeChanged, SelectionChanged, KeyframeMoved, KeyframeRequested,
  KeyframeRemoved, PlaybackChanged, TrackMoved, TrackCollapsed}`；`KeyframeMoved` 同时带
  `time` 与 `value`（纵向拖拽后的值）。
- **纯几何**：`time_to_x`/`x_to_time`、`snap_time`、`ruler_step`（1/2/5×10ⁿ 整齐步长）、
  `format_time`（`1.20s` / `1:02.5`）、`clamp_px_per_second`；值轴 `value_to_y`/`y_to_value`
  （行内上下各留 `VALUE_PADDING`=3px，越界夹到值域两端，互为逆映射）；值曲线 `curve_points`
  （逐段按 `CURVE_SAMPLE_SPACING`=6px 采样，段尾给 `to.time` 补点——`Hold` 因此得到竖直跳变
  而不是斜坡；总采样数受 `MAX_CURVE_SAMPLES`=512 限制），路径由内部 `polyline_path` 连成
  `CURVE_STROKE_WIDTH`=1.5px 圆头折线。
- **绘制**：每条展开且至少有 2 个关键帧的轨道在行内画一条值曲线（`canvas` 画布 + 80% 不透明度
  主题主色），关键帧方块压在曲线对应高度上（`value_to_y`，夹在行内不越界），选中帧用主色高亮；
  折叠行不画曲线与方块。
- **示例**：`examples/showcase/src/bin/timeline.rs`（六轨：位置 X / 缩放 / 透明度 / 旋转 / 模糊 /
  颜色，后三轨把轨道区撑到需要纵向滚动；工具条含播放、回到开头、加/删关键帧、缩放、吸附开关、
  轨道上移 / 下移 / 折叠；预览方块由前三条轨道的当前值驱动，播放时后台按 1/60s 推进）。
- **测试**：`anim_timeline::tests` 18 项（归一化与去重、三种插值求值、命中 / 移动 / 删除关键帧、
  改值与值域留白、值轴映射往返与夹取、曲线采样（首尾对齐 / 单帧与空轨退化 / `Hold` 竖直跳变）、
  时间映射与吸附 / 缩放夹取 / 标尺步长 / 时间格式化、总时长与当前值、进度式动画转换、播放头推进与
  循环 / 播放状态、缩放滚动吸附与选中删除、纵向拖拽改值（抓取偏移 / 冻结值域 / 事件带值）、
  命中测试坐标映射、模型层轨道重排（目标夹取 / 原地不动）、状态层轨道重排（选中跟随 +
  `TrackMoved`）、纵向滚动（内容坐标命中 / 夹取 / 滚入视野）、折叠行高与状态切换、
  折叠后行位 / 命中 / 可滚动范围 / `TrackCollapsed`）。

**本版暂缓**（后续增补）：多选关键帧与框选、轨道静音 / 独奏、音视频波形与对齐吸附、
播放时钟内置到控件（现由应用推进 `advance`）。

> 更新：值曲线的手柄拖拽此前**已实现**（状态里本就有 `dragging` / `set_keyframe_value`）；
> 曲线预设（`EaseIn` / `EaseOut` / `EaseInOut`）已落地，见 §六十。

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

**本版暂缓**（后续增补）：取色器色域（RGB/HSV 面板 + 色环）与多色板自定义收藏。

> 更新：吸管取色（屏幕捕获）已于 §三十三 落地（`screen-capture` feature 门控的
> `crate::eyedropper`，`ColorPickerState::begin_eyedropper` 接线）。

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
  `TaskPaneState`（侧 / 宽度 / 最小最大宽度 / 可见 / 浮动 / 浮动矩形 / `closable` / 页签
  `tabs` / 激活页签 `active_tab` / 自动隐藏 `auto_hide` / 收起 `collapsed` / 悬停 `hovered` /
  窗格窗口矩形 / 拖拽会话 / `FocusHandle`）；
  `TaskPaneHit { None, TitleBar, Tab(index), Close, Grip, Resize(edge), Body }`；
  `TaskPaneEvent { VisibilityChanged, SideChanged, WidthChanged, FloatingChanged, Collapsed, TabSelected }`。
- **操作**：`show`/`hide`/`toggle`/`set_visible`、`set_side`、`set_width`（夹取）、`set_closable`、
  `float(bounds)` / `dock` / `toggle_float(parent)`（浮动时贴父容器右下角内侧）、`set_rect`
  （预绘制回写）、`set_auto_hide` / `is_auto_hide`、`is_collapsed` / `set_collapsed` /
  `toggle_collapsed`、`set_hovered`（元素 `on_hover` 调用）、`effective_width`（收起时为
  `RAIL_WIDTH`）、`hit_test`、`on_pointer_down/move/up`、`on_key_down`（`Esc` 取消拖拽、
  `Ctrl+Tab` / `Ctrl+Shift+Tab` 循环切页签）。
  builder：`with_side`/`with_width`/`with_width_range`/`with_visible`/`with_closable`/`with_tabs`。
- **页签组**：`set_tabs`（列表变化时激活下标夹回）、`tabs`/`active_tab`/`active_tab_title`、
  `set_active_tab`（越界 / 同值返回 `false`，变化时抛 `TabSelected`）、`cycle_tab(delta)`（循环）、
  `tab_rects` / `tab_at`（页签几何与命中：标题栏内等宽平铺，右侧预留关闭按钮宽度，左起
  `TITLE_PADDING`）。标题栏有页签时渲染页签组（激活页签高亮并带下划线，正文由应用按激活页签
  切换），否则显示单个 `title`；收起轨道的竖排标题优先显示激活页签标题。
- **自动隐藏**：停靠态鼠标移出后收起为 `RAIL_WIDTH`（28px）宽的竖排标题轨道，移入即展开；
  收起时整条按 `TitleBar` 命中（不提供关闭 / 缩放），点轨道可展开（关闭自动隐藏后手动收起时
  的恢复入口），正文回调**不执行**；浮动态、隐藏态与关闭自动隐藏时保持展开，`dock()` /
  `set_visible(true)` 会按当前悬停状态重新套用。
- **纯几何**：`clamp_width`（非有限值回落最小宽）、`resize_width`（拖左缘取负号 → 向左变宽）、
  `grip_rect`、`close_button_rect`、`floating_default_bounds`；浮动态的八向缩放直接复用
  `mdi` 的 `resize_edge_at`/`resize_edge_rects`/`resize_bounds`（`resize_edge_rects` 因此提为 pub）。
  同侧多窗格：`normalize_stack_weight`（非有限 / 非正数回退 `1.0`，过小夹到 `MIN_STACK_WEIGHT`）、
  `resize_stack_weights`（按「位移 × 总权重 / 可用高度」换算，保持相邻两格权重之和不变、
  两格都夹到下限、其他格与总高度不动）。
- **多窗格堆叠**：`TaskPaneStackState`（各格权重 / 堆叠矩形 / 子元素数 / 分隔条拖拽会话）+
  `TaskPaneStack` 元素：子元素自上而下按 `flex_grow(weight)` 分高，相邻两格之间是
  `STACK_SEPARATOR_HEIGHT`（4px）的分隔条，拖动时按「起点权重 + 总位移」重算（避免逐帧累积
  误差），分隔条带 `cursor_row_resize` 光标、拖拽中高亮主题主色；`usable_height` 扣掉分隔条，
  预绘制回写矩形与子元素数。`with_weights`/`weights`/`weight`/`set_weights`/`set_weight`
  （越界补 `1.0`）/`begin_drag`/`update_drag`/`end_drag`。**隐藏的窗格不要加入**（元素会占位，
  应在构建子元素列表时过滤）。
- **元素**：`TaskPane::new(&state).title(…).body(|window, cx| …)` —— 停靠态是流式子元素
  （`flex_none` + 固定宽 + `h_full` + 内缘边框 + 内缘把手），浮动态是 `absolute` 子元素
  （**父容器须为定位祖先** `relative()`）+ 八向缩放块；把手 / 按钮 / 页签只设光标，命中与拖拽
  统一由容器层处理（与 MDI 同构，避免子元素吞掉拖拽）。`on_prepaint` 回写窗格矩形，指针 /
  键盘事件经 `window.listener_for` 转发给状态（页签点击在按下时直接 `set_active_tab`，不入
  拖拽会话），`on_hover` 维护悬停态；命中优先顺序：缩放边 / 角 → 内缘把手 → 关闭按钮 →
  页签 → 标题栏 → 正文（收起时直接 `TitleBar`）。
- **键盘**：`Esc` 取消当前拖拽；`Ctrl+Tab` / `Ctrl+Shift+Tab` 循环切页签。
- **示例**：`examples/showcase/src/bin/task_pane.rs`（显示 / 隐藏、停靠左 / 右、浮动 / 回停靠、
  加宽 / 收窄、关闭按钮开关、自动隐藏开关、下一个页签；标题栏是「属性 / 大纲」页签组，正文
  实时读取窗格宽度与停靠侧并随页签切换内容）。
- **测试**：14 项 —— 纯几何 4 项（宽度夹取与内缘拖拽换算、把手 / 关闭按钮 / 浮动默认矩形、
  页签等宽平铺与右侧预留、堆叠权重换算与夹取）；状态 7 项（显隐 / 换侧 / 改宽事件、浮动 / 停靠
  切换与最小浮动尺寸、停靠态两向把手拖拽改宽、浮动态标题栏平移 + 右下角缩放、关闭按钮「按下后
  滑走不触发」、堆叠分隔条拖拽（起点重算 / 下限 / 结束清理）、真实绘制回写矩形且隐藏 / 浮动
  两态均可渲染）；自动隐藏 1 项（悬停展开 / 移出收起、收起态命中与有效宽度、浮动与关闭自动隐藏
  时保持展开、`Collapsed` 事件）；页签 1 项（点击命中 / 关闭按钮优先 / 事件 / 循环切换 / 越界
  与缩减夹取）；堆叠渲染 1 项（2:1 权重 → 200 / 100px、分隔条占位、第二格依次下移）。

**本版暂缓**（后续增补）：窗格内滚动位置记忆、跨侧拖拽停靠（把窗格从一侧拖到另一侧）。

## 二十八、P2 落地：画布语义（核心，无 feature 门控）

`crates/rgpui/src/canvas_a11y.rs` —— 把自绘（canvas）内容暴露给辅助技术。自绘组件用
`Window::paint_*` 直接画像素，元素树里只有**一个**叶子节点，屏幕阅读器看不到「一格单元格」
「一笔笔迹」；本模块补上这段，并顺带让上游遗留的
`Element::a11y_synthetic_children`（此前**没有生产调用点**）有了真正的消费方。

- **模型**：`CanvasNodeId`（u64，应用自定义的稳定 id）、`CanvasNode`（`role`/`label`/
  `bounds`（画布局部坐标）/`value`/`description`/`disabled`/`selected`/`focused`/`actions`，
  builder 式构造 + 就地 setter，`with_bounds`/`set_bounds` 改矩形）、`CanvasSemantics`
  （按绘制顺序存放的节点集合，含 `with_row_tolerance`、`fill_empty_bounds`）。
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
  `a11y_synthetic_children` 全部接上，预绘制记下画布原点与矩形供发布时换算。宽高都为 0 的
  语义节点在发布前由 `CanvasSemantics::fill_empty_bounds` 展开成整块画布——走势图 / 波形这类
  「整体摘要」节点在构建时还不知道自己的布局大小，正好用这条规则。
- **已接线组件**：`InkLayerElement::a11y`（每笔一个 `GraphicsObject`）、`Sparkline::a11y(id, label)`
  （摘要：数据点数 / 最新 / 最低 / 最高 + 样式与趋势）、`Waveform::a11y(id, label)`
  （摘要：采样点数 / 峰值 + 播放进度）、仪表盘图表家族（`charts` feature）：
  `PieChart::a11y(id, label)`（摘要：扇区数 / 总计 + 占比降序列举，零值剔除、最多 5 项，两变体
  `Pie` / `Donut` 共用）、`Gauge::a11y(id, label)`（摘要：格式化数值 + 当前百分比，钳到量程）、
  `RadarChart::a11y(id, label)`（摘要：轴数 / 数据集数 + 按轴序列出数值，超长截断、空数据退化）、
  `TreeMap::a11y(id, label)`（摘要：顶层项数 / 总计 + 总值降序列举，子节点计入父项）、
  `LineChart::a11y(id, label)`（摘要：折线数 / 总点数 + 逐系列 y 极值）、
  `AreaChart::a11y(id, label)`（摘要：系列数 / 总点数 + 逐系列 y 极值）、
  `Chart::a11y(id, label)`（摘要：系列数 / 总点数 + 逐系列类型与 y 最值，走 y 轴刻度格式化器）。
- **示例**：`examples/showcase/src/bin/canvas_a11y.rs`（六个自绘卡片 / 圆形；指针点击、键盘
  ← → ↑ ↓ / `Home` / `End` / `Enter` / `Esc`、无障碍动作三条路径等价；工具栏显示阅读顺序）。
- **测试**：7 项 —— 阅读顺序分行与导航环绕、命中测试（后画者在上 / 半开边界）、局部 → 窗口
  矩形、AccessKit 节点映射（状态 / 动作 / 物理像素矩形）、集合维护与零尺寸节点填充，以及
  `window::a11y` 侧的桥接测试（合成子节点带矩形发布 + 焦点对象成为容器 active descendant）；
  组件侧另有走势图 / 波形摘要节点 2 项 + 图表摘要节点 7 项（饼 / 仪表盘 / 雷达 / 树图 /
  折线 / 面积 / 通用坐标轴，`charts` feature 门控）。

**本版暂缓**（后续增补）：虚拟滚动下的语义节点裁剪 / `ScrollIntoView`。

> 更新：`Role::Cell` 行 / 列语义已落地（`DataTable` / `SheetGrid` 的 `Role` 与
> `aria_row_index` / `aria_column_index`），见 §五十九。注：`SheetGrid` / `PageView` 本身全部经
div / 虚拟列表渲染、原生进入无障碍树（单元格还带 `sheet-cell:{r}:{c}` 元素 id），
**画布语义接线不适用** —— 后续方向是给这些 div 标注结构化语义，而非画布语义。

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
| 动画时间线 `anim_timeline` | 纵向滚动（`vscroll`/`set_vscroll`/`max_vscroll`/`scroll_to_track` + 轨道区裁剪容器）、轨道重排（`Timeline::move_track` / `TimelineState::move_track` / `TimelineEvent::TrackMoved`）、轨道折叠（`Track::collapsed` / `row_height` / `with_collapsed` / `set_collapsed`、`tracks_height` / `track_top` / `toggle_track_collapsed`、`TrackCollapsed`、折叠箭头）、值曲线与纵向改值（`Track::value_range` / `set_keyframe_value`、`value_to_y` / `y_to_value` / `curve_points`、`KeyframeMoved.value`、冻结值域 + 抓取偏移） |
| 任务窗格 `task_pane` | 自动隐藏（`auto_hide`/`collapsed`/`hovered` + `RAIL_WIDTH` 竖排标题轨道 + `set_auto_hide`/`set_collapsed`/`toggle_collapsed`/`set_hovered`/`effective_width`、`TaskPaneEvent::Collapsed`）、页签组（`tabs`/`active_tab`/`with_tabs`/`set_tabs`/`set_active_tab`/`cycle_tab`/`tab_rects`/`tab_at`、`TaskPaneHit::Tab`、`TaskPaneEvent::TabSelected`）、多窗格堆叠（`TaskPaneStack`/`TaskPaneStackState`、`weights`/`set_weight(s)`、`begin_drag`/`update_drag`/`end_drag`、`STACK_SEPARATOR_HEIGHT`、`resize_stack_weights`/`normalize_stack_weight`） |
| Ink `ink` | `InkDocument` + `InkDocumentLayer` 多图层、`InkLayerElement::opacity`（见 §二十二）、逐段压感变宽（`pressure_width` / `stroke_paths` / `extend_stroke_with_pressure`）、画布语义接线（`InkLayer::semantics` / `InkLayerElement::a11y`，每笔一个 `GraphicsObject` 节点） |
| 走势图 / 波形 `sparkline` `waveform` | 画布语义接线（`Sparkline::a11y` / `Waveform::a11y` 整体摘要节点：点数 / 极值 / 峰值 / 播放进度）、`CanvasSemantics::fill_empty_bounds`（零尺寸摘要节点按画布大小发布）、`CanvasNode::with_bounds`/`set_bounds` |
| 仪表盘图表 `components/charts`（`charts` feature） | 画布语义接线：`Gauge::a11y` / `PieChart::a11y`（含 `Donut` 变体）/ `RadarChart::a11y` / `TreeMap::a11y` / `LineChart::a11y` / `AreaChart::a11y` / `Chart::a11y` 各自发布一个整体摘要节点（扇区占比 / 进度 / 轴值 / 顶层占比 / 逐系列极值，零值剔除、超长截断）；div 点阵渲染的 `DonutChart` / `Heatmap` / `BarChart` 本身有子节点，无需接线 |

仍未做（保持暂缓）：富文本分节 / 表格 / 批注 / 脚注、bidi 视觉重排
（取色器吸管已于 §三十三 增补落地）。

> 备注：此前暂缓的「更多自绘组件接线画布语义」至此收官——canvas 绘制的图表家族已全部接入
> （走势图 / 波形 / 饼 / 仪表盘 / 雷达 / 树图 / 折线 / 面积 / 通用坐标轴），
> div 点阵渲染的 `DonutChart` / `Heatmap` / `BarChart` 与 div / 虚拟列表渲染的
> `SheetGrid` / `PageView` 原生进入无障碍树，画布语义接线对其不适用
> （后续方向见 §二十八 暂缓：`Role::Cell` 行列语义标注）。

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

## 三十三、增补：屏幕取色（吸管，`screen-capture` feature 门控）

此前吸管因「`screen-capture` feature 已知编译失败」整块暂缓（§二十五）。实测该阻塞已过期：
feature 在 Windows 本地编译通过（AGENTS 的「不要启用」记录系历史结论，已更正），于是把
rucap 里成熟的取色浮层移植进 rgpui。

### 流程（rgpui 侧无单帧 API，故「截一张图」= 推流取首帧后立即停流）

1. `open_eyedropper(cx, on_result)`：不支持屏幕捕获时直接回 `Failed`；否则主线程拿到
   捕获源 oneshot 接收端 + 前台 / 后台执行器句柄，`App::spawn` 起前台异步任务。
2. 选主显示器源（`SourceMetadata::is_main`，取不到回退第一个源）→ `source.stream` 推流，
   捕获线程回调把首帧 `frame.to_rgba()` 存进共享槽位（`Mutex<Option<Arc<RgbaImage>>>` +
   `AtomicBool`，只留第一帧）。
3. 等待推流启动（最多 5 秒，50ms 轮询）→ 显式 `drop(stream)` 停流 → 全程**留在内存**，不落盘。
4. RGBA → BGRA（与 rgpui img 解码口径一致）构造 `RenderImage`，回主线程开全屏冻结浮层：
   `WindowKind::Overlay` + `WindowBounds::Fullscreen(主屏 bounds)` + `Opaque` 背景（X11 上
   alpha 合成不可用会导致 ARGB 浮层整体不可见，与 rucap 区域截图浮层同规格）。
5. 浮层视图：冻结帧 1:1 铺满 + 十字线 + 9×9 放大镜（`canvas` 逐格绘制，中心格白描边，
   面板贴光标右下越界翻左上）+ 顶部 HEX/RGB 信息条；坐标严格按「视图逻辑像素 × 缩放比 =
   图片物理像素」换算。左键 `Picked(Hsla)`，右键 / `Esc` `Cancelled`；结果回调
   `Fn(EyedropperResult, &mut App)` 恰好一次（视图侧 `finished` 标志防重复）。

### 接线

- 新模块 `crates/rgpui/src/eyedropper.rs`（feature 门控，`#[deny(unwrap/expect/panic)]`）：
  `open_eyedropper` + `EyedropperResult`（`Picked(Hsla)` / `Cancelled` / `Failed(SharedString)`）；
  纯函数 `sample_at` / `rgb_to_hex` / `mag_cell`（放大镜越界钳制）随模块带 2 项测试。
- `ColorPickerState`（feature 门控接线）：新增 `pending_pick: Option<Hsla>` 私有字段 + 
  `begin_eyedropper`（weak 回写 `pending_pick` + `notify`）+ 取色面板「屏幕取色」小按钮；
  render 侧在滑块同步前先 `take()` `pending_pick` → `set_color` + `push_recent`
  （取色回调运行在浮层上下文，没有来源窗口的 `&mut Window`，与 `hex_dirty` 同一套延后机制）。
- feature 关闭时字段 / 方法 / 按钮整块消失，取色器行为与之前完全一致。

### 验证

- `cargo check -p rgpui --features screen-capture --lib` 与默认 feature 侧均通过；
  `cargo clippy` 双侧 `-D warnings` 通过。
- `cargo test -p rgpui --features screen-capture --lib` 802 passed + 1 ignored（含
  `eyedropper::tests` 2 项：采样越界与 HEX 格式、放大镜角落钳制）。
- `cargo check --workspace` 通过；`cargo fmt --all` 无变更。

## 三十四、增补：Markdown LaTeX 公式（`rgpui-markdown` 的 `latex` feature）

### 背景：rumark 调研结论

调研 `D:\code\rumark`（基于已发布 rgpui 的块式 Markdown 编辑器，约 5.5 万行）后确认：
其**编辑器内核是自建的 WYSIWYG 块模型**（`InlineTextTree` + 每块直接 `impl
EntityInputHandler`，完全绕开 `input_ui::InputState`），与 `RichTextDoc` / `InputState`
不兼容，**不能整体移植**。可独立移植的只有与块模型弱耦合的三层：

| 层次 | 内容 | 规模 | 判定 |
|------|------|------|------|
| P1 | LaTeX 渲染管线（ratex 体系） | ~250 行 | **本批落地** |
| P1 | Mermaid 完整渲染（`mermaid-rs-renderer`） | ~765 行 | 后续批（核心已有 flowchart 子集 div 渲染，需先定「两套并存」口径） |
| P2 | tree-sitter 15 语言高亮包 | ~815 行 | 后续批（并入核心 `tree_sitter.rs` / `highlight`） |
| P2 | 块 ↔ 源码偏移映射（source map） | ~1700 行 | 规划项（需在 `RichTextDoc` 模型上重写） |
| P3 | 表格在位编辑 / 大纲工作区 / 导出 | ~10000 行 | 不搬（应用层，核心已有 `DataTable` / `SheetGrid` / `Tree` 对应能力） |

### 落地实现

- **依赖**：`ratex-parser` / `ratex-layout` / `ratex-svg`（`embed-fonts`，含 `standalone`），
  均为纯 Rust、无系统依赖；仅在 `latex` feature 下启用。
- **管线**（`crates/rgpui-markdown/src/latex.rs`）：`ratex_parser::parse` → `ratex_layout::layout`
  → `ratex_layout::to_display_list` → `ratex_svg::render_to_svg`（`embed_glyphs` 把字形转成
  **路径轮廓**，usvg 不认识 KaTeX 字体，只能走轮廓）→ 核心 `SvgRenderer`（usvg / resvg）
  光栅化为 `RenderImage`。
- **尺寸口径**：ratex 以 `pt` 标注尺寸而 usvg 按 1pt = 4/3px 换算，直接光栅化会与排版字号
  脱节；因此先把根元素 `pt` 改写为 `px`，再以改写后的数值作为逻辑尺寸（显式设 `w` / `h`，
  不依赖图片自身的缩放约定）。
- **着色**：ratex 默认黑色填充按主题前景色替换（`rgba(0,0,0,1)` → 主题色）。
- **缓存**：按「源码 + 颜色 + 字号」哈希缓存光栅化结果（上限 64 条），长文档里同一公式
  只光栅化一次。
- **模型**：`RichInline::Math`（行内 `$...$`）与 `RichBlock::Math`（展示 `$$...$$`）加入块模型；
  行内公式按「文本段 + 公式图片段」切分后混排（段落内多段文本各自唯一元素 id，避免无障碍
  树重名），展示公式居中排布。
- **降级**：未启用 `latex` feature、没有 SVG 渲染器、或公式解析失败时，一律退回等宽源码
  文本，不中断整篇渲染。
- **接线**：`Markdown` 组件在 `latex` feature 下取 `cx.svg_renderer()` 走
  `render_blocks_with_svg`；`render_blocks` / `render_blocks_with` 旧签名保持不变（公式按
  文本降级）。

### 验证

- `cargo check -p rgpui-markdown --features latex` 与默认 feature 侧均通过；
  `cargo clippy -p rgpui-markdown --all-targets -- -D warnings` 双侧通过。
- `cargo test -p rgpui-markdown --features latex --lib` 10 passed（公式 5 项：SVG 生成 /
  尺寸 / 非法 LaTeX / 主题着色 / 缓存键；解析 1 项：行内与展示公式入模）；默认 feature 侧
  4 passed。
- `cargo check --workspace` 通过；`cargo fmt --all` 无变更；
  `cargo publish -p rgpui-markdown --dry-run --registry crates-io --features latex` 通过。
- 示例：`examples/rgpui_story` 新增「LaTeX 公式」页（该示例 crate 启用 `latex` feature）。

## 三十五、增补：Mermaid 图表渲染（`rgpui-markdown` 的 `mermaid` feature）

承接 §三十四 的调研结论，把「可独立移植三层」中的第二层落地。

### 与核心 `MermaidDiagram` 的分工

rgpui 核心 `components/mermaid.rs` 已有 `MermaidDiagram`：**flowchart 子集 + div 全彩渲染**
（不走 SVG 管道、可直接用 div 排版，但只支持 `flowchart` 与有限形状 / 边）。本模块走的是
**完整 Mermaid 语法 + SVG 渲染**，两者并存、定位不同：

| | 核心 `MermaidDiagram` | 本模块（`mermaid` feature） |
|---|---|---|
| 语法 | `flowchart` 子集 | 完整 Mermaid（渲染器支持范围内） |
| 渲染 | div 元素（彩色、可交互、无需光栅化） | SVG → usvg 光栅化 → 图片 |
| 入口 | 直接构造组件 | Markdown 围栏代码块（` ```mermaid `）自动识别 |
| 依赖 | 无（核心自带） | `mermaid-rs-renderer`（可选依赖） |

### 落地实现

- **依赖**：`mermaid-rs-renderer 0.3`（`default-features = false`，去掉 CLI 与 PNG 输出，
  只用 SVG 渲染路径），纯 Rust、无系统依赖。
- **管线**（`crates/rgpui-markdown/src/mermaid.rs`）：`mermaid_rs_renderer::render_with_options`
  → SVG → 核心 `SvgRenderer` 光栅化 → `RenderImage`；与公式管线共用 `svg_image.rs`
  （尺寸解析 / 单位改写 / 光栅化）。
- **识别**：代码块语言为 `mermaid` 或 `mmd`（大小写不敏感，可带 info 串）时走图表路径；
  自定义代码块渲染器（`CodeBlockRenderer`）优先级更高，仍可覆盖。
- **配色**：按 rgpui 主题背景亮度自动选 `Theme::dark()` / `Theme::modern()`，深色主题下
  图表不会白底刺眼。
- **尺寸**：按 SVG 固有尺寸光栅化（内部 2 倍超采样），超出 600px 宽时按比例缩放展示。
- **缓存**：按「源码哈希 + 深浅配色」缓存（上限 32 条）。
- **降级**：解析失败 / 无 SVG 渲染器时退回普通代码块，不中断整篇渲染。

### 验证

- `cargo check -p rgpui-markdown` 四种 feature 组合（默认 / `latex` / `mermaid` /
  `latex,mermaid`）均通过；`cargo clippy -p rgpui-markdown --all-targets -- -D warnings`
  四态通过；`rgpui_story` clippy 通过。
- `cargo test -p rgpui-markdown --lib`：默认 4 passed / `latex` 11 / `mermaid` 9 /
  `latex,mermaid` 15 全 passed（图表 4 项：语言识别 / SVG 渲染 / 非法源码报错 / 缓存键）。
- `cargo check --workspace` 通过；`cargo fmt --all` 无变更。
- 示例：`examples/rgpui_story` 新增「Mermaid 图表」页（该示例 crate 启用 `mermaid` feature）。

### 后续（§三十四 调研表的剩余项）

- tree-sitter 15 语言高亮包并入核心 `tree_sitter.rs` / `highlight`。
- 块 ↔ 源码偏移映射（source map）：需在 `RichTextDoc` 模型上重写，属规划项。

## 三十六、增补：tree-sitter 多语言包 + Markdown 代码块真实高亮

承接 §三十四 调研表的 P2 项：把 rumark 的多语言高亮能力并入 rgpui 核心的
`highlight` 扩展点（而不是移植它那套自建的高亮器）。

### 核心：14 个语言包（各一个 feature）

核心 `highlight` 模块早就有扩展点（`register_highlighter` + `highlight_for` + 按语言
构造器），本轮按「三步法」补齐语言包：

| feature | 语言 | grammar crate | 备注 |
|---|---|---|---|
| `tree-sitter` | Rust | `tree-sitter-rust` | 既有地基 |
| `tree-sitter-json` | JSON | `tree-sitter-json` | 既有 |
| `tree-sitter-toml` | TOML | `tree-sitter-toml-ng` | 既有 |
| `tree-sitter-javascript` | JavaScript | `tree-sitter-javascript` | 含注入 / 局部查询 |
| `tree-sitter-typescript` | TypeScript / TSX | `tree-sitter-typescript` | 两种语言同一查询 |
| `tree-sitter-python` | Python | `tree-sitter-python` | |
| `tree-sitter-bash` | Bash | `tree-sitter-bash` | |
| `tree-sitter-go` | Go | `tree-sitter-go` | |
| `tree-sitter-c` | C | `tree-sitter-c` | 查询名为单数 `HIGHLIGHT_QUERY` |
| `tree-sitter-cpp` | C++ | `tree-sitter-cpp` | 内置查询较稀疏（仅 C++ 专有词） |
| `tree-sitter-c-sharp` | C# | `tree-sitter-c-sharp` | 发布包未含注入 / 局部查询 |
| `tree-sitter-java` | Java | `tree-sitter-java` | |
| `tree-sitter-ruby` | Ruby | `tree-sitter-ruby` | |
| `tree-sitter-php` | PHP | `tree-sitter-php` | |
| `tree-sitter-css` | CSS | `tree-sitter-css` | 无符号（大纲为空） |
| `tree-sitter-html` | HTML | `tree-sitter-html` | 含注入查询 |
| `tree-sitter-yaml` | YAML | `tree-sitter-yaml` | 无符号 |

要点：

- **查询常量命名不一致**：各 grammar crate 暴露的常量名不统一（`HIGHLIGHT_QUERY` /
  `HIGHLIGHTS_QUERY`），且部分 crate 未发布查询文件（`tree-sitter-md` 无 `queries/`，
  因此本轮不提供 Markdown 语言包）。
- **符号大纲复用一套映射**：各语言节点名差异大，用统一的
  `common_symbol_kind` / `common_symbol_name`（按 `name` 字段取名）覆盖
  函数 / 类 / 结构体 / 枚举 / 接口 / 模块 / 常量等；CSS / HTML / YAML 无符号大纲。
- **捕获名改写**：主题名表（`HIGHLIGHT_NAMES`）按「点分部件」匹配（`string.escape`
  能命中 `string.escape.special`），但裸名命中不了多段名；新增 `rewrite_captures`
  把各语言查询里的非标准捕获名映射到名表（`escape` → `string.escape`、
  CSS at-rule → `keyword` 等）。改写器是**状态机**：字符串字面量、字符类
  `[…]`、注释里的 `@` 不改写（CSS 的 `"@media"` 不会被误伤）。
- **别名**：`highlighter_for` 接受常见别名（`js` / `ts` / `py` / `sh` / `yml` / `c++` 等）。

### rgpui-markdown：代码块接入真实高亮

- 新增 feature `syntax-highlight`（蕴含核心 `tree-sitter` 地基 + `ropey`）。
- `CodeBlock` 按围栏语言查核心高亮器：命中则按字节区间把高亮运行裁剪到各行渲染
  （颜色 / 字重 / 斜体 / 背景），未命中回退原有的简易分词高亮。
- **缓存**：高亮结果按「语言 + 代码内容哈希 + 主题语法色哈希」缓存（上限 32 条），
  避免每帧重解析；主题切换自动失效。
- `rgpui_story` 新增「多语言代码高亮」页（Rust / Python / JavaScript / JSON / 未注册
  语言各一段）。

### 验证

- `cargo check -p rgpui --features <14 个语言包>` 通过；`cargo test -p rgpui --lib
  highlight`：30 passed（14 个语言包各 1 项 + 捕获名改写 1 项 + 既有 Rust/JSON 用例）。
- `cargo check -p rgpui-markdown` 四种 feature 组合通过；`cargo check --workspace` 通过；
  `cargo fmt --all` 无变更。

## 三十七、增补：核心富文本的批注与脚注

§三十二 仍未做清单里的「富文本批注 / 文末脚注」落地。两者都做成**旁挂层**（不改段落 /
运行的块级结构），因此不破坏既有的字符坐标编辑模型。

### 模型

- `Annotation { id, range, note, author }`：挂在文档字符区间上的备注。
- `Footnote { id, range, text }`：正文引用标记（普通字符，插入时一并写入）+ 文末脚注文本。
- `RichTextDoc` 新增 `annotations` / `footnotes` 两个字段（默认空）。

### 接口

- 批注：`add_annotation(range, note) -> u32`（超界自动夹取）、`annotation(id)`、
  `annotation_mut(id)`（设置作者等）、`annotations_at(index)`（按位置查）、`remove_annotation(id)`。
- 脚注：`insert_footnote(index, marker, text) -> u32`（写标记 + 登记）、`footnote(id)`、
  `remove_footnote(id)`（记录与正文标记一并删除）。

### 编辑语义（区间随编辑平移）

- 插入：插入点之前的区间不动，之后的整体后移，覆盖插入点的区间变长。
- 删除：删除点之前的保持不变，之后的整体前移，相交的按存活文本收缩（删除后前后两段
  相邻，区间长度取存活字符数）。
- `insert_text` / `delete` 均已挂钩（`split_paragraph` / `merge_paragraph` 走同一路径，
  自动继承）。

### 渲染与 HTML 往返

- 渲染：`RichText` 按区间切分运行 —— 批注区间给**背景高亮**（主题 warning 色），
  脚注引用标记给**主题主色**；`build_paragraph` 收到段内区间列表，逐运行按边界切分。
- HTML：批注序列化为 `<mark data-annotation="id" data-note="…">`，脚注引用序列化为
  `<sup data-footnote="id" data-text="…">`；`from_html` 反向还原（解析期记段内区间，
  收尾时按段落起点换算成文档级区间）。属性值额外转义引号。

### 示例

`examples/showcase/src/bin/rich_text.rs` 增加「添加批注 / 删除首条批注 / 插入脚注 /
删除首条脚注」四个按钮与批注 / 脚注清单区（样例文档预置一条批注与一个脚注）。

### 验证

- `cargo test -p rgpui --lib rich_text`：16 passed（新增 4 项：批注增删查、批注区间随
  编辑平移、脚注插入与删除、批注/脚注 HTML 往返）。
- `cargo check -p showcase --bin rich_text` 通过；`cargo check --workspace` 通过。

## 三十八、增补：Markdown 文档大纲（TOC）提取

`rgpui-markdown` 新增 `toc` 模块：从 Markdown 源码提取标题，供阅读器渲染目录、
做「点击标题跳转」与生成锚点链接。

- `extract_headings(source) -> Vec<Heading>`；`Heading { level, text, range, anchor }`：
  - `level`：标题级别（1–6）；`text`：去掉行内标记（`**加粗**`、`代码`）后的纯文本；
  - `range`：标题在源码中的字节区间（含 `#` 前缀与行尾换行，便于定位 / 跳转）；
  - `anchor`：锚点 slug（小写，空白与标点转连字符，保留字母数字与 CJK；同名标题
    依次追加 `-1`、`-2` 去重，与 GitHub 习惯一致）。
- 实现走 pulldown-cmark 的 offset 迭代器（`Event::Start(Tag::Heading)` → 收集行内文本
  → `Event::End(TagEnd::Heading)` 收尾），不额外解析一遍。
- 示例：`examples/rgpui_story` 新增「文档大纲」页（按层级缩进展示标题 + 锚点）。

### 验证

- `cargo test -p rgpui-markdown --lib`：8 passed（新增 4 项：级别 / 文本 / 区间 / 锚点、
  行内标记剥离、同名锚点去重、无标题为空）。

## 三十九、增补：核心富文本的查找替换与文档统计

给 `RichTextDoc` 补上编辑器常用的三块能力（§三十七 批注 / 脚注之后的自然延伸）。

- `find(needle, case_sensitive) -> Vec<Range<usize>>`：返回全部命中的**字符**区间
  （按起点升序）；大小写无关时按字符做 `to_lowercase` 比较。
- `replace_all(needle, with, case_sensitive) -> usize`：替换全部命中并返回次数；
  - **从后往前替换**：前面的命中偏移不受影响；
  - 替换文本沿用命中区间的行内属性（取区间起点处的 `RunProps`）；
  - 批注 / 脚注区间随删除 / 插入自动平移（复用 §三十七 的区间平移规则）。
- `props_at(index) -> Option<RunProps>`：取某字符位置的行内属性（段落分隔符返回 `None`）。
- `stats() -> DocStats`：`chars`（不含段落分隔符）/ `paragraphs` / `lines`。

### 示例

`examples/showcase/src/bin/rich_text.rs` 增加「替换『项目符号』→『条目』」按钮，演示
查找替换与属性沿用。

### 附带修复：标题级别在 HTML 往返中丢失

`from_html` 此前把 `<h1>`…`<h6>` 当普通块标签处理，**标题级别被丢弃**（`<h1>` 与 `<h2>`
往返后都变成 `<p>`）。本轮补上：

- `ParagraphStyle.heading: Option<u8>`（1–6，`None` 为普通段落）+ builder `heading(level)`。
- `from_html` 在标题标签闭合时给承载标题文本的段落记级别；`to_html` 按级别输出
  `<h1>`…`<h6>`（其余仍为 `<p>`）。
- 渲染：`RichText` 的标题段落加粗，字号按级别倍率放大（1.8 / 1.5 / 1.3 / 1.15 / 1.05 / 1.0）；
  基准字号取 `RichText::base_font_size`（新增 builder），未设置时取窗口默认文本字号
  （rem 字号无法换算时沿用外层字号，仅加粗）。

### 验证

- `cargo test -p rgpui --lib rich_text`：21 passed（新增 5 项：查找区间、替换与属性沿用、
  替换后批注区间对齐、文档统计、标题级别往返）。

## 四十、增补：Markdown 任务列表的真复选框

`rgpui-markdown` 此前把任务列表渲染成纯文本标记（`[x] ` / `[ ] `），本轮换成真复选框：

- 勾选框：12×12 圆角方框，勾选时填充主题主色并显示「✓」，未勾选时描边主题边框色。
- 已勾选项的文本**加删除线 + 变暗**（`InlineFlattener` 新增 `done` 状态，走 muted 前景色 +
  删除线样式）。
- 普通列表项的标记（`• ` / `1. `）保持原样。

### 验证

- `cargo test -p rgpui-markdown --lib`：9 passed（新增 1 项：已勾选项加删除线并变暗）。
- 示例：`examples/rgpui_story` 的「Markdown 基础」页补任务列表段落。

## 四十一、增补：核心富文本的撤销 / 重做（快照式历史栈）

`RichTextDoc` 可编辑但没有历史栈，编辑器只能自己在应用层维护副本。本轮把历史做进
核心（文档模型与渲染同 crate，状态与视图分离更干净）。

- `RichTextHistory`：
  - `edit(&mut doc, f)`：**编辑前**压入文档快照，再执行 `f`；未产生改动的编辑不压栈
    （避免空撤销步骤）；新编辑会清空重做链。
  - `undo(&mut doc)` / `redo(&mut doc)`：用栈顶快照整份替换文档，当前文档压入另一侧栈。
  - `record(&doc)`：手动记录（供在历史之外修改文档的路径使用）。
  - `can_undo()` / `can_redo()` / `clear()`。
  - 栈上限 100 条（超出丢弃最旧快照，限制内存占用）。
- 快照是文档拷贝，适合中小文档；超大文档可改用差分 / 块级快照（后续增补）。

### 示例

`examples/showcase/src/bin/rich_text.rs`：行内样式与「替换」操作改走
`history.edit`，新增「撤销 / 重做」按钮。

### 验证

- `cargo test -p rgpui --lib rich_text`：23 passed（新增 2 项：撤销 / 重做往返与重做链
  清空、未改动编辑不压栈）。

## 四十二、增补：核心富文本的链接支持

此前核心富文本模型**没有链接概念**：HTML 里的 `<a href="…">` 解析后只保留文字、地址丢失，
序列化也不会输出链接。本轮补齐：

- `Run` 新增 `link: Option<String>`（链接地址），构造器 `Run::with_link(text, props, link)`。
- 解析：`from_html` 识别 `<a href>`，地址挂在解析样式状态上（随标签栈进出自动恢复 /
  还原）；`StyleState` 因此不再 `Copy`（改为 `Clone`）。
- 归一：`Paragraph::normalize` 合并相邻同属性运行时**同时比较链接地址**（否则链接会被
  并进普通文本而丢地址）。
- 序列化：`to_html` 输出 `<a href="…">`（地址按属性口径转义）。
- 渲染：链接文本用主题主色 + 下划线。
- 交互：`RichText::on_link_click(handler)` 回传被点击的链接地址；与批注点击共用一个
  `InteractiveText`（两类区间合并到一个序号空间，按序号分发）。

### 示例

`examples/showcase/src/bin/rich_text.rs`：样例正文加入「链接」运行，并演示
`on_link_click` 回传地址。

### 验证

- `cargo test -p rgpui --lib rich_text`：24 passed（新增 1 项：链接地址解析与序列化往返）。

## 四十三、增补：段落级行高 / 段前距 + 消除已知 flaky 测试

### 段落级行高 / 段前距

`ParagraphStyle` 新增两个字段（都是「有则用、无则继承外层」的口径）：

- `line_height: Option<f32>`：行高倍数（`line-height:1.8` → `Some(1.8)`；`line-height:24px`
  按 16px 基准字号换算成倍数）。
- `space_before: Option<Pixels>`：段前距（`margin-top:12px`）。

HTML 双向：`from_html` 解析 `line-height` / `margin-top`，`to_html` 输出同样的 CSS；渲染时
段落容器应用 `line_height(relative(…))` 与 `mt(…)`。

### 消除 flaky：测试调度器 park 硬超时可配置

`util::scheduler` 的 park 硬超时此前写死 15 秒（真实时间）。依赖**外部 OS 线程唤醒**的
用例（`test_nondeterministic_wake_allowed_with_parking`）在机器满载（CI 并行编译）时
唤醒可能被拖到十几秒后，触发「测试在 park 时超时」误报 —— 这就是文档里长期记录的已知波动
（重跑即过）。

修法：

- `TestSchedulerConfig` 新增 `park_timeout: Duration`（默认 15 秒），park 硬超时改用它。
- 外部唤醒用例单独放宽到 60 秒：真死锁仍会超时失败，满载误报消除。

### 验证

- `cargo test -p rgpui --lib rich_text`：25 passed（新增 1 项：行高 / 段前距 CSS 往返）。
- `cargo test -p rgpui --lib scheduler`：17 passed。

## 四十四、修复：Markdown 脚注引用标记被丢弃

`rgpui-markdown` 此前解析了脚注**定义**（`[^1]: …` → `RichBlock::FootnoteDefinition`，
渲染为文末带左边框的块），但**正文里的引用标记**（`[^1]`）在事件流里没有对应处理，
被直接丢弃 —— 读者只看到文末的脚注，正文里没有任何指向它的标记。

修复：

- 模型新增 `RichInline::FootnoteRef(String)`（脚注标签）。
- 解析：`Event::FootnoteReference(label)` → 行内脚注引用。
- 渲染：按链接色渲染 `[^label]` 标记（与脚注定义块的 `[^label]` 标题一致）。
- `inlines_to_plain_text` 同步输出 `[^label]` 文本。

### 验证

- `cargo test -p rgpui-markdown --lib`：10 passed（新增 1 项：脚注引用进入段落内容）。
- 示例：`examples/rgpui_story` 的「语法全览」页补脚注段落。




## 四十五、增补：核心富文本的引用块

`ParagraphStyle` 新增 `quote: bool`：段落属于引用块。

- 解析：`from_html` 识别 `<blockquote>`（含嵌套深度计数），块内段落标记为引用样式；
  没有 `<p>` 包裹时（`<blockquote>直接文字</blockquote>`）内容段落同样被标记。
- 序列化：`to_html` 把**连续**的引用段落合并输出为 `<blockquote>…</blockquote>`。
- 渲染：引用段落加左边线 + 浅底 + 内缩 + 弱化文字色（跟随主题）。

### 验证

- `cargo test -p rgpui --lib rich_text`：26 passed（新增 1 项：引用块往返）。

## 四十六、增补：行内代码 + 编辑器示例的多语言入口

### 行内代码（`<code>`）

核心富文本此前把 `<code>` 降级为普通文本（只留文字、丢掉等宽语义）。本轮补齐：

- `RunProps` 新增 `code: bool` + builder `RunProps::code()`。
- 解析：`from_html` 识别 `<code>`；序列化：`to_html` 输出 `<code>…</code>`。
- 渲染：等宽字体（主题等宽族）+ 浅底（主题 muted 色）。

### 编辑器示例的多语言入口

`examples/showcase/src/bin/editor.rs` 的语言按钮从只有 `rust` 扩到
`rust` / `python` / `javascript` / `go` / `json` / `demo` / 未知语言降级，
对应 showcase 的 `editor` feature 追加启用 `tree-sitter-json` / `tree-sitter-toml` /
`tree-sitter-javascript` / `tree-sitter-python` / `tree-sitter-go` 语言包。

### 组件库新增「富文本」页

`examples/rgppui_story` 新增「富文本」页（`stories/rich_text.rs`），集中展示本轮富文本
能力：标题级别、行内样式（粗 / 斜 / 下划线 / 删除线 / 行内代码）、链接（可点击回传地址）、
引用块、行高与段前距、批注（可点击回传 id）、脚注引用，以及撤销 / 重做按钮。

### 验证

- `cargo test -p rgpui --lib rich_text`：27 passed（新增 1 项：行内代码往返）。
- `cargo check -p showcase --bin editor --features editor` 与
  `cargo check -p showcase --all-features` 通过。

## 四十七、增补：LSP 格式化（文档 / 选区）

LSP 客户端 trait 早就有 `formatting` 请求，但**编辑器侧没有触发入口** —— 应用即便接了
语言服务器也用不上格式化。本轮补齐整条链路：

- 新增 provider trait `crate::lsp::FormattingProvider`：
  - `formatting(text, window, cx)`：整篇格式化，返回 `Vec<TextEdit>`；
  - `range_formatting(text, range, window, cx)`：范围格式化（默认不支持，返回空编辑）。
- `EditorState` 新增 `set_formatting_provider`（注入 / 断开）与 `request_formatting`
  （触发请求）；请求经 **epoch 防抖**（与补全 / 悬停同款）：请求发出后文档再变则结果作废。
- 编辑应用：LSP 行列按 UTF-8 字节列换算（`PositionMapping`，与诊断下划线同款），
  编辑**按字节偏移倒序**应用（靠前编辑的位置不受后面改动影响），应用前抑制一次自动补全，
  应用后光标回到原位置（与「接受代码操作」同款逻辑）。
- 示例：`examples/showcase/src/bin/editor.rs` 注入假格式化 provider（去行尾空白）并加
  「格式化文档（LSP）」按钮。

### 验证

- `cargo test -p rgpui --features editor --lib lsp_attach`：14 passed（新增 1 项：
  格式化请求的编辑被应用到文档）。

## 四十八、增补：LSP 重命名

与格式化同款缺口：LSP 客户端 trait 有 `rename`，编辑器侧没有触发入口。

- 新增 provider trait `crate::lsp::RenameProvider`：
  `rename(text, offset, new_name, window, cx)` → `Option<WorkspaceEdit>`（`None` 表示服务端
  不支持或位置无符号）。
- `EditorState` 新增 `set_rename_provider`（注入 / 断开）与 `request_rename(new_name)`
  （epoch 防抖，与格式化 / 补全 / 悬停同款）。
- 编辑应用：复用格式化那条路径（`apply_lsp_text_edits`：字节列换算 + 倒序应用 + 光标复位），
  并把「从工作区编辑里取当前文档的编辑」抽成公共 helper
  （`code_actions::edits_for_workspace_edit`，代码操作与重命名共用）——
  `changes` / `documentChanges` 两种形式都支持，未设文档 URI 时只在编辑确实只涉及一个
  文档时应用，避免猜错目标。
- 示例：`examples/showcase/src/bin/editor.rs` 注入假重命名 provider（把光标处的词改名）
  并加「重命名光标处 → renamed」按钮。

### 验证

- `cargo test -p rgpui --features editor --lib lsp_attach`：15 passed（新增 1 项：
  重命名请求的工作区编辑被应用）。

## 四十九、增补：LSP 定义跳转 / 查找引用

`crate::lsp::DefinitionProvider` 早就有（定义 / 引用 / 同符号高亮），编辑器侧同样没有
触发入口。本轮补齐：

- 新增状态 `crate::lsp::DefinitionState`（`locations` + `visible`）。
- `EditorState` 新增 `set_definition_provider`（注入 / 断开）、`goto_definition`
  （跳转：命中位置在**当前文档**时直接移动光标）、`find_references`（结果写入状态供应用
  渲染列表）、`definition_locations`（应用读取结果）。请求同样走 epoch 防抖。
- 位置换算复用 `PositionMapping`（UTF-8 字节列，与诊断下划线同款）。
- 示例：`examples/showcase/src/bin/editor.rs` 注入假定义 provider 并加「跳转定义（LSP）」按钮。

### 验证

- `cargo test -p rgpui --features editor --lib lsp_attach`：16 passed（新增 1 项：
  定义跳转把光标移到目标偏移，结果同时可供应用层渲染）。

## 五十、增补：LSP 签名提示

- 新增 `crate::lsp::SignatureHelpProvider`（`signature_help(text, offset, …)` →
  `Option<SignatureHelp>`）与状态 `crate::lsp::SignatureHelpState`（响应 + 激活签名 / 参数 +
  可见性）。
- `EditorState` 新增 `set_signature_help_provider`（注入 / 断开）、
  `request_signature_help`（请求，epoch 防抖）、`signature_help_state`（应用层读状态渲染浮层）。
- 示例：`examples/showcase/src/bin/editor.rs` 注入假签名提示 provider 并加「签名提示（LSP）」按钮。

至此 LSP 客户端 trait 里的主要请求（补全 / 悬停 / 诊断 / 格式化 / 重命名 / 定义引用 /
签名提示）在编辑器侧都有了触发入口。

### 附：同符号高亮（document highlights）

复用 `DefinitionProvider::document_highlights`（默认不支持时为空）：

- `EditorState::request_document_highlights`：请求后把返回的范围渲染为**背景装饰**
  （主题主色浅底），走内部输入上独立的一层装饰集合（与诊断下划线分层）。
- 示例：showcase 的「同符号高亮（LSP）」按钮（假 provider 标出光标所在的词）。

### 验证

- `cargo test -p rgpui --features editor --lib lsp_attach`：18 passed（新增 1 项：
  同符号高亮请求后装饰集合带上高亮范围）。

## 五十一、增补：LSP 语义高亮（编辑器最后一个 LSP 缺口）

`crate::lsp` 早就有 `SemanticTokensProvider` / `SemanticTokensState` / `SemanticTokenTypeMap`，
但编辑器侧没接线（语义高亮是 tree-sitter 高亮之外的另一条通路，服务端更准）。

- **载荷类型** `SemanticTokensPayload`（增量 token 数据 + 类型 / 修饰符图例）；
  provider 新增 `semantic_tokens_full_with_legend`（默认不支持，返回 `None`）。
- **解码** `SemanticTokensHighlighter::decode_runs`：增量 token → 「字节区间 + 主题 token 名」
  （列按**字节列**解释，与仓库内其它 LSP 换算口径一致；类型名经 `SemanticTokenTypeMap`
  映射，未映射的类型不高亮）。
- **高亮器** `SemanticTokensHighlighter` 实现核心 `highlight::Highlighter` trait，
  可直接装进 `set_highlighter`（与 tree-sitter 高亮器同一条通路，应用层渲染方式不变）。
- **编辑器接线**：`set_semantic_tokens_provider` / `set_semantic_token_type_map` /
  `request_semantic_tokens`（epoch 防抖）。
- 示例：showcase 的「语义高亮（LSP）」按钮（假 provider 把首行前两个词标成 function）。

至此 LSP 客户端 trait 里的主要请求在编辑器侧都有触发入口：补全 / 悬停 / 诊断 / 快速修复 /
格式化 / 重命名 / 定义引用 / 签名提示 / 同符号高亮 / 语义高亮。

### 验证

- `cargo test -p rgpui --features editor --lib semantic`：2 passed（token 解码与样式解析）。

## 五十二、增补：核心富文本的表格

§三十二 仍未做清单里的「富文本表格」。难点是核心模型是**段落序列 + 字符坐标**——
直接引入块级表格会打破字符索引语义。本轮的做法是**表格不引入新的文本空间**：

- **模型** `RichTable { header, rows, alignments }`：单元格存**段落索引**（不是文本副本），
  因此单元格内容就是普通段落：字符坐标编辑、批注 / 脚注区间、行内样式对表格内文本同样生效。
  对齐方式 `TableAlign`（左 / 中 / 右）。
- **接口** `RichTextDoc::add_table(header, rows, alignments) -> usize`（按「表头 → 数据行」
  建段落并登记表格，返回表格索引）、`table(index)`。
- **渲染**：表格出现在其**首个单元格段落**所在位置（其余单元格段落不再单独输出）；
  表头加粗 + 浅底，单元格按列对齐，表格整体圆角边框。
- **HTML**：输出 `<table>` + `<tr>` / `<th>` / `<td>`（单元格段落不再单独输出 `<p>`）。
  （HTML → 表格的解析暂不做，解析侧仍按普通段落处理。）

### 示例

`examples/showcase/src/bin/rich_text.rs` 与 `examples/rgppui_story` 的「富文本」页都补了
一张两列表格。

### 验证

- `cargo test -p rgpui --lib table_cells`：1 passed（单元格复用段落 + HTML 输出为表格）。

## 五十三、增补：核心富文本的节 / 分栏

§三十二 清单里「节（section）/分栏」的落地。与表格同理，**节不引入新的文本空间**：

- **模型** `RichSection { paragraphs, columns, title }`：节内段落复用文档段落（段落索引），
  `columns` 为分栏数（1 = 不分栏，上限 4），`title` 为可选节标题。
- **接口** `RichTextDoc::add_section(paragraphs, columns, title) -> usize`、`section(index)`。
- **渲染**：节出现在其**首个段落**所在位置（节内段落不再单独输出）；多栏时段落按
  「列序轮流」分配到各栏（`index % columns`），单栏时与普通段落同款。
- **HTML**：输出 `<section>`（标题为 `<h3>`），节内段落按 `<p>` 输出。

### 验证

- `cargo test -p rgpui --lib section_reuses`：1 passed（段落复用 + 分栏夹取 + HTML 输出）。

## 五十四、增补：核心富文本的修订标记（tracked changes）

§三十二 清单里的「修订」落地。做法是**在行内属性上打标记**（不引入新的文本空间）：

- `RunProps` 新增 `inserted` / `deleted` 两个标记（+ builder `inserted()` / `deleted()`）。
- 接口：
  - `mark_inserted(range)`：标记为修订插入（渲染为主色 + 下划线）；
  - `mark_deleted(range)`：标记为修订删除（**文本保留在文档里**，渲染为删除线 + 变暗）；
  - `clear_revisions(range)`：清除标记；
  - `effective_text()`：不含删除内容的文本（复制 / 导出用；`plain_text()` 仍是模型原文）。
- HTML：插入输出 `<ins>`、删除输出 `<del>`。
- 示例：showcase 与 story 的富文本页都加了「插入 / 删除」演示段。

至此 §三十二 富文本清单（分节 / 分栏 / 表格 / 批注 / 脚注 / 修订）全部落地。

### 验证

- `cargo test -p rgpui --lib revisions_mark`：1 passed（标记落到运行上 + 有效文本排除删除内容 +
  HTML `<ins>` / `<del>`）。

## 五十五、增补：核心富文本的 HTML 解析侧（表格 / 节往返闭环）

§五十二 / §五十三 的表格与节此前**只出不进**（解析侧仍按普通段落处理），本轮补上解析：

- **表格**：`<table>` / `<tr>` / `<th>` / `<td>` 解析为 `RichTable`；每个单元格是新建的
  段落（与 `add_table` 的表示一致），列对齐取 `align` 属性或 `style` 的 `text-align`。
  注意 `<tr>` 原本落在通用块级标签分支里，表格处理需前置才能拿到行 / 单元格边界。
- **节**：`<section>` 解析为 `RichSection`（节内段落连续，标题取 `data-title` 属性）。
- 序列化侧同步：节标题改用 `data-title` 属性输出（不再输出 `<h3>`），避免标题与内容
  段落重复。

### 验证

- `cargo test -p rgpui --lib html_tables_and_sections`：1 passed（解析后仍是表格 / 节，
  列对齐与节标题都还原；再序列化仍带表格与节）。

## 五十六、增补：核心富文本的 Markdown 导出

与 HTML 导出互补，给「文档编辑器导出 .md」一条现成路径。

- `RichTextDoc::to_markdown() -> String`：
  - 标题级别 → `#`×级别；列表 → `- ` / `1. `；引用块 → `> `；
  - 行内样式 → `**加粗**` / `*斜体*` / `~~删除线~~` / `` `代码` ``；链接 → `[文本](地址)`；
  - 修订标记 → 内联 HTML `<ins>` / `<del>`（纯 Markdown 无对应语法，内联 HTML 是通用做法）；
  - 表格 → Markdown 表格（表头行 + `---` 分隔行 + 数据行）；节标题 → `## 标题`。
- 段落 / 表格 / 节的输出顺序与渲染一致（表格在其首个单元格段落处、节标题在其首个段落处）。

### 验证

- `cargo test -p rgpui --lib exports_markdown`：1 passed（标题 / 引用 / 样式 / 链接 / 表格都按预期导出）。

## 五十七、增补：核心富文本的查找高亮

`find` 此前只给区间，渲染层不体现「命中在哪」。本轮把命中区间接到渲染上：

- `RichText::highlight_ranges(ranges)`：传入**文档级字符区间**（可直接用
  `RichTextDoc::find` 的结果），渲染为搜索高亮（主题 warning 浅底），与批注背景分层
  互不干扰。
- HTML 序列化不受影响（查找命中只是渲染态，HTML 里不额外标记）。

### 示例

showcase 与 story 的富文本页都加了「查找并高亮『加粗』」按钮（命中数显示在状态行）。

### 验证

- `cargo test -p rgpui --lib`：821 passed + 1 ignored（新增用例覆盖导出与往返；查找高亮为
  渲染态，由示例页展示）。

## 五十八、增补：Ink 笔迹的曲线平滑与压感整形

ink 模块的两个暂缓项落地（模块文档同步更新）：

- `catmull_rom_path(points, width)`：Catmull-Rom 曲线平滑（三次贝塞尔拟合，控制点
  `p1 + (p2 - p0) / 6`、`p2 - (p3 - p1) / 6`，两端复制端点）。与既有
  `stroke_path` 的「中点二次贝塞尔近似」相比，曲线**穿过全部采样点**，笔迹更贴合。
- `smooth_pressure(pressure, gamma)`：压感曲线整形（幂曲线；`gamma < 1` 让轻压区更饱满、
  `gamma > 1` 让重压区更突出；非有限值按满压、非法 gamma 退化为原值）。

### 验证

- `cargo test -p rgpui --lib ink`：新增 2 项（Catmull-Rom 路径覆盖全部采样点；压感曲线
  整形与越界处理）。

## 五十九、增补：表格的结构化无障碍语义

§二十八 暂缓项「`Role::Cell` 行 / 列语义」的落地（画布语义不适用于表格这类 div 渲染，
改为给 div 标注结构化语义）：

- `div` 的 `role()` 从 `StatefulInteractiveElement` 上移到 `InteractiveElement`
  （任何 div 都能标角色；`GenericContainer` 仍被过滤 ✓）。
- `DataTable`：根节点 `Role::Table`；数据行 `Role::Row`；单元格 `Role::Cell` +
  `aria_row_index` / `aria_column_index`；行号列 `Role::RowHeader`；表头列
  `Role::ColumnHeader`。
- `SheetGrid`（电子表格）：根节点 `Role::Grid`；单元格 `Role::Cell` + 行列索引；列头
  `Role::ColumnHeader`；行头 `Role::RowHeader`。
- 行列索引同时覆盖「左侧固定列」与「滚动区」两条渲染路径 ✓。

### 验证

- `cargo test -p rgpui --lib a11y_role_and_indices`：1 passed（角色与行列索引写入
  AccessKit 节点）。

## 六十、增补：排序器拖拽实时预览 + 时间线缓动插值预设

### 排序器：拖拽实时预览

此前拖拽时只有「原格半透明 + 插入指示线」，看不到被拖项跟手移动。本轮补上预览：

- 状态新增 `drag_position`（拖拽期间的指针位置，窗口坐标）：`update_drag` 记录，
  `finish_drag` / `cancel_drag` 清空。
- 渲染：拖拽中在指针位置画一份半透明副本（等尺寸、主题主色描边），随指针移动。

### 时间线：缓动插值预设

§二十六 暂缓项里的「曲线预设」落地（值曲线的手柄拖拽此前**已实现** —— 状态里本就有
`dragging` / `set_keyframe_value`，本轮核对后修正文档口径）：

- `Interpolation` 新增 `EaseIn` / `EaseOut` / `EaseInOut`（CSS 标准缓动曲线），
  `ease(t)` 按曲线映射进度；新增 `label()` 给中文名（UI 展示用）。
- 状态新增 `set_track_interpolation(index, interpolation, cx)`：改轨道插值并抛出
  `TimelineEvent::TrackInterpolation`。
- 示例：showcase 的 `timeline` 演示页加「插值：…」按钮组（作用于选中轨道，当前插值
  用主题主色标出）。

### 验证

- `cargo test -p rgpui --lib`：826 passed + 1 ignored（新增 3 项：拖拽指针位置记录与
  清理、缓动曲线取值、设置轨道插值与事件）。
## 六十一、增补：任务窗格跨侧拖拽停靠

§二十七 暂缓项「跨侧拖拽停靠（把窗格从一侧拖到另一侧）」落地（另一项「窗格内滚动位置
记忆」属应用层：面板内容由应用提供，滚动状态也在应用侧，故不占框架）。

- 状态新增 `parent_bounds`（父容器矩形，窗口坐标）+ `set_parent_bounds(bounds, cx)`：
  停靠态把手越过父容器中线时换边停靠。
- 拖拽中：停靠态把手（外缘）被拖过父容器中线 → 调 `set_side` 换边（抛出
  `TaskPaneEvent::SideChanged`），并**重置拖拽基线**（宽度继续跟随指针，不跳变）。
- 示例：showcase 的 `task_pane` 演示页把窗口视口作为父容器矩形传给状态，状态栏提示
  「把把手拖过窗口中线可换边停靠」。

### 验证

- `cargo test -p rgpui --lib docked_grip_drag_crosses`：1 passed（把手越过中线换边、再拖回
  换回）。
## 六十二、增补：Ink 橡皮的局部切割

§二十二 暂缓项「橡皮的局部切割」落地：此前橡皮只能整笔删除（`erase_at`）。

- `InkLayer::erase_local(center, radius) -> usize`：剪掉落在圆内的**采样点**，笔画被切成
  前后两段（残留不足两点的段视为整段被擦除）；返回被剪掉的点数。
- `InkDocument::erase_local`：逐个可编辑图层应用（与 `erase_at` 的图层策略一致）。
- 示例：showcase 的 `ink` 演示页加「橡皮：局部 / 整笔」切换按钮，状态栏显示擦除的点数 /
  笔画数。

### 验证

- `cargo test -p rgpui --lib erase_local`：1 passed（中段被剪 → 笔画切成两段，端点正确）。
## 六十三、增补：动画时间线的多选关键帧与框选

§二十四 暂缓项「多选关键帧与框选」落地：

- 状态新增 `multi_keyframes`（附加多选集合，仅对当前轨道有效）：
  - `toggle_keyframe_selection(track, index, cx)`：Ctrl / Cmd 点击切换附加选中；换轨道时
    落到新轨道的主选中项并清空集合；
  - `selected_keyframe_indices()`：主选中项在前、其余按升序；
  - `is_keyframe_selected(track, keyframe)`：主项或附加项均算选中（渲染高亮用）；
  - `remove_selected_keyframes(cx)`：删除全部选中（逐个抛 `KeyframeRemoved`）。
- 交互：关键帧上 **Ctrl / Cmd 点击**切换多选（不进入拖拽）；Delete / Backspace 键删除
  全部选中（此前只删主选中项）。
- 指针事件：`on_pointer_down` 增加 `modifiers` 参数（其它调用点传 `Modifiers::default()`）。

### 框选

- 状态新增 `box_select`（轨道 + 锚点 + 当前位置，行局部坐标）与 `box_selecting()` 查询。
- 交互：轨道内空白处**按下并拖拽**即框选（框内关键帧全选、框外清除，拖动过程中实时
  更新）；双击仍是「请求新增关键帧」；释放后保留选中、矩形消失。
- 渲染：框选矩形画在轨道行内（主题主色描边 + 浅底）。

### 验证

- `cargo test -p rgpui --lib multi_select_keyframes`：1 passed（切换附加选中、取消、批量删除）。
- `cargo test -p rgpui --lib box_select_picks`：1 passed（框内关键帧全选、框外清除、释放后保留）。
## 六十四、增补：lint 收紧第一批（`unwrap` / `expect` 迁移，7 个模块 / 74 处）

§三十一 暂缓项「全 crate 的 `unwrap` / `expect` 迁移」的**第一批**（按子系统分批 ✓）：
新增代码在已迁移模块里用 `unwrap` 会直接编译失败。已迁移模块从 24 个增至 **31 个**。

### 迁移口径

分两类处理：

- **可恢复的错误**（`Option` / `Result` 来自真实外部状态）：改为显式处理
  ——`executor` 的测试调度器接口、`util::paths` 的主目录 / 正则、`lsp::stdio` 的互斥锁。
- **框架内部不变量**（节点 id 有效、状态已测量、树结构自洽）：语义不变（同样 panic、同样
  消息），但收敛到模块内的 `invariant` / `taffy_invariant` 助手，调用点写明「这是不变量断言」，
  并给助手本身加局部 `#[allow(clippy::panic)]`（含注释说明）。这样模块能挂 deny，
  新代码随手写 `unwrap` 仍会被编译失败拦住。

### 逐模块

- **`executor.rs`**（12 处）：`dispatcher.as_test()` 的 `Option` 改为「不是测试调度器就
  忽略 / 返回默认值」；`Scope::spawn` 的通道已关闭时跳过任务并记日志；`Scope::drop` 取通道
  改为忽略返回值；`rng()` 返回类型改为 `Option<SharedRng>`（无外部调用方）；`block_test`
  的「future 未完成」用显式 `panic!` + 局部 allow（测试专用路径，失败要能看清）。
- **`util/paths.rs`**（4 处）：取不到主目录时退回当前目录而非 panic；静态正则改为
  `LazyLock<Option<Regex>>`（编译期常量，编译失败按「无匹配」处理）；`split_once` 改为
  `unwrap_or` 兜底；`rsplitn` 首段改为 `unwrap_or_default`。
- **`lsp/stdio.rs`**（8 处）：全部是 `Mutex::lock().unwrap()`，改用中毒可恢复写法
  `lock().unwrap_or_else(|err| err.into_inner())`。
- **`taffy.rs`**（20 处）：全部是 taffy 布局树的不变量断言（节点 id 失效），统一走
  `taffy_invariant` 助手；`absolute_outer_origins` 的 Option 走 `invariant` 助手。
- **`util/sum_tree`**（12 处：`mod.rs` 4 + `cursor.rs` 6 + `tree_map.rs` 2）：树结构自洽性
  断言（`Arc::get_mut` 唯一持有、`nodes.pop()` 非空、`last_mut` / `first_mut` 存在）统一走
  `invariant` 助手；叶节点 / 内部节点的互斥 panic 加局部 allow。
- **`elements/text.rs`**（18 处）：文本状态未测量 / 未 prepaint 的断言统一走 `invariant`
  助手；`with_runs` 的「文本段与文本长度不符」属调用方契约错误，保留 panic + 局部 allow。

### 验证

- `cargo test -p rgpui --lib`：830 passed + 1 ignored；`--features lsp,editor`：924 passed。
- `cargo test --workspace`：全绿。
- `cargo clippy -p rgpui --lib --all-targets -- -D warnings` 通过（默认与
  `lsp,editor,test-support` 两组 feature）。

### 后续批次候选（按风险从低到高）

`app/entity_map.rs`（27 处，含泄漏检测的 `panic!`）、`app.rs`（32）、
`keymap` / `keymap/context.rs`（84）、`lsp/stdio.rs` 余下的进程管理、
`view.rs`（15，核心视图机制）、`window.rs`（24）、`elements/div.rs`（39）。

- **`executor.rs`**（12 处）：`dispatcher.as_test()` 的 `Option` 改为「不是测试调度器就
  忽略 / 返回默认值」；`Scope::spawn` 的通道已关闭时跳过任务并记日志；
  `Scope::drop` 取通道改为忽略返回值；`rng()` 返回类型改为 `Option<SharedRng>`（无外部
  调用方）；`block_test` 的「future 未完成」用显式 `panic!` + 局部
  `#[allow(clippy::panic)]`（测试专用路径，失败要能看清）。
- **`util/paths.rs`**（4 处）：取不到主目录时退回当前目录而非 panic；静态正则改为
  `LazyLock<Option<Regex>>`（编译期常量，编译失败按「无匹配」处理）；`split_once` 改为
  `unwrap_or` 兜底；`rsplitn` 首段改为 `unwrap_or_default`。
- **`lsp/stdio.rs`**（8 处）：全部是 `Mutex::lock().unwrap()`，改用中毒可恢复写法
  `lock().unwrap_or_else(|err| err.into_inner())`。

三个模块均加上 `#[deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]`
（`executor` 在 `rgpui.rs`、`util::paths` 在 `util/mod.rs`、`lsp::stdio` 在 `lsp/mod.rs`）。

后续批次候选（按风险从低到高）：`elements/text.rs`（18）、`util/sum_tree`（20）、
`taffy.rs`（20，全是 `expect`）、`view.rs`（15，核心视图机制，改动面大）、
`app/entity_map.rs`（27，含泄漏检测的 `panic!`）。

### 验证

- `cargo test -p rgpui --lib`：830 passed + 1 ignored；`--features lsp,editor`：924 passed + 1 ignored。
- `cargo clippy -p rgpui --lib --all-targets -- -D warnings` 通过（默认与
  `lsp,editor,test-support` 两组 feature）。
## 六十五、增补：lint 收紧第二批（4 个模块 / 25 处）

延续 §六十四 的口径（可恢复错误显式处理、内部不变量收敛到 `invariant` 助手），
本批再迁移 4 个模块：已迁移模块数 31 → **35**。

- **`elements/div.rs`**（8 处）：`DragMoveEvent::drag` 的活动拖拽类型、`action` / 拖拽载荷 /
  drag-over 样式的 `downcast_ref`（都按 `TypeId` 校验过类型）、`active_drag` 的取出、
  全局 hitbox 栈的取出，统一走 `invariant`。
- **`theme/color.rs`**（7 处）：内嵌 `default-colors.json` 的解析、HSL 通道字符串的解析与
  三段切分、色阶 500 权重的兜底取值，走 `invariant` / `invariant_result`。
- **`inspector.rs`**（3 处）：检查器候选元素非空、状态与值的 `downcast`（按 `TypeId` 校验），
  走 `invariant` / `invariant_result`（反射调用 `invoke` 的类型不符也走 `invariant_result`）。
- **`app/async_context.rs`**（7 处）：「只取一次」的 `build_entity` / `args` 约定走 `invariant`；
  「app 已释放」与「异步上下文禁止 `as_mut`」属 API 误用，保留 panic + 局部 allow。

### 验证

- `cargo test -p rgpui --lib`：830 passed + 1 ignored。
- `cargo clippy -p rgpui --lib --all-targets -- -D warnings` 通过（默认与
  `lsp,editor,test-support,inspector` 两组 feature）。

### 剩余候选（按风险从低到高）

`app/entity_map.rs`（25 处，含泄漏检测的 `panic!`）、`app.rs`（32）、`window.rs`（24）、
`view.rs`（15，核心视图机制）、`util/scheduler/test_scheduler.rs`（8，测试调度器）、
`theme/highlight.rs`、`elements/scroll/*` 等。
## 六十六、增补：lint 收紧第三批（`app::entity_map` / 25 处）

延续 §六十四 / §六十五 的口径，本批迁移实体系统的核心 `app/entity_map.rs`：已迁移模块数
35 → **36**。

- **不变量**（走 `invariant`）：实体 id 非零、租约必持有实体、租约实体的类型与租约一致、
  被释放实体的引用计数必存在、实体映射必活得比句柄久。
- **契约违规**（保留 panic + 局部 allow + 注释）：同一实体被重复租约（`double_lease_panic`）、
  `Lease` 未经 `end_lease` 释放、实体 / 句柄被过度释放、泄漏检测发现泄漏句柄。
- 泄漏报告里的 `writeln!` 改为 `let _ = writeln!(...)`（写 `String` 不会失败，无需断言）。

### 验证

- `cargo test -p rgpui --lib`：830 passed + 1 ignored；`--features leak-detection` 同样通过。
- `cargo clippy -p rgpui --lib --all-targets -- -D warnings` 通过（默认与
  `lsp,editor,test-support,inspector,leak-detection` 两组 feature）。

### 剩余候选

`app.rs`（32）、`window.rs`（24）、`view.rs`（15，核心视图机制）、
`util/scheduler/test_scheduler.rs`（8，测试调度器）、`theme/highlight.rs`、
`elements/scroll/*` 等。
## 六十七、增补：lint 收紧第四批（13 个文件 / 约 45 处）

延续 §六十四～§六十六 的口径，本批覆盖「元素生命周期 + 无障碍 + HTTP + 工具层」：
已迁移模块数 36 → **39**（新增 `element` / `http_client` / `window::a11y`）。

- **`element.rs`**：元素生命周期的调用顺序契约（必须先 `request_layout` 再 `prepaint` 再
  `paint`）—— 4 处 `panic!` 属**内部契约违规**，集中在 `impl GlobalElementId`（一个
  `#[allow(clippy::panic)]` + 注释覆盖该 impl 内的同类 panic）；`debug_assert` 里的
  `global_id` 取值走 `invariant`。
- **`window/a11y.rs`**：无障碍节点状态被破坏时的 5 处 `panic!`（`set_focus` 未注册、
  `active_descendant` 越界等）属契约违规，改为逐处局部 allow + 注释；「构造器只在
  元素节点在栈上时存在」的不变量走 `invariant`。
- **`http_client/`**：测试替身的 `as_fake` 误用、假响应体构造失败等契约违规保留 panic
  + 局部 allow；JSON 序列化失败走 `invariant`。
- **`util/`**：`util/mod.rs`（shell 路径、JSON 目标、文件读取、静态正则）、
  `util/rel_path.rs`（`file_stem` / `extension` 改用 `?` 传播非 UTF-8 情况）、
  `util/redact.rs`（静态正则改 `Option`）、`util/path_list.rs`（`write!` 忽略返回值）、
  `util/scheduler/{clock,executor,test_scheduler}.rs`（起始时间解析、调度器状态不变量）、
  `util/test.rs` 与 `util/test/marked_text.rs`（临时目录 / 仓库创建失败、标记文本格式非法）。

### 验证

- `cargo test --workspace`：全绿（`rgpui` 830 passed + 1 ignored）。
- `cargo clippy -p rgpui --lib --all-targets -- -D warnings` 通过（默认与
  `lsp,editor,test-support,inspector,leak-detection` 两组 feature）。
- `cargo fmt --all` 无变更。

### 剩余候选

`app.rs`（32）、`window.rs`（24）、`app/test_context.rs`（28）、`app/bench_context.rs`（15）、
`app/test_app.rs`（12）、`util/scheduler/tests.rs`、`elements/scroll/*`、`theme/highlight.rs`
等 —— 这些属于「核心框架 / 测试支撑」的大文件，改造面大，留给后续批次（每批一个文件，
配套单测与 CI 兜底）。
## 六十八、增补：lint 收紧第五批（`app` 模块 / 约 40 处）

延续 §六十四～§六十七 的口径，本批覆盖应用核心 `app` 模块（含子模块 `app/context.rs`）：
已迁移模块数 39 → **40**。

### 迁移口径

- **类型 / 状态不变量**（downcast 类型一致、标签页 id 存在、窗口已登记、借用期内实体存活等）：
  走模块内 `invariant` / `invariant_result` 助手，语义不变（同样 panic、同样消息）。
- **可恢复的结果**（`update_window` / `Entity::update` 返回 `Result`）：改为显式处理并记
  debug 日志 —— 窗口已关闭时跳过重绘 / 取消焦点，不中断整帧处理。
- **契约违规**（quit 后 spawn、全局变量未注册、flush effects 时窗口未出栈）：保留 panic +
  局部 `#[allow(clippy::panic)]` + 注释。

### 关键实现细节

- `Box<dyn Any>::downcast::<T>()` 返回 `Result<Box<T>, Box<dyn Any>>`，`downcast_mut` 返回
  `Option<&mut T>` —— 两者要用不同的助手（`invariant_result` / `invariant`）。
- `GpuiBorrow` 持有的是 `Option<Lease<T>>`，`Lease<T>: Deref<Target = T>`，因此
  `Borrow` / `Deref` 实现里要显式标注 `let lease: &Lease<T> = ...` 再交给自动 deref。

### 测试支撑子模块的口径

`app/` 下的 `test_context.rs` / `test_app.rs` / `bench_context.rs` /
`visual_test_context.rs` / `headless_app_context.rs` 只在 `test` / `test-support` 下编译，
不是生产路径；它们的 `unwrap` / `expect` / `panic!` 就是测试断言本身（"未找到窗口"、
"根视图类型不匹配"、"benchmark 窗口意外关闭"）。这批给它们整体加
`#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` + 注释说明，
与根目录 `clippy.toml` 的 `allow-*-in-tests` 同口径 —— 不用逐个改写成 `match`。

### 验证

- `cargo test --workspace`：全绿（`rgpui` 830 passed + 1 ignored）。
- `cargo clippy --workspace --lib --bins -- -D warnings` 通过（CI 一致口径）；
  `cargo clippy -p rgpui --features test-support --lib --all-targets -- -D warnings` 通过。
- `cargo check --workspace` / `--workspace --examples` / `-p showcase --all-features` 通过。
- `cargo publish -p rgpui --dry-run` 通过。

### 剩余候选

`window.rs`（24）、`view.rs`（15，核心视图机制）、`util/scheduler/test_scheduler.rs`（8）、
`theme/highlight.rs`、`elements/scroll/*` 等。
## 六十九、增补：lint 收紧第六批（`window` 模块 / 24 处）

延续 §六十四～§六十八 的口径，本批覆盖窗口核心 `window.rs`：
已迁移模块数 40 → **41**。

### 迁移要点

- **状态 / 类型不变量**（焦点句柄已登记、布局引擎存在、根视图存在、渲染实体栈非空、
  事件监听器存在、downcast 类型一致、精灵图集回调必返 `Some`）：走模块内
  `invariant` / `invariant_result` 助手，语义不变。
- **可恢复的失败**（平台窗口映射失败）：改为记 `error` 日志，不中断窗口创建。
- **重入调用**（同一状态类型与元素 id 在 `with_element_state` 回调内再次进入）：
  属契约违规，走 `invariant` 并保留原消息。

### 实现细节

- `sprite_atlas.get_or_insert_with(...)` 返回 `Result<Option<AtlasTile>, E>`：`?` 解掉
  `Result` 后还剩一层 `Option`，要用 `invariant` 再解一次。
- `Box<dyn Any>::downcast::<T>()` 返回 `Result<Box<T>, _>`，`downcast_mut` 返回
  `Option<&mut T>` —— 两个助手不能混用。

### 验证

- `cargo test --workspace`：全绿（`rgpui` 830 passed + 1 ignored）。
- `cargo clippy --workspace --lib --bins -- -D warnings` 通过（CI 一致口径）。
- `cargo check --workspace --examples` / `-p showcase --all-features` 通过。

### 剩余候选

`view.rs`（15，核心视图机制）、`util/scheduler/test_scheduler.rs`（8）、
`theme/highlight.rs`、`elements/scroll/*` 等。
## 七十、增补：lint 收紧第七批（`view` 模块 / 15 处）

延续 §六十四～§六十九 的口径，本批覆盖视图机制核心 `view.rs`：
已迁移模块数 41 → **42**。

### 迁移要点

`view.rs` 的 `unwrap` 全是**框架内部不变量**（视图实体在渲染前必然存在、视图类型由
`ViewContext` 保证一致、带 id 的视图必有 `global_element_id`、`with_element_state`
回调必带元素状态、无状态路径的渲染产物必然存在），语义完全不变，统一收敛到模块内
`invariant` / `invariant_result` 助手。

内嵌的 `mod any_view` 看不到外层助手，用 `use super::invariant_result;` 显式引入。

### 验证

- `cargo test -p rgpui --lib`：830 passed + 1 ignored。
- `cargo clippy --workspace --lib --bins -- -D warnings` 通过（CI 一致口径）。

### 剩余候选

`theme/highlight.rs`、`elements/scroll/*`、`platform.rs`、`platform/test/platform.rs`、
`util/scheduler/test_scheduler.rs`（测试调度器）、`util/scheduler/tests.rs`（测试模块）
等；`app/test_context.rs` / `app/bench_context.rs` / `app/test_app.rs` 等测试支撑模块
已在第五批按测试断言语义整体放行。

## 七十一、增补：lint 收紧第八批（平台 / 分发 / 采样层 / 22 处）

延续 §六十四～§七十 的口径，本批覆盖 4 个模块：`platform`（含 `keystroke` /
`threaded_dispatcher`）、`platform_scheduler`、`key_dispatch`、`profiler`（含
`profiler::actions`）；已迁移模块数 42 → **46**。

### 迁移要点

- **不变量收敛**：`key_dispatch`（node/view 栈非空、活动节点必存在、被重放前缀区间
  非空）、`profiler`（固定长度 `[TaskTiming; 5]` 数组迭代器必非空、`save_task_timing`
  只在计时开始后调用）、`profiler::actions`（运行栈必有元素、heapless 集合 push 前已
  查未满，`push` 返回 `Result` 走 `invariant_result`）、`keystroke`（`len()==1` 的
  字符串恰有一个字符）与 `platform.rs` 的 `NoopTextSystem::layout_line`（自身
  `glyph_for_char` 恒 `Some`、`advance` 恒 `Ok`）统一走模块内 `invariant` /
  `invariant_result` 助手，panic 语义不变。
- **可恢复失败**：`ClipboardString::with_json_metadata` 的 `serde_json::to_string`
  失败（自定义 `Serialize` 报错、非字符串键等）改为记警告并保留原元数据，剪贴板
  文本不受影响。
- **契约 / 环境致命错误保留 panic + 局部 allow**：`threaded_dispatcher` 的 3 处
  线程创建失败（worker / 定时器 / 基准实时线程，失败即调度器不可用）与 1 处
  dispatcher 拆解后派发；`platform_scheduler` 的 wasm 阻塞 panic 收进带 allow 的
  `cannot_block_on_wasm() -> !` 辅助函数（属性挂在 wasm 尾表达式 block 上不稳定）。
- **测试支撑整体放行**：`platform/test`（`TestPlatform` 的 prompt 模拟断言）与
  macOS 的 `platform/visual_test` 按第五批口径在模块声明处 `#[allow]` + 注释。

### 验证

- `cargo test -p rgpui --lib`：830 passed + 1 ignored。
- `cargo clippy --workspace --lib --bins -- -D warnings` 通过（CI 一致口径）。
- `cargo clippy -p rgpui --features test-support --lib -- -D warnings` 通过
  （`threaded_dispatcher` / `platform::test` 只在 test 或 test-support feature 下编译，
  单跑 `--lib` 验不到）。

### 剩余候选

`scene.rs`、`arena.rs`、`action.rs`、`root.rs`、`dom.rs`（`dom-backend` feature）、
`text_system.rs`、`theme/highlight.rs`、`elements/scroll/*`、
`util/scheduler/test_scheduler.rs` 等。

## 七十二、增补：lint 收紧第九批（场景 / 分配器 / 动作 / 根视图 / DOM / 文本系统 / 27 处）

延续 §六十四～§七十一 的口径，本批覆盖 6 个模块：`scene`、`arena`、`action`、
`root`、`dom`（`dom-backend` feature 门控）、`text_system`（含 `line_layout` /
`font_features` 子模块）；已迁移模块数 46 → **52**。

### 迁移要点

- **不变量收敛**（模块内 `invariant` / `invariant_result` 助手，panic 语义不变）：
  - `scene`：三种精灵批（monochrome / subpixel / polychrome）的 `peek().unwrap()` ——
    `batch_kind` 取自已排序种类表里 order 为 Some 的首项，对应迭代器必有待处理元素；
  - `arena`：`Layout::from_size_align`（chunk_size 合法时必成功）、
    `NonZeroUsize::try_from`（chunk_size 必非零）走 `invariant_result`；
  - `action`：`all_names` 与 `by_name` 由同一 `insert_action` 维护，必有一一对应；
  - `root`：窗口首层必然挂载 `rgpui::Root`（外层 Option 为「有无根视图」、内层为
    downcast 是否成功，两层各解一次）；
  - `dom`：DOM 栈与 origins 平行且非空（`begin_frame` 保证）；
  - `text_system`：`LineWrapperHandle` 构造时必带 wrapper（Drop/Deref 各一处）、
    wrapper 池必有对应槽位；`line_layout` 的字形索引必指向字符起点、行内 x 必有对应
    索引（上方已夹取到行内区间）。
- **可恢复失败**：`font_features` 的 `is_u64()` + `as_u64().unwrap()` 收敛为
  `if let Some(v) = value.as_u64()`，失败路径的错误日志保持不变。
- **契约 / 环境错误保留 panic + 函数级 allow**：`arena::alloc`（chunk_size 放不下单个
  T）、`action::insert_action`（动作/别名重复注册）、`text_system::resolve_font`
  （字体与全部回退字体解析失败，公开 API 已在 `# Panics` 声明）。
  注意 `#[allow(clippy::panic)]` 挂在 `panic!` **宏调用**上会被 rustc 忽略并告警
  （unused attribute），必须上移到函数级。
- `rgpui.rs` 新增 6 个 deny 模块声明（deny 模块 46 → 52）。
- docs §七十二 + AGENTS.md 名单同步。

### 验证

- `cargo clippy --workspace --lib --bins -- -D warnings` 通过（CI 一致口径）。
- `cargo clippy -p rgpui --features test-support --lib` 与
  `cargo clippy -p rgpui --features dom-backend --lib` 通过（`dom` 只在
  `dom-backend` feature 下编译）。
- `cargo test -p rgpui --lib`：830 passed + 1 ignored；
  `cargo test -p rgpui --features dom-backend --lib`：833 passed + 1 ignored。

### 剩余候选

`theme/highlight.rs`、`elements/scroll/*`、`util/scheduler/test_scheduler.rs`、
`keymap`（余项在测试区）、`elements/*` 小元素、`components/*` 等。

## 七十三、增补：lint 收紧第十批（滚动 / 列表 / 菜单 / 输入元素 / 20 处）

延续 §六十四～§七十二 的口径，本批覆盖 5 个模块：`elements::list`、
`elements::scroll`（含 `auto_scroll`，`scrollable` / `scrollbar` /
`scrollable_mask` 本就干净）、`menu::context_menu`、`input_ui::auto_scroll`、
`input_ui::input::element`；已迁移模块数 52 → **57**。

### 迁移要点

- **可恢复失败（锁中毒恢复）**：两份 `auto_scroll.rs`（`elements::scroll` 与
  `input_ui` 各一份，结构相同）的 4 处 `shared.lock().unwrap()` 改为
  `lock().unwrap_or_else(|poisoned| poisoned.into_inner())` —— 共享状态只是
  `Option<Pixels>`，没有跨 panic 的不变量，持锁线程中毒后仍可安全取回数据。
- **不变量收敛**（模块内 `invariant` / `invariant_result` 助手，panic 语义不变）：
  - `list`：可见项必已完成测量（`size` 为空或在可见区都会走 layout 分支）、
    自动滚动调整后的重排不会再次请求滚动（第二次以 `autoscroll=false` 调用）；
  - `context_menu`：传入 `Some(id)` 后外层状态必为 `Some`、`ContextMenu::id()`
    恒返回 `Some` 故框架必带元素 id（×2）、`element` 构造时为 `Some` 且
    `request_layout` 每帧只 `take` 一次；
  - `input::element`：行首/行尾索引必在行内（`position_for_index` 必有结果）、
    上方已检查 `line_corners` 非空故路径必有点、可见行索引来自 display_map 快照
    必能取到。
- **彻底消除**：`list` 的 `Count(usize)` 换 `std::cmp::Ord::cmp`（整数比较恒有
  次序；须完全限定以避开 `SeekTarget::cmp` 同名冲突）、`Height(Pixels)` 换
  `f32::total_cmp`（NaN 也给确定次序，消除 `partial_cmp().unwrap()`）。
- deny 属性加在子模块声明处（`elements/mod.rs` 的 `mod list` / `mod scroll`、
  `menu/mod.rs` 的 `mod context_menu`、`input_ui/mod.rs` 的 `mod auto_scroll`、
  `input_ui/input/mod.rs` 的 `mod element`），不扩大到父模块整棵树。
- docs §七十三 + AGENTS.md 名单同步。

### 验证

- `cargo clippy --workspace --lib --bins -- -D warnings` 通过（CI 一致口径）。
- `cargo test -p rgpui --lib`：830 passed + 1 ignored。
- `cargo fmt --all --check` 干净。

### 剩余候选

`theme/highlight.rs`、`util/scheduler/test_scheduler.rs`（已带 invariant 助手）、
`rgpui_util/mod.rs`（`debug_panic!` 宏定义处需单独处理）、`perf/implementation.rs`、
`util/mod.rs`（余项）、`elements/*` 小元素、`components/*` 等。

## 七十四、增补：lint 收紧第十一批（util 全树 / refineable / rgpui_util / perf / 19 处）

延续 §六十四～§七十三 的口径，本批覆盖 4 个 deny 落点：`util`（全树，含
`scheduler` / `rel_path` / `collections` 等子模块）、`refineable`、`rgpui_util`、
`perf::implementation`；已迁移模块数 57 → **61**。

### 迁移要点

- **可恢复失败**：
  - `util::load_shell_from_passwd` 的 passwd shell 非 UTF-8 panic 改为 `?` 转
    `Err` —— 该函数本就返回 `Result` 且调用方 `.log_err()`，与既有
    `anyhow::ensure!` 失败通道一致；
  - `test_scheduler` 两处错位的语句级 `#[allow]`（只盖住 `let` 语句、盖不住后面
    的 `panic!`）上移到函数级。
- **不变量收敛**（`invariant` / `invariant_result` / `invariant_err` 助手）：
  - `util`：内嵌资源构建期必为 UTF-8（×2）、emoji 正则为编译期字面量；
  - `refineable`：级联槽位 0 必为基础细化（`base` / `merged` 各一处）；
  - `perf::implementation`：成功用例必有迭代次数（×3）、无元数据时用例必为失败
    （`unwrap_err` 走新增的 `invariant_err`）。
- **契约 / 公开语义保留 panic + 函数级 allow**：`rgpui_util::some_or_debug_panic`
  （该函数职责就是「缺失即暴露」）、`UnwrapFuture::poll`（文档已声明 Err 即 panic）、
  `rel_path` / `rel_path_buf`（测试支撑辅助，路径写错是测试作者的错误，`unwrap` 走
  allow）。
- **测试支撑整体放行**：`util::test`（`#[cfg(any(test, feature = "test-support"))]`）
  在模块声明处 `#[allow]` + 注释，覆盖其子模块 `assertions` / `marked_text`。
- **宏展开不触发 lint 的实证**：`debug_panic!`（本地 `macro_rules!`）在已 deny 的
  `app.rs` 中调用通过 —— `clippy::panic` 不追本地宏展开，故 `rgpui_util` 的宏定义
  体与 `debug_panic!` 调用点无需处理；deny 加在 `rgpui.rs` 的 `mod rgpui_util`、
  `mod util`、`pub mod refineable` 与 `perf/mod.rs` 的 `mod implementation` 处。
- 注意 `unwrap_err()` 属 `clippy::unwrap_used`，纯 `.unwrap()` 文本扫描会漏，需
  单独扫（本批在 `perf` 抓到 1 处；`util` 余项都在 `#[cfg(test)]` 区由 clippy.toml
  放行）。
- docs §七十四 + AGENTS.md 名单同步。

### 验证

- `cargo clippy --workspace --lib --bins -- -D warnings` 通过（CI 一致口径）。
- `cargo clippy -p rgpui --features test-support --lib` 通过（`util::test`、
  `rel_path` 辅助只在 test / test-support 下编译）。
- `cargo test -p rgpui --lib` 与 `--features test-support --lib`：各 830 passed +
  1 ignored。
- `cargo fmt --all --check` 干净。

### 剩余候选

`theme/highlight.rs`、`fast/*`（余项在测试区）、`rgpui_util/arc_cow.rs`、
`elements/*` 小元素、`components/*`、`table/state.rs`、`input_ui/indent.rs` 等。

## 七十五、增补：lint 收紧第十二批（画布 / 表格 / 分栏 / 缩进 / 11 处）

延续 §六十四～§七十四 的口径，本批覆盖 4 个模块：`elements::canvas`、
`table::state`、`components::resizable`、`input_ui::indent`；已迁移模块数
61 → **65**。另给根 `test` 模块（`#[cfg(any(test, feature = "test-support"))]`）
补了测试支撑放行声明。

### 迁移要点

- **不变量收敛**（模块内 `invariant` / `invariant_result` 助手）：
  - `canvas`：prepaint / paint 回调构造时必带、每帧各 `take` 一次（×3）；
  - `table::state`：列索引来自命中测试与表头渲染循环，必在 `col_groups` 内
    （原两处 `expect("BUG: invalid col index")`）；
  - `resizable`：`ResizeHandle::id()` 恒返回 `Some`，框架必带元素 id（×2）。
- **彻底消除**：`table::state` 的 `sort` 先 `is_none` 返回再 `unwrap` 收敛为
  `let Some(sort) = sort else { return; }`，不再需要助手。
- **契约保留 panic + 函数级 allow**：`resizable::RenderOnce::render` 的
  `expect("ResizablePanel must be used within a ResizablePanelGroup")` ——
  面板必须由 group 渲染注入 state，属公开用法契约。
- **助手类型注意**：`indent` 的 `PathBuilder::build()` 返回 `Result`（非
  `Option`），走 `invariant_result`；同时删掉该模块无使用点的 Option 版助手，
  避免 `dead_code` 告警。
- deny 属性加在子模块声明处（`elements/mod.rs` 的 `mod canvas`、`table/mod.rs`
  的 `mod state`、`components/mod.rs` 的 `pub mod resizable`、`input_ui/mod.rs`
  的 `mod indent`），共 4 处（deny 模块 61 → 65）。
- docs §七十五 + AGENTS.md 名单同步。

### 验证

- `cargo clippy --workspace --lib --bins -- -D warnings` 通过（CI 一致口径）。
- `cargo clippy -p rgpui --features test-support --lib` 通过（根 `test` 模块只在
  test / test-support 下编译）。
- `cargo test -p rgpui --lib` 与 `--features test-support --lib`：各 830 passed +
  1 ignored。
- `cargo fmt --all --check` 干净。

### 剩余候选

`theme/highlight.rs`、`fast/*`（余项在测试区）、`rgpui_util/arc_cow.rs`、
`elements/*` 小元素（`alert` / `avatar` / `badge` / `tag` / `kbd` 等）、
`components/*`（`pagination` / `steps` / `timeline` 等）、`menu/` 余项、
`form/*`、`title_bar/*` 等。

## 七十六、增补：lint 收紧第十三批（elements 小元素族 / 9 处）

延续 §六十四～§七十五 的口径，本批覆盖 `elements` 树下 6 个小元素模块：
`anchored`、`animation`、`deferred`、`image_cache`、`uniform_list`、
`virtual_list`；已迁移模块数 65 → **71**。

### 迁移要点

- **不变量收敛**（模块内 `invariant` / `invariant_result` 助手）：
  - `anchored`：上方已 `is_empty` 提前返回，非空集合求归约必有结果；
  - `animation`：`id()` 恒返回 `Some` 故框架必带元素 id；`element` 构造时必带
    且每帧只取一次（原 `expect("should only be called once")`）；
  - `deferred`：`child` 构造时必带、`request_layout` / `paint` 各取用一次；
  - `image_cache`：调用方按 `I` 声明 `AnyEntity` 实体类型，`downcast::<I>()`
    返回 `Result` 走 `invariant_result`；
  - `uniform_list`：本元素是滚动容器，`request_layout` 阶段必已设置共享
    `scroll_offset`；
  - `virtual_list`：`id()` 恒返回 `Some`，框架必带元素 id。
- deny 属性加在 `elements/mod.rs` 的 6 个子模块声明处，不扩大到 `elements`
  父模块整棵树（deny 模块 65 → 71）。
- docs §七十六 + AGENTS.md 名单同步。

### 验证

- `cargo clippy --workspace --lib --bins -- -D warnings` 通过（CI 一致口径）。
- `cargo clippy -p rgpui --features test-support --lib` 与
  `cargo clippy -p rgpui --features dom-backend --lib` 通过（`anchored` 含
  dom-backend cfg 代码）。
- `cargo test -p rgpui --lib`：830 passed + 1 ignored。
- `cargo fmt --all --check` 干净。

### 剩余候选

`components/*`（`otp_input` / `sparkline` / `status_bar` / `charts/*` /
`navigation_menu` / `description_list` / `date_picker` / `segmented_nav` /
`animated_collapsible`）、`tabs::accordion`、`keymap::binding`、
`theme::registry` / `theme::schema`、`input_ui::history`、`file_watcher`、
`queue`、`single_instance`、`webview`、`tokio`、`system/*`、
`highlight::tree_sitter`、`animation::animate`、`elements::alert/avatar/badge` 等
（`alert` / `avatar` / `badge` / `tag` / `kbd` / `form` / `title_bar` 当前无站点）。

## 七十七、增补：lint 收紧第十四批（components 族 + charts 全子树 / 13 处）

延续 §六十四～§七十六 的口径，本批覆盖 11 个模块：`components` 下的
`otp_input` / `sparkline` / `status_bar` / `animated_collapsible` /
`navigation_menu` / `description_list` / `date_picker` / `segmented_nav`
8 个组件，以及 `components::charts`（deny 挂在 `pub mod charts` 上，覆盖
`chart` / `area_chart` / `line_chart` 等全部 10 个子模块）；已迁移模块数
71 → **80**（本批 9 个 deny 落点）。

### 迁移要点

- **不变量收敛**（全部 13 处均为「守卫已保证存在」型，模块内 `invariant` 助手）：
  - `is_some()` 守卫型：`otp_input` 的分隔符（`should_show_separator` 含
    `separator.is_some()`）、`status_bar` 的回调（`clickable = on_click.is_some()
    && !disabled`）、`animated_collapsible` / `navigation_menu` 的 `.when(…
    on_toggle.is_some())` 分支、`segmented_nav` 的 `.when(active_index.is_some())`；
  - 非空集合型：`sparkline`（`point_count < 2` 提前返回）、`charts::area_chart`
    （`s.points.len() < 2 → continue` + 同迭代内 `baselines` 已 push）、
    `charts::line_chart` / `charts::chart`（`screen_points.len() >= 2` 守卫）、
    `description_list`（`rows.is_empty()` 时已 push 首行）；
  - 构造不变量型：`otp_input` 的焦点句柄（构造时必带 ≥1 个）、`date_picker` 的
    `NaiveDate::from_ymd_opt`（日历网格年月日由合法值推导）。
- deny 属性加在 `components/mod.rs` 的 8 个模块声明处 + `pub mod charts`
  （`#[cfg(feature = "charts")]` 之下，10 个子模块整体覆盖；除 3 个已迁移文件外
  其余 7 个子模块本就干净）（deny 模块 71 → 80）。
- docs §七十七 + AGENTS.md 名单同步。

### 验证

- `cargo clippy --workspace --lib --bins -- -D warnings` 通过（CI 一致口径，
  workspace 特性并集含 `charts`）。
- `cargo clippy -p rgpui --features charts --lib` 与
  `cargo clippy -p rgpui --features test-support --lib` 通过。
- `cargo test -p rgpui --lib`：830 passed + 1 ignored。
- `cargo fmt --all --check` 干净。

### 剩余候选

`tabs::accordion`、`keymap::binding`、`theme::registry` / `theme::schema`、
`input_ui::history`、`file_watcher`、`queue`、`single_instance`、`webview`、
`tokio`、`system::svg_renderer` / `system::focus_trap`、`highlight::tree_sitter`、
`animation::animate`、`fast::layout_key`、`menu/` 余项、`form/*`、`title_bar/*` 等。

## 七十八、增补：lint 收紧第十五批（杂项模块族 / 19 处）

延续 §六十四～§七十七 的口径，本批覆盖 15 个分散模块：`tabs::accordion`、
`keymap::binding`、`theme::registry`、`theme::schema`、`input_ui::history`、
`queue`、`single_instance`、`file_watcher`、`tokio`、`webview`、
`system::focus_trap`、`system::svg_renderer`、`highlight::tree_sitter`、
`animation::animate`、`fast::layout_key`；已迁移模块数 80 → **95**。

### 迁移要点

- **不变量收敛**：
  - `id()` 恒返回 `Some` 型：`tabs::accordion`（×2）、`system::focus_trap`；
  - 内嵌资源构建期有效型（`invariant_result`）：`theme::registry` 的内嵌默认主题
    JSON（×2）、`theme::schema` 的自生成 JSON Schema 序列化、
    `highlight::tree_sitter` 的内置高亮查询、`system::svg_renderer` 的 emoji
    字面正则；
  - 守卫非空型：`input_ui::history`（同版本 `count > 0` 守卫内 pop，×2）、
    `queue::pop`（`recv()` 非空守卫 + 末级优先级权重必中 w/w = 1.0）、
    `fast::layout_key`（进入子步进计算前必已 push 栈帧）；
  - 位图一致性型：`svg_renderer` 的 `ImageBuffer::from_raw`（尺寸×缓冲一致）。
- **彻底消除**：`animation::animate` 关键帧排序用 `f32::total_cmp` 取代
  `partial_cmp().unwrap()`；`single_instance` 的 `is_err` 提前返回 + `unwrap`
  收敛为 `let Ok(handle) = handle else { return Err(AlreadyRunning) };`。
- **可恢复失败**：`file_watcher` 两处 `Mutex::lock().unwrap()` 走锁中毒恢复
  `unwrap_or_else(|p| p.into_inner())`（监视器注册表无跨 panic 不变量，
  轮询任务中毒后继续用现有状态）。
- **契约保留 panic + 函数级 allow**：`keymap::binding::new`（文档声明「解析错误时
  触发 panic」）、`tokio::init`（运行时构建失败属环境致命，启动即失败）。
- **webview 无代码改动**：唯一 `.unwrap()` 位于文档示例（`//!` 代码块）中，
  不参与 lint；deny 直接加在模块声明上（`feature = "webview"` 门控）。
- deny 属性 15 处：`tabs/mod.rs`、`keymap.rs`、`theme/mod.rs`（×2）、
  `input_ui/mod.rs`、`rgpui.rs`（queue / single_instance / webview / file_watcher /
  tokio ×5）、`system/mod.rs`（×2）、`highlight/mod.rs`、`animation/mod.rs`、
  `fast/mod.rs`（deny 模块 80 → 95）。
- docs §七十八 + AGENTS.md 名单同步。

### 验证

- `cargo clippy --workspace --lib --bins -- -D warnings` 通过（CI 一致口径）。
- `cargo clippy -p rgpui --features tree-sitter --lib`（tree_sitter deny）、
  `--features tokio --lib`、`--features webview --lib`、
  `--features test-support --lib` 均通过。
- `cargo test -p rgpui --lib`：830 passed + 1 ignored。
- `cargo fmt --all --check` 干净。

### 剩余候选

`menu/` 余项、`form/*`、`title_bar/*`、`tabs/` 其余（`tab_bar` 等）、
`theme/` 其余（`settings` / `highlight` 等）、`keymap/` 其余（`context` /
`file` / `keymap`）、`system/` 其余（`tab_stop` / `tray` / `window_border` /
`window_ext`）、`animation/` 其余、`fast/` 其余（`dependencies` / `stats`）
等（各父模块其余子模块当前无站点）。

> §七十九 复核更正：上面这份「剩余候选」是按 grep 命中数排的，而 grep 把
> `#[cfg(test)]` 里的站点也算了进来——这些模块的生产代码当时其实早已迁移干净，
> 缺的只是 deny 落点。第十六批因此改为一次性把核心 crate 全量封死，
> 后续批次口径换成兄弟 crate（见 §七十九 的表）。

## 七十九、增补：lint 收紧第十六批（核心 crate 全量封死 + 构建脚本 + markdown/character 补章 / 核心生产代码 0 处改动）

延续 §六十四～§七十八 的口径，本批收尾核心 crate：**`rgpui` 的生产代码已经没有
`unwrap` / `expect` / `panic!` 站点**，所以不再逐模块盖章，改为一条 crate 根属性把
整个 crate 封死。

### 度量方法（重要，之前的「剩余候选」名单失真）

用 `cargo clippy --workspace --lib --bins --all-features -- -W clippy::unwrap_used
-W clippy::expect_used -W clippy::panic` 把三个 lint 在**全 crate、全 feature** 范围
临时打开计数。要点：

- `--lib` 排除 `#[cfg(test)]`，测的就是生产代码；测试代码由根目录 `clippy.toml` 的
  `allow-*-in-tests` 放行，不受收紧影响。
- `--all-features` 把 feature 门控代码（`dom-backend` / `control-channel` / `editor`
  / `webview` / `latex` …）一并纳入，避免第四批那种「单包验证漏掉门控代码」的翻车。
- 据此复核 §六十四 以来的「剩余候选」：`rich_text.rs`（29 处命中）、`ink.rs`（24）、
  `mdi.rs`（27）、`keymap/context.rs`（44）、`elements/scroll/scrollable.rs`（40）
  **全部落在第一个 `#[cfg(test)]` 之后**，即早已迁移完毕；grep 直接计数会把测试站点
  算成待办，是名单失真的原因。

### 本批改动

- **crate 根属性**：`rgpui.rs` 顶部 `#![deny(clippy::unwrap_used, clippy::expect_used,
  clippy::panic)]`，覆盖面等价于给 `rgpui.rs` 里 62 个尚无祖先 deny 的顶层模块声明
  逐个补章，但只有一行、且把根文件自身的条目也纳入了。
- **清理冗余**：删除散在 19 个文件里的 **95 处**模块级 `#[deny(...)]`（写法完全一致，
  整行删除）。就近放行**一律保留**，全 crate 共 **149 处**：
  `allow(clippy::panic)` 122、`allow(expect_used, panic)` 12、
  `allow(unwrap_used, expect_used, panic)` 9、`allow(unwrap_used, panic)` 3、
  `allow(expect_used)` 2、`allow(unwrap_used)` 1。其中挂在模块声明上的只有那 9 处三合一，
  且全部由 `cfg(test)` / `feature = "test-support"` / `feature = "bench"` 门控
  （`app::{bench_context, headless_app_context, test_app, test_context,
  visual_test_context}`、`platform::{test, visual_test}`、`rgpui::test`、`util::test`）；
  其余是函数/语句级就近放行，带中文注释说明为什么 panic 是契约。crate 根属性生效后
  这些仍然必要——审计过一遍，**生产侧没有一处整棵子树的豁免**。
- **封死的边界要说清**：三个 lint 看不见 `unreachable!` / `unimplemented!`（它们展开成
  `panic` 但不是 `panic!` 的直接调用）。本批给核心 crate **所有裸 `unreachable!()`
  补上消息**（生产侧 `util/sum_tree/cursor.rs` 6 处、`util/paths.rs` / `util/mod.rs` /
  `theme/color.rs` 各 1 处；复核又补了 `util/sum_tree/mod.rs` 3 处 —— 等高合并的两处
  let-else 与 `_ =>` 兜底、测试侧 `util/test/marked_text.rs` 1 处 +
  `platform/app_menu.rs` 2 处），不变量被破坏时能一眼看出是哪个分支。
  终态：`grep -rn 'unreachable!()' crates/rgpui/src` **为空**。
  `unimplemented!` 只存在于 `platform/test/*` 的测试桩、`key_dispatch.rs` 的
  `#[cfg(test)]` 假输入源与 `action.rs` 的文档示例（`/// #` 行），生产侧没有，
  因此不把 `clippy::unimplemented` 加进 deny 列表。
- **构建脚本纳入体系**：`rgpui/build.rs`（1 处）、`rgpui-windows/build.rs`（12 处）、
  `rgpui-macos/build.rs`（13 处）此前完全在模块属性之外。三处统一：
  `fn main() -> Result<(), Box<dyn std::error::Error>>` + `?` 传播，顶部加
  `#![deny(...)]`。语义不变（失败仍然中止构建），诊断反而更好——
  - `rgpui-windows`：`find_fxc_compiler` 由 `panic!("Failed to find fxc.exe")` 改为
    返回 `Err`，且 Windows SDK 注册表查询的错误不再被 `if let Ok(Some(..))` 吞掉；
    着色器编译失败仍走 `cargo::error=` + `process::exit(1)`（本来就是 lint 干净路径）。
  - `rgpui-macos`：`xcrun --show-sdk-path` / bindgen / metal / metallib 的
    `unwrap` / `expect` / `panic!` 换成带上下文的 `Err`。
  - `rgpui`：`manifest_required().unwrap()` 改为 `map_err` 携带原始错误。
- **顺带封掉两个纯 Rust crate**：`rgpui-markdown`、`rgpui-character` 实测 0 站点，
  且都不含平台 `cfg` 门控代码，直接在 crate 根（`src/lib.rs` /
  `src/rgpui_character.rs`）补 `#![deny(clippy::unwrap_used, clippy::expect_used,
  clippy::panic)]`，零代码改动。

### 验证

- `cargo clippy --workspace --lib --bins --all-features -- -D warnings` 通过
  （CI 一致口径，含 crate 根 deny 与三个 build.rs 的文件级 deny；Windows 端）。
- 核心 crate 生产代码在全 feature 下**零告警**，即根属性生效后无需任何就地 allow。
- **反向验证**（证明根属性确实在此前裸奔的模块上生效）：临时在 `color.rs::rgb` 里加
  `Some(hex).unwrap()` / `.expect("probe")` / `panic!("probe")` 三种写法，
  `cargo clippy -p rgpui --lib -- -D warnings` 三条全部报错
  （`used unwrap() on an Option value` / `used expect() ...` /
  `panic should not be present in production code`），随后撤掉探针。
- `cargo test -p rgpui --lib`：830 passed + 1 ignored（与第十五批持平）。
- `cargo test -p rgpui-markdown --lib`：10 passed（新章生效后测试代码不受影响，
  `clippy.toml` 的 `allow-*-in-tests` 仍然放行）。
- `cargo fmt --all` 干净。

### 剩余候选（口径改口：兄弟 crate）

核心 crate 已封死，下一站是把同一套 lint 推到平台/工具 crate（分支名
`feat/lint-unwrap-crates-a` 的 crates-b）。

度量口径（本次修正）：一次 `cargo clippy --workspace --lib --bins --all-features
-- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic`，把每条
warning 的主 span `文件:行:列` 去重 —— clippy 对同一站点会随编译路径重复播报，
而 `-p <crate> --all-features` 又会把依赖 crate 的告警一起带进来，两个口径都虚高。

| crate | 站点 | 集中位置 |
|-------|------|----------|
| `rgpui-windows` | 59 | `directx_renderer` 24、`direct_write` 11、`platform` 6、`directx_atlas` 6、`directx_devices` 3，尾部 `window` / `events` 各 2、`vsync` / `keyboard` / `global_hotkey` / `dispatcher` / `clipboard` 各 1 |
| `rgpui-wgpu` | 21 | `wgpu_renderer` 18、`wgpu_atlas` 3 |
| `rgpui-macros` | 12 | `derive_refineable` 5、`icon_named` 3、`derive_action` 2、`styles` 1、`rgpui_macros` 1 |
| `rgpui-term` / `rgpui-3d` / `rgpui-platform` | 2 / 2 / 1 | `terminal_element`、`context`、`rgpui_platform` |
| 合计 | **97** | |

`rgpui-markdown` 与 `rgpui-character` 实测 **0 站点**（两者都没有平台 `cfg` 门控
代码），本批直接补上 crate 根 `#![deny]`，零代码改动封死。

同一批复核还漏了 **裸 `unreachable!()`**（三个 lint 看不见它）：核心已清零，
兄弟 crate 的存量是 `rgpui-macos` 6、`rgpui-windows` 4、`rgpui-linux` 4、
`rgpui-term` 2、`rgpui-macros` 1 —— 各批次迁移时一并补消息，别只盯 97 这个数字。

需要 CI 矩阵度量（本地交叉编译缺 C 工具链，`x86_64-unknown-linux-gnu` /
`aarch64-apple-darwin` 都停在 `cc-rs` 找不到编译器），下面是 **grep 上界**
（含 `#[cfg(test)]`，只用于排批次顺序，不是精确站点数）：

| crate | grep 上界 | 集中位置 |
|-------|-----------|----------|
| `rgpui-linux` | 104 | `linux/wayland/client` 40、`linux/platform` 18、`linux/x11/client` 16、`linux/dispatcher` 13，尾部 `wayland/window` 5、`x11/window` 4、`wayland/clipboard` / `headless/client` 各 3 |
| `rgpui-macos` | 70 | `metal_renderer` 21、`pasteboard` 10、`text_system` 8、`window` 6、`metal_atlas` 5、`screen_capture` / `platform` / `notifications` 各 4 |
| `rgpui-dom` | 18 | 全在 `web.rs`（`#[cfg(target_family = "wasm")]` 门控，本地根本不编译） |
| `rgpui-web` | 4 | `dispatcher` 3、`platform` 1 |

渲染后端批次的口径与 UI 层不同：多为「这帧画不出来」的可恢复失败，落到
`Result` + 日志降级，改动面大于核心批，按 crate 逐个开。
`rgpui-dom` 非 wasm 部分（`lib.rs` / `css.rs` / `diff.rs`）实测 0 站点，
但 `web.rs` 有 18 处，所以它不能像 markdown / character 那样零成本封死。

### CI 口径的第二处盲区：`--lib --bins` 看不见 tests / examples

CI 的 clippy 步骤是 `cargo clippy --workspace --lib --bins -- -D warnings`，
`--lib` 不编译 `#[cfg(test)]` 代码，也不编译 `examples/` 目标。workspace 级
`deny` 的 `redundant_clone` 因此一直没管到测试代码，本地按 AGENTS.md 的开发命令
（`cargo clippy -p <pkg> --all-targets -- -D warnings`）一跑就红：

- `rgpui-dom` 测试代码 **13 处** `redundant_clone`（`diff.rs` 7、`lib.rs` 6，
  全是「克隆完就丢弃」的末次使用），已用 `cargo clippy --fix` 就地删掉克隆；
  `cargo test -p rgpui-dom --all-features` 15 passed。
- `rgpui-3d` 两个示例各 1 处：`viewer3d.rs` / `scenekit_skin.rs` 里最后一个回调
  的 `let state = state.clone();` —— 闭包本来就是 `move`，直接接管外层 `state`
  即可，删掉这行 shadow。

复核后 `cargo clippy --workspace --all-targets --all-features --keep-going -- -D warnings`
在 Windows 端全绿（含 tests / examples / benches），说明这 15 处是存量唯一的
`--all-targets` 违规。

**建议（未采纳，需决策）**：把 CI 的 clippy 步骤改成 `--all-targets` 就能永久堵住
这个盲区，但 mac / linux runner 上 `cfg` 门控的测试代码从未按此口径度量过，
贸然改法可能让 CI 因存量问题变红 —— 先在两条 runner 上单独跑一次
`--all-targets` 看清单，再决定要不要并入 CI。
