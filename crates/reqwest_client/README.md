# reqwest_client

基于 reqwest 的 HTTP 客户端实现，实现 `http_client::HttpClient` trait。

## 描述

`reqwest_client` crate 提供了 `HttpClient` trait 的具体实现，底层使用 `reqwest` 库。它集成了 rustls TLS 配置、代理支持和自定义 User-Agent，并内置了独立的 tokio 运行时。

## 主要功能

### `ReqwestClient` 结构体

实现了 `http_client::HttpClient` trait，提供：

- **`new()`** — 创建默认 HTTP 客户端
- **`user_agent()`** — 创建带自定义 User-Agent 的客户端
- **`proxy_and_user_agent()`** — 创建带代理和 User-Agent 的客户端

### TLS 配置

- 默认使用 rustls TLS（通过 `http_client_tls`）
- 连接超时设置为 10 秒

### 代理支持

支持多种代理协议：
- HTTP/HTTPS 代理
- SOCKS4/SOCKS4A/SOCKS5/SOCKS5H 代理

自动遵守 `NO_PROXY` 环境变量。

### 内置运行时

通过 `runtime()` 函数提供独立的 tokio 多线程运行时（1 个工作线程），在没有现有 tokio 运行时的情況下自动创建。

### 安全特性

- 自动脱敏错误日志中的 API 密钥（`key=REDACTED`）
- 支持可配置的重定向策略

### 流式响应

内部实现了 `StreamReader`，将异步读取器转换为字节流，支持高效的流式响应处理。

## 使用示例

```rust
use reqwest_client::ReqwestClient;
use http_client::HttpClient;
use http_client::Url;

// 创建默认客户端
let client = ReqwestClient::new();

// 创建带 User-Agent 的客户端
let client = ReqwestClient::user_agent("MyApp/1.0")?;

// 创建带代理的客户端
let proxy = Url::parse("http://proxy:8080")?;
let client = ReqwestClient::proxy_and_user_agent(
    Some(proxy),
    "MyApp/1.0"
)?;

// 作为 HttpClient 使用
let response = client.send(request).await?;
```

## 依赖

- `reqwest` — HTTP 客户端库
- `http_client` — HTTP 客户端 trait 定义
- `http_client_tls` — TLS 配置（非 wasm 平台）
- `tokio` — 异步运行时
- `bytes` — 字节缓冲区
- `futures` — 异步流处理
- `regex` — URL 脱敏正则
- `gpui_util` — 工具函数

## 特性标志

- `test-support`: 启用测试支持
