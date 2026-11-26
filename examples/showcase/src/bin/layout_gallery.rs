//! 「layout_gallery」示例集合。
//!
//! 用法：`cargo run -p showcase --bin layout_gallery -- <demo>`
//! 可用 demo：grid_layout / status_bar / dock / chat / tree / paths_bench
//! 不带参数时默认运行 `grid_layout`。

#![cfg_attr(target_family = "wasm", no_main)]

#[path = "demos/chat.rs"]
mod chat;
#[path = "demos/dock.rs"]
mod dock;
#[path = "demos/grid_layout.rs"]
mod grid_layout;
#[path = "demos/paths_bench.rs"]
mod paths_bench;
#[path = "demos/status_bar.rs"]
mod status_bar;
#[path = "demos/tree.rs"]
mod tree;

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
        .unwrap_or_else(|| "grid_layout".to_string());
    match name.as_str() {
        "grid_layout" => grid_layout::run_example(),
        "status_bar" => status_bar::run_example(),
        "dock" => dock::run_example(),
        "chat" => chat::run_example(),
        "tree" => tree::run_example(),
        "paths_bench" => paths_bench::run_example(),
        _ => eprintln!(
            "未知 demo：{name}。可用：grid_layout / status_bar / dock / chat / tree / paths_bench"
        ),
    }
}
