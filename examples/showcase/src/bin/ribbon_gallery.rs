//! 「ribbon_gallery」示例集合（feature：`ribbon`）。
//!
//! 用法：`cargo run -p showcase --bin ribbon_gallery --features ribbon -- <demo>`
//! 可用 demo：command_registry / ribbon / qat / mini_toolbar / backstage；不带参数时默认运行 `command_registry`。

#![cfg_attr(target_family = "wasm", no_main)]

#[path = "demos/backstage.rs"]
mod backstage;
#[path = "demos/command_registry.rs"]
mod command_registry;
#[path = "demos/mini_toolbar.rs"]
mod mini_toolbar;
#[path = "demos/qat.rs"]
mod qat;
#[path = "demos/ribbon.rs"]
mod ribbon;

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
        .unwrap_or_else(|| "command_registry".to_string());
    match name.as_str() {
        "command_registry" => command_registry::run_example(),
        "ribbon" => ribbon::run_example(),
        "qat" => qat::run_example(),
        "mini_toolbar" => mini_toolbar::run_example(),
        "backstage" => backstage::run_example(),
        _ => eprintln!(
            "未知 demo：{name}。可用：command_registry / ribbon / qat / mini_toolbar / backstage"
        ),
    }
}
