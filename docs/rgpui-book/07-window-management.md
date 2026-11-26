# 窗口管理

> 窗口创建、视图与焦点管理。

## 概述

rgpui 的窗口系统提供跨平台的窗口管理能力：创建与关闭、根视图挂载、焦点管理、
窗口状态（最小化/最大化/隐藏）与系统托盘。平台差异见
[平台契约审计](../linux-api-parity-audit.md)。

## 创建窗口

### 基本创建

通过 `rgpui_platform::application()` 启动，用 `WindowOptions` 配置窗口，闭包里用
`cx.new(...)` 创建根视图实体：

```rust
use rgpui::{
    App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use rgpui_platform::application;

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| MyView::new(cx)),
        );
    });
}
```

旧版的 `Window::default()` / `window.replace_root_view(...)` 写法已不存在。

### 自定义窗口配置

```rust
cx.open_window(
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("My Application".into()),
            ..Default::default()
        }),
        app_id: Some("com.example.myapp".into()),
        window_min_size: Some(size(px(400.0), px(300.0))),
        ..Default::default()
    },
    |_, cx| cx.new(|_| MyView::new(cx)),
);
```

`is_movable` / `is_resizable` / `is_minimizable` 在 Windows / X11 真正生效；Wayland
是尽力而为（协议不提供强制禁用）。详见 `WindowOptions` 的 rustdoc。

## 视图管理

根视图是 `Entity<T>`，由框架持有。业务侧通常持有 `WeakEntity<T>` 或 `Entity<T>`：
通过 `entity.update(cx, |view, cx| ...)` 读写视图状态，`cx.notify()` 触发重绘。

## 焦点管理

```rust
use rgpui::Focusable;

impl Focusable for MyView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MyView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle)
            .child("Click me")
            .on_click(cx.listener(|this, _, _, cx| {
                this.focus_handle.focus(cx);
            }))
    }
}
```

`Focusable::focus_handle` 是必需方法。旧版的 `impl Focusable for MyView {}` 空实现
已不再合法。

## 窗口操作

常用 `Window` 方法（具体行为因平台而异）：

```rust
window.minimize_window();      // 最小化（Wayland 可能不支持）
window.zoom_window();          // 切换最大化
window.activate_window();      // 带到前台
window.hide_window();          // 从任务栏和屏幕隐藏
window.set_title("New Title");
window.set_position(point(px(100.0), px(100.0))); // Wayland 尽力而为
let bounds = window.bounds();
```

注意 `hide_window()` 是窗口级隐藏；`App::hide()` 隐藏整个应用，两者语义不同。
注册关闭拦截用 `window.on_window_should_close(cx, |window, cx| ...)`，返回 `false`
可阻止关闭（例如隐藏到托盘）。退出应用用 `cx.quit()`。

## 系统托盘

```rust
use rgpui::{TrayIcon, TrayMenuItem};

cx.set_tray_icon(Some(icon_bytes));
cx.set_tray_menu(vec![
    TrayMenuItem::action("Show", "show"),
    TrayMenuItem::separator(),
    TrayMenuItem::action("Quit", "quit"),
]);
cx.set_tray_tooltip("My App");
cx.on_tray_menu_action(|id: SharedString, cx: &mut App| {
    match id.as_ref() {
        "show" => { /* 显示窗口 */ }
        "quit" => cx.quit(),
        _ => {}
    }
});
```

平台差异：Ubuntu 的 appindicators 主机在左键单击时弹出菜单、**双击**才产出
`LeftClick`，右键不会产生 `LeftClick`；`set_tray_panel_mode` 在该主机上会被忽略；
`get_tray_icon_bounds` 在 Linux 恒为 `None`。详见技能文档 `tray.md` 与审计清单。

## 多窗口示例

```rust
fn open_window(cx: &mut App, title: &str) {
    let bounds = Bounds::centered(None, size(px(500.0), px(400.0)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |_, cx| cx.new(|_| SecondaryView::new(cx)),
    );
}
```

## 窗口层级

`WindowKind::Overlay` / `WindowKind::LayerShell` 用于浮层与面板类窗口。X11 上
LayerShell 通过 EWMH DOCK + strut 实现，Wayland 走 layer-shell 协议；详见
`examples/layer_shell`。旧的 `create_window_group` / `set_z_index` API 不存在。
