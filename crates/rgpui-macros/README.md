# rgpui_macros

RGPUI 使用的过程宏（procedural macros）集合。

## 简介

`rgpui_macros` 是 [RGPUI](https://github.com/launcher-rs/rgpui) 框架的配套宏库，提供了一系列过程宏，用于简化 RGPUI 应用开发中的常见模式，包括组件渲染、上下文管理、动作定义、样式生成以及测试支持。

## 可用的宏

### 派生宏（Derive Macros）

| 宏 | 说明 |
|---|---|
| `#[derive(Render)]` | 为实现了 `Render`  trait 的类型自动生成渲染逻辑 |
| `#[derive(IntoElement)]` | 将实现了 `RenderOnce` trait 的类型转换为 UI 组件 |
| `#[derive(AppContext)]` | 为持有 `&mut App` 的结构体生成应用上下文，需配合 `#[app]` 属性使用 |
| `#[derive(VisualContext)]` | 为持有 `&mut Window` 并实现了 `AppContext` 的结构体生成视觉上下文，需配合 `#[app]` 和 `#[window]` 属性使用 |
| `#[derive(Action)]` | 为类型自动实现 `Action` trait，支持 `#[action]` 属性 |
| `#[derive_inspector_reflection]` | 为 trait 生成反射模块，用于 inspector 工具（仅在 `inspector` 特性或 debug 模式下可用） |

### 函数宏（Function Macros）

| 宏 | 说明                                     |
|---|----------------------------------------|
| `register_action!` | 手动实现 `Action` trait 时用于向 RGPUI 运行时注册动作 |
| `style_helpers!` | 生成样式辅助函数（内部使用）                         |
| `visibility_style_methods!` | 生成可见性样式方法                              |
| `margin_style_methods!` | 生成外边距样式方法                              |
| `padding_style_methods!` | 生成内边距样式方法                              |
| `position_style_methods!` | 生成定位样式方法                               |
| `overflow_style_methods!` | 生成溢出样式方法                               |
| `cursor_style_methods!` | 生成光标样式方法                               |
| `border_style_methods!` | 生成边框样式方法                               |
| `box_shadow_style_methods!` | 生成盒阴影样式方法                              |

### 测试宏

| 宏                         | 说明                                                              |
|---------------------------|-----------------------------------------------------------------|
| `#[rgpui::test]`          | 用于标注支持 RGPUI 的测试函数，支持同步/异步测试、种子控制、迭代测试等                         |
| `#[rgpui::property_test]` | 支持基于属性的测试（property-based testing），集成 proptest 并支持值收缩（shrinking） |

## 使用示例

### 派生 Render

```rust
#[derive(Render)]
struct MyComponent {
    label: String,
}
```

### 派生 IntoElement

```rust
#[derive(IntoElement)]
struct MyButton {
    text: SharedString,
}
```

### 派生 AppContext

```rust
#[derive(AppContext)]
struct MyContext<'a> {
    #[app]
    app: &'a mut rgpui::App,
}
```

### 派生 VisualContext

```rust
#[derive(VisualContext)]
struct MyVisualContext<'a, 'b> {
    #[app]
    app: &'a mut rgpui::App,
    #[window]
    window: &'b mut rgpui::Window,
}
```

### 派生 Action

```rust
#[derive(Action)]
#[action(name = "my_app.quit")]
struct Quit;
```

### 使用 rgpui::test

```rust
#[rgpui::test]
async fn test_example(mut cx: &TestAppContext) {
    // 测试代码
}

// 指定种子
#[rgpui::test(seed = 42)]
async fn test_with_seed(mut cx: &TestAppContext) {
    // 测试代码
}

// 多次迭代
#[rgpui::test(iterations = 10)]
async fn test_iterations(mut cx: &TestAppContext) {
    // 测试代码
}
```

### 使用 rgpui::property_test

```rust
#[rgpui::property_test]
fn test_arithmetic(x: i32, y: i32) {
    assert!(x == y || x < y || x > y);
}

// 自定义策略
#[rgpui::property_test]
fn int_test(#[strategy = 1..10] x: i32, #[strategy = "[a-zA-Z0-9]{20}"] s: String) {
    assert!(s.len() > (x as usize));
}
```

### 样式宏

```rust
// 生成各种样式方法，通常在 rgpui 内部使用
visibility_style_methods!();
margin_style_methods!();
padding_style_methods!();
```

## 与 rgpui 的关系

`rgpui_macros` 是 `rgpui` crate 的底层依赖，为其提供核心的宏支持。两者关系如下：

- **rgpui**：RGPUI 框架的主体，提供 UI 组件、布局引擎、事件系统等核心功能
- **rgpui_macros**：为 rgpui 提供过程宏支持，包括：
  - 组件渲染和元素转换的派生宏
  - 上下文管理的派生宏
  - 动作系统的派生宏
  - 样式方法生成的函数宏
  - RGPUI 测试框架的测试宏

在项目中使用时，通常只需要依赖 `rgpui` crate，宏会通过 `rgpui` 重新导出。例如：

```toml
[dependencies]
rgpui = "0.1.0"
```

然后在代码中使用：

```rust
use rgpui::*;

#[derive(Render)]
struct MyComponent;

#[rgpui::test]
async fn test_something(cx: &TestAppContext) {
    // ...
}
```

## 特性（Features）

| 特性 | 说明 |
|---|---|
| `inspector` | 启用 inspector 反射功能，用于调试和开发工具 |

## 许可证

Apache-2.0
