//! 系统托盘示例
//!
//! 此示例演示如何创建带有上下文菜单的系统托盘图标。
//! 功能包括：
//! - 窗口内按钮：隐藏到托盘、激活窗口、退出（便于验证托盘恢复流程）
//! - 点击关闭按钮最小化到托盘而不是退出
//! - 托盘菜单显示/隐藏窗口
//! - 自定义托盘图标
//!
//! 诊断 Wayland（尤其 KDE Plasma）下的托盘恢复时，可用 `RUST_LOG=debug` 运行，
//! 观察是否收到并使用了主机提供的 xdg-activation 令牌。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Button, ButtonVariants, Context, Render, SharedString, TrayIconEvent,
    TrayMenuItem, Window, WindowBounds, WindowOptions, div, h_flex, prelude::*, px, rgb, size,
    title_bar::TitleBar,
};
use rgpui_platform::application;

struct TrayExample {
    message: SharedString,
}

impl Render for TrayExample {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                // 自绘标题栏：Linux 下窗口选项用 TitleBar::window_options() 请求客户端装饰，
                // 由这里提供可拖动、可双击最大化、带窗口控制按钮的标题栏。
                // 关闭按钮改为隐藏到托盘，与窗口关闭按钮行为一致。
                TitleBar::new()
                    .child("GPUI Tray Example")
                    .on_close_window(|_, window, _| {
                        window.hide_window();
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .flex_1()
                    .justify_center()
                    .items_center()
                    .text_xl()
                    .text_color(rgb(0x505050))
                    .child(div().child(self.message.clone()))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x808080))
                            .child("点击「隐藏到托盘」后，从系统托盘菜单选择「显示窗口」恢复"),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("hide-to-tray")
                                    .label("隐藏到托盘")
                                    .primary()
                                    .on_click(|_, window, _cx| {
                                        window.hide_window();
                                    }),
                            )
                            .child(Button::new("activate-window").label("激活窗口").on_click(
                                |_, window, _cx| {
                                    window.activate_window();
                                },
                            ))
                            .child(Button::new("quit").label("退出").danger().on_click(
                                |_, _, cx| {
                                    cx.quit();
                                },
                            )),
                    ),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        // 设置即使没有窗口也保持运行
        cx.set_keep_alive_without_windows(true);

        // 设置托盘工具提示
        cx.set_tray_tooltip("GPUI Tray Example");

        // 设置自定义托盘图标（使用 PNG 格式）
        let icon_bytes = include_bytes!("image/app-icon.png");
        cx.set_tray_icon(Some(icon_bytes.as_slice()));

        cx.on_tray_icon_event(|event, cx| match event {
            TrayIconEvent::LeftClick => {
                // GNOME/Ubuntu 主机把左键当菜单键：点击时主机会经 dbusmenu 拉取并
                // 弹出菜单。这里绝不能顺手 activate 窗口——Wayland 下激活抢焦点，
                // 会把刚弹出的菜单顶掉。恢复窗口交给菜单「显示窗口」项。
                eprintln!("Tray icon left-clicked");
            }
            TrayIconEvent::RightClick => {
                eprintln!("Tray icon right-clicked");
            }
            TrayIconEvent::DoubleClick => {
                eprintln!("Tray icon double-clicked");
                show_main_window(cx);
            }
        });

        // 设置托盘菜单（使用新的 TrayMenuItem API）
        cx.set_tray_menu(vec![
            TrayMenuItem::Action {
                label: "显示窗口".into(),
                id: "show_window".into(),
            },
            TrayMenuItem::Separator,
            TrayMenuItem::Action {
                label: "退出".into(),
                id: "quit".into(),
            },
        ]);

        // 注册托盘菜单动作回调
        cx.on_tray_menu_action(|id, cx| match id.as_ref() {
            "quit" => {
                cx.quit();
            }
            "show_window" => show_main_window(cx),
            _ => {}
        });

        // 打开主窗口
        create_main_window(cx);

        cx.activate(true);
    });
}

/// 从托盘恢复主窗口：优先激活已存在的窗口，不存在时才新建。
fn show_main_window(cx: &mut App) {
    let existing_windows = cx.windows();
    if let Some(window_handle) = existing_windows.first() {
        let handle = *window_handle;
        cx.update_window(handle, |_, window, _cx| {
            window.activate_window();
        })
        .ok();
    } else {
        create_main_window(cx);
    }
}

fn create_main_window(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(400.), px(360.0)), cx);

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            // Linux 下自动请求客户端装饰，标题栏由 TitleBar 自绘，避免与 WM 原生标题栏重复
            ..TitleBar::window_options()
        },
        |window, cx| {
            let view = cx.new(|_| TrayExample {
                message: "Hello from RGPUI Tray!".into(),
            });

            // 拦截窗口关闭事件，隐藏到托盘而不是退出
            window.on_window_should_close(cx, |window, _cx| {
                // 隐藏窗口（从任务栏移除）
                window.hide_window();
                // 返回 false 阻止窗口关闭
                false
            });

            view
        },
    )
    .ok();
}

fn main() {
    // 诊断时用 RUST_LOG=debug 运行，可看到托盘注册等内部日志
    rgpui::init_logging();
    run_example();
}
