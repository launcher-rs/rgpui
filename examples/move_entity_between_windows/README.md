# move_entity_between_windows（实体跨窗口迁移）

演示 `_in` 系列回调 API：实体先在一个窗口注册回调，点击后被迁移到新窗口；
迁移后派发的回调仍正确指向实体**当前**所在窗口，而非注册时的窗口。

## 运行

```text
cargo run -p move_entity_between_windows
```
