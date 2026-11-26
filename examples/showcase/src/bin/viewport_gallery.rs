//! 「viewport_gallery」示例集合（feature：`viewport`）。
//!
//! 用法：`cargo run -p showcase --bin viewport_gallery --features viewport -- <demo>`
//! 可用 demo：viewport / ruler / canvas_objects；不带参数时默认运行 `viewport`。

#![cfg_attr(target_family = "wasm", no_main)]

#[path = "demos/canvas_objects.rs"]
mod canvas_objects;
#[path = "demos/ruler.rs"]
mod ruler;
#[path = "demos/viewport.rs"]
mod viewport;

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
        .unwrap_or_else(|| "viewport".to_string());
    match name.as_str() {
        "viewport" => viewport::run_example(),
        "ruler" => ruler::run_example(),
        "canvas_objects" => canvas_objects::run_example(),
        _ => eprintln!("未知 demo：{name}。可用：viewport / ruler / canvas_objects"),
    }
}
