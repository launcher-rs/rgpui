//! 系统托盘示例
//!
//! 此示例演示如何创建带有上下文菜单的系统托盘图标。
//! 功能包括：
//! - 点击关闭按钮最小化到托盘而不是退出
//! - 托盘菜单显示/隐藏窗口
//! - 自定义托盘图标

#![cfg_attr(target_family = "wasm", no_main)]

use gpui::{
    actions, div, prelude::*, px, rgb, size, App, Bounds, Context,
    Render, SharedString, TrayMenuItem, Window, WindowBounds, WindowOptions,
};
use gpui_platform::application;

actions!(tray, [Quit, ToggleWindow]);

struct TrayExample {
    message: SharedString,
}

impl Render for TrayExample {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .size(px(400.0))
            .justify_center()
            .items_center()
            .text_xl()
            .text_color(rgb(0x505050))
            .child(div().child(self.message.clone()))
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x808080))
                    .child("查看系统托盘以获取菜单"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x808080))
                    .child("点击关闭按钮将最小化到托盘"),
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
            "show_window" => {
                // 尝试恢复已存在的窗口，而不是创建新窗口
                let existing_windows = cx.windows();
                if let Some(window_handle) = existing_windows.first() {
                    // 窗口存在，激活它
                    let handle = *window_handle;
                    cx.update_window(handle, |_, window, _cx| {
                        window.activate_window();
                    }).ok();
                } else {
                    // 窗口不存在，创建新窗口
                    create_main_window(cx);
                }
            }
            _ => {}
        });

        // 打开主窗口
        create_main_window(cx);

        cx.activate(true);
    });
}

fn create_main_window(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(400.), px(300.0)), cx);

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|_| TrayExample {
                message: "Hello from GPUI Tray!".into(),
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
    run_example();
}
