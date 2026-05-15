# gpui_shared_string

共享字符串类型，基于 `smol_str` 实现，为 GPUI 任务提供高效的不可变字符串。

## 描述

`gpui_shared_string` 提供了 `SharedString` 类型，这是一个不可变的、可 cheaply clone 的字符串类型，适用于在 GPUI 任务之间共享字符串数据。底层基于 `SmolStr` 实现，对小字符串进行了内联优化。

## 主要功能与类型

### SharedString

- **`new_static(str: &'static str)`** — 从静态字符串创建 `SharedString`（const 函数）
- **`new(str: impl AsRef<str>)`** — 从任意字符串引用创建 `SharedString`
- **`as_str()`** — 获取底层字符串的 `&str` 引用

### 特性实现

- `Deref<Target = str>` — 直接作为 `&str` 使用
- `AsRef<str>` / `Borrow<str>` — 字符串引用转换
- `Default` — 默认值为空字符串
- `Debug` / `Display` — 格式化和显示
- `Eq` / `PartialEq` — 与 `String`、`str`、`&str`、`SharedString` 的比较
- `PartialOrd` / `Ord` / `Hash` — 排序和哈希支持
- `Clone` — 廉价的克隆操作（共享底层数据）
- `Serialize` / `Deserialize` — serde 序列化/反序列化支持
- `JsonSchema` — schemars JSON Schema 生成支持

### From 转换

支持从以下类型转换：
- `&'static str`、`&str`、`&mut str`
- `String`、`&String`
- `Box<str>`
- `Arc<str>`、`&Arc<str>`
- `Cow<'a, str>`
- `&SharedString`

转换为：
- `Arc<str>`
- `String`

## 使用示例

```rust
use gpui_shared_string::SharedString;

// 创建
let s1 = SharedString::new("hello");
let s2 = SharedString::new_static("world");
let s3: SharedString = String::from("dynamic").into();

// 使用
assert_eq!(s1.as_str(), "hello");
assert_eq!(*s1, "hello"); // Deref

// 廉价克隆
let s1_clone = s1.clone(); // 共享底层数据

// 比较
assert_eq!(s1, "hello");
assert_eq!(s1, s1_clone);

// 序列化
let json = serde_json::to_string(&s1).unwrap();
```

## 依赖关系

- `smol_str` (0.3.6) — 小字符串优化类型
- `serde` — 序列化/反序列化
- `schemars` — JSON Schema 生成
