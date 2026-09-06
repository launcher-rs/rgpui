# a11y（无障碍支持）

演示 RGPUI 的无障碍 API：给元素树附加结构化信息（AccessKit），让读屏软件等
辅助技术可以程序化地查看和操作界面。

## 运行

```text
cargo run -p a11y
```

Linux 需启用对应后端：

```text
cargo run -p a11y --features rgpui_platform/wayland,rgpui_platform/x11
```
