# rgpui

rgpui 是一个从 [zed-industries/gpui](https://github.com/zed-industries/zed) 和 [longbridge/gpui-component](https://github.com/longbridge/gpui-component) 项目移植而来的跨平台 GPU 加速 UI 框架。

## 新增功能

### 增强透明窗口支持

改进了窗口透明度的实现，支持更灵活的透明效果配置，适用于需要半透明、毛玻璃等视觉效果的桌面应用。

### 系统托盘（System Tray）

添加了完整的系统托盘功能：

- 支持自定义托盘图标（PNG/ICO 格式）
- 支持托盘右键菜单
- 支持窗口隐藏/恢复与托盘交互
- 跨平台托盘 API 抽象

## 项目结构

```
crates/
├── rgpui/                   # 核心 UI 框架，平台无关逻辑
├── rgpui-3d/                # 3D 渲染支持
├── rgpui-adabraka-ui/       # Adabraka UI 组件库
├── rgpui-character/         # 字符/文本处理
├── rgpui-linux/             # Linux 平台实现
├── rgpui-macos/             # macOS 平台实现
├── rgpui-macros/            # 过程宏
├── rgpui-platform/          # 平台选择入口
├── rgpui-term/              # 终端组件
├── rgpui-tokio/             # Tokio 异步运行时集成
├── rgpui-web/               # Web/WASM 平台实现
├── rgpui-wgpu/              # wgpu 渲染后端
├── rgpui-windows/           # Windows 平台实现
├── rgpui-yororen-ui/        # Yororen UI 组件库
└── rgpui-component-workspce/  # 组件子工作区
    ├── rgpui-component/          # 通用 UI 组件框架
    ├── rgpui-component-assets/   # 组件资源文件
    ├── rgpui-component-macros/   # 组件过程宏
    ├── rgpui-component-story/    # 组件 Storybook（原生）
    ├── rgpui-component-story-web/ # 组件 Storybook（Web）
    ├── rgpui-webview/            # WebView 组件
    └── themes/                   # 22 套 JSON 颜色主题
```

> **注意**：`crates/rgpui-component/` 和 `crates/rgpui-component-macros/` 是已废弃的旧目录（已迁移至 `crates/rgpui-component-workspce/` 下）。合并上游 PR 时若被重建，请立即删除。

## 示例程序

```
examples/
├── desktop_pet/                       # 桌面宠物（系统托盘、窗口管理）
├── desktop_pet_3d/                    # 桌面宠物（3D 渲染）
├── rgpui_async_demo/                  # 异步运行时演示
├── rgpui_editor_example/              # 编辑器组件演示
├── rgpui_editor_lsp_example/          # 编辑器 LSP 集成演示
├── rgpui_term_basic/                  # 终端模拟器基础演示
├── rgpui_term_component_integration/  # 终端 + 组件框架集成
├── rgpui_yororen_ui_counter/          # 计数器（Yororen UI）
├── rgpui_yororen_ui_file_browser/     # 文件浏览器（Yororen UI）
├── rgpui_yororen_ui_toast_notification/ # 通知提示（Yororen UI）
└── rgpui_yororen_ui_todolist/         # 待办事项（Yororen UI，多语言）
```

## 开发命令

```bash
# 检查工作区
cargo check --workspace

# 构建示例
cargo build --example hello_world

# 运行示例
cargo run --example hello_world

# 运行测试
cargo test --workspace

# Clippy 检查
cargo clippy --workspace
```

## 许可证

Apache-2.0
