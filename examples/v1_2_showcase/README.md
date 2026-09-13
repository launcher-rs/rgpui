# v1_2_showcase（1.2 新功能演示）

1.2 版本新增功能的演示与手动测试入口（含 #14 提前合入的搜索实战 6 项）。

## 运行

```text
cargo run -p v1_2_showcase --bin dock        # Dock 布局：四区域/标签拖拽/关闭/持久化
cargo run -p v1_2_showcase --bin components  # Upload + Carousel + Mermaid + Sidebar 分组 + B 系列（Select/Combobox/DatePicker/ColorPicker/Avatar/Alert/Breadcrumb/Card/Typography/Pagination/Steps/Timeline/Rate/Toggle/Popconfirm）
cargo run -p v1_2_showcase --bin chat        # Bubble + MessageScroller + Marker
cargo run -p v1_2_showcase --bin editor      # Editor 全家桶：LSP/片段/inlay/粘性顶栏/语言切换 + 搜索面板 + 右键追加档 + 只读预览
cargo run -p v1_2_showcase --bin keymap      # keymap.json 加载应用 + HotkeyInput 录制绑定 + 回显 + 冲突提示
```
