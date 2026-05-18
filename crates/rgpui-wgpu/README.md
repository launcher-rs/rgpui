# gpui_wgpu

GPUI 框架的 WebGPU 渲染后端，为 Linux 和 Web 平台提供跨平台的 GPU 渲染能力。

## 概述

`gpui_wgpu` 是基于 `wgpu` 的跨平台 GPU 渲染后端，为 GPUI 提供现代化的图形渲染管线。它支持 Linux（通过 Vulkan/Metal/OpenGL）和 Web（通过 WebGPU）平台，使用 `cosmic-text` 和 `swash` 进行文本渲染和字形光栅化。

## 功能特性

### WebGPU 渲染

- 基于 `wgpu` 的现代 GPU 渲染管线
- 支持多种图形后端：Vulkan、Metal、DX12、OpenGL、WebGPU
- 高效的纹理管理和渲染图集（render atlas）
- GPU 加速的 2D 图形绘制和合成

### 文本渲染

- **cosmic-text**：高性能的文本布局和排版引擎
- **swash**：OpenType 字体解析和字形光栅化
- 支持字体回退（font fallback）
- 亚像素抗锯齿和_hinting_ 支持
- 多语言和复杂脚本支持

### 纹理管理

- 使用 `etagere` 进行 2D 矩形分配（纹理图集打包）
- 高效的纹理上传和缓存策略
- 支持多种纹理格式和采样器配置

### 跨平台支持

- **Linux**：通过 Vulkan 或 OpenGL 后端
- **Web**：通过 WebGPU API（`wasm32-unknown-unknown` 目标）
- 统一的渲染接口，平台差异由 `wgpu` 抽象层处理

### 其他功能

- **raw-window-handle** 0.6 协议支持
- 性能分析集成（`profiling` crate）
- 可选的 `font-kit` 字体加载支持（主要用于 Linux 平台）

## 系统要求

### 编译依赖

- Rust 工具链（最新稳定版）
- C 编译器（用于部分依赖构建）

### 运行时依赖

- **Linux**：
  - Vulkan 驱动（推荐）或 OpenGL 支持
  - `libfontconfig` 和 `libfreetype`（字体支持）
- **Web**：
  - 支持 WebGPU 的浏览器（Chrome 113+、Firefox Nightly 等）

## 特性标志

| 特性 | 描述 |
|------|------|
| `font-kit` | 启用 `font-kit` 字体加载支持（主要用于 Linux 多字体源场景） |

## 架构

```
gpui_wgpu/
├── src/
│   ├── gpui_wgpu.rs         # 入口点
│   └── wgpu/
│       ├── renderer.rs      # 主渲染器实现
│       ├── atlas.rs         # 纹理图集管理
│       ├── text.rs          # 文本渲染（cosmic-text + swash）
│       ├── bounds.rs        # 边界和裁剪处理
│       └── drawable.rs      # 可绘制表面抽象
```

## 使用示例

```rust
use rgpui_wgpu::WgpuRenderer;

// 创建 WebGPU 渲染器
let renderer = WgpuRenderer::new(surface_config)?;

// 渲染器会自动初始化 GPU 设备和渲染管线
// 支持纹理上传、绘制命令和帧合成
```

## 依赖关系

- `gpui` — GPUI 核心框架
- `wgpu` — WebGPU 的 Rust 实现
- `cosmic-text` — 文本布局和排版引擎
- `swash` — OpenType 字体解析和字形光栅化
- `etagere` — 2D 矩形分配器（纹理图集管理）
- `gpui_util` — GPUI 工具库
- `bytemuck` — 零成本类型转换
- `raw-window-handle` — 跨平台窗口句柄抽象
- `pollster` — 异步阻塞执行器（非 Wasm 平台）
- `font-kit`（可选）— 跨平台字体加载库
