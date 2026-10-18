# rgpui 做 Office 套件的基础缺口分析

> 调研文档。参考 `temp/storytold` 下 4 个「用 egui 复刻 Office/Adobe」的 Rust 项目，
> 梳理 rgpui 作为 UI 基座要落地 Office 类应用的前置能力：哪些要**开发**（框架缺组件/能力）、
> 哪些要**借鉴**（工程范式）、哪些**不该进框架**（领域/引擎职责）。
> 目的：在做 Office 之前先把地基打牢，避免重复造轮子。

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

**结论**：rgpui 是「通用 UI 框架」，Office 所需的**特征组件（Ribbon 系）与文档/画布类能力基本缺席**；
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

### B. 现有能力需增强（有但不敷 Office 用）

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

**P1 特征控件**：KeyTips、QAT/Mini toolbar、Backstage、公式栏、电子表格网格、文档页视图、
画布对象工具、富剪贴板、富文本/文档编辑控件雏形、Ink。

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

## 七、结论

- storytold 已证明「纯 Rust、引擎优先、UI 可换」路线可行，**rgpui 完全可以作为其 UI 基座**；
- 真正的缺口不在渲染/布局（rgpui 有），而在 **Office 特征 UI（Ribbon 系）** 与
  **文档/画布/网格类控件**，以及**命令注册表**这一贯穿全栈的骨架；
- 建议先落 P0「命令 + Ribbon + 视口 + 标尺/参考线」，它们被四个应用共享，且是 Office 的
  身份特征；引擎类能力一律留在应用层。
