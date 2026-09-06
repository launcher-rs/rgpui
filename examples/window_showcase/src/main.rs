//! window_showcase 启动器：打印所含演示列表。
//!
//! 本 crate 包含多个窗口管理演示 binary，请用 `--bin` 指定运行：
//! - `window`：窗口类型与 prompt
//! - `window_movable`：可移动开关组合
//! - `window_positioning`：窗口定位
//! - `window_shadow`：窗口阴影
//! - `transparent`：透明背景窗口
//! - `opacity`：窗口不透明度
//! - `shadow`：阴影样式大全

fn main() {
    println!("window_showcase 包含以下演示（用 --bin 指定）：");
    println!("  cargo run -p window_showcase --bin window              # 窗口类型与prompt");
    println!("  cargo run -p window_showcase --bin window_movable      # 可移动开关组合");
    println!("  cargo run -p window_showcase --bin window_positioning  # 窗口定位");
    println!("  cargo run -p window_showcase --bin window_shadow       # 窗口阴影");
    println!("  cargo run -p window_showcase --bin transparent         # 透明背景窗口");
    println!("  cargo run -p window_showcase --bin opacity             # 窗口不透明度");
    println!("  cargo run -p window_showcase --bin shadow              # 阴影样式大全");
}
