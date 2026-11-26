//! text_showcase：合并示例，用 `-- <demo>` 选择具体演示。
//!
//! 可用 demo：text / text_layout / text_wrapper；不带参数时默认运行 `text`。

#![cfg_attr(target_family = "wasm", no_main)]

#[path = "bin/demos/text.rs"]
mod text;
#[path = "bin/demos/text_layout.rs"]
mod text_layout;
#[path = "bin/demos/text_wrapper.rs"]
mod text_wrapper;

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
        .unwrap_or_else(|| "text".to_string());
    match name.as_str() {
        "text" => text::run_example(),
        "text_layout" => text_layout::run_example(),
        "text_wrapper" => text_wrapper::run_example(),
        _ => eprintln!("未知 demo：{name}。可用：text / text_layout / text_wrapper"),
    }
}
