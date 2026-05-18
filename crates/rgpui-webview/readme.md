# rgpui-webview

基于 [wry](https://github.com/tauri-apps/wry) 的 WebView 组件，用于在 rgpui 应用中嵌入网页内容。

本项目移植自 [longbridge/gpui-component/webview](https://github.com/longbridge/gpui-component/tree/main/crates/webview)。

## 功能特性

- 在 rgpui 应用中嵌入 WebView
- 支持加载 URL 和历史记录导航
- 自动处理布局和鼠标事件
- 支持显示/隐藏控制

## 安装

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
rgpui-webview = { path = "../rgpui-webview" }
```

### 可选功能

| 功能 | 说明 |
|------|------|
| `inspector` | 启用开发者工具（DevTools） |

## 快速开始

```rust
use rgpui_webview::WebView;

// 创建 WebView 并加载 URL
let webview = WebView::new(wry_webview, window, cx);
webview.load_url("https://example.com");
```

## API 参考

### `WebView`

| 方法 | 说明 |
|------|------|
| `new(webview, window, cx)` | 从 wry WebView 创建新的 WebView 组件 |
| `show()` | 显示 WebView |
| `hide()` | 隐藏 WebView |
| `visible()` | 获取可见状态 |
| `bounds()` | 获取当前边界 |
| `back()` | 返回上一页 |
| `load_url(url)` | 加载指定 URL |
| `raw()` | 获取底层 wry WebView 引用 |

## 依赖

- `rgpui` - 核心 UI 框架
- `lb-wry` (wry 0.53.3) - WebView 引擎
- `anyhow` - 错误处理

## 许可证

Apache-2.0
