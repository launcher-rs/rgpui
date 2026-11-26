# 快速入门

> 从零开始搭建 rgpui 开发环境并运行第一个应用。

## 环境要求

- **Rust**：1.88 或更新版本（edition 2024）
- **操作系统**：Windows 10+、macOS 12+（代码保留、暂无设备验证）、Linux（X11/Wayland，需装依赖）
- **GPU**：支持 Vulkan/Metal/WebGPU 的显卡（软件渲染可用 llvmpipe/lavapipe）

## 创建项目

```bash
cargo new my_rgpui_app
cd my_rgpui_app
```

在 `Cargo.toml` 中添加依赖。**必须同时依赖 `rgpui-platform` 并用它启动应用**：
单独依赖 `rgpui` 在 Linux 上会因 feature 未开启而静默退化成无头后端（编译成功、
没有窗口、零报错）。

```toml
[dependencies]
rgpui = { path = "../path/to/rgpui" }
rgpui-platform = { path = "../path/to/rgpui/crates/rgpui-platform" }
# 或者使用 git 依赖
# rgpui = { git = "https://github.com/launcher-rs/rgpui.git" }
# rgpui-platform = { git = "https://github.com/launcher-rs/rgpui.git" }
```

Linux 上还需要系统库（用于 webview 示例，影响 `cargo check --workspace`）：

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libglib2.0-dev
```

细节见 [AGENTS.md](../../AGENTS.md) 的「Linux 端构建前置」。

## 第一个应用

创建 `src/main.rs`：

```rust
use rgpui::{
    App, Bounds, Context, SharedString, TitlebarOptions, Window, WindowBounds, WindowOptions,
    div, prelude::*, px, size,
};
use rgpui_platform::application;

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(500.0), px(400.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Hello rgpui".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|_| HelloView {
                text: "Hello, rgpui!".into(),
            }),
        );
    });
}

struct HelloView {
    text: SharedString,
}

impl Render for HelloView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_center()
            .size_full()
            .text_xl()
            .child(self.text.clone())
    }
}
```

## 运行应用

```bash
cargo run
```

你应该会看到一个包含 "Hello, rgpui!" 文本的窗口。

## 代码解析

### `application().run()`

这是应用入口。`rgpui_platform::application()` 会按当前平台选择后端（Windows / macOS /
Linux 的 Wayland 或 X11 / Web），并把闭包里的 `cx: &mut App` 交给应用。传 `rgpui` 自带
的 `Application` 而绕过 `rgpui-platform` 会漏掉平台后端的选择。

### `cx.open_window(options, build_view)`

`open_window` 接收 [`WindowOptions`] 和一个构建根视图的闭包。视图通过 `cx.new(...)` 创建成
`Entity<T>`，由框架持有；旧版的 `Window::default()` / `window.replace_root_view(...)` 写法
已不存在。

### `Render` trait

每个视图实现 `Render`，`render` 在视图需要重绘时被调用，返回实现 `IntoElement` 的元素树
（通常用 `div()` 起手）。`Context<Self>` 提供 `cx.listener(...)`、`cx.notify()` 等视图级能力。

## 下一步

- [实体系统](./02-entity-system.md) - 了解 Entity、Context 与生命周期管理
- [订阅系统](./03-subscription.md) - 学习事件订阅和组件间通信
- [快捷键系统](./04-keymap.md) - 为应用添加键盘快捷键
- [平台契约审计](../../docs/linux-api-parity-audit.md) - 哪些「跨平台」API 实际只有某平台生效
