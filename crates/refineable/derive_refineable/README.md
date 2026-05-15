# derive_refineable

为结构体派生 `Refineable` trait 的过程宏 crate。

## 描述

`derive_refineable` 是 `refineable` crate 的配套过程宏库，提供 `#[derive(Refineable)]` 派生宏。它会自动为结构体生成对应的精细化类型（`*Refinement`），并实现 `Refineable`、`IsEmpty`、`Default`、`Clone` 和 `From` 等 trait。

## 主要功能

### 自动生成的精细化类型

对于结构体 `Foo`，宏会生成 `FooRefinement` 类型，其中：

- **标记 `#[refineable]` 的字段**: 转换为对应的精细化类型（如 `Bar` → `BarRefinement`）
- **`Option<T>` 字段**: 保持为 `Option<T>`
- **普通字段**: 转换为 `Option<T>`

### 自动实现的 trait

- `Refineable` — 核心精细化 trait
- `IsEmpty` — 判断配置是否为空
- `Default` — 默认值（所有字段为 `None`）
- `Clone` — 克隆支持
- `From<FooRefinement> for Foo` — 从精细化类型转换回原类型

### 支持的派生属性

在结构体上：
- `#[refineable(Debug)]` — 实现 `Debug`，仅显示非 `None` 字段
- `#[refineable(Serialize)]` — 派生 `Serialize`，跳过空值
- `#[refineable(OtherTrait)]` — 派生其他 trait

在字段上：
- `#[refineable]` — 标记字段为可精细化类型

## 使用示例

```rust
use refineable::Refineable;

#[derive(Refineable, Clone, Default)]
#[refineable(Debug, Serialize)]
struct Style {
    #[refineable]
    border: BorderStyle,
    padding: f32,
    margin: Option<f32>,
}

#[derive(Refineable, Clone, Default)]
struct BorderStyle {
    width: f32,
    color: String,
    radius: Option<f32>,
}

// 宏会自动生成 StyleRefinement 和 BorderStyleRefinement 类型
// 并实现所有必要的 trait

let style = Style::default();
let refinement = StyleRefinement {
    border: BorderStyleRefinement {
        width: Some(2.0),
        color: Some("black".to_string()),
        radius: None,
    },
    padding: Some(10.0),
    margin: Some(5.0),
};

let refined_style = style.refined(refinement);
```

## 依赖

- `proc-macro2`
- `quote`
- `syn`
