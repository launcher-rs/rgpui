# rgpui

rgpui 是一个跨平台 GPU 加速 UI 框架，从 [zed-industries/gpui](https://github.com/zed-industries/zed) 和 [longbridge/gpui-component](https://github.com/longbridge/gpui-component) 项目移植而来，支持 Windows、macOS、Linux（X11/Wayland）和 Web/WASM。

## 功能特性

### 渲染引擎
- GPU 加速渲染（wgpu 后端，支持 Direct3D 12、Metal、Vulkan、WebGPU）
- 响应式元素系统（Entity-Component 架构）
- 文本渲染（HarfBuzz 排版 + GPU 渲染，亚像素渲染支持）
- 动画系统（Spring、Keyframe、13 种动画组件）
- 手势检测（Tap、LongPress、Pan、Swipe）
- 滚动物理引擎

### 组件库
- **基础**：Button、Checkbox、Radio、Switch、Slider、Spinner、Badge、Tag、Separator、Tooltip、Icon
- **表单**：Form、Field、Input、MaskedInput、NumberInput、PasswordInput、TextArea
- **菜单**：PopupMenu、ContextMenu、DropdownMenu、Menu、MenuBar、MenuItem、HoverCard、Notification、Toast
- **对话框**：Dialog、AlertDialog
- **列表/表格**：List、VirtualList、Table、DataTable
- **标签页**：Tab、TabBar、Accordion、Collapsible、TabDragDrop（拖拽排序）
- **滚动**：Scrollable、Scrollbar、ScrollHandle、VirtualScroll（虚拟滚动）
- **状态栏**：StatusBar（v1.1.0）
- **搜索**：SearchPanel（v1.1.0）
- **聊天**：ChatView（v1.1.0）

### 编辑器能力（v1.1.0）
- **LSP 核心**：LspClient trait + 补全/悬停/定义/诊断/语义高亮子系统
- **语法高亮**：Highlighter trait + HighlightStyleResolver
- **补全弹窗**：CompletionPopup UI
- **诊断标记**：DiagnosticMarkers + DiagnosticTooltip
- **大文件加载**：AsyncFileLoader（流式进度 + 取消支持）
- **源码映射**：SourceMap / BidirectionalSourceMap
- **块级渲染**：BlockRenderer / BlockElement

### 系统集成
- 系统托盘（图标、右键菜单、窗口隐藏/恢复）
- 全局系统快捷键
- 开机自启动、系统通知
- 剪贴板（含 Linux Primary Selection、macOS Find Pasteboard）
- 凭据管理（系统密钥链）
- 屏幕捕获（feature-gated）
- 电源管理、网络状态、媒体键、生物识别

### 桌面增强（v1.1.0）
- **文件监视**：FileWatcher API（实时文件变更监听）
- **配置持久化**：ConfigStore（JSON 配置加载/保存/监听）
- **国际化**：I18nManager（多语言支持 + 复数规则）
- **主题热重载**：ThemeWatcher / ThemeManager（运行时主题切换）
- **性能监控**：FPS HUD（帧率/CPU/内存实时监控）
- **Markdown 插件**：MarkdownPlugin / PluginManager（自定义渲染扩展）

### 平台特有
- **Windows**：Mica/Acrylic 毛玻璃、鼠标穿透、自动启动、窗口扩展样式
- **macOS**：标签页管理、红绿灯按钮、Dock 徽标、系统字符面板
- **Linux**：X11/Wayland 双后端、CSD、layer-shell
- **Web/WASM**：Canvas 渲染、DOM 覆盖层（文本选择/复制）

## 项目结构

```
crates/
├── rgpui/              # 核心 UI 框架（组件库、动画、手势、滚动物理）
├── rgpui-3d/           # 3D 渲染支持
├── rgpui-character/    # 字符/文本处理
├── rgpui-dom/          # Web DOM 后端
├── rgpui-linux/        # Linux 平台实现
├── rgpui-macos/        # macOS 平台实现
├── rgpui-macros/       # 过程宏
├── rgpui-platform/     # 平台选择入口
├── rgpui-term/         # 终端组件
├── rgpui-web/          # Web/WASM 平台实现
├── rgpui-webview/      # WebView 独立库（v1.0）
├── rgpui-wgpu/         # wgpu 渲染后端
└── rgpui-windows/      # Windows 平台实现

extensions/
├── rgpui-markdown/     # Markdown 渲染（pulldown-cmark）

examples/
├── desktop_pet/        # 桌面宠物（托盘、窗口管理、单实例）
├── desktop_pet_3d/     # 桌面宠物（3D 渲染）
├── screen_capture/     # 屏幕捕获演示
├── rgpui_story/        # 组件 Storybook（Web/WASM 兼容）
├── extended_components/# 扩展组件演示
├── rgpui_term_basic/   # 终端组件基础用法
└── rgpui_term_integration/ # 终端集成演示
```

## 开发命令

```bash
# 检查工作区
cargo check --workspace

# 运行示例
cargo run --example tray

# 运行测试
cargo test --workspace

# Clippy 检查
cargo clippy --workspace

# 格式化代码
cargo fmt --all

# Web 开发（需 Trunk）
cd examples/rgpui_story && trunk serve
```

## 文档

- [8 章教程](docs/rgpui-book/)
- [组件整合指南](docs/component-integration-plan.md)
- [从 rgpui-component 迁移](docs/migration-guide-from-rgpui-component.md)
- [Web DOM 后端用法](docs/web-dom-backend-usage.md)
- [开发指南](AGENTS.md)
- [贡献指南](CONTRIBUTING.md)

## 许可证

Apache-2.0
