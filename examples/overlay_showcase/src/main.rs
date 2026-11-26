//! overlay_showcase：合并示例，用 `-- <demo>` 选择具体演示。
//!
//! 可用 demo：popover / anchor；不带参数时默认运行 `popover`。

#![cfg_attr(target_family = "wasm", no_main)]

#[path = "bin/demos/anchor.rs"]
mod anchor;
#[path = "bin/demos/popover.rs"]
mod popover;

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
        .unwrap_or_else(|| "popover".to_string());
    match name.as_str() {
        "popover" => popover::run_example(),
        "anchor" => anchor::run_example(),
        _ => eprintln!("未知 demo：{name}。可用：popover / anchor"),
    }
}
