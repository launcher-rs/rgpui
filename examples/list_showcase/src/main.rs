//! list_showcase 启动器：打印所含演示列表。
//!
//! 本 crate 包含多个列表/表格演示 binary，请用 `--bin` 指定运行：
//! - `list_example`：虚拟列表与滚动条
//! - `uniform_list`：等高列表懒加载
//! - `data_table`：数据表格
//! - `tree`：深层级压力测试

fn main() {
    println!("list_showcase 包含以下演示（用 --bin 指定）：");
    println!("  cargo run -p list_showcase --bin list_example  # 虚拟列表与滚动条");
    println!("  cargo run -p list_showcase --bin uniform_list  # 等高列表懒加载");
    println!("  cargo run -p list_showcase --bin data_table    # 数据表格");
    println!("  cargo run -p list_showcase --bin tree          # 深层级压力测试");
}
