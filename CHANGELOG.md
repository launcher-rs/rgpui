# Changelog

本项目遵循 [语义化版本控制](https://semver.org/lang/zh-CN/)。

## [Unreleased] - 1.3.0（`feat/1.3.0` 开发中）

### 检查器完善（Inspector）

- **I1 树节点 ↔ 界面双向映射**：`Inspector::select` 公开 +
  `select_ancestor(levels_up)`（全局路径前缀反查 + hitbox 包含消歧）+
  `Window::select_inspector_ancestor`；打开期间常驻 hitbox，选中区橙框常驻高亮
- **I2 完整元素树 + 逐节点折叠**：prepaint 嵌套记录 parent→children
  （`Frame` 存树，面板自举暂停，关闭零开销）；`is_inspector_open` 门控视图缓存
  与全量 Div hitbox；公开 `roots/children/parent` + `select_inspector_element` +
  `InspectorElementId::{short_label, source_label, tree_key}`
- **I3 检查面板内置化**：新增 `rgpui::inspector_panel` +
  `App::enable_default_inspector()`（两行出完整面板）；
  `examples/inspector/` 瘦身为纯默认面板演示；新增 `examples/inspector_custom/`
 （全自写面板 + 覆盖 Div 展示的 living recipe，两示例演示内容各自独立）；
  选中 Div 卡新增盒模型示意图 + 已指定样式列表（颜色 hex 可读）；
  值统一点击复制 + 打钩反馈；面板左缘拖拽调宽；完整树卡新增“复制树文本” +
  `Window::inspector_tree_text`（AI 可读导出）；面板底部新增运行卡
  （帧率/CPU/内存/GPU，仅打开时采样）与报错卡（`App::report_error` 错误环）；
  崩溃快照（`InspectorSnapshot` + `last.json` 滚动落盘 + panic 日志钩子）；
  选中 Div 卡新增盒模型示意图 + 已指定样式列表；面板底部新增运行卡
  （帧率/CPU/内存/GPU，仅打开时采样）与报错卡（`App::report_error` 错误环）；
  崩溃快照（`InspectorSnapshot` + `last.json` 滚动落盘 + panic 日志钩子）；
  祖先链面板移除（完整树唯一）+ 顶栏固定 + 删调试按钮只留 F12 +
  F12 全局绑定修复 + release 自动剥离（示例默认不开 feature）
- **I4 自定义接口文档化**：`docs/rgpui-book/09-inspector.md` 检查器章节
  （启用/分工表/双 recipe/树与选中 API）

### 回调签名统一（C1，breaking）

- 纯点击一律 `on_click: Fn(&ClickEvent, &mut Window, &mut App)`（`BreadcrumbItem` 补事件参数）；
  值变更一律 `on_change: Fn(Value, …)` 按值传递；存储统一 `Arc + Send + Sync`
- 改名：`Checkbox` / `Switch` / `Radio` / `RadioGroup` / `TabBar` 的 `on_click`→`on_change`；
  `Sidebar` / `Upload` / `NavigationMenu` 的 `on_select`→`on_change`
- 传导：`dialog` / `alert_dialog` / `empty_state` / `notification_center` / `search_panel`
- 新增 `Context::listener_value`（按值版 `listener`）；book 回调章节更新

### 应用实战回流（G）

- **G1**：`Root::open_dialog` 返回 `DialogId` + `close_dialog_by(id)` 按标识关闭
- **G2**：book 落 App 回调回实体 recipe（直挂/Root 穿透两段式）
- **G3**：`TreeEvent::Confirmed(id)`（Enter 文件行触发）
- **G4**：新增值驱动 `rgpui::tabs::{Tabs, TabsItem}`（无实体静态页签）
- **G5**：菜单位置约束记入 book（光标直接取，锚点前置计算，不硬上 API）
- **G6**：`impl Global for I18nManager` + `load_locale_dir` + `I18nSnapshot` 快照回退
- **G7**：`Dialog::overlay_visible(bool)` setter（裸挂出变暗背景）

### 移除

- **Z1**：删除 `chat_ui` deprecated 别名（1.2.0 迁移到 `chat`，按计划 1.3.0 删除）

### 第二批加菜（H，breaking 一次收完）

- **H1 回调残留统一（C2）**：`InteractiveText::on_click`→`on_change`（范围索引按值）；
  `CompletionPopup::on_select`→`on_change`（补全索引按值）；
  `Link::on_click` / `StatusBarItem::on_click` 补 `&ClickEvent`；
  `HotkeyInput::on_change` 由 `Option<&HotkeyValue>` 改按值 `Option<HotkeyValue>`，
  `HotkeyListInput::on_change` 由 `&[HotkeyValue]` 改按值 `Vec<HotkeyValue>`；
  `OTPInput::on_change` / `on_complete` 补 `&mut Window`
  （订阅常在按键分发中触发、同步拿不到窗口，故经 `spawn` 延后分发，
  无活动窗口时跳过；回调不再与触发同步，见迁移指南第七节）；
  `PopupMenuItem::on_click` / `Notification::on_click` / `on_close` /
  `ListItem::on_click` / `SegmentedNav::on_change` / `Command::on_select` /
  `CommandPalette::on_close` 由 `Rc` / `Box` 转 `Arc + Send + Sync`
  （`Command` 执行语义保留原名）；book 回调章节同步
- **H2 全局动作 helper**：`App::on_global_action(action, keystroke, handler)`
 （全局绑定 + 打活动窗口 + spawn 延后更新三件套，F12 沉淀；`handler` 只要求
  `'static`）；两检查器示例改吃 helper，book 同步
- **H3 i18n 小补强**：`I18nText::translate_global(cx)`（读全局管理器，未设置回退 key）
- **H4**：`rgpui_story` tabs 页加静态 `Tabs` 演示
- **H5 面板插槽化**：`InspectorPanelSlots::{header, section}` +
  `App::set_inspector_panel_slots`（默认外皮 `default_inspector_header` /
  `default_inspector_section` 可复用包裹；注册表状态展示不经过 section 插槽）+
  回归测试 + book recipe
- **H6**：新增 `examples/v1_3_showcase`（1.3 新 API 集中演示二进制）

## [1.2.2] - 2026-09-15

### 修复

- **Checkbox / Radio 选中对勾缺失**（`elements/checkbox.rs`）：对勾图标此前仅设路径、
  运行时依赖宿主 `AssetSource` 解析，未配置资源的窗口中画不出，只剩纯色底（黑框）；
  现附带编译期嵌入字节，不依赖宿主资源；`Radio` 复用同一绘制，一并修复

### 新增

- **Select 下拉跟随触发器宽度**：`Popover::match_trigger_width`（默认关闭）+
  `DropdownMenuPopover` 透传 + `PopupMenu` 菜单实体 min/max 宽度同步；
  `Select` 默认开启，弹层卡片与选项高亮均与触发器等宽，普通操作菜单保持内容宽度
- **form / inspector 示例**：表单组件示例（垂直/水平/网格表单、提交校验）；
  元素检查器示例（开关面板、拾取、祖先链树、布局边界展示）
- **window_showcase 自定义标题栏示例**（PR #20）

## [1.2.1] - 2026-09-14

### 修复

- **Sidebar 选中态用错 token**（`components/sidebar.rs`，PR #18）：选中底/文字/hover/角标改用
  `sidebar_accent` 系列（浅色 `#e5e5e5` 底 + `#171717` 字，深色配套值），浅色主题选中行恢复可读

### 新增

- **SidebarItem 自定义选中颜色**：`with_selected_background` / `with_selected_foreground`
  （条目级覆盖，仅作用于本条目；未设置回退主题 token）
- **sidebar 示例**：亮色/暗色切换 + `sidebar_accent` vs `accent` 色板 + 角标与自定义颜色演示

## [1.2.0] - 2026-09-14

### 新增组件

- **Sidebar**：导航侧边栏（条目/选中/折叠图标栏）
- **Select / Combobox / DatePicker / ColorPicker**：下拉选择、可搜索下拉、日历、颜色选择
- **Avatar / Alert / Breadcrumb / Card / Typography / Toggle(+Group)**：基础元素
- **Pagination / Steps / Timeline / Rate / Popconfirm**：导航反馈件
- **AI-chat**（对标 AntDX，`chat/`）：`Prompts`、`Suggestion`、`ThoughtChain`、
  `Attachments` + `FileCard`、`Sources`、`Actions`、`Sender`；
  新增 `Bubble`（左右气泡）、`MessageScroller`（吸底 + 新消息浮标）、`Marker`（时间/未读分隔线）
- **Dock 布局**（`components/dock.rs`）：左/右/底部/中央四区域标签页，标签拖拽跨区、
  关闭/显隐、布局 JSON 持久化
- **Upload**：选择区 + 文件列表 + 进度条，对接平台原生文件对话框（Web 降级）
- **Carousel**：索引切换 + 自动播放 + 循环 + 指示器 + 滑动手势
- **MermaidDiagram**：`flowchart` 子集（LR/TB/RL/BT，矩形/圆角/菱形/圆形/直线箭头/边标签）转 SVG
- **Sidebar 增强**：`SidebarSection` 分组 + 可折叠 + 条目 `badge` 角标
- **Tree**：无需新写（`TreeItem`/`TreeState`/`Tree` 已存在），随 `dialog`/`sidebar` 示例验证可用

### 编辑器与输入

- **编辑器分拆结算**（`input_ui/input/` 表单 + `input_ui/editor/` 编辑器 + `TextCore` 共享核）：
  `EditorState`（包 `Entity<InputState>` 编排外壳 + 大纲缓存）+ `Editor` 组件（复用 `Input` +
  状态行）；`InputMode::CodeEditor` 删除，`TextArea` 补为表单多行；差集透传
  `set_value`/`reveal_*`/`set_read_only`/`set_line_comment_prefix`/`set_highlighter`/
  `goto_symbol`（高亮/写入补刷大纲）；`input/state.rs` 新功能冻结
- **LSP 编辑器侧接线**（`editor/lsp_attach.rs`，拉模型）：`set_completion/diagnostics/hover_provider`
  注入（传输层由应用实现后注入）+ epoch 防抖触发 + 补全弹窗状态同步（`CompletionPopup::on_select`
  点击确认）+   诊断下划线装饰（错误波浪红/警告直线黄）；`editor` 演示页接假 provider 可点；
  `editor` feature 蕴含 `lsp`
- **snippets 最小版**（`editor/snippets.rs`）：`$1`/`${1:缺省}`/`$0` 子集 +
  `Tab`/`Shift-Tab` 跳转 + 补全联动（`insertTextFormat == Snippet` 即展开）；
  占位经隐形装饰集合跟踪（`adjust_for_edit` 保留塌缩点）；`editor` 演示页可点
- **inlay hints**（`editor/inlay_hints.rs`）：`InlayProvider`（默认空）+ 防抖请求 +
  paint 阶段 overlay 灰字绘制（三不：不占布局/不进 Rope/不碰选区，只画可见行）；
  `editor` 演示页可点
- **sticky scroll**（`editor/sticky_scroll.rs`）：大纲范围包含推导嵌套栈 +
  `Editor` 顶栏面包屑（点击走 `goto_symbol`，无大纲不显示）；`Editor::new`
  改接 `EditorState` 实体（大纲/光标/开关一次读齐，不在 `InputState` 上冗余数据）
- **多语言注册表**（`highlight`）：`register_highlighter`/`highlighter_for`
 （注册表优先 → Rust 内置 → 静默降级）+ `EditorState::set_language`（高亮/大纲联动）；
  加新语言三步文档；Rust 之外 grammar 不进；`editor` 演示页可切语言验证降级
- **快捷键用户层**（`keymap/file.rs`）：`load_keymap_json`（注释/尾逗号可写；
  `null` = `NoAction`；未知动作/非法按键/非法谓词报错）+ 设置页 recipe 落地
  （`v1_2_showcase --bin keymap`：JSON 加载应用 + `HotkeyInput` 录制绑定 +
  `bindings_for_action` 回显 + 同键异动作冲突提示）

- **stdio LSP 传输**（`lsp/stdio.rs`）：子进程 spawn + JSON-RPC Content-Length 收发 +
  请求 id 路由 + 通知分发 + `StdioLspClient: LspClient` + stderr 日志透出；
  进程异常退出在途请求全失败、不守护重启；内存 duplex 假服务端单测
- **Minimap**（`editor/minimap.rs`）：缩略文本块 + 可视区高亮 + 点击/拖动跳转 +
  开关（默认关）；纯 overlay 三不；大文档抽样渲染；渲染冒烟单测
- **Vim 模式**（`editor/vim.rs`）：normal/insert/visual 三态 + hjkl/wb/0/$/gg/G 移动 +
  i/a/o/x/dd/yy/p/u 编辑 + vim_mode 上下文谓词 + 状态行指示；默认关；
  模式切换与核心操作单测
- **CodeLens**（`editor/codelens.rs`）：`CodeLensProvider` trait + 行上透镜行 +
  点击回调；overlay 层不挤占滚动；假 provider 演示 + 单测
- **标尺 + 括号彩虹**（`editor/state.rs`）：标尺指定列竖线（可配列数/颜色，默认关）；
  括号彩虹嵌套层级按调色轮着色（默认关，与匹配高亮共存）；各单测
- **JSON + TOML grammar**（`tree_sitter.rs`）：按 M6 三步机制加 JSON + TOML
  （各独立 feature 门）+ 高亮 query + document_symbols + 注册；大纲非空单测
- **extensions + on_edit 表**（`editor/extensions.rs`）：扩展注册表 + 文本变更订阅表；
  注册+触发单测

- **StickyPosition**（`editor/sticky_scroll.rs`）：`StickyPosition::Top`/`Status` 枚举 +
  `set_sticky_position`；面包屑可定位到状态行
- **搜索弹窗**（`components/search_panel.rs`）：`Ctrl+F`/`Ctrl+R` 切换显示，左上浮动定位 +
  `set_show_replace` API
- **Gutter 菜单化**（`editor/gutter.rs`）：单 `menu` provider，右键弹出菜单
  （断点/书签/运行），`capture_any_mouse_down` + `stop_propagation` 抑制编辑器右键菜单
- **布局调试标尺**（`editor/state.rs`）：`chrome_geometry()` 暴露四边界 x
  （gutter/行号/折叠/文本起始），演示页四色竖线浮层辅助定位

- **`cx.debounce` 方法版**：`App` 全局防抖注册表 + key 隔离（`Debouncer` 结构版保留）
- **tree-sitter 后端**（`--features tree-sitter`，默认关，wasm 禁用）：Rust 单语言
  `Highlighter` 实现 + fold 数据源；`InputState::set_highlighter` 接入，
  编辑自动刷新高亮装饰与折叠候选
- **SearchPanel 老版删除**：RenderOnce 版删除约 370 行，`SearchPanelState` 补 `on_close` + `Styled`
- **Input 右键菜单**（`input_ui/context_menu.rs`）：默认菜单（剪切/复制/粘贴/全选/撤销/重做按状态自动禁用）+
  三档定制（总开关/追加保留默认/接管可拼回）+ `InputState` 层同名 builder/`set_*`；
  `editor` 演示页右键开箱即用
- **行操作命令**（`input_ui/line_ops.rs`）：复制/删除/上移下移/注释切换/合并行 6 命令 + 默认键位 +
  `line_comment_prefix` 可配；6 单测
- **键入体验**：自动闭合（环绕/跳过/补对/成对退格，多行默认开）+ 只读模式（用户动作拦截、API 照常写）+
  电缩进（`{` 加级/`{}` 拆行）+ 括号匹配高亮（accent 底色）；各单测
- **导航阅读**：当前行高亮（独立装饰集合，仅多行默认开）+ 符号大纲
  （`Highlighter::document_symbols`，Rust 实现函数/结构体/枚举/Trait/impl/模块/常量/静态）+
  查找接线（`SearchPanelState::attach_editor` 文本同步/自动标黄/默认跳转 + `SearchHighlight` 三色预设）
- **多光标多选区**：`extra_selections`（主选区不动）+ `AddCursorAbove/Below` +
  键入/删除/粘贴/回车扇出编辑 + 并组撤销；选区 path 向量化 + 额外光标 quad
- **撤销语义**：回车打断撤销分组 + 初始内容不进撤销栈
- **补全体验**：光标锚定 + 主题色 + 自动开关 + 键盘接管 + 前缀替换
- **Editor 字号覆盖**：`Editor`/`EditorState` 级别字号设置（builder 直透传 render）
- **搜索实战补齐**（#14）：`TextDecoration` + `TextDecorationCollection` 公开导出（多区间高亮/标黄）+
  `InputState::reveal_offset`/`reveal_range` 只读滚动（不改光标选区）
- **Markdown 扩展**（`rgpui-markdown`）：Callout/脚注定义 + `CodeBlockRenderer` hook +
  块级源码区间（WYSIWYG 定位拼回）+ 解析缓存（Arc 共享，高频重渲染命中）

### 示例

- 新增 `dialog` 示例（Dialog/AlertDialog/焦点陷阱）、`sidebar` 示例
- 新增 `v1_2_showcase` 示例（Dock/组件/chat/Editor/搜索 5 个演示 bin）
- `editor` 演示迁移到 `EditorState`/`Editor`（右键菜单/查找面板/行操作/多光标键位可点；
  原 `search`/`context_menu` 独立演示页并入）

### 修复

- **活动行号背景溢出**（`element.rs`）：活动行的行号背景矩形宽度未减去 gutter 列宽，
  导致右缘越过文本起点遮挡行首 1-2 个字符（宽度 `line_number_width - MARGIN`，
  起点从 `gutter_width` 开始，溢出 `gutter_width - MARGIN` 像素）；修复：宽度减去
  `gutter_width`
- **Popover 未初始化即 panic**：`menu::GlobalState` 读写改懒创建（未调 `menu::init`
  也能开合；键盘绑定仍需 init）；`components` 演示补 `init_all`
- **Combobox 打不开/丢焦点**：聚焦即展开；选中（点击/回车）后回焦输入框；
  打字/退格/聚焦链路 3 单测
- **默认构建门控遗漏**（CI）：`rulers` 布局、`Hsla` 导入、`vim` 上下文变量补
  feature 门；`test-support` 下 `OsStr` 缺 import
- **RenderOnce 组件 id 每帧都变**：`Select`/`Alert`/`Popconfirm`/`Pagination`/
  `Rate`/`Link` 默认 ID 改取调用点（`#[track_caller]`，跨帧稳定，下拉/弹层
  状态存得住；循环内多实例必须显式 `.id()`，`Popconfirm`/`Alert` 等补 `.id()`）
- **Select 选项点不动**：下拉项误用非交互 `Label` 变体（`on_click` 被静默丢弃），
  改 `PopupMenuItem::new`（可交互 `Item`）
- **自定义 gutter 列**（`editor/gutter.rs`）：行号左侧第三列，具名 provider
  注册/开关（`add/remove/set_gutter_provider_enabled` + 列总开关，默认关）+
  paint overlay 固定格绘制；`editor` 演示页 run/断点/书签三 provider 可点
- **中文字符边界 panic 收敛**：全 workspace `SafeStrSlice` 统一收敛 + markdown 代码块 CJK 切片 +
  IME 路径高亮刷新与装饰吸附字符边界
- **自定义标题栏 8 方向拖拽**：边缘/边角拖拽调整窗口大小
- **折叠崩溃与滚动空白**：折叠展开崩溃修复 + 滚动后长文本空白修复
- **Mermaid 走线**：div 全彩渲染（替代单色 SVG 管道）+ 按入边轴走线 + 箭头回偏不压边 + 边标签前景色
- **搜索弹窗标黄不清**：关闭后标黄残留清理
- **Minimap 增强**：还原文形状 + 整条可点拖 + 防穿透误触（仍纯 overlay，默认关）
- **设置页与展示页**：settings 崩溃修复 + 字号缩放行高 + gutter 折叠图标
- **chat 发送者名对齐**：按气泡方向对齐到头像正下方

### 布局整理

- `system/` 子系统目录；15 个公模块同名进目录（路径不变）；
  `chat_ui` → `chat`（`chat_ui` 弃用别名，1.3.0 删除）；`rgpui.rs` 三段式分组注释

## [1.1.1] - 2026-09-06

### 修复

- **WebView 空白**：DComp `CreateTargetForHwnd(hwnd, true)` 把视觉树渲染在所有子 HWND 之上，
  WebView2 从第一天起就被遮挡；改为 `false`（`rgpui-windows/src/directx_renderer.rs`），子 HWND 自然透出
- **WebView 示例**：补 `cx.notify()`（原生窗口否则保持 0×0）、修布局链零高、加百度入口
- **animation 示例坏导入**：`use mestd::time::Duration`（不存在的 crate）→ `std::time::Duration`
- **跨 crate 资产引用失效**：`animation`、`desktop_pet_3d`、`image_loading` 改指 `image_showcase/assets/`

### 变更

- **发布流程除环**：删 `rgpui-macros → rgpui` 测试期 dev 依赖（3 个集成测试搬到 `rgpui/tests/`，
  消费方已有正向依赖）、删 `rgpui → rgpui-web` 未使用的 wasm dev 依赖；
  此后发版无需临时注释 dev 依赖，按拓扑顺序 publish 即可
- **WebView 后端**：停更的 `lb-wry` fork → 上游 `wry 0.56.1`（API 兼容，零代码改动）
- **示例合并**：62 → 45 个 crate（`image_showcase` / `window_showcase` / `text_showcase` /
  `list_showcase` / `overlay_showcase` / `tray` 双 binary，一 crate 多 binary，行为零变化）
- **示例文档**：全部 45 个示例补 `README.md`（演示说明 + 运行命令）
- **CI**：`concurrency` group，同一分支 push + PR 只跑一次

## [1.1.0] - 2026-09-04

### 新增

#### v1.1.0-alpha — API 增强
- **push_notification**：App 级通知便捷 API
- **init_all**：统一初始化所有子系统
- **minimize_to_tray**：隐藏所有窗口到托盘
- **全局窗口管理**：show_all_windows / hide_all_windows / minimize_all_windows
- **SearchPanel**：搜索替换面板组件
- **rgpui-webview**：WebView 已并入核心（feature `webview` 门控）

#### v1.1.0-beta — 编辑器核心能力
- **LSP 核心 trait**：LspClient trait（15 个方法）+ 补全/悬停/定义/诊断/语义高亮子系统
- **语法高亮 trait**：Highlighter trait + HighlightStyleResolver
- **StatusBar 组件**：状态栏组件
- **AsyncFileLoader**：大文件异步加载（流式进度 + 取消支持）
- **CompletionPopup**：LSP 补全弹窗 UI
- **DiagnosticMarkers**：LSP 诊断标记组件

#### v1.1.0-rc — 桌面应用增强
- **FileWatcher**：文件监视 API（FileWatcher/FileEvent/FileWatcherConfig）
- **ConfigStore**：配置持久化 API（ConfigStore/加载/保存/监听）
- **Chat UI**：聊天消息组件（ChatView/Message/MessageGroup）
- **FPS HUD**：性能监控浮层（FpsHud/FpsHudState/CPU/内存监控）

#### v1.1.0 — 高级功能
- **Tab 拖拽排序**：TabDragDrop/TabDragState/TabDragEvent
- **国际化支持**：I18nManager/I18nText/PluralRule
- **主题热重载**：ThemeWatcher/ThemeEvent/ThemeManager
- **块级渲染**：BlockRenderer/BlockElement/BlockType
- **虚拟滚动**：VirtualScroll/VirtualScrollList/VirtualScrollConfig
- **源码映射**：SourceMap/BidirectionalSourceMap/SourceLocation
- **Markdown 插件**：MarkdownPlugin/PluginManager/MarkdownRenderer

## [1.0.2] - 2026-09-03

### 修复

- **修复 debug 模式 STATUS_ACCESS_VIOLATION 崩溃**：还原 `D3D_COMPILE_STANDARD_FILE_INCLUDE` 的 `transmute_copy` 为 `transmute`，修复 D3DCompileFromFile include handler 指针错误导致的崩溃（regression from `f6646c6`）
- **禁用 debug 模式 D3D11 调试层**：避免调试层初始化失败导致的栈溢出

### 变更

- **wgpu 29 → 30 升级**：适配 `present()` → `drop(frame)`、`VertexState::buffers` 包装、`color_space` 等 breaking changes
- **scenekit 集成**：rgpui-3d 从 scenix 迁移到 scenekit（已发布 crates.io 0.1.0）
- **examples 合并入根 workspace**：消除嵌套 workspace，统一依赖管理

## [1.0.1] - 2026-08-31

### 新增

#### 核心框架
- GPU 加速渲染引擎（wgpu 后端），支持 Direct3D 12、Metal、Vulkan、WebGPU
- 跨平台窗口管理（Windows、macOS、Linux X11/Wayland、Web/WASM）
- 响应式元素系统（Entity-Component 架构，支持状态管理和动画）
- 文本渲染系统（HarfBuzz 排版 + GPU 渲染，支持亚像素渲染）
- 焦点管理、键盘快捷键绑定系统

#### 组件库
- 基础组件：Button、Checkbox、Radio、Switch、Slider、Spinner、Badge、Tag、Separator、Tooltip、Icon
- 表单组件：Form、Field、FieldBuilder、Input、MaskedInput、NumberInput、PasswordInput、TextArea
- 菜单组件：PopupMenu、ContextMenu、DropdownMenu、Menu、MenuBar、MenuItem、HoverCard、Notification、Toast
- 对话框组件：Dialog、AlertDialog、FocusTrapElement
- 列表组件：List、VirtualList
- 表格组件：Table、DataTable
- 标签页组件：Tab、TabBar、Accordion、Collapsible
- 标题栏组件：TitleBar、WindowBorder
- 滚动组件：Scrollable、Scrollbar、ScrollHandle
- 动画系统：13 种动画组件、Spring、KeyframeAnimation
- 手势检测：TapGesture、LongPressGesture、PanGesture、SwipeGesture
- 滚动物理引擎

#### 系统集成
- 系统托盘（图标、右键菜单、窗口隐藏/恢复）
- 全局系统快捷键
- 开机自启动
- 系统通知
- 剪贴板（含 Linux Primary Selection、macOS Find Pasteboard）
- 凭据管理（系统密钥链集成）
- 屏幕捕获（feature-gated: `scap`/`screen-capture`）
- 电源管理（休眠阻止、系统唤醒、空闲时间检测）
- 网络状态监控
- 媒体键事件
- 生物识别（指纹/面容 ID）
- 辅助功能（accesskit 集成）

#### Windows 特有
- Mica/Acrylic 毛玻璃材质
- 鼠标穿透（NCHITTEST 实现）
- 自动启动（注册表）
- 窗口扩展样式管理

#### macOS 特有
- 标签页管理（窗口标签页分组）
- 红绿灯按钮位置自定义
- Dock 徽标
- 系统字符面板

#### Linux 特有
- X11/Wayland 双后端
- CSD（客户端装饰）
- Wayland layer-shell（独占区域、输入区域）
- 主选择区剪贴板

#### Web/WASM
- 纯 Canvas 渲染后端
- DOM 覆盖层（feature-gated: `dom-backend`），支持文本选择/复制
- Trunk 开发服务器集成

#### 扩展库
- Markdown 渲染（`rgpui-markdown`，基于 pulldown-cmark）
- 3D 渲染支持（`rgpui-3d`，OpenGL/wgpu）
- 终端组件（`rgpui-term`）
- 图表组件（feature-gated: `charts`）
- 特效组件（feature-gated: `effects`）
- 二维码生成（feature-gated: `qr-code`）
- Tokio 异步运行时集成（feature-gated: `tokio`）

#### 开发工具
- 过程宏（`#[derive(Render, Context)]`）
- 测试框架（`test` 模块，支持渲染快照测试）
- 8 章教程文档（`docs/rgpui-book/`）
- 组件整合指南和迁移文档
