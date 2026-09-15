# inspector（元素检查器）

检查器完整用法演示：`window.toggle_inspector` 开关右侧面板（工具栏按钮或 F12）、
`set_inspector_renderer` 提供面板 UI、`register_inspector_element` 展示
`DivInspectorState`（布局边界与内容尺寸）。面板内「拾取元素」后悬停高亮、
点击选中、滚轮穿透重叠层级，选中元素显示源码位置、实例号与元素树
（`GlobalElementId` 祖先链，HTML 树式缩进展示；超长链自动折叠，
点击节点复制该层完整路径到剪贴板）。

## 运行

```text
cargo run -p inspector
```
