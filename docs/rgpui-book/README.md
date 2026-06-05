# rgpui-book

> rgpui GPU 加速 UI 框架学习指南

## 目录

1. [快速入门](./01-getting-started.md) - 环境搭建与第一个应用
2. [实体系统](./02-entity-system.md) - Entity、Context 与生命周期管理
3. [订阅系统](./03-subscription.md) - 观察者模式的事件订阅与通知
4. [快捷键系统](./04-keymap.md) - Action、Keymap 与键盘事件分发
5. [异步任务](./05-async-tasks.md) - 后台任务、前台执行器与定时器
6. [元素系统](./06-elements.md) - Element trait、布局与绘制管线
7. [窗口管理](./07-window-management.md) - 窗口创建、视图与焦点管理

## 关于 rgpui

rgpui 是基于 Zed 编辑器的 `gpui` 框架的跨平台移植版本。它提供了一套完整的 GPU 加速 UI 系统，适用于构建高性能的桌面应用程序。

### 核心特性

- **实体系统**：基于 Arena 分配器的高效实体管理，支持强弱引用
- **响应式事件系统**：观察者模式的订阅和通知机制
- **键盘事件分发**：层级化的按键事件处理管线
- **布局引擎**：基于 Taffy 的 Flexbox 布局
- **文本渲染**：GPU 加速的文本排版和渲染
- **平台抽象层**：支持 Windows、macOS、Linux、Web 等平台

### 架构概览

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
└── rgpui_tokio/     # Tokio 异步运行时集成
```
