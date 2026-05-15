# refineable

提供 `Refineable` trait 和 `Cascade` 类型，用于实现分层配置和部分更新模式。

## 描述

`refineable` crate 提供了一种"精细化"（refinement）类型系统，允许对复杂结构体进行部分初始化和部分更新。这种模式特别适用于样式系统、主题层级和配置管理等场景，类似于 CSS 的级联机制。

## 主要功能

### `Refineable` trait

核心 trait，定义了精细化类型的行为：

- **`refine()`**: 将精细化配置应用到实例上，仅更新非空值
- **`refined()`**: 返回应用了精细化配置的新实例
- **`from_cascade()`**: 从级联配置中创建实例
- **`is_superset_of()`**: 检查实例是否包含精细化配置中的所有值
- **`subtract()`**: 计算两个实例之间的差异

### `Cascade<S>` 类型

维护一个按优先级排序的精细化配置序列，后面的条目优先于前面的条目。支持：

- **`reserve()`**: 预留新的配置槽位
- **`base()`**: 获取基础配置的引用
- **`set()`**: 设置特定槽位的配置
- **`merged()`**: 合并所有配置为单一精细化对象

### `IsEmpty` trait

判断精细化配置是否为空（应用后不会产生任何效果）。

## 使用示例

```rust
use refineable::{Refineable, Cascade};

// 定义可精细化的结构体
#[derive(Refineable, Clone, Default)]
#[refineable(Debug, Serialize)]
struct Theme {
    #[refineable]
    colors: ColorScheme,
    font_size: f32,
    line_height: f32,
}

#[derive(Refineable, Clone, Default)]
struct ColorScheme {
    background: String,
    foreground: String,
    accent: Option<String>,
}

// 创建基础配置
let base = Theme::default();

// 创建精细化配置
let refinement = ThemeRefinement {
    colors: ColorSchemeRefinement {
        background: Some("dark".to_string()),
        foreground: None,
        accent: Some("blue".to_string()),
    },
    font_size: Some(14.0),
    line_height: None,
};

// 应用精细化配置
let refined = base.refined(refinement);

// 使用级联配置
let mut cascade = Cascade::<Theme>::default();
let slot = cascade.reserve();
cascade.set(slot, Some(ThemeRefinement {
    font_size: Some(16.0),
    ..Default::default()
}));

// 从级联创建实例
let theme = Theme::from_cascade(&cascade);
```

## 派生宏属性

- `#[refineable(Debug)]`: 为精细化类型实现 `Debug`
- `#[refineable(Serialize)]`: 派生 `Serialize`，跳过序列化 `None` 值
- `#[refineable(OtherTrait)]`: 派生其他 trait

字段属性：
- `#[refineable]`: 标记字段本身可精细化（使用嵌套的精细化类型）
