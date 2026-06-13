# 全局状态

**目录：** [概述](#概述) · [快速入门](#快速入门) · [常见用例](#常见用例) · [最佳实践](#最佳实践) · [何时使用](#何时使用)

## 概述

rgpui 中的全局状态提供可从任何上下文访问的应用级共享数据。

**关键 Trait**：`Global` - 在类型上实现以使其全局可访问

## 快速入门

### 定义全局状态

```rust
use rgpui::Global;

#[derive(Clone)]
struct AppSettings {
    theme: Theme,
    language: String,
}

impl Global for AppSettings {}
```

### 设置和访问全局变量

```rust
fn main() {
    let app = Application::new();
    app.run(|cx: &mut App| {
        // 设置全局变量
        cx.set_global(AppSettings {
            theme: Theme::Dark,
            language: "en".to_string(),
        });

        // 访问全局变量（只读）
        let settings = cx.global::<AppSettings>();
        println!("Theme: {:?}", settings.theme);
    });
}
```

### 更新全局变量

```rust
impl MyComponent {
    fn change_theme(&mut self, new_theme: Theme, cx: &mut Context<Self>) {
        cx.update_global::<AppSettings, _>(|settings, cx| {
            settings.theme = new_theme;
            // 全局更新不会触发自动通知
            // 手动通知关心的组件
        });

        cx.notify(); // 重新渲染此组件
    }
}
```

## 常见用例

### 1. 应用配置

```rust
#[derive(Clone)]
struct AppConfig {
    api_endpoint: String,
    max_retries: u32,
    timeout: Duration,
}

impl Global for AppConfig {}

// 启动时设置一次
cx.set_global(AppConfig {
    api_endpoint: "https://api.example.com".to_string(),
    max_retries: 3,
    timeout: Duration::from_secs(30),
});

// 在任何地方访问
let config = cx.global::<AppConfig>();
```

### 2. 功能标志

```rust
#[derive(Clone)]
struct FeatureFlags {
    enable_beta_features: bool,
    enable_analytics: bool,
}

impl Global for FeatureFlags {}

impl MyComponent {
    fn render_beta_feature(&self, cx: &App) -> Option<impl IntoElement> {
        let flags = cx.global::<FeatureFlags>();

        if flags.enable_beta_features {
            Some(div().child("Beta 功能"))
        } else {
            None
        }
    }
}
```

### 3. 共享服务

```rust
#[derive(Clone)]
struct ServiceRegistry {
    http_client: Arc<HttpClient>,
    logger: Arc<Logger>,
}

impl Global for ServiceRegistry {}

impl MyComponent {
    fn fetch_data(&mut self, cx: &mut Context<Self>) {
        let registry = cx.global::<ServiceRegistry>();
        let client = registry.http_client.clone();

        cx.spawn(async move |cx| {
            let data = client.get("api/data").await?;
            // 处理数据...
            Ok::<_, anyhow::Error>(())
        }).detach();
    }
}
```

## 最佳实践

### ✅ 使用 Arc 共享资源

```rust
#[derive(Clone)]
struct GlobalState {
    database: Arc<Database>,  // 克隆成本低
    cache: Arc<RwLock<Cache>>,
}

impl Global for GlobalState {}
```

### ✅ 默认不可变

全局变量默认是只读的。需要时使用内部可变性：

```rust
#[derive(Clone)]
struct Counter {
    count: Arc<AtomicUsize>,
}

impl Global for Counter {}

impl Counter {
    fn increment(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }

    fn get(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
}
```

### ❌ 不要：过度使用全局变量

```rust
// ❌ 坏：太多全局变量
cx.set_global(UserState { ... });
cx.set_global(CartState { ... });
cx.set_global(CheckoutState { ... });

// ✅ 好：使用实体管理组件状态
let user_entity = cx.new(|_| UserState { ... });
```

## 何时使用

**使用全局变量：**
- 应用级配置
- 功能标志
- 共享服务（HTTP 客户端、日志记录器）
- 只读参考数据

**使用实体：**
- 组件特定状态
- 频繁变化的状态
- 需要通知的状态
