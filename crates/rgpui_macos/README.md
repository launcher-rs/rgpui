# gpui_macos

GPUI 的 macOS 平台实现 crate。提供 macOS 原生 API 的封装，包括 Cocoa 窗口管理、Metal GPU 渲染、Core Text 字体系统以及原生事件处理。

## 概述

`gpui_macos` 是 GPUI 图形框架在 macOS 上的底层平台适配层，通过直接调用 macOS 原生框架实现高性能的 GUI 渲染和系统交互。

## macOS 专属特性

### Cocoa 窗口管理
- 基于 `NSWindow`/`NSPanel` 的原生窗口创建与管理
- 支持多种窗口类型：普通窗口、弹出窗口、浮动窗口、对话框
- 窗口标签页（Tabbing）、全屏、最小化等原生行为
- 自定义红绿灯（交通灯）按钮位置
- 毛玻璃背景效果（`NSVisualEffectView`）
- 拖放文件支持

### Metal GPU 渲染
- 使用 Apple Metal API 进行硬件加速渲染
- 支持 Apple Silicon 和 Intel Mac（自动选择最佳 GPU）
- 4x MSAA 抗锯齿（路径渲染）
- 精灵图集（Atlas）纹理管理
- 支持透明窗口渲染
- 无头（Headless）离屏渲染模式，适用于测试场景
- 渲染图元类型：四边形、阴影、路径、下划线、单色/彩色精灵、视频表面

### Core Text 字体集成
- 基于 Core Text 的字体塑形（Font Shaping）
- 使用 font-kit 进行字体加载与匹配
- 支持系统字体、内存嵌入字体
- 灰度抗锯齿渲染
- 字体平滑（Font Smoothing）支持
- Emoji 字形渲染（AppleColorEmoji）
- 复杂文本布局（Attributed String）

### 键盘与事件系统
- 原生 NSEvent 事件转换
- 支持多键盘布局映射（QWERTY、QWERTZ、AZERTY 等 60+ 种布局）
- 快捷键布局自适应（解决非英文布局下快捷键不可用问题）
- Force Touch 压力感应支持
- 触控板手势（捏合、滑动）
- 滚轮精确/行滚动支持

### 系统集成
- 剪贴板与查找粘贴板
- 钥匙串凭据存储（Keychain）
- 文件/目录选择对话框（NSOpenPanel/NSSavePanel）
- URL 方案注册
- 菜单栏与 Dock 菜单
- 应用生命周期管理（启动、退出、重新打开）
- 热状态监控（Thermal State）
- 键盘布局变更通知

## 模块结构

| 模块 | 说明 |
|------|------|
| `platform` | macOS 平台实现，实现 `gpui::Platform` trait |
| `window` | 原生窗口管理，包括视图、事件处理 |
| `display` | 多显示器支持，基于 CoreGraphics |
| `metal_renderer` | Metal GPU 渲染器 |
| `metal_atlas` | Metal 纹理图集管理 |
| `text_system` | Core Text 文本与字体系统 |
| `keyboard` | 键盘布局与映射 |
| `events` | 原生事件到 GPUI 事件的转换 |
| `dispatcher` | GCD 任务调度器 |
| `display_link` | 垂直同步显示链接 |
| `pasteboard` | 系统剪贴板封装 |
| `window_appearance` | 窗口外观（深色/浅色模式） |
| `screen_capture` | 屏幕捕获支持（可选功能） |
| `open_type` | OpenType 字体特性处理 |

## 系统要求

- **操作系统**: macOS 12.0 (Monterey) 或更高版本
- **架构**: Apple Silicon (M1/M2/M3/M4) 或 Intel (x86_64)
- **GPU**: 支持 Metal API 的显卡（所有现代 Mac 均支持）
- **Rust**: 最新稳定版

## 功能标志 (Features)

| Feature | 说明 |
|---------|------|
| `font-kit` | 启用 Core Text 字体系统（默认启用） |
| `runtime_shaders` | 运行时编译 Metal 着色器（开发调试用） |
| `screen-capture` | 启用屏幕捕获功能（需 macOS 12.3+） |
| `test-support` | 启用测试支持，包括无头渲染和图像捕获 |

## 使用示例

```rust
use gpui_macos::MacPlatform;

// 创建 macOS 平台实例
let platform = MacPlatform::new(false);

// 获取显示器信息
let displays = platform.displays();
let primary = platform.primary_display();

// 运行应用事件循环
platform.run(Box::new(|| {
    println!("应用启动完成");
}));
```

## 架构说明

```
┌─────────────────────────────────────┐
│            GPUI Core                │
│      (跨平台图形抽象层)              │
└──────────────┬──────────────────────┘
               │ Platform trait
┌──────────────▼──────────────────────┐
│         MacPlatform                 │
│  ┌──────────┬──────────┬─────────┐  │
│  │  Cocoa   │  Metal   │  Core   │  │
│  │  Window  │ Renderer │  Text   │  │
│  └──────────┴──────────┴─────────┘  │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│         macOS 原生框架               │
│  AppKit · Metal · CoreText · GCD    │
└─────────────────────────────────────┘
```

## 许可证

本 crate 采用与主项目相同的许可证。详见 `LICENSE-APACHE` 文件。
