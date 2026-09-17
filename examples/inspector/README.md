# inspector（元素检查器·默认面板）

检查器开箱用法演示：一行 `cx.enable_default_inspector()` 启用默认面板
（含 `Div` 布局展示），按 F12 开关右侧面板
（正式 UI 不放调试按钮，检查器只走快捷键）。

自定义面板见兄弟示例 `inspector_custom`（全自写面板的 living 范例）；
两示例演示内容各自独立、无共享代码。

默认面板行为：顶栏固定（标题/拾取按钮滚不走）、「拾取元素」后悬停高亮、
点击选中、滚轮穿透重叠层级；选中元素显示源码位置、实例号；
完整树逐节点折叠，点击行选中画布对应区域（展开集只增不重置，浏览进度不塌）。

## 发布剥离

示例默认不开 `inspector` feature：dev 下靠 `debug_assertions` 自动生效；
release 下检查器代码（面板装配 + F12 绑定）自动剥离，零成本。
release 如需保留（如内部工具）：`cargo run -p inspector --release --features inspector`。

默认面板行为：「拾取元素」后悬停高亮、点击选中、滚轮穿透重叠层级；
选中元素显示源码位置、实例号；完整树逐节点折叠，点击行选中画布对应区域
（展开集只增不重置，浏览进度不塌）。

## 运行

```text
cargo run -p inspector
```
