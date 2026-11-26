# webview（原生网页嵌入，`webview` feature 门控）

在 rgpui 窗口中嵌入原生 WebView（Windows WebView2）：加载 URL、加载本地 HTML、
执行 JavaScript。含 Rust 官网 / GitHub / Rust 文档 / 百度快捷入口。

## 运行

```text
cargo run -p webview_example --features webview
```

`wry` / `rgpui-webview` 链需要各平台 SDK（Linux 为 WebKitGTK），已设为可选 feature，
默认 `cargo check --workspace` 不会编译它；CI 用
`cargo check -p webview_example --features webview` 单独覆盖。

注意包名为 `webview_example`（目录名为 `webview`）。
