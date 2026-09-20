# rgpui

rgpui 是一个独立演进的 GPU 加速跨平台 UI 框架，支持 Windows、macOS、Linux（X11/Wayland）和 Web/WASM。

## 功能特性

### 渲染引擎
- GPU 加速渲染（wgpu 后端，支持 Direct3D 12、Metal、Vulkan、WebGPU）
- 响应式元素系统（Entity-Component 架构）
- 文本渲染（HarfBuzz 排版 + GPU 渲染，亚像素渲染支持）
- 动画系统（Spring、Keyframe、13 种动画组件）
- 手势检测（Tap、LongPress、Pan、Swipe）
- 滚动物理引擎

### 组件库
- **基础**：Button、Checkbox、Radio、Switch、Toggle、Slider、Spinner、Badge、Tag、Separator、Tooltip、Icon、Avatar、Alert、Card、Typography
- **表单**：Form、Field、Input、MaskedInput、NumberInput、PasswordInput、TextArea
- **菜单**：PopupMenu（含 danger 样式）、ContextMenu、DropdownMenu、MenuBar、HoverCard、Notification、Toast、Popconfirm
- **对话框**：Dialog（按标识关闭 `DialogId`）、AlertDialog、遮罩可见性控制
- **列表/表格**：List、VirtualList、DataTable
- **标签页**：Tab、TabBar、无状态 Tabs（值驱动，v1.3）、Accordion、TabDragDrop（拖拽排序）
- **滚动**：Scrollable、Scrollbar、ScrollHandle、VirtualScroll
- **状态栏**：StatusBar（v1.3 数据驱动重写，纯值驱动无实体）
- **搜索**：SearchPanel
- **聊天**：ChatView、Bubble、MessageScroller、Marker、Prompts、Suggestion、ThoughtChain
- **侧边栏**：Sidebar、SidebarSection
- **导航**：Breadcrumb、Steps、Timeline、Pagination
- **选择器**：Select、Combobox、DatePicker、ColorPicker、Rate
- **其他**：Carousel、Upload、TagInput、Skeleton、Kbd

### 编辑器能力
- **编辑器核心**：EditorState + Editor 组件（右键菜单/查找/行操作/多光标可点）
- **LSP 接线**：补全/悬停/诊断注入 + 防抖触发 + 诊断下划线
- **语法高亮**：Highlighter trait + tree-sitter 后端（Rust/JSON/TOML）
- **片段**：`$1`/`${1:缺省}`/`$0` 子集 + Tab 跳转
- **行内提示**：InlayProvider + 防抖请求 + paint overlay
- **粘性滚动**：大纲范围推导 + 面包屑导航
- **Minimap**：缩略文本 + 可视区高亮 + 点击跳转
- **Vim 模式**：normal/insert/visual 三态
- **CodeLens**：行上透镜 + 点击回调
- **标尺 + 括号彩虹**、搜索弹窗、Gutter 菜单化

### 检查器（Inspector，v1.3）
- **两行启用**：`App::enable_default_inspector()` + F12 开关
- **完整元素树**：逐节点折叠/展开，点击选中画布区域并高亮
- **盒模型展示**：margin/border/padding 四边值 + 已指定样式列表
- **运行卡**：帧率/CPU/内存/GPU 实时监控
- **崩溃快照**：`InspectorSnapshot` + `last.json` 滚动落盘 + panic 日志钩子
- **面板插槽化**：Header/Section 可覆盖，支持自定义面板
- **AI 可读**：`Window::inspector_tree_text` 导出 Markdown 缩进树

### 系统集成
- 系统托盘（图标、右键菜单、窗口隐藏/恢复）
- 全局系统快捷键 + 全局动作 helper（`on_global_action`）
- 开机自启动、系统通知
- 剪贴板（含 Linux Primary Selection、macOS Find Pasteboard）
- 凭据管理（系统密钥链）
- 屏幕捕获（feature-gated）
- 电源管理、网络状态、媒体键、生物识别

### 桌面增强
- **国际化**：I18nManager（Global + `load_locale_dir` + 快照回退 + `translate_global`）
- **主题热重载**：ThemeWatcher / ThemeManager / `Theme::apply_named`
- **性能监控**：FPS HUD（帧率/CPU/内存实时监控）
- **Markdown 渲染**：rgpui-markdown（Callout/脚注/代码块 renderer hook）

### 平台特有
- **Windows**：Mica/Acrylic 毛玻璃、鼠标穿透（NCHITTEST）、自动启动、窗口扩展样式
- **macOS**：标签页管理、红绿灯按钮、Dock 徽标、系统字符面板
- **Linux**：X11/Wayland 双后端、CSD、layer-shell
- **Web/WASM**：Canvas 渲染、DOM 覆盖层（文本选择/复制）、tokio 集成

## 项目结构

```
crates/
├── rgpui/              # 核心 UI 框架（组件库、动画、手势、滚动物理、检查器、编辑器）
├── rgpui-3d/           # 3D 渲染支持
├── rgpui-character/    # 字符/文本处理
├── rgpui-dom/          # Web DOM 后端
├── rgpui-linux/        # Linux 平台实现
├── rgpui-macos/        # macOS 平台实现
├── rgpui-macros/       # 过程宏
├── rgpui-markdown/     # Markdown 渲染（pulldown-cmark）
├── rgpui-platform/     # 平台选择入口
├── rgpui-term/         # 终端组件
├── rgpui-web/          # Web/WASM 平台实现
├── rgpui-wgpu/         # wgpu 渲染后端
└── rgpui-windows/      # Windows 平台实现

examples/               # 45+ 示例（showcase 类多 binary）
├── v1_3_showcase/      # v1.3 新 API 集中演示
├── v1_2_showcase/      # v1.2 组件演示
├── inspector/          # 检查器默认面板
├── inspector_custom/   # 检查器自定义面板
├── dialog/             # Dialog/AlertDialog/焦点陷阱
├── sidebar/            # 侧边栏（亮/暗切换）
├── rgpui_story/        # 组件 Storybook
├── desktop_pet/        # 桌面宠物（托盘、单实例）
├── image_showcase/     # 图片加载/查看
├── input/              # 输入组件
├── form/               # 表单组件
├── ...                 # 更多见 examples/ 目录
```

## 开发命令

```bash
cargo check --workspace            # 日常检查
cargo test --workspace             # 全量测试
cargo test -p rgpui                # 单包测试
cargo clippy -p <pkg> --all-targets -- -D warnings  # lint
cargo fmt --all                    # 格式化
cargo run -p <pkg>                 # 运行示例
```

## 文档

- [8 章教程](docs/rgpui-book/)
- [1.2 → 1.3 迁移指南](docs/migration-guide-1.2-to-1.3.md)
- [Web DOM 后端用法](docs/web-dom-backend-usage.md)
- [UTF-8 切片安全](docs/utf8-slice-safety.md)
- [开发指南](AGENTS.md)
- [贡献指南](CONTRIBUTING.md)

## 许可证

Apache-2.0
