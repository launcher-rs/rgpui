//! text_showcase 启动器：打印所含演示列表。
//!
//! 本 crate 包含多个文本演示 binary，请用 `--bin` 指定运行：
//! - `text`：字体排印上下文与文本样式
//! - `text_layout`：对齐、装饰与高亮
//! - `text_wrapper`：换行、省略与截断

fn main() {
    println!("text_showcase 包含以下演示（用 --bin 指定）：");
    println!("  cargo run -p text_showcase --bin text          # 字体排印上下文与样式");
    println!("  cargo run -p text_showcase --bin text_layout   # 对齐、装饰与高亮");
    println!("  cargo run -p text_showcase --bin text_wrapper  # 换行、省略与截断");
}
