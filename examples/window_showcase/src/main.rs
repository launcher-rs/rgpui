//! window_showcase：合并示例，用 `-- <demo>` 选择具体演示。
//!
//! 可用 demo：window_movable / window_positioning / window_shadow / transparent / opacity / shadow；不带参数时默认运行 `window_movable`。

#![cfg_attr(target_family = "wasm", no_main)]

#[path = "bin/demos/opacity.rs"]
mod opacity;
#[path = "bin/demos/shadow.rs"]
mod shadow;
#[path = "bin/demos/transparent.rs"]
mod transparent;
#[path = "bin/demos/window_movable.rs"]
mod window_movable;
#[path = "bin/demos/window_positioning.rs"]
mod window_positioning;
#[path = "bin/demos/window_shadow.rs"]
mod window_shadow;

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
        .unwrap_or_else(|| "window_movable".to_string());
    match name.as_str() {
        "window_movable" => window_movable::run_example(),
        "window_positioning" => window_positioning::run_example(),
        "window_shadow" => window_shadow::run_example(),
        "transparent" => transparent::run_example(),
        "opacity" => opacity::run_example(),
        "shadow" => shadow::run_example(),
        _ => eprintln!(
            "未知 demo：{name}。可用：window_movable / window_positioning / window_shadow / transparent / opacity / shadow"
        ),
    }
}
