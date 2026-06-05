# 快速入门

> 从零开始搭建 rgpui 开发环境并运行第一个应用。

## 环境要求

- **Rust**：1.75.0 或更新版本
- **操作系统**：Windows 10+、macOS 12+、Linux（需安装依赖）
- **GPU**：支持 Vulkan/Metal/WebGPU 的显卡

## 创建项目

```bash
cargo new my_rgpui_app
cd my_rgpui_app
```

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
rgpui = { path = "../path/to/rgpui" }
# 或者使用 git 依赖
# rgpui = { git = "https://github.com/your-org/rgpui.git" }
```

## 第一个应用

创建 `src/main.rs`：

```rust
use rgpui::prelude::*;
use rgpui::{div, App, Window};

fn main() {
    App::new().run(|cx| {
        // 创建第一个窗口
        cx.open_window(Window::default(), |window, cx| {
            // 替换窗口的根视图
            window.replace_root_view(cx, |window, cx| {
                // 创建一个简单的文本视图
                HelloView { text: "Hello, rgpui!".into() }
            });
        });
    });
}

/// 一个简单的视图结构体
struct HelloView {
    text: SharedString,
}

/// 实现 Render trait，定义视图的渲染内容
impl Render for HelloView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_center()
            .w_full()
            .h_full()
            .child(
                div()
                    .text_lg()
                    .child(self.text.clone())
            )
    }
}
```

## 运行应用

```bash
cargo run
```

你应该会看到一个包含 "Hello, rgpui!" 文本的窗口。

## 代码解析

### App::new().run()

这是 rgpui 应用的入口点。`run` 方法接收一个闭包，闭包参数 `cx` 是 `App` 的引用。
在闭包中，你可以：

- 打开窗口（`cx.open_window`）
- 设置全局状态（`cx.set_global`）
- 注册快捷键（`cx.bind_keys`）

### Window 和 View

- **Window**：代表一个操作系统窗口，包含标题栏、菜单等
- **View**：窗口中的内容，通过 `Render` trait 定义其 UI 结构

### Render trait

每个视图都需要实现 `Render` trait：

```rust
impl Render for MyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 返回一个或多个元素
    }
}
```

`render` 方法在每次视图需要重新绘制时被调用。

## 下一步

- [实体系统](./02-entity-system.md) - 了解 Entity、Context 与生命周期管理
- [订阅系统](./03-subscription.md) - 学习事件订阅和组件间通信
- [快捷键系统](./04-keymap.md) - 为应用添加键盘快捷键
