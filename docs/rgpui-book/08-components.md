# 组件库

> rgpui 核心自带的完整 UI 组件库（`rgpui-component` 已删除，基础组件全部并入核心；新增组件见 `docs/ui-crate-plan.md` 的 rgpui-ui 计划）

## 概览

组件库已全部并入 `rgpui` 核心，无需额外依赖。按子系统划分：

| 子系统 | 模块 | 关键类型 |
|--------|------|----------|
| 滚动 | `elements/scroll/` | `Scrollable`、`Scrollbar`、`scrollable()`、`ScrollHandle` |
| 基础元素 | `elements/` | `Button`、`Checkbox`、`Radio`、`Switch`、`Slider`、`Spinner`、`Skeleton`、`Badge`、`Tag`、`Separator`、`Kbd`、`Tooltip`、`Icon` |
| 表单 | `form/` | `Form`、`Field`、`FieldBuilder`、`v_form`/`h_form`/`field` |
| 输入 | `input_ui/` | `Input`、`MaskedInput`、`NumberInput`、`PasswordInput`、`TextArea` |
| 菜单 | `menu/` | `PopupMenu`、`ContextMenu`、`DropdownMenu`、`HoverCard`、`Notification`、`Toast` |
| 对话框 | `dialog/` | `Dialog`、`AlertDialog`、`DialogHeader/Content/Footer/Title/Description` |
| 列表 | `list/` | `List`、`VirtualList`、`ListDelegate`、`ListState` |
| 表格 | `table/` | `Table`、`DataTable`、`Column`、`TableState`、`TableHeader/Body/Footer/Row/Head/Cell/Caption` |
| 标签页 | `tabs/` | `Tab`、`TabBar`、`TabVariant`、`Accordion`、`AccordionItem`、`Collapsible` |
| 标题栏 | `title_bar/` | `TitleBar`、`WindowBorder`、`window_paddings()` |

## 预导入（prelude）

推荐通过 `rgpui::prelude::*` 引入组件扩展 trait，避免逐个导入：

```rust
use rgpui::prelude::*;
```

prelude 暴露的扩展 trait 包括：`ActiveTheme`（`cx.theme()` 主题访问）、`ElementExt`（`on_prepaint`）、`InteractiveElementExt`（`on_double_click`）、`Selectable`（`selected`/`is_selected`）、`Sizable`（`small`/`xsmall`/`with_size`）、`StyledExt`（`h_flex`/`v_flex`/`paddings`/`refine_style`）、`FluentBuilder`（`when`/`when_some`/`map`）。

## 基础元素

```rust
use rgpui::{Button, ButtonVariants as _, Sizable as _, prelude::*};

Button::new("提交")
    .primary()
    .small()
    .on_click(|_, window, cx| {
        // 处理点击
    });
```

- `Button` 支持 `primary`/`secondary`/`danger`/`warning`/`success`/`info`/`ghost`/`link`/`text` 等变体。
- `Checkbox`、`Radio`、`Switch` 实现 `Selectable`，通过 `selected(bool)` 控制状态。
- `Icon::new(IconName::ArrowRight)` 配合 `IconName` 枚举（由 `assets/icons` 自动生成）。

## 表单与输入

```rust
use rgpui::{Form, Field, FieldBuilder as _, v_form, field, prelude::*};
use rgpui::input_ui::Input;

let form = v_form().child(
    field("用户名").child(
        Input::new("name")
            .placeholder("请输入用户名"),
    ),
);
```

- `Input` 系列（`Input`/`MaskedInput`/`NumberInput`/`PasswordInput`/`TextArea`）位于 `input_ui` 模块。
- `Form` + `Field` 提供表单容器与字段标签、错误提示布局。

## 菜单与对话框

```rust
use rgpui::{Button, ButtonVariants as _, Dialog, DialogHeader, DialogTitle, prelude::*};

Dialog::new("dialog")
    .title("提示")
    .child(DialogHeader::new().child(DialogTitle::new("确认操作？")))
    .child("内容");
```

- 菜单系统位于 `menu/`：`PopupMenu`（弹出菜单）、`ContextMenu`（右键菜单）、`DropdownMenu`（下拉菜单，可挂在任意元素上）、`HoverCard`（悬浮卡片）。
- 通知：`Notification`/`NotificationList`、`Toast`。
- 对话框：`Dialog`/`AlertDialog` 及 `DialogHeader/Content/Footer/Title/Description` 组合子组件。

## 列表与表格

```rust
use rgpui::{Table, TableHeader, TableBody, TableRow, TableHead, TableCell, prelude::*};

Table::new().header(TableHeader::new().child(TableRow::new().child("列A")))
    .body(TableBody::new().child(
        TableRow::new().child(TableCell::new().child("值A")),
    ));
```

- 列表：`List`/`VirtualList` 与 `ListDelegate`、`ListState` 用于虚拟化长列表。
- 表格：`Table` 可组合组件，或 `DataTable` + `TableState` + `Column` 实现完整数据表格（排序、列宽调整、拖拽列）。

## 标签页与手风琴

```rust
use rgpui::{TabBar, Tab, TabVariant as _, prelude::*};

TabBar::new("tabs")
    .segmented()
    .selected_index(0)
    .children(vec![Tab::new("概览"), Tab::new("详情")])
    .on_click(|ix, _, _| {
        // 处理标签切换
    });
```

- `TabBar` 支持 `Tab`/`Outline`/`Pill`/`Segmented`/`Underline` 五种变体，`Segmented`/`Pill`/`Underline` 带滑动指示器动画。
- `Accordion`/`AccordionItem`（手风琴）与 `Collapsible`（折叠面板）。

## 标题栏与窗口边框

```rust
use rgpui::{TitleBar, WindowBorder, window_paddings, WindowOptions, prelude::*};

// 创建窗口时使用 TitleBar::window_options() 作为基础，
// 让标题栏自行处理拖拽与双击
let options = WindowOptions {
    window_min_size: None,
    ..TitleBar::window_options()
};

// 渲染时组合 TitleBar 与内容
fn render() -> impl IntoElement {
    v_flex()
        .child(TitleBar::new().child("应用标题"))
        .child(WindowBorder::new().child(/* 内容 */))
        .paddings(window_paddings(window))
}
```

- `TitleBar`：自定义标题栏，自动绘制最小化/最大化/关闭按钮（Windows 走系统点击区域，Linux 手动处理）。
- `WindowBorder`：Linux 客户端装饰窗口边框与阴影，支持边缘调整大小（`resize_hit_size` 命中带）。

## 主题访问

所有组件通过 `cx.theme()`（`ActiveTheme` trait）读取当前主题颜色与令牌，跟随亮/暗模式自动切换：

```rust
let color = cx.theme().primary;            // 主色
let bg = cx.theme().tokens.tab_bar;        // 令牌颜色
```

更多主题系统细节见后续章节。