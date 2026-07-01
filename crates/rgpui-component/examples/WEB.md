# rgpui Web 开发指南

本指南介绍如何使用 rgpui 和 rgpui-component 构建 WebAssembly 应用。

## 前置条件

### 1. 安装 Rust Nightly 工具链

Web 构建需要 nightly 工具链和 `wasm32-unknown-unknown` 目标：

```bash
rustup toolchain install nightly
rustup target add wasm32-unknown-unknown --toolchain nightly
rustup component add rust-src --toolchain nightly
```

### 2. 安装 Trunk

[Trunk](https://trunkrs.dev/) 是 WASM 应用的构建和开发服务器工具：

```bash
cargo install trunk
```

### 3. 浏览器要求

- 支持 WebGPU 的浏览器（Chrome 113+、Edge 113+、Opera 99+）
- 或支持 WebGL2 的浏览器（Firefox、Safari 作为降级方案）
- 需要本地服务器环境（Trunk 自动处理）

## 运行 Web 示例

### 快速开始

```bash
# 运行 hello_world_web 示例
cd crates/rgpui-component/examples/hello_world_web
trunk serve

# 运行 components_web 示例
cd crates/rgpui-component/examples/components_web
trunk serve

# 运行 rgpui-web 的 hello_web 示例
cd crates/rgpui-web/examples/hello_web
trunk serve
```

Trunk 会自动：
- 编译 WASM 二进制文件
- 生成 `index.html` 胶水代码
- 启动本地开发服务器（默认 `http://127.0.0.1:8080`）
- 打开浏览器

### 构建生产版本

```bash
trunk build --release
```

输出文件在 `dist/` 目录。

## 创建新的 Web 示例

### 目录结构

```
my_web_example/
├── .cargo/
│   └── config.toml      # WASM 编译配置
├── src/
│   └── main.rs          # 应用代码
├── index.html           # HTML 外壳
├── trunk.toml           # Trunk 配置
├── rust-toolchain.toml  # 工具链配置
└── Cargo.toml           # 包配置
```

### 1. Cargo.toml

```toml
[workspace]

[package]
name = "my_web_example"
version = "0.1.0"
edition = "2024"
publish = false

[[bin]]
name = "my_web_example"
path = "src/main.rs"

[dependencies]
rgpui = { path = "../../../rgpui" }
rgpui_platform = { path = "../../../rgpui-platform" }
# 如果使用组件库：
rgpui_component = { path = "../../" }
rgpui_component_assets = { path = "../../../rgpui-component-assets" }
wasm-bindgen = "0.2"
```

### 2. .cargo/config.toml

```toml
[target.wasm32-unknown-unknown]
rustflags = [
    "-C", "target-feature=+atomics,+bulk-memory,+mutable-globals",
    "-C", "link-arg=--shared-memory",
    "-C", "link-arg=--max-memory=1073741824",
    "-C", "link-arg=--import-memory",
    "-C", "link-arg=--export=__wasm_init_tls",
    "-C", "link-arg=--export=__tls_size",
    "-C", "link-arg=--export=__tls_align",
    "-C", "link-arg=--export=__tls_base",
    "-C", "link-arg=--export=__heap_base",
]

[unstable]
build-std = ["std", "panic_abort"]
```

### 3. rust-toolchain.toml

```toml
[toolchain]
channel = "nightly"
targets = ["wasm32-unknown-unknown"]
components = ["rust-src", "rustfmt", "clippy"]
```

### 4. index.html

```html
<!doctype html>
<html lang="zh-CN">
    <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, height=device-height, initial-scale=1.0, user-scalable=0" />
        <title>My Web Example</title>
        <link data-trunk rel="rust" data-bin="my_web_example" data-bindgen-target="web" data-keep-debug data-wasm-opt="0" />
        <style>
            * { margin: 0; padding: 0; box-sizing: border-box; }
            html, body { margin: 0; height: 100%; }
            canvas {
                display: block; width: 100%; height: 100%;
                touch-action: none; outline: none;
                -webkit-user-select: none; user-select: none;
            }
        </style>
    </head>
    <body></body>
</html>
```

### 5. trunk.toml

```toml
[serve]
addresses = ["127.0.0.1"]
port = 8080
open = true

# WebGPU / SharedArrayBuffer 所需的 COOP/COEP 头
headers = { "Cross-Origin-Embedder-Policy" = "require-corp", "Cross-Origin-Opener-Policy" = "same-origin" }
```

### 6. main.rs

```rust
#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::*;
use rgpui_component::{button::*, *};
use rgpui_component_assets::Assets;

struct MyApp;

impl Render for MyApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .items_center()
            .justify_center()
            .child("Hello from Web!")
    }
}

fn run_example() {
    let app = rgpui_platform::application().with_assets(Assets);
    app.run(move |cx| {
        rgpui_component::init(cx);
        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|_| MyApp);
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("Failed to open window");
        })
        .detach();
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}
```

## Web 平台 API 参考

### 初始化

```rust
// 在 wasm_bindgen 入口函数中调用
rgpui_platform::web_init();  // 初始化 panic hook 和日志
```

### 应用创建

```rust
// 多线程模式（默认，支持 Web Worker）
let app = rgpui_platform::application();

// 单线程模式（无 Web Worker）
let app = rgpui_platform::single_threaded_web();
```

### 平台能力

| 功能 | Web 支持 | 说明 |
|------|----------|------|
| 窗口创建 | ✅ | 通过 Canvas 元素 |
| GPU 渲染 | ✅ | WebGPU / WebGL2 |
| 多线程 | ✅ | Web Worker（需要 COOP/COEP 头） |
| 键盘输入 | ✅ | 完整支持 |
| 鼠标/触摸 | ✅ | Pointer Events |
| 拖放 | ✅ | 文件拖放 |
| IME 输入 | ✅ | Composition Events |
| 暗色模式 | ✅ | `prefers-color-scheme` 媒体查询 |
| 全屏 | ✅ | Fullscreen API |
| 剪贴板 | ⚠️ | 未实现（返回 None） |
| 文件对话框 | ❌ | 浏览器安全限制 |
| 凭据存储 | ❌ | 浏览器安全限制 |
| 系统托盘 | ❌ | 浏览器不支持 |
| 原生菜单 | ❌ | 浏览器不支持 |

### 多线程注意事项

Web 多线程需要特殊 HTTP 头：

```
Cross-Origin-Embedder-Policy: require-corp
Cross-Origin-Opener-Policy: same-origin
```

这些头启用 `SharedArrayBuffer`，是 Web Worker 间通信的基础。Trunk 通过 `trunk.toml` 的 `headers` 配置自动添加。

### 资源加载

Web 模式下，`rgpui-component-assets` 通过 CDN 下载 SVG 图标（运行时），而非使用 `rust-embed` 本地嵌入。确保网络连接可用。

## 已知限制

1. **Tree-sitter 语法高亮不可用**：WASM 中无法编译 C 依赖，编辑器相关功能使用空实现
2. **剪贴板未实现**：`read_from_clipboard()` 返回 None，`write_to_clipboard()` 为空操作
3. **文件系统 API 受限**：浏览器安全沙箱阻止直接文件访问
4. **部分原生功能不可用**：系统托盘、原生菜单、全局快捷键等桌面特有功能
5. **性能差异**：WASM 执行速度约为原生的 50-80%，多线程通过 Web Worker 部分补偿

## 故障排除

### 编译错误：`can't find crate for std`

确保使用 nightly 工具链且 `.cargo/config.toml` 包含 `build-std = ["std", "panic_abort"]`。

### 运行时错误：`SharedArrayBuffer is not defined`

需要 COOP/COEP HTTP 头。确保 `trunk.toml` 配置了正确的 headers，或使用 HTTPS。

### 图标不显示

Web 模式下图标从 CDN 下载，检查网络连接和浏览器控制台是否有加载错误。

### 性能问题

- 使用 `single_threaded_web()` 避免 Web Worker 开销（简单应用）
- 减少每帧的元素数量
- 使用 `overflow_y_scrollbar()` 而非全量重绘
