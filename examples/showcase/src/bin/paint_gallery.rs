//! 「paint_gallery」示例集合。
//!
//! 用法：`cargo run -p showcase --bin paint_gallery -- <demo>`
//! 可用 demo：gradient / pattern / painting
//! 不带参数时默认运行 `gradient`。

#![cfg_attr(target_family = "wasm", no_main)]

#[path = "demos/gradient.rs"]
mod gradient;
#[path = "demos/painting.rs"]
mod painting;
#[path = "demos/pattern.rs"]
mod pattern;

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
        .unwrap_or_else(|| "gradient".to_string());
    match name.as_str() {
        "gradient" => gradient::run_example(),
        "pattern" => pattern::run_example(),
        "painting" => painting::run_example(),
        _ => eprintln!("未知 demo：{name}。可用：gradient / pattern / painting"),
    }
}
