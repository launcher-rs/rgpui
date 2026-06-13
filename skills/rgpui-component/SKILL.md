---
name: rgpui-component
description: 如何在 RGPIU 应用中使用 rgpui-component UI 库。适用于使用 rgpui-component 组件（Button、Input、Select、Dialog、Tabs、Sidebar、List、Table 等）构建 UI、设置库、处理组件状态、主题配置或查找适合特定 UI 需求的组件。
---

## 文档


## 快速参考

**设置** — 始终必需：
```rust
rgpui_component::init(cx);               // 在 app.run() 中，必须首先调用
Root::new(view, window, cx)             // 每个窗口中的第一级视图
```

**无状态** — 直接在 render 中使用：
```rust
Button::new("id").primary().label("OK").on_click(|_, _, _| {})
```

**有状态** — 在结构体中持有 `Entity<State>`，在 render 中传递引用：
```rust
// 在 new() 中：let input = cx.new(|cx| InputState::new(window, cx));
// 在 render 中：Input::new(&self.input)
```

**尺寸**：`.xsmall()` `.small()` `.medium()`（默认）`.large()`

**主题**：`cx.theme().primary` · `.background` · `.foreground` · `.border` · `.muted`

## 组件目录

需要组件时，在此处查找。完整 API 请获取其 `.md` 文档。

### 输入与表单
| 组件 | 导入路径 | 说明 |
|------|----------|------|
| `Input` | `input::{Input, InputState}` | 有状态。文本、密码、掩码、验证 |
| `NumberInput` | `number_input::{NumberInput, NumberInputState}` | 有状态。带步长的数值输入 |
| `OtpInput` | `otp_input::{OtpInput, OtpInputState}` | 有状态。一次性密码 |
| `Select` | `select::{Select, SelectState}` | 有状态。下拉选择器 |
| `Combobox` | `combobox::{Combobox, ComboboxState}` | 有状态。可搜索选择 |
| `Checkbox` | `checkbox::Checkbox` | 无状态。`on_click(|&bool, ...|)` |
| `Switch` | `switch::Switch` | 无状态。开关切换 |
| `Radio` | `radio::{Radio, RadioGroup}` | 无状态。 |
| `Slider` | `slider::{Slider, SliderState}` | 有状态。 |
| `Toggle` | `toggle::Toggle` | 无状态。 |
| `Rating` | `rating::Rating` | 无状态。 |
| `Stepper` | `stepper::Stepper` | 无状态。增减步进 |
| `ColorPicker` | `color_picker::{ColorPicker, ColorPickerState}` | 有状态。 |
| `DatePicker` | `time::date_picker::{DatePicker, DatePickerState}` | 有状态。 |
| `Form` | `form::{v_form, h_form, field}` | 表单字段布局容器 |

### 显示与反馈
| 组件 | 导入路径 | 说明 |
|------|----------|------|
| `Button` | `button::{Button, ButtonGroup}` | 无状态。主要 UI 操作 |
| `Icon` | `{Icon, IconName}` | 无状态。Lucide 图标 |
| `Badge` | `badge::Badge` | 无状态。 |
| `Tag` | `tag::Tag` | 无状态。可关闭标签 |
| `Avatar` | `avatar::Avatar` | 无状态。 |
| `Label` | `label::Label` | 无状态。表单标签 |
| `Kbd` | `kbd::Kbd` | 无状态。键盘按键显示 |
| `Alert` | `alert::Alert` | 无状态。信息/成功/警告/错误 |
| `Spinner` | `spinner::Spinner` | 无状态。加载指示器 |
| `Skeleton` | `skeleton::Skeleton` | 无状态。加载占位符 |
| `Progress` | `progress::{ProgressBar, ProgressCircle}` | 无状态。 |
| `Tooltip` | `tooltip::Tooltip` | 通过元素上的 `.tooltip()` 使用 |
| `HoverCard` | `hover_card::{HoverCard, HoverCardState}` | 有状态。 |
| `Image` | `image::Image` | 无状态。 |
| `Clipboard` | `clipboard::Clipboard` | 无状态。复制按钮 |

### 覆盖层与弹窗
| 组件 | 导入路径 | 说明 |
|------|----------|------|
| `Dialog` | `dialog::Dialog` + `WindowExt` | 通过 `window.open_modal(...)` |
| `AlertDialog` | `WindowExt` | 通过 `window.open_alert_dialog(...)` |
| `Sheet` | `sheet::Sheet` + `WindowExt` | 侧面板，通过 `window.open_sheet(...)` |
| `Notification` | `notification::Notification` + `WindowExt` | 通过 `window.push_notification(...)` |
| `Popover` | `popover::Popover` | 浮动覆盖层 |
| `Menu` | `menu::{PopupMenu, DropdownMenu}` | 上下文菜单 |
| `DropdownButton` | `button::DropdownButton` | 带下拉菜单的按钮 |

### 导航与布局
| 组件 | 导入路径 | 说明 |
|------|----------|------|
| `Tabs` / `TabBar` | `tab::{Tab, TabBar}` | 标签页界面 |
| `Sidebar` | `sidebar::{Sidebar, SidebarMenu, ...}` | 应用导航面板 |
| `TitleBar` | `title_bar::TitleBar` | 窗口标题栏 |
| `Breadcrumb` | `breadcrumb::Breadcrumb` | 导航面包屑 |
| `Pagination` | `pagination::Pagination` | 页面导航 |
| `Accordion` | `accordion::Accordion` | 可折叠区域 |
| `Collapsible` | `collapsible::Collapsible` | 单个可折叠组件 |
| `GroupBox` | `group_box::GroupBox` | 带标签容器 |
| `Resizable` | `resizable::Resizable` | 可拖拽分割窗格 |
| `Scrollable` | `scroll::Scrollbar` | 自定义滚动条 |
| `FocusTrap` | `focus_trap::FocusTrap` | 模态框键盘陷阱 |

### 数据展示
| 组件 | 导入路径 | 说明 |
|------|----------|------|
| `DataTable` | `table::{DataTable, TableState, TableDelegate}` | 有状态。全功能表格 |
| `Table` | `table::{Table, ...}` | 简单表格 |
| `VirtualList` | `{v_virtual_list, h_virtual_list}` | 高性能大列表 |
| `List` | `list::{List, ListState, ListDelegate}` | 有状态。可搜索列表 |
| `Tree` | `tree::{Tree, TreeState, TreeDelegate}` | 有状态。层级结构 |
| `DescriptionList` | `description_list::DescriptionList` | 键值对列表 |
| `Settings` | `settings::Settings` | 设置面板 |

### 图表
| 组件 | 导入路径 | 说明 |
|------|----------|------|
| `Chart` | `chart::Chart` | 柱状图、折线图、面积图、饼图 |
| `Plot` | `plot::Plot` | `#[derive(IntoPlot)]` 用于数据 |

## 参考文件

- [usage.md](references/usage.md) — 设置模式、组件类型、常见示例
- [style-guide.md](references/style-guide.md) — 贡献者代码风格
