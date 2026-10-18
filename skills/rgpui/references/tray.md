# 系统托盘（Tray）

**目录：** [概述](#概述) · [快速入门](#快速入门) · [菜单项](#菜单项) · [事件回调](#事件回调) · [隐藏到托盘](#隐藏到托盘) · [Tray 结构体（旧 API）](#tray-结构体旧-api) · [API 速查](#api-速查) · [最佳实践](#最佳实践)

## 概述

系统托盘提供常驻通知区图标、右键菜单与点击事件，并可让应用在无窗口时保持运行。

**类型导出：** `pub use system::tray::*`（`rgpui.rs:543`）→
crate 根可用 `rgpui::{TrayIconEvent, TrayMenuItem, Tray, TrayIconData}`。
**不在 prelude**，需显式 `use rgpui::TrayMenuItem;`。

**两套 API：**

| 名称 | 入口 | 说明 |
|------|------|------|
| 新 API（推荐） | `App::set_tray_icon` / `set_tray_menu` / `set_tray_tooltip` / `on_tray_menu_action` | 按 id 标识符回调 |
| 旧 API | `App::set_tray(Tray, Option<Vec<MenuItem>>)` | 菜单项自带 `Action` |

可运行示例：`examples/tray`（`cargo run -p tray`，
含 `tray_simple` / `tray_menu_action` 两个 bin）、
`examples/showcase/src/bin/daemon_app.rs`、`examples/desktop_pet`。

## 快速入门

```rust
use rgpui::{App, TrayIconEvent, TrayMenuItem};

fn run_example() {
    application().run(|cx: &mut App| {
        // 无窗口时保持运行（关窗后托盘仍在）
        cx.set_keep_alive_without_windows(true);           // app.rs:2618

        cx.set_tray_tooltip("My App");                     // app.rs:2567
        let icon = include_bytes!("image/app-icon.png");
        cx.set_tray_icon(Some(icon.as_slice()));           // app.rs:2557（PNG/ICO 字节）

        cx.on_tray_icon_event(|event, _cx| match event {   // app.rs:2595
            TrayIconEvent::LeftClick => { /* ... */ }
            TrayIconEvent::RightClick => { /* ... */ }
            TrayIconEvent::DoubleClick => { /* ... */ }
        });

        cx.set_tray_menu(vec![                            // app.rs:2562
            TrayMenuItem::Action { label: "显示窗口".into(), id: "show_window".into() },
            TrayMenuItem::Separator,
            TrayMenuItem::Action { label: "退出".into(), id: "quit".into() },
        ]);

        cx.on_tray_menu_action(|id, cx| match id.as_ref() { // app.rs:2605
            "quit" => cx.quit(),
            "show_window" => { /* 恢复窗口 */ }
            _ => {}
        });

        /* open_window ... */
    });
}
```

## 菜单项

`TrayMenuItem`（`system/tray.rs:20`）：

```rust
pub enum TrayMenuItem {
    // 可点击操作项，id 是回调里拿到的唯一标识
    Action { label: SharedString, id: SharedString },
    Separator,
    // 子菜单（嵌套 TrayMenuItem）
    Submenu { label: SharedString, items: Vec<TrayMenuItem> },
    // 带复选标记的切换项
    Toggle { label: SharedString, checked: bool, id: SharedString },
}
```

```rust
TrayMenuItem::Submenu {
    label: "动画".into(),
    items: anim_names.iter().map(|name| TrayMenuItem::Action {
        label: name.clone().into(),
        id: name.clone().into(),
    }).collect(),
}
TrayMenuItem::Toggle { label: "可交互".into(), checked: true, id: "interactive".into() }
```

用法参考 `examples/desktop_pet_3d/src/main.rs:577`（动态构建子菜单 + Toggle）。

## 事件回调

```rust
pub enum TrayIconEvent { LeftClick, RightClick, DoubleClick }   // system/tray.rs:9
```

- `App::on_tray_icon_event(impl FnMut(TrayIconEvent, &mut App) + 'static)`（`app.rs:2595`）
- `App::on_tray_menu_action(impl FnMut(SharedString, &mut App) + 'static)`（`app.rs:2605`）
  —— 回调收到的是菜单项的 `id`。

**面板模式：** `App::set_tray_panel_mode(true)`（`app.rs:2573`）后，点击托盘图标触发
`LeftClick` 而非弹出菜单；`App::tray_icon_bounds()`（`app.rs:2590`）返回图标屏幕边界，
用于在其下方定位窗口。

## 隐藏到托盘

关闭窗口时隐藏而非退出，托盘菜单再恢复：

```rust
cx.open_window(opts, |window, cx| {
    let view = cx.new(|_| MyView { /* ... */ });
    window.on_window_should_close(cx, |window, _cx| {   // window.rs:6241
        window.hide_window();                            // 从任务栏移除
        false                                            // 返回 false 阻止关闭
    });
    view
});
```

- `Window::hide_window()`（`window.rs:6004`）：隐藏并从任务栏移除。
- `Window::minimize_window()`（`window.rs:5999`）：最小化但保留任务栏图标。
- `Window::activate_window()`（`window.rs:5989`）：恢复并激活。
- 恢复已存在窗口而非新建（托盘 `show_window` 场景）：

```rust
let windows = cx.windows();                          // app.rs:1240
if let Some(handle) = windows.first().copied() {
    cx.update_window(handle, |_, window, _cx| window.activate_window()).ok();
} else {
    create_main_window(cx);
}
```

## Tray 结构体（旧 API）

`App::set_tray(Tray, Option<Vec<MenuItem>>)`（`app.rs:2552`）配合构建器风格
`Tray`（`system/tray.rs:50`）：

```rust
let tray = Tray::new()                       // 也可 Tray::new().tooltip(..).icon(..).visible(..)
    .tooltip("My App")
    .icon(icon_image)
    .menu(|cx| vec![ /* MenuItem，自带 Action */ ]);
cx.set_tray(tray, None);
```

- `Tray::new()`（`:94`）、`tooltip(impl Into<SharedString>)`（`:105`）、
  `icon(impl Into<Image>)`（`:111`）、`menu(Fn(&mut App) -> Vec<MenuItem>)`（`:117`）、
  `visible(bool)`（`:126`）。
- `Tray::render_icon(SvgRenderer)`（`:65`）把 `icon` 渲染为平台图标数据。
- 旧 API 的菜单项 `MenuItem` 自带 `Action`，点击后经 `App::on_app_menu_action` 回到应用
  （见 `examples/tray/src/bin/tray_menu_action.rs`）。
- 需要标识符语义时用新 API（`set_tray_menu` + `on_tray_menu_action`）。

## API 速查

| App 方法 | 位置 | 说明 |
|----------|------|------|
| `set_tray(Tray, Option<Vec<MenuItem>>)` | `app.rs:2552` | 旧 API：图标 + 菜单 |
| `set_tray_icon(Option<&[u8]>)` | `app.rs:2557` | 设置图标（PNG/ICO 字节） |
| `set_tray_menu(Vec<TrayMenuItem>)` | `app.rs:2562` | 设置菜单 |
| `set_tray_tooltip(&str)` | `app.rs:2567` | 设置工具提示 |
| `set_tray_panel_mode(bool)` | `app.rs:2573` | 点图标触发 `LeftClick` 而非弹菜单 |
| `tray_icon_bounds() -> Option<Bounds<Pixels>>` | `app.rs:2590` | 图标屏幕边界 |
| `on_tray_icon_event(FnMut(TrayIconEvent, &mut App))` | `app.rs:2595` | 图标点击回调 |
| `on_tray_menu_action(FnMut(SharedString, &mut App))` | `app.rs:2605` | 菜单项回调（按 id） |
| `set_keep_alive_without_windows(bool)` | `app.rs:2618` | 无窗口时保持运行 |

平台层 `Platform` trait 同名方法（`platform.rs`）为底层实现：
`set_tray`、`set_tray_icon`、`set_tray_menu`、`set_tray_tooltip`、
`set_tray_panel_mode`、`get_tray_icon_bounds`、`on_tray_icon_event`、
`on_tray_menu_action`（均 `platform.rs:458-474`）。

## 最佳实践

- **优先新 API**：`set_tray_menu` + `on_tray_menu_action` 用 id 分流，避免菜单项携带 `Action`。
- **关窗隐藏要 `set_keep_alive_without_windows(true)`**：否则关窗即退出，托盘随之消失。
- **`hide_window` vs `minimize_window`**：前者从任务栏移除（真托盘常驻），后者保留任务栏图标。
- **恢复窗口走 `activate_window`**，并先 `cx.windows()` 查已有窗口，避免重复开窗。
- 图标接受 PNG/ICO 字节，用 `include_bytes!` 内嵌即可。
