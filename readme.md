# rgpui

rgpui 是一个从 [zed-industries/gpui](https://github.com/zed-industries/zed) 和 [longbridge/gpui-component](https://github.com/longbridge/gpui-component) 项目移植而来的跨平台 GPU 加速 UI 框架。


## 项目背景

本项目诞生的原因：

- **zed gpui 专注于编辑器/IDE**：上游 gpui 主要服务于 Zed 编辑器，通用 UI 功能发展受限
- **gpui 长期无新版本**：上游 gpui 更新缓慢，一直停留在开发版状态
- **gpui-component 依赖问题**：gpui-component 强依赖 zed gpui，导致其也长期处于开发版，无法稳定发布

rgpui 旨在解决这些问题，提供一个独立维护、持续更新的通用 GUI 框架。

## 为什么重命名为 rgpui？

在 Rust 生态中，多个 gpui 分支/版本（如 gpui 0.2.2、最新 git 源版本、gpui-ce、adabraka-gpui 等）都在各自的 `Cargo.toml` 中使用 `name = "gpui"` 进行重命名。虽然名称相同，但这些版本的 API 差异巨大，导致依赖它们的 UI 库无法共存，产生严重的版本冲突。

将所有分支统一重命名为 `rgpui`，虽然看起来不够优雅，但能彻底解决依赖混乱问题，让不同版本的 UI 组件可以明确区分并独立演进。

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
├── rgpui/           # 核心 UI 框架，平台无关逻辑
├── rgpui_platform/  # 平台选择入口
├── rgpui_windows/   # Windows 平台实现
├── rgpui_macos/     # macOS 平台实现
├── rgpui_linux/     # Linux 平台实现
├── rgpui_web/       # Web/WASM 平台实现
├── rgpui_wgpu/      # wgpu 渲染后端
├── rgpui_macros/    # 过程宏
├── rgpui_tokio/     # Tokio 异步运行时集成
├── rgpui_webview/   # WebView 集成
└── rgpui_component/ # UI 组件库
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