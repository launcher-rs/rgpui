# rgpui_linux

RGPUI 框架的 Linux/FreeBSD 平台后端实现，支持 Wayland 和 X11 显示服务器，集成 XIM 输入法支持和桌面门户（Portal）功能。

## 概述

`rgpui_linux` 为 GPUI 提供 Linux 和 FreeBSD 系统上的完整平台抽象层，包括窗口管理、输入处理、剪贴板、文件选择器、密钥环等核心功能。本 crate 通过特性标志（feature flags）灵活选择 Wayland 或 X11 后端，并在运行时自动检测当前会话的显示服务器类型。

## 功能特性

### Wayland 支持

- 基于 Wayland 协议的现代显示服务器支持
- 使用 `wayland-client` 和 `wayland-protocols` 实现
- 支持 `wp_cursor_shape_v1` 光标形状协议
- 支持 `layer_shell` 协议（用于面板、通知、锁屏等桌面层表面）
- 集成 wl-clipboard 实现剪贴板功能
- 通过 ashpd 集成 XDG Desktop Portal

### X11 支持

- 基于 Xlib 的传统 X11 显示服务器支持
- 集成 XIM（X Input Method）输入法，支持多语言文本输入
- 支持多显示器配置
- 完整的剪贴板和选择（selection）支持
- 通过 ashpd 集成 XDG Desktop Portal

### 其他功能

- **无头模式（Headless）**：支持无图形界面的服务器环境运行
- **XKB 键盘处理**：基于 libxkbcommon 的键盘布局解析和修饰键处理
- **死键（Dead Key）支持**：支持输入带附加符号的字符
- **文件选择器**：通过 ashpd 桌面门户提供原生文件对话框
- **密钥环集成**：使用 oo7 库集成 Secret Service 协议
- **GPU 适配器选择**：通过 sysfs 解析合成器 GPU 信息，优化渲染设备选择

## 显示服务器对比

| 特性 | Wayland | X11 |
|------|---------|-----|
| 架构 | 现代合成器架构 | 传统 X Server 架构 |
| 安全性 | 客户端隔离，更安全 | 客户端可互相访问 |
| 输入方法 | IBus/Fcitx5 原生支持 | XIM 输入法支持 |
| 多显示器 | 原生支持 | 需 XRandR 扩展 |
| 屏幕共享 | PipeWire + Portal | 需额外配置 |
| 兼容性 | 较新，部分旧软件不支持 | 广泛兼容 |
| 远程桌面 | 支持（PipeWire） | 支持（X11 forwarding） |

### Wayland

Wayland 是新一代显示服务器协议，设计目标是替代 X11。它采用更简洁的架构，合成器直接管理客户端窗口的渲染，提供更好的安全性和性能。主流合成器包括 GNOME (Mutter)、KDE (KWin)、Sway 等。

### X11

X11（X Window System）是 Linux 桌面环境的传统显示服务器，拥有数十年的历史和广泛的软件兼容性。虽然架构较为陈旧，但在远程显示、网络透明性等方面仍有独特优势。

## 系统要求

### 编译依赖

- Rust 工具链（最新稳定版）
- pkg-config
- C 编译器（gcc 或 clang）

### 运行时依赖

- **Wayland 后端**：
  - Wayland 合成器（Mutter、KWin、Sway 等）
  - `libwayland-client`
  - `libxkbcommon`
  - `libseat`（可选）

- **X11 后端**：
  - X Server
  - `libX11`
  - `libX11-xcb`
  - `libxcb`
  - `libxkbcommon`
  - `libxcb-xkb`
  - `libxcb-xinput`
  - `libxcb-randr`
  - `libxcb-shape`
  - `libxcb-render`

- **通用依赖**：
  - `libfontconfig`（字体配置）
  - `libfreetype`（字体渲染）
  - D-Bus（桌面门户通信）
  - xdg-desktop-portal（文件选择器等）

## 特性标志

| 特性 | 描述 |
|------|------|
| `wayland` | 启用 Wayland 后端支持 |
| `x11` | 启用 X11 后端支持 |
| `screen-capture` | 启用屏幕捕获支持 |

## 架构

```
rgpui_linux/
├── src/
│   ├── rgpui_linux.rs      # 入口点，导出 current_platform
│   ├── linux.rs           # 模块整合，平台选择逻辑
│   └── linux/
│       ├── platform.rs    # LinuxPlatform 和 LinuxClient trait
│       ├── dispatcher.rs  # 事件循环调度器
│       ├── headless/      # 无头模式实现
│       ├── keyboard.rs    # 键盘布局抽象
│       ├── text_system.rs # 文本系统（Cosmic Text）
│       ├── wayland/       # Wayland 后端实现
│       ├── x11/           # X11 后端实现
│       └── xdg_desktop_portal.rs  # 桌面门户集成
```
