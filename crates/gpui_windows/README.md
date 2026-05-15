# gpui_windows

GPUI 框架的 Windows 平台后端实现，使用 DirectX 渲染、DirectWrite 文本渲染和 Win32 API 进行窗口管理。

## 概述

`gpui_windows` 为 GPUI 提供 Windows 系统上的完整平台抽象层，包括窗口管理、输入处理、DirectX 硬件加速渲染、DirectWrite 字体渲染以及系统级集成（剪贴板、文件对话框等）。

## 功能特性

### DirectX 渲染

- 基于 DirectX 11 的硬件加速 2D 渲染
- 使用 Direct2D 进行图形绘制和合成
- 支持 GPU 加速的纹理上传和渲染
- 高效的帧缓冲管理和交换链配置

### DirectWrite 文本渲染

- 高质量的 ClearType 字体渲染
- 支持 OpenType 字体特性
- 字体回退（font fallback）机制
- 文本布局和排版支持

### Win32 API 集成

- 原生窗口创建和管理
- 完整的输入事件处理（鼠标、键盘、触控）
- DPI 感知和高 DPI 缩放支持
- 系统托盘和通知集成

### 其他功能

- **剪贴板**：支持文本和图像数据的复制粘贴
- **文件对话框**：使用 Windows 通用文件对话框
- **屏幕捕获**：可选的屏幕录制和截图支持（通过 `scap` crate）
- **注册表访问**：通过 `windows-registry` 读取系统配置
- **原始窗口句柄**：支持 `raw-window-handle` 0.6 协议

## 系统要求

### 编译依赖

- Rust 工具链（最新稳定版）
- MSVC 或 MinGW 工具链
- Windows SDK

### 运行时依赖

- Windows 10 或更高版本
- DirectX 11 兼容的 GPU 驱动

## 特性标志

| 特性 | 描述 |
|------|------|
| `screen-capture` | 启用屏幕捕获支持（依赖 `scap`） |
| `test-support` | 启用测试支持工具 |

## 架构

```
gpui_windows/
├── src/
│   ├── gpui_windows.rs      # 入口点
│   └── windows/
│       ├── platform.rs      # WindowsPlatform 实现
│       ├── window.rs        # 窗口管理
│       ├── renderer.rs      # DirectX 渲染器
│       ├── text_system.rs   # DirectWrite 文本系统
│       ├── input.rs         # 输入事件处理
│       └── clipboard.rs     # 剪贴板实现
```

## 使用示例

```rust
use gpui_windows::WindowsPlatform;

// 创建 Windows 平台实例
let platform = WindowsPlatform::new();

// 平台会自动初始化 DirectX 渲染器和 DirectWrite 文本系统
// 并通过 Win32 API 管理窗口和输入事件
```

## 依赖关系

- `gpui` — GPUI 核心框架
- `windows` / `windows-core` — Windows API 绑定
- `windows-numerics` — Windows 数值计算库
- `raw-window-handle` — 跨平台窗口句柄抽象
- `etagere` — 2D 矩形分配器（用于纹理图集管理）
- `image` — 图像处理
- `scap`（可选）— 屏幕捕获库
