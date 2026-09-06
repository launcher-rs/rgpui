//! image_showcase 启动器：打印所含演示列表。
//!
//! 本 crate 包含多个图片相关演示 binary，请用 `--bin` 指定运行：
//! - `image`：本地/远程/资产三种图片来源
//! - `image_loading`：加载态与失败态
//! - `image_gallery`：图片画廊与缓存策略
//! - `gif_viewer`：GIF 动图播放
//! - `svg`：SVG 加载与着色

fn main() {
    println!("image_showcase 包含以下演示（用 --bin 指定）：");
    println!("  cargo run -p image_showcase --bin image          # 本地/远程/资产图片");
    println!("  cargo run -p image_showcase --bin image_loading  # 加载态与失败态");
    println!("  cargo run -p image_showcase --bin image_gallery  # 图片画廊与缓存");
    println!("  cargo run -p image_showcase --bin gif_viewer     # GIF 播放");
    println!("  cargo run -p image_showcase --bin svg            # SVG 着色");
}
