//! 系统托盘简单测试示例
//!
//! 此示例直接参考 adabraka-gpui 的 tray_test.rs，
//! 使用新的 API 创建托盘图标和菜单。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{App, TrayMenuItem};
use rgpui_platform::application;

fn main() {
    application().run(|cx: &mut App| {
        // 设置即使没有窗口也保持运行
        cx.set_keep_alive_without_windows(true);

        // 设置托盘工具提示
        cx.set_tray_tooltip("Test Tray App");

        // 设置托盘菜单
        cx.set_tray_menu(vec![
            TrayMenuItem::Action {
                label: "Hello".into(),
                id: "hello".into(),
            },
            TrayMenuItem::Separator,
            TrayMenuItem::Action {
                label: "Quit".into(),
                id: "quit".into(),
            },
        ]);

        // 注册托盘菜单动作回调
        cx.on_tray_menu_action(|id, cx| {
            eprintln!("Menu action: {}", id);
            if id.as_ref() == "quit" {
                cx.quit();
            }
        });

        eprintln!("Tray should be visible now.");
    });
}
