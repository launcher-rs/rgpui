//! 「interaction_gallery」示例集合。
//!
//! 用法：`cargo run -p showcase --bin interaction_gallery -- <demo>`
//! 可用 demo：drag_drop / focus_visible / tab_stop / mouse_pressure / move_entity_between_windows / on_window_close_quit / ownership_post / set_menus / native_notification
//! 不带参数时默认运行 `drag_drop`。

#![cfg_attr(target_family = "wasm", no_main)]

#[path = "demos/drag_drop.rs"]
mod drag_drop;
#[path = "demos/focus_visible.rs"]
mod focus_visible;
#[path = "demos/mouse_pressure.rs"]
mod mouse_pressure;
#[path = "demos/move_entity_between_windows.rs"]
mod move_entity_between_windows;
#[path = "demos/native_notification.rs"]
mod native_notification;
#[path = "demos/on_window_close_quit.rs"]
mod on_window_close_quit;
#[path = "demos/ownership_post.rs"]
mod ownership_post;
#[path = "demos/set_menus.rs"]
mod set_menus;
#[path = "demos/tab_stop.rs"]
mod tab_stop;

#[cfg(not(target_family = "wasm"))]
fn main() {
    run();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run();
}

fn run() {
    let name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "drag_drop".to_string());
    match name.as_str() {
        "drag_drop" => drag_drop::run_example(),
        "focus_visible" => focus_visible::run_example(),
        "tab_stop" => tab_stop::run_example(),
        "mouse_pressure" => mouse_pressure::run_example(),
        "move_entity_between_windows" => move_entity_between_windows::run_example(),
        "on_window_close_quit" => on_window_close_quit::run_example(),
        "ownership_post" => ownership_post::run_example(),
        "set_menus" => set_menus::run_example(),
        "native_notification" => native_notification::run_example(),
        _ => eprintln!(
            "未知 demo：{name}。可用：drag_drop / focus_visible / tab_stop / mouse_pressure / move_entity_between_windows / on_window_close_quit / ownership_post / set_menus / native_notification"
        ),
    }
}
