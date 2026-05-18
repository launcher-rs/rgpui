# RGPUI

RGPUI 是一个混合即时/保留模式、GPU 加速的 Rust UI 框架，旨在支持广泛的应用程序开发。

## 主要特性

- **混合即时/保留模式**：结合即时模式的高性能与保留模式的状态管理优势
- **基于 Entity 的状态管理**：通过智能指针安全地在应用各部分之间共享和通信状态
- **声明式视图（View）**：实现 `Render` trait 即可构建声明式 UI，每帧自动刷新
- **命令式元素（Element）**：底层构建块，提供对渲染和布局的完全控制
- **GPU 加速渲染**：利用 Metal/Vulkan 进行高性能图形渲染
- **Taffy 布局引擎**：支持 Flexbox 和 Grid 布局
- **丰富的文本系统**：支持字体渲染、文本布局和高亮
- **输入处理与快捷键**：完整的键盘/鼠标事件处理和可配置的键位映射
- **动画系统**：内置动画支持
- **跨平台**：支持 macOS、Linux 和 Windows
- **Tailwind 风格 API**：通过 `div` 元素提供熟悉的样式链式调用

## 快速开始

GPUI 目前处于活跃开发阶段（pre-1.0），版本之间可能会有破坏性变更。请确保使用最新稳定版 Rust。

### 安装依赖

**macOS**：
```sh
xcode-select --install
sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
```

**Linux**：需要安装相应的图形库依赖（如 Wayland/X11 开发包）。

### 添加依赖

在 `Cargo.toml` 中添加：

```toml
[dependencies]
rgpui = "0.2"
```

### 最小示例

```rust
use rgpui::*;

struct HelloWorld;

impl Render for HelloWorld {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_center()
            .size_full()
            .child("Hello, GPUI!")
    }
}

fn main() {
    App::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |window, cx| {
            cx.new(|cx| HelloWorld)
        });
    });
}
```

更多示例请查看 [examples 目录](../examples/) 或运行：

```sh
cargo run -p rgpui --example hello_world
```

## 架构概述

GPUI 提供三种不同层次的抽象，以满足不同场景的需求：

### 1. Entity — 状态管理

Entity 是 RGPUI 的状态容器，由框架统一管理生命周期。通过 `Entity<T>` 智能指针访问，类似 `Rc`，但支持跨组件通信和响应式更新。

```rust
// 创建 Entity
let counter = cx.new(|cx| Counter { value: 0 });

// 更新 Entity
cx.update_entity(&counter, |counter, cx| {
    counter.value += 1;
});
```

### 2. View — 声明式 UI

View 是可渲染的 Entity，通过实现 `Render` trait 定义 UI 结构。每帧开始时 GPUI 会调用根 View 的 `render` 方法，构建元素树。

```rust
impl Render for MyView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child("Title")
            .child(Button::new("click_me", "Click me"))
    }
}
```

### 3. Element — 命令式 UI

Element 是 UI 的底层构建块，提供对布局、绘制和事件处理的完全控制。适合实现高性能列表、自定义编辑器等场景。

## 核心概念

| 概念 | 说明 |
|------|------|
| `Application` | 应用入口点，管理全局状态和事件循环 |
| `Window` | 窗口上下文，处理渲染、输入和布局 |
| `Entity<T>` | 状态容器，由 GPUI 所有，通过智能指针访问 |
| `View` | 可渲染的 Entity，实现 `Render` trait |
| `Element` | UI 构建块，控制布局和绘制 |
| `Context` | 上下文 trait，提供与 GPUI 交互的主要接口 |
| `Action` | 用户定义的操作，用于将按键映射到逻辑操作 |
| `Global` | 全局状态，在应用范围内共享 |

### 数据流

```
用户输入 → Action → Entity 更新 → View 重新渲染 → Element 树 → GPU 渲染
```

## 示例

| 分类 | 示例 |
|------|------|
| 入门 | `hello_world`, `input`, `uniform_list` |
| 布局与样式 | `grid_layout`, `opacity`, `shadow`, `text` |
| 交互 | `drag_drop`, `scrollable`, `tab_stop`, `popover` |
| 图像与动画 | `image`, `svg`, `gradient`, `animation` |
| 窗口行为 | `set_menus`, `window_positioning`, `window_shadow` |

完整示例列表请查看 [examples/README.md](../examples/README.md)。

## 文档

- [上下文系统](docs/contexts.md)
- [按键分发](docs/key_dispatch.md)
- [所有权与数据流](src/_ownership_and_data_flow.rs)

## 许可证

GPUI 采用 Apache-2.0 许可证。详见 [LICENSE](LICENSE)。

## 相关链接

- [GitHub 仓库](https://github.com/launcher-rs/rgpui)
