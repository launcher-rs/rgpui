# rgpui-component

基于 rgpui 的跨平台 UI 组件库，用于构建出色的桌面应用程序。

本项目移植自 [longbridge/gpui-component](https://github.com/longbridge/gpui-component)。

## 功能特性

- **丰富组件**：60+ 跨平台桌面 UI 组件
- **原生体验**：融合 macOS/Windows 控件与 shadcn/ui 设计风格
- **易于使用**：无状态 `RenderOnce` 组件，简单直观
- **主题定制**：内置 `Theme` 和 `ThemeColor`，支持多主题和变量配置
- **多种尺寸**：支持 `xs`、`sm`、`md`、`lg` 尺寸
- **灵活布局**：Dock 布局支持面板排列、拖拽调整和自由布局
- **高性能**：虚拟化的 Table 和 List 组件，流畅渲染大数据
- **内容渲染**：原生支持 Markdown 和简单 HTML
- **图表**：内置图表组件，可视化数据
- **代码编辑器**：高性能代码编辑器（稳定支持 20 万行代码），支持 LSP
- **语法高亮**：基于 Tree Sitter 的语法高亮支持

## 组件列表

### 基础组件

| 组件 | 说明 |
|------|------|
| `Button` | 按钮，支持主要/次要样式 |
| `Label` | 文本标签 |
| `Icon` | 图标组件 |
| `Separator` | 分隔线 |
| `Skeleton` | 加载占位骨架屏 |
| `Spinner` | 加载旋转指示器 |

### 表单组件

| 组件 | 说明 |
|------|------|
| `Input` | 文本输入框 |
| `Checkbox` | 复选框 |
| `Radio` | 单选按钮 |
| `Switch` | 开关 |
| `Slider` | 滑块 |
| `Select` | 下拉选择器 |
| `Combobox` | 组合框 |
| `ColorPicker` | 颜色选择器 |
| `Rating` | 评分组件 |
| `Stepper` | 步进器 |
| `Form` | 表单容器与字段验证 |

### 布局组件

| 组件 | 说明 |
|------|------|
| `Dock` | 面板布局系统（支持拖拽、拆分视图） |
| `Resizable` | 可调整大小的面板 |
| `Tab` | 标签页 |
| `Sidebar` | 侧边栏 |
| `Collapsible` | 可折叠容器 |
| `Accordion` | 手风琴折叠面板 |
| `GroupBox` | 分组框 |

### 数据展示

| 组件 | 说明 |
|------|------|
| `Table` | 虚拟表格（支持大数据量） |
| `List` | 虚拟列表 |
| `Tree` | 树形控件 |
| `Chart` | 图表组件 |
| `Avatar` | 头像及头像组 |
| `Badge` | 徽章 |
| `Tag` | 标签 |
| `Breadcrumb` | 面包屑导航 |
| `Pagination` | 分页组件 |
| `DescriptionList` | 描述列表 |

### 反馈组件

| 组件 | 说明 |
|------|------|
| `Alert` | 警告提示 |
| `Notification` | 通知 |
| `Progress` | 进度条 |
| `ProgressCircle` | 环形进度条 |

### 导航组件

| 组件 | 说明 |
|------|------|
| `Menu` | 菜单 |
| `Tooltip` | 工具提示 |
| `Popover` | 弹出框 |
| `HoverCard` | 悬停卡片 |

### 覆盖层

| 组件 | 说明 |
|------|------|
| `Dialog` | 对话框 |
| `Sheet` | 抽屉面板 |

### 内容渲染

| 组件 | 说明 |
|------|------|
| `Text` | 富文本渲染 |
| `Markdown` | Markdown 渲染 |
| `Highlighter` | 代码语法高亮 |

### 其他

| 组件 | 说明 |
|------|------|
| `Clipboard` | 剪贴板 |
| `FocusTrap` | 焦点陷阱 |
| `TitleBar` | 自定义标题栏 |
| `WindowBorder` | 窗口边框 |
| `Kbd` | 快捷键显示 |
| `Link` | 链接 |
| `Time` | 日期选择器/日历 |
| `Scrollbar` | 滚动条 |
| `SearchableList` | 可搜索列表 |
| `Setting` | 设置项 |
| `Inspector` | 开发调试面板 |

## 安装

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
rgpui-component = { path = "../rgpui-component" }
```

### 可选功能

| 功能 | 说明 |
|------|------|
| `decimal` | 启用 `rust_decimal` 支持 |
| `inspector` | 启用开发调试面板 |

## 快速开始

```rust
use rgpui::*;
use rgpui_component::{button::*, *};

pub struct HelloWorld;

impl Render for HelloWorld {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .child("Hello, World!")
            .child(
                Button::new("ok")
                    .primary()
                    .label("开始")
                    .on_click(|_, _, _| println!("点击了！")),
            )
    }
}

fn main() {
    rgpui_platform::application().run(move |cx| {
        // 必须先初始化组件库
        rgpui_component::init(cx);

        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|_| HelloWorld);
                // 窗口根节点必须是 Root
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("无法打开窗口");
        })
        .detach();
    });
}
```

### 图标

组件库提供 `Icon` 元素，但不默认包含 SVG 文件。推荐使用 [Lucide](https://lucide.dev) 图标，将 SVG 文件按 [IconName](https://github.com/longbridge/gpui-component/blob/main/crates/ui/src/icon.rs#L86) 定义命名后放入项目即可。

## 示例

运行示例项目：

```bash
# 基础示例
cargo run -p hello_world

# 系统监控器（实时图表）
cargo run -p system_monitor

# 输入框示例
cargo run -p input

# 侧边栏示例
cargo run -p sidebar

# 对话框覆盖层
cargo run -p dialog_overlay

# 焦点陷阱
cargo run -p focus_trap

# 自定义标题栏
cargo run -p window_title
```

## 主题

内置主题系统支持自定义颜色和主题配置：

```rust
// 使用默认主题
let theme = Theme::default();

// 自定义主题色
let color = theme.primary;
```

## 编辑器

代码编辑器和语法高亮能力已迁移到 `rgpui-editor`。

## 待办事项

- [x] 提取 Editor 组件为独立模块
- [ ] 提取 LSP 集成为独立模块

## 依赖

- `rgpui` - 核心 UI 框架
- `ropey` - 文本缓冲区（Rope 数据结构）
- `chrono` - 日期时间处理
- `markdown` - Markdown 解析
- `notify` - 文件监听

## 许可证

Apache-2.0

- UI 设计基于 [shadcn/ui](https://ui.shadcn.com)，部分来自 [Reui](https://reui.io)
- 图标来自 [Lucide](https://lucide.dev)
