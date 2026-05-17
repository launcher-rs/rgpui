# gpui_web

GPUI 框架的 WebAssembly 平台实现，使用 `web-sys` API 与浏览器环境交互。

## 概述

`gpui_web` 为 GPUI 提供 WebAssembly (Wasm) 目标平台的完整后端实现，使 GPUI 应用能够编译为 Wasm 并在现代浏览器中运行。本 crate 通过 `web-sys` 和 `wasm-bindgen` 与浏览器 API 进行无缝交互，并使用 `gpui_wgpu` 作为渲染后端。

## 功能特性

### 浏览器 API 集成

- **DOM 操作**：通过 `web-sys` 访问和操作 HTML 元素
- **Canvas 渲染**：使用 `HtmlCanvasElement` 作为 WebGPU 渲染目标
- **事件处理**：完整的鼠标、键盘、指针和滚轮事件支持
- **拖放支持**：通过 `DragEvent` 和 `DataTransfer` 实现文件拖放

### 输入系统

- 键盘事件处理（`KeyboardEvent`）
- 鼠标和指针事件（`MouseEvent`、`PointerEvent`）
- 滚轮事件（`WheelEvent`）
- 组合事件支持（`CompositionEvent`）— 用于 IME 输入法
- 视觉视口（`VisualViewport`）跟踪

### 多线程支持

- 可选的 `wasm_thread` 支持，启用 Web Worker 多线程
- 通过 `multithreaded` 特性标志控制
- 使用 `parking_lot` 的 nightly 特性实现高效同步

### 其他功能

- **ResizeObserver**：响应式布局调整
- **MediaQueryList**：媒体查询监听（如暗色模式检测）
- **Storage API**：本地存储持久化
- **Fetch API**：通过 `web-sys` 的 HTTP 请求支持
- **console_error_panic_hook**：将 Rust panic 输出到浏览器控制台

## 系统要求

### 编译依赖

- Rust 工具链（最新稳定版）
- `wasm32-unknown-unknown` 目标工具链
- `wasm-bindgen-cli`（用于生成 JS 绑定）

### 运行时依赖

- 支持 WebAssembly 的现代浏览器
- 支持 WebGPU 的浏览器（Chrome 113+、Firefox Nightly 等）

## 特性标志

| 特性 | 描述 |
|------|------|
| `multithreaded` | 启用 Web Worker 多线程支持（依赖 `wasm_thread`） |

## 架构

```
gpui_web/
├── src/
│   ├── gpui_web.rs          # 入口点
│   └── web/
│       ├── platform.rs      # WebPlatform 实现
│       ├── window.rs        # 浏览器窗口管理
│       ├── input.rs         # 浏览器输入事件处理
│       ├── text_system.rs   # Web 文本系统
│       └── assets.rs        # 资源加载
```

## 使用示例

```rust
use gpui_web::WebPlatform;

// 初始化 panic hook，将 Rust panic 输出到浏览器控制台
console_error_panic_hook::set_once();

// 创建 Web 平台实例
let platform = WebPlatform::new();

// 平台会自动设置 WebGPU 渲染器和浏览器事件监听
// 应用将渲染到 HtmlCanvasElement 上
```

### 编译为 WebAssembly

```bash
# 添加 wasm32-unknown-unknown 目标
rustup target add wasm32-unknown-unknown

# 编译
cargo build --target wasm32-unknown-unknown

# 使用 wasm-bindgen 生成 JS 绑定
wasm-bindgen --out-dir pkg target/wasm32-unknown-unknown/debug/gpui_web.wasm
```

## 依赖关系

- `gpui` — GPUI 核心框架
- `gpui_wgpu` — WebGPU 渲染后端
- `web-sys` — Web API 的 Rust 绑定
- `wasm-bindgen` / `wasm-bindgen-futures` — Rust 与 JavaScript 互操作
- `js-sys` — JavaScript 内置对象的绑定
- `http_client` — HTTP 客户端抽象
- `wasm_thread`（可选）— Web Worker 线程支持
- `raw-window-handle` — 跨平台窗口句柄抽象
