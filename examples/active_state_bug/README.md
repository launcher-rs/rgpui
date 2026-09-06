# active_state_bug（已知问题复现）

点击态复现用例：点击按钮后，`.active()` 背景大约每隔一次点击会粘滞不消失。

这是一个待修复的已知问题，不是功能演示。人工验证修复后即可删除本示例。

## 运行

```text
cargo run -p active_state_bug
```

点击窗口中的 "Click me" 按钮，观察按下态背景是否残留。
