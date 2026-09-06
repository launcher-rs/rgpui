# list_showcase（列表与表格展示）

列表、表格相关示例合集，原为 4 个独立 crate（`list_example`、`uniform_list`、
`data_table`、`tree`），现合并为一个 crate 下的多个 binary。

| binary | 说明 | 运行 |
|--------|------|------|
| `list_example` | 虚拟 `List`：底部对齐 40 项 + 自定义滚动条 | `cargo run -p list_showcase --bin list_example` |
| `uniform_list` | 等高 `uniform_list`：50 项懒加载渲染 | `cargo run -p list_showcase --bin uniform_list` |
| `data_table` | 数据表格：随机数据 + 表头/排序（`rand` 生成） | `cargo run -p list_showcase --bin data_table` |
| `tree` | 深层级嵌套压力测试（`GPUI_TREE_DEPTH` 环境变量调深度，默认 50） | `GPUI_TREE_DEPTH=200 cargo run -p list_showcase --bin tree` |

不带 `--bin` 直接 `cargo run -p list_showcase` 会打印本列表。
