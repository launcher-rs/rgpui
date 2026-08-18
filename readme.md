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
├── rgpui/                   # 核心 UI 框架（含基础组件库），平台无关逻辑
├── rgpui-3d/                # 3D 渲染支持
├── rgpui-character/         # 字符/文本处理
├── rgpui-linux/             # Linux 平台实现
├── rgpui-macos/             # macOS 平台实现
├── rgpui-macros/            # 过程宏
├── rgpui-platform/          # 平台选择入口
├── rgpui-term/              # 终端组件
├── rgpui-tokio/             # Tokio 异步运行时集成
├── rgpui-web/               # Web/WASM 平台实现
├── rgpui-wgpu/              # wgpu 渲染后端
└── rgpui-windows/           # Windows 平台实现
```

> **注意**：旧 UI 库（`rgpui-component`、`rgpui-adabraka-ui`、`rgpui-yororen-ui`、`rgpui-webview`）已删除，基础组件全部并入 `rgpui` 核心。新增组件的发展计划见 `docs/ui-crate-plan.md`。

## 示例程序

```
examples/
├── desktop_pet/                       # 桌面宠物（系统托盘、窗口管理）
├── desktop_pet_3d/                    # 桌面宠物（3D 渲染）
└── screen_capture/                    # 屏幕捕获演示
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
