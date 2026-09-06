//! overlay_showcase 启动器：打印所含演示列表。
//!
//! 本 crate 包含多个浮层定位演示 binary，请用 `--bin` 指定运行：
//! - `popover`：deferred 浮动层
//! - `anchor`：9 种锚点定位

fn main() {
    println!("overlay_showcase 包含以下演示（用 --bin 指定）：");
    println!("  cargo run -p overlay_showcase --bin popover  # deferred浮动层");
    println!("  cargo run -p overlay_showcase --bin anchor   # 锚点定位");
}
