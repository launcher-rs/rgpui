# inspector（元素检查器·默认面板）

检查器开箱用法演示：一行 `cx.enable_default_inspector()` 启用默认面板
（含 `Div` 布局展示），按 F12 开关右侧面板
（正式 UI 不放调试按钮，检查器只走快捷键）。

自定义面板见兄弟示例 `inspector_custom`（全自写面板的 living 范例）；
两示例演示内容各自独立、无共享代码。

默认面板行为：顶栏固定（标题/拾取按钮滚不走）、「拾取元素」后悬停高亮、
点击选中、滚轮穿透重叠层级；选中元素显示源码位置、实例号；
完整树逐节点折叠，点击行选中画布对应区域（展开集只增不重置，浏览进度不塌）。

## 崩溃快照 & AI 导出

示例已开启整条“死后也有据可查”链路（见 `run_example`）：

- `cx.enable_crash_recorder(".rgpui-crash")`：`last.json` 约 2 秒一写、
  原子替换（仅检查器打开的窗口写入），含选中/祖先链/全树/错误环/视口；
- `rgpui::runtime_stats::install_crash_hook(".rgpui-crash")`：
  panic 时写 `panic-*.log`（负载 + 位置 + 强制回溯）。

页面底部“崩溃快照 & AI 导出”区是 AI 调取 GUI 的可点击演示，
即文档“喂给 AI”节的程序化入口：

- 「复制树文本」→ `Window::inspector_tree_text`（Markdown 缩进树）；
- 「复制快照 JSON」→ `Window::capture_inspector_snapshot`（JSON 可序列化）。

两个按钮都要求先按 F12 打开检查器（未打开时 API 返回 `None`，
按钮会提示）；崩溃后则直接读 `.rgpui-crash` 目录，无需开检查器。
完整说明见 `docs/rgpui-book/09-inspector.md`（“崩溃快照”与“喂给 AI”两节）。

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
