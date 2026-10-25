# rgpui_term_basic（终端演示）

`rgpui-term` 的两个可运行入口：同一个视觉骨架（悬浮圆角侧边栏 + 主终端内容区避让 +
透明/模糊窗口背景 + 标题栏多会话标签），两种字节来源。

## 运行

```text
# 真实 shell：本地 PTY，含内容搜索条（字面量/正则切换、上下一处）、OSC 8 超链接、IME
cargo run -p rgpui_term_basic --bin basic

# 外接 IO：无本地 PTY，模拟远端注入字节并把键入/缩放回吐给「远端」
cargo run -p rgpui_term_basic --bin external_io
```

- `--bin basic` 走 `TerminalBuilder::new`（起真实 shell），搜索条默认按**字面量**匹配，
  `Alt+R` 切字面/正则，`Esc` 关闭；`Ctrl`/`Cmd` + 单击带 OSC 8 链接的文字会派发
  `Event::HyperlinkClick`。
- `--bin external_io` 走 `TerminalBuilder::new_external` + `take_external_handles()`，
  演示 SSH/串口这类「字节来自别处」的接入方式：`FeedHandle::feed` 下行注入，
  `PtyBoundReceiver::recv` 上行 `PtyBoundMessage::{Input, Resize}`；内置的模拟远端
  应答 `help` / `date` / `echo` / `exit`。
