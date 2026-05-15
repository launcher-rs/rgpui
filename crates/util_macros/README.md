# util_macros

实用过程宏集合，用于处理跨平台测试中的路径、URI和行尾转换，以及性能基准测试标记。

## 描述

`util_macros` 提供了一组在编译时根据目标平台自动转换字符串的过程宏，主要用于编写跨平台测试用例。同时包含 `#[perf]` 属性宏，用于标记性能敏感的测试。

## 主要功能

### `#[line_endings]` 宏

根据目标平台自动转换换行符。在 Windows 上将 `\n` 替换为 `\r\n`，其他平台保持不变。

### `#[path]` 宏

根据目标平台自动转换路径字符串。在 Windows 上将 `/` 替换为 `\\`，并为绝对路径添加 `C:` 前缀。

### `#[uri]` 宏

根据目标平台自动转换文件 URI。在 Windows 上将 `file:///` 替换为 `file:///C:/`。

### `#[perf]` 属性宏

标记测试为性能敏感型，自动应用 `#[test]` 属性，并支持配置迭代次数、权重和重要性级别。

## 使用示例

```rust
use util_macros::{path, uri, line_endings, perf};

// 跨平台路径
let path = path!("/Users/user/file.txt");
// Windows: "C:\\Users\\user\\file.txt"
// 其他平台: "/Users/user/file.txt"

// 跨平台 URI
let uri = uri!("file:///path/to/file");
// Windows: "file:///C:/path/to/file"
// 其他平台: "file:///path/to/file"

// 跨平台行尾
let text = line_endings!("Hello\nWorld");
// Windows: "Hello\r\nWorld"
// 其他平台: "Hello\nWorld"

// 性能测试标记
#[perf]
fn generic_test() {
    // 测试代码
}

#[perf(critical, weight = 80)]
fn important_test() {
    // 关键性能测试
}

#[perf(fluff, weight = 30)]
fn cold_path_test() {
    // 边缘路径测试
}
```

## 特性标志

- `perf-enabled`: 启用性能测量功能
