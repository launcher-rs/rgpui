# Changelog

本项目遵循 [语义化版本控制](https://semver.org/lang/zh-CN/)。

## [Unreleased] - 1.2.0（开发中，分支 `feat/1.2.0`）

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

- **`cx.debounce` 方法版**：`App` 全局防抖注册表 + key 隔离（`Debouncer` 结构版保留）
- **tree-sitter 后端**（`--features tree-sitter`，默认关，wasm 禁用）：Rust 单语言
  `Highlighter` 实现 + fold 数据源；`InputState::set_highlighter` 接入，
  编辑自动刷新高亮装饰与折叠候选
- **SearchPanel 老版删除**：RenderOnce 版删除约 370 行，`SearchPanelState` 补 `on_close` + `Styled`

### 示例

- 新增 `dialog` 示例（Dialog/AlertDialog/焦点陷阱）、`sidebar` 示例

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
