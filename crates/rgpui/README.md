# rgpui

rgpui 是 [rgpui](https://github.com/launcher-rs/rgpui) 项目的核心 crate，是一个混合即时/保留模式、GPU 加速的 Rust UI 框架。

> 关于项目背景、重命名原因和整体架构，请参见[项目根目录 README](../../README.md)。

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
- **系统托盘**：完整托盘图标、右键菜单及窗口隐藏/恢复功能
- **增强透明窗口**：支持半透明、毛玻璃等视觉效果

## 快速开始

### 添加依赖

在 `Cargo.toml` 中添加：

```toml
[dependencies]
rgpui = "0.1"
rgpui_platform = "0.1"
```

### 最小示例

```rust
use rgpui::*;
use rgpui_platform::application;

struct HelloWorld;

impl Render for HelloWorld {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_center()
            .size_full()
            .child("Hello, RGPUI!")
    }
}

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Hello你好".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_window, cx| cx.new(|_cx| HelloWorld),
        )
            .unwrap();
    });
}
```

## 许可证

Apache-2.0
