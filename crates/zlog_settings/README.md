# zlog_settings

将 zlog 日志系统与 GPUI 设置系统集成，支持通过配置文件动态调整日志级别。

## 描述

`zlog_settings` crate 提供了 `ZlogSettings` 结构体，将日志作用域与日志级别的映射集成到 GPUI 的设置系统中。用户可以通过配置文件动态控制不同模块的日志输出级别，无需重启应用。

## 主要功能

### `ZlogSettings` 结构体

- **`scopes`**: 日志作用域到日志级别的映射（`HashMap<String, String>`）

### 设置集成

- 实现 `Settings` trait，从设置内容中解析日志配置
- 自动监听 `SettingsStore` 变化，实时更新日志过滤器

### 配置示例

```json
{
  "log": {
    "client": "warn",
    "project": "debug",
    "agent": "off",
    "editor": "info"
  }
}
```

支持的日志级别：
- `off` / `none` — 关闭日志
- `error` — 仅错误
- `warn` — 警告及以上
- `info` — 信息及以上
- `debug` — 调试及以上
- `trace` — 所有日志

## 使用示例

```rust
use zlog_settings::init;
use gpui::App;

// 在应用启动时初始化
fn setup_app(cx: &mut App) {
    // 初始化设置系统...
    
    // 集成 zlog 设置
    init(cx);
}
```

## 依赖

- `gpui` — GPUI 应用框架
- `settings` — 设置系统
- `collections` — 集合类型
- `zlog` — 日志系统
