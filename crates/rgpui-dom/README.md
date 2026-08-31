# rgpui-dom

Web 平台 DOM 覆盖层后端，为 Canvas 渲染提供原生 DOM 能力（文本选择、复制等）。

## 功能

- DOM 树增量对账（`reconcile`）
- 支持 `div` 和文本节点的 DOM 化
- 通过 `data-gpui-id` 属性与 gpui 元素系统关联
- 事件委托（点击、滚动事件通过 DOM key 链反查）

## 启用方式

在 `Cargo.toml` 中启用 `dom-backend` feature：

```toml
rgpui = { version = "1.0.0", features = ["dom-backend"] }
```

应用启动前调用：

```rust
rgpui::set_dom_layer_enabled(true);
```

## 用法详情

详见 [Web DOM 后端用法文档](../../docs/web-dom-backend-usage.md)。
