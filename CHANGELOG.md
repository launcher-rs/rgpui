# Changelog

本项目遵循 [语义化版本控制](https://semver.org/lang/zh-CN/)。

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
