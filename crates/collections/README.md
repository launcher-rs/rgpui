# collections

标准集合类型库，提供 GPUI 和 Zed 项目中常用的数据结构。

## 描述

`collections` 是一个集合类型 crate，重新导出了常用的标准集合类型并统一使用 FxHash 哈希器，同时提供了针对小数据集优化的 `VecMap` 实现。

## 主要功能与类型

### 哈希集合类型

- **`FxHashMap<K, V>`** — 使用 FxHash 的 HashMap，性能优于默认 hasher
- **`FxHashSet<T>`** — 使用 FxHash 的 HashSet
- **`HashMap<K, V>`** — `FxHashMap` 的类型别名
- **`HashSet<T>`** — `FxHashSet` 的类型别名
- **`FxHasher`** — FxHash 哈希器实现

### 有序集合类型

- **`IndexMap<K, V>`** — 保持插入顺序的 HashMap（使用 FxBuildHasher）
- **`IndexSet<T>`** — 保持插入顺序的 HashSet（使用 FxBuildHasher）
- **`Equivalent`** — 等价比较 trait

### 标准库重导出

- **`std::collections::*`** — 标准库集合类型的完整重导出

### VecMap

基于向量的轻量级键值映射，适用于小型数据集。

- **`VecMap<K, V>`** — 使用向量作为底层存储的 map
  - 结构体数组（SoA）布局，缓存友好
  - 按插入顺序迭代（类似 `IndexMap`）
  - 适合数据量较小的场景

#### VecMap API

- `new()` — 创建空 map
- `iter()` — 迭代器
- `entry(key)` — Entry API（按值获取键）
- `entry_ref(&key)` — Entry API（按引用获取键，避免克隆）

#### Entry API

- `or_insert(value)` — 不存在时插入值
- `or_insert_with(f)` — 不存在时通过闭包生成值
- `or_insert_with_key(f)` — 不存在时通过键生成值
- `or_insert_default()` — 不存在时插入默认值

## 使用示例

```rust
use collections::{HashMap, IndexMap, VecMap};

// FxHashMap
let mut map: HashMap<String, i32> = HashMap::default();
map.insert("key".to_string(), 42);

// IndexMap（保持插入顺序）
let mut ordered: IndexMap<&str, i32> = IndexMap::default();
ordered.insert("first", 1);
ordered.insert("second", 2);

// VecMap（适合小数据集）
let mut vec_map = VecMap::new();
vec_map.entry("key").or_insert(42);

// 使用 entry_ref 避免键克隆
let key = String::from("expensive");
vec_map.entry_ref(&key).or_insert(100);
```

## 依赖关系

- `indexmap` — 有序 map/set 实现
- `rustc-hash` — FxHash 哈希器
