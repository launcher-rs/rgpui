# http_client

抽象 HTTP 客户端 trait，为 Zed 和 GPUI 提供统一的 HTTP 请求接口。

## 描述

`http_client` crate 定义了一个平台无关的 HTTP 客户端抽象层，通过 `HttpClient` trait 统一了 HTTP 请求的发送、响应处理和代理配置。该 crate 还包含 GitHub API 交互和文件下载的工具函数。

## 主要功能

### `HttpClient` trait

核心 trait，定义了 HTTP 客户端的基本行为：

- **`user_agent()`** — 获取 User-Agent 头部
- **`proxy()`** — 获取代理 URL
- **`send()`** — 发送 HTTP 请求
- **`get()`** — 发送 GET 请求
- **`post_json()`** — 发送 JSON POST 请求

### 装饰器类型

- **`HttpClientWithProxy`**: 添加代理支持的 HTTP 客户端包装器
- **`HttpClientWithUrl`**: 添加基础 URL 支持的 HTTP 客户端包装器，提供便捷方法：
  - `build_url()` — 构建完整 URL
  - `build_zed_api_url()` — 构建 Zed API URL
  - `build_zed_cloud_url()` — 构建 Zed Cloud URL
  - `build_zed_llm_url()` — 构建 Zed LLM URL

### 请求扩展

- **`HttpRequestExt`**: 为请求构建器提供条件修改方法
  - `when()` — 条件修改
  - `when_some()` — 条件解包修改
  - `follow_redirects()` — 设置重定向策略

### 重定向策略

- `NoFollow` — 不跟随重定向
- `FollowLimit(u32)` — 限制重定向次数
- `FollowAll` — 跟随所有重定向

### 测试支持

- **`FakeHttpClient`**: 用于测试的模拟 HTTP 客户端
  - `create()` — 创建自定义处理器的模拟客户端
  - `with_200_response()` — 返回 200 响应的模拟客户端
  - `with_404_response()` — 返回 404 响应的模拟客户端
  - `replace_handler()` — 动态替换处理器

### 其他功能

- **`BlockedHttpClient`**: 阻止所有请求的客户端（安全沙箱）
- **`read_proxy_from_env()`**: 从环境变量读取代理配置
- **`read_no_proxy_from_env()`**: 从环境变量读取不代理列表
- **GitHub 模块**: GitHub API 交互和文件下载工具

## 使用示例

```rust
use http_client::{HttpClient, HttpClientWithUrl, HttpClientWithProxy, AsyncBody};
use std::sync::Arc;

// 创建带基础 URL 的客户端
let client = Arc::new(HttpClientWithUrl::new(
    underlying_client,
    "https://api.example.com",
    Some("http://proxy:8080".to_string()),
));

// 构建 URL
let url = client.build_url("/users/123");

// 发送请求
let response = client.get(&url, AsyncBody::default(), true).await?;

// 发送 JSON POST 请求
let response = client.post_json(
    &client.build_url("/api/data"),
    AsyncBody::from(serde_json::to_vec(&data)?),
).await?;

// 测试中使用模拟客户端
let fake_client = FakeHttpClient::create(|req| async move {
    Ok(Response::builder()
        .status(200)
        .body(AsyncBody::from("OK"))
        .unwrap())
});
```

## 特性标志

- `test-support`: 启用 `FakeHttpClient` 测试支持
