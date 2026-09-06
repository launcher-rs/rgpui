# ownership_post（实体订阅与事件）

无窗口的逻辑演示（控制台断言）：`Entity` 订阅（`cx.subscribe`）、事件派发
（`EventEmitter` + `cx.emit`）、`update` + `notify` 更新链。适合理解实体
所有权与消息传递，不打开任何窗口。

## 运行

```text
cargo run -p ownership_post
```
