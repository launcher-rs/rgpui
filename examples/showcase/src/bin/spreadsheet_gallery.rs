//! 「spreadsheet_gallery」示例集合（feature：`spreadsheet`）。
//!
//! 用法：`cargo run -p showcase --bin spreadsheet_gallery --features spreadsheet -- <demo>`
//! 可用 demo：formula_bar / sheet_grid；不带参数时默认运行 `formula_bar`。

#![cfg_attr(target_family = "wasm", no_main)]

#[path = "demos/formula_bar.rs"]
mod formula_bar;
#[path = "demos/sheet_grid.rs"]
mod sheet_grid;

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
        .unwrap_or_else(|| "formula_bar".to_string());
    match name.as_str() {
        "formula_bar" => formula_bar::run_example(),
        "sheet_grid" => sheet_grid::run_example(),
        _ => eprintln!("未知 demo：{name}。可用：formula_bar / sheet_grid"),
    }
}
