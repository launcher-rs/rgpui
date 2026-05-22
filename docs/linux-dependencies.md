# Linux 平台依赖

## 系统库依赖

在 Linux 上编译和运行 rgpui 需要以下系统库：

### 基础依赖（必需）

```bash
# X11 开发库
sudo apt-get install -y libxcb-xfixes0-dev libxcb-shape0-dev libxcb-xkb-dev

# XKB 键盘处理
sudo apt-get install -y libxkbcommon-dev libxkbcommon-x11-dev

# 字体渲染
sudo apt-get install -y libfontconfig-dev libfreetype-dev

# 通用工具
sudo apt-get install -y pkg-config libssl-dev libcairo2-dev
```

### GTK/WebView 依赖（用于 rgpui-webview）

```bash
sudo apt-get install -y libgtk-3-dev libsoup-3.0-dev libwebkit2gtk-4.1-dev
```

### Wayland 依赖（可选，使用 wayland 特性时需要）

Wayland 相关系统库通常随桌面环境已安装。如需手动安装：

```bash
sudo apt-get install -y libwayland-dev wayland-protocols
```

### 完整安装命令

同时安装所有依赖：

```bash
sudo apt-get install -y \
    pkg-config \
    libfontconfig-dev \
    libfreetype-dev \
    libssl-dev \
    libxcb-xfixes0-dev \
    libxcb-shape0-dev \
    libxcb-xkb-dev \
    libxkbcommon-dev \
    libxkbcommon-x11-dev \
    libgtk-3-dev \
    libsoup-3.0-dev \
    libwebkit2gtk-4.1-dev \
    libwayland-dev \
    libcairo2-dev
```

## 显卡/GPU 驱动

rgpui 使用 wgpu 进行 GPU 加速渲染，需要系统支持以下之一：

- **Vulkan**（首选）：安装 `mesa-vulkan-drivers` 或对应厂商 Vulkan 驱动
- **OpenGL ES 3.0+**：通常已随 mesa 驱动安装
- **DX12**（通过 Vulkan 转译层）：仅 Windows

```bash
# 安装 Vulkan 驱动（Intel/AMD）
sudo apt-get install -y mesa-vulkan-drivers

# 或 NVIDIA 专有驱动
# sudo apt-get install -y nvidia-driver-XXX
```

### 虚拟机环境

在虚拟机（如 VirtualBox、VMware）中运行时，**没有物理 GPU** 会导致 wgpu 初始化失败并报错：

```
thread 'main' panicked at .../wgpu-hal-.../src/gles/egl.rs:...:
not yet implemented: xcb
```

这是因为 wgpu 的 GLES/EGL 后端在 X11/XCB 上的实现尚不完整，且虚拟机的软渲染环境无法提供 Vulkan 或完整的 OpenGL ES 支持。

**影响范围：** 任何创建窗口的示例都会触发此错误，因为 wgpu 在窗口初始化时就需要 GPU 上下文。

**解决方案：**
- 在虚拟机中开发时，所有带窗口渲染的示例均无法运行
- 如需测试非图形相关的功能（如托盘、全局快捷键等），可尝试设置 `ZED_HEADLESS=1` 环境变量启动无头模式，该模式使用 `HeadlessClient` 避免初始化 GPU
- 如需完整图形渲染，建议在物理机或支持 GPU 直通的 VM 环境上运行
- 或设置 `WGPU_BACKEND=gl` 环境变量尝试使用 GLES 后端（但在 X11 下可能仍会失败）
