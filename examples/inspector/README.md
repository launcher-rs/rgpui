# inspector（元素检查器）

检查器开箱用法演示：一行 `cx.enable_default_inspector()` 启用默认面板
（含 `Div` 布局展示），`window.toggle_inspector` 开关右侧面板
（工具栏按钮或 F12）。本例再以 `set_inspector_renderer` 整板替换包一层
自定义横幅，内部复用 `rgpui::default_inspector_panel`
（整板替换 recipe 的 living 范例；按状态类型扩展用
`register_inspector_element`）。

默认面板行为：「拾取元素」后悬停高亮、点击选中、滚轮穿透重叠层级；
选中元素显示源码位置、实例号、祖先链树（点击节点选中画布对应区域
并复制路径）与完整树（逐节点折叠，点击行选中）。

## 运行

```text
cargo run -p inspector
```
