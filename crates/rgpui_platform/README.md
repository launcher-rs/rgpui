# gpui_platform

GPUI 平台抽象层。本包根据目标操作系统自动重新导出对应的平台实现（`gpui_macos`、`gpui_windows`、`gpui_linux` 或 `gpui_web`），使使用者无需手动编写 `#[cfg]` 条件编译代码。

## 功能概述

- 重新导出 `gpui::Platform` trait
- 提供 `current_platform()` 函数，根据编译目标自动创建正确的平台实例
- 提供便捷的应用程序构造函数：`application()`、`headless()`
- 提供 Web (Wasm) 平台的专用函数：`single_threaded_web()`、`web_init()`
- 提供测试支持：`current_headless_renderer()`（需开启 `test-support` 特性）

## 支持的平台

| 目标系统 | 后端包 |
|---|---|
| macOS | `gpui_macos` |
| Windows | `gpui_windows` |
| Linux / FreeBSD | `gpui_linux` |
| Wasm (Web) | `gpui_web` |

## 与 GPUI 其他包的关系

```
gpui_platform
├── gpui              (核心框架)
├── gpui_macos        (macOS 平台实现)
├── gpui_windows      (Windows 平台实现)
├── gpui_linux        (Linux 平台实现)
└── gpui_web          (Web/Wasm 平台实现)
```

`gpui_platform` 位于 `gpui` 核心包与各平台包之间，充当统一入口。应用层只需依赖 `gpui_platform`，无需关心底层平台差异。

## 使用示例

### 创建 GUI 应用程序

```rust
use gpui_platform;

// 创建 GUI 应用
let app = gpui_platform::application();

// 或创建无头模式应用
let app = gpui_platform::headless();
```

### 获取平台实例

```rust
use gpui_platform;

// 获取当前平台的 Platform 实现
let platform = gpui_platform::current_platform(false);
```

### 获取后台执行器

```rust
use gpui_platform;

let executor = gpui_platform::background_executor();
```

### Web (Wasm) 平台

```rust
use gpui_platform;

#[wasm_bindgen(start)]
pub fn main() {
    // 初始化 panic hook 和日志
    gpui_platform::web_init();

    // 创建单线程 Web 应用
    let app = gpui_platform::single_threaded_web();
}
```

## 特性 (Features)

| 特性 | 说明 |
|---|---|
| `font-kit` | 启用字体发现功能 |
| `test-support` | 启用测试支持，提供 `current_headless_renderer()` |
| `screen-capture` | 启用屏幕捕获功能 |
| `runtime_shaders` | 启用运行时着色器（macOS） |
| `wayland` | 启用 Wayland 显示服务器支持（Linux） |
| `x11` | 启用 X11 显示服务器支持（Linux） |
