# v1_2_showcase（1.2 新功能演示）

1.2 版本新增功能的演示与手动测试入口（含 #14 提前合入的搜索实战 6 项）。

## 运行

```text
cargo run -p v1_2_showcase --bin dock        # Dock 布局：四区域/标签拖拽/关闭/持久化
cargo run -p v1_2_showcase --bin components  # Upload + Carousel + Mermaid + Sidebar 分组
cargo run -p v1_2_showcase --bin chat        # Bubble + MessageScroller + Marker
cargo run -p v1_2_showcase --bin editor      # Editor + tree-sitter 高亮/折叠/大纲 + 行操作 + 多光标 + 只读预览
cargo run -p v1_2_showcase --bin search      # SearchPanelState 嵌入 + 匹配标黄 + 替换
cargo run -p v1_2_showcase --bin context_menu # Input 右键菜单：默认 + 追加自定义 + 完全接管 + 总开关
```
