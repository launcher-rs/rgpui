//! image_showcase：合并示例，用 `-- <demo>` 选择具体演示。
//!
//! 可用 demo：image_loading / image_gallery / gif_viewer / svg；不带参数时默认运行 `image_loading`。

#![cfg_attr(target_family = "wasm", no_main)]

#[path = "bin/demos/gif_viewer.rs"]
mod gif_viewer;
#[path = "bin/demos/image_gallery.rs"]
mod image_gallery;
#[path = "bin/demos/image_loading.rs"]
mod image_loading;
#[path = "bin/demos/svg.rs"]
mod svg;

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
        .unwrap_or_else(|| "image_loading".to_string());
    match name.as_str() {
        "image_loading" => image_loading::run_example(),
        "image_gallery" => image_gallery::run_example(),
        "gif_viewer" => gif_viewer::run_example(),
        "svg" => svg::run_example(),
        _ => eprintln!("未知 demo：{name}。可用：image_loading / image_gallery / gif_viewer / svg"),
    }
}
