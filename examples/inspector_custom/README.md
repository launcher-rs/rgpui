# inspector_custom（元素检查器·自定义面板）

整板替换 + 按类型覆盖的 living 范例，面板为完全自写（不用默认面板）：
自有顶栏/拾取按钮、选中信息卡、单层子节点列表
（点行选中画布对应区域，无选中时展树根）；
`register_inspector_element` 覆盖默认的 Div 布局展示（同 `TypeId`
后注册覆盖先注册），布局卡呈单行紧凑式。

面板开关走 F12（正式 UI 不放调试按钮）；发布剥离同 `inspector`
（默认不开 `inspector` feature，release 自动剥离；
内部工具可用 `--release --features inspector` 保留）。

演示内容（色块/嵌套盒/交互控件三组拾取目标）自带一份，
与兄弟示例 `inspector` 各自独立、无共享代码。

## 运行

```text
cargo run -p inspector_custom
```
