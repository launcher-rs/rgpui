//! 系统托盘示例（旧 API）：菜单项自带 `Action`
//!
//! `App::set_tray` 传入的 `MenuItem` 自带动作，点击后经 `App::on_app_menu_action` 回到应用：
//! macOS 是全局菜单被选中，Windows 是托盘与跳转列表被点击，Linux 是 dbusmenu 的 `clicked`。
//! 只需要标识符的场景用新 API（`set_tray_menu` + `on_tray_menu_action`），见 `tray_simple`。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{App, MenuItem, Tray, actions};
use rgpui_platform::application;

actions!(tray_menu_action, [Greet, Quit]);

fn main() {
    application().run(|cx: &mut App| {
        // 设置即使没有窗口也保持运行
        cx.set_keep_alive_without_windows(true);

        cx.set_tray(
            Tray {
                tooltip: Some("Legacy Tray API".into()),
                icon: None,
                icon_data: None,
                menu_builder: None,
                visible: true,
            },
            Some(vec![
                MenuItem::action("Greet", Greet),
                MenuItem::separator(),
                MenuItem::action("Quit", Quit),
            ]),
        );

        // 图标可以继续用新 API 单独设置（PNG/ICO 字节）
        cx.set_tray_icon(Some(include_bytes!("../image/app-icon.png")));

        // 菜单项动作回到这里，按类型分流
        cx.on_app_menu_action(|action, cx| {
            if action.as_any().downcast_ref::<Greet>().is_some() {
                eprintln!("Menu action: Greet");
            } else if action.as_any().downcast_ref::<Quit>().is_some() {
                cx.quit();
            }
        });

        eprintln!("Tray should be visible now.");
    });
}
