# http_client_tls

基于 rustls 的 TLS 配置管理。

## 描述

`http_client_tls` 提供了一个全局单例的 rustls TLS 客户端配置，使用平台原生证书验证器，确保安全的 HTTPS 连接。

## 主要功能

### `tls_config()` 函数

返回一个全局共享的 `rustls::ClientConfig` 实例，具有以下特性：

- 使用 `aws_lc_rs` 作为默认加密提供者
- 使用 `rustls_platform_verifier` 进行平台原生证书验证
- 通过 `OnceLock` 实现懒加载和线程安全的单例模式

## 使用示例

```rust
use http_client_tls::tls_config;
use reqwest::ClientBuilder;

// 在 reqwest 中使用预配置的 TLS
let client = ClientBuilder::new()
    .use_preconfigured_tls(tls_config())
    .build()?;

// 或直接获取配置进行检查
let config = tls_config();
```

## 依赖

- `rustls` — TLS 实现
- `rustls_platform_verifier` — 平台原生证书验证
