# sum_tree

Sum Tree 数据结构实现，一种并发友好的 B+ 树变体。

## 描述

`sum_tree` 实现了一种特殊的 B+ 树数据结构，其中每个节点维护其子树中所有条目的摘要（Summary）。这种设计使得可以高效地按任意维度在树中定位和切片数据，非常适合文本编辑器中的 gap buffer、区间树等场景。

## 主要功能与类型

### 核心 trait

- **`Item`** — 可存储在 SumTree 中的条目 trait
  - 关联类型 `Summary` 定义条目的摘要类型
  - `summary()` 方法计算条目的摘要

- **`KeyedItem`** — 具有可排序键的条目 trait
  - 关联类型 `Key` 定义键类型
  - `key()` 方法获取条目的键

- **`Summary`** — 子树摘要 trait
  - `zero()` — 零值/单位元
  - `add_summary()` — 累加摘要

- **`ContextLessSummary`** — 无需上下文的摘要 trait（自动实现 `Summary`）

- **`Dimension<'a, S>`** — 维度 trait，用于在树中导航
  - 一个 Summary 可以有多个 Dimension（如文本的行数、字符数、字节数）

- **`SeekTarget`** — 搜索目标 trait，用于定位树中的位置

### SumTree

- **`SumTree<T: Item>`** — 核心的 B+ 树结构
  - 基于 `Arc` 的共享所有权，支持高效克隆
  - 最大每节点条目数：`TREE_BASE * 2`（生产环境为 12）

#### 构造方法

- `new(cx)` — 创建空树
- `from_item(item, cx)` — 从单个条目创建
- `from_iter(iter, cx)` — 从迭代器构建
- `from_par_iter(iter, cx)` — 从并行迭代器构建（使用 Rayon）

#### 查询方法

- `find()` / `find_exact()` — 高效查找条目
- `find_with_prev()` — 查找并返回前一个条目
- `first()` / `last()` — 获取首/末条目
- `is_empty()` — 检查是否为空
- `summary()` — 获取整棵树的摘要
- `extent<D>()` — 获取指定维度的范围

#### 修改方法

- `push(item, cx)` — 追加单个条目
- `extend(iter, cx)` — 追加多个条目
- `par_extend(iter, cx)` — 并行追加
- `append(other, cx)` — 合并另一棵树
- `insert_or_replace(item, cx)` — 插入或替换（KeyedItem）
- `remove(key, cx)` — 删除条目（KeyedItem）
- `edit(edits, cx)` — 批量编辑（KeyedItem）
- `update_first(f, cx)` / `update_last(f, cx)` — 更新首/末条目

#### 游标与迭代

- `cursor<D>(cx)` — 创建游标用于导航和切片
- `filter<F, U>(cx, filter)` — 创建过滤游标
- `iter()` — 标准迭代器

### Cursor

- **`Cursor<'a, 'b, T, D>`** — 树导航游标
  - `seek(target, bias)` — 定位到目标位置
  - `seek_forward(target, bias)` — 从当前位置向前搜索
  - `next()` / `prev()` — 前进/后退
  - `search_forward(filter)` / `search_backward(filter)` — 带过滤的搜索
  - `slice(end, bias)` — 切片并返回新树
  - `suffix()` — 获取剩余部分的树
  - `summary(end, bias)` — 计算范围内的摘要
  - `item()` / `item_summary()` — 获取当前条目/摘要
  - `start()` / `end()` — 当前位置的起止点

### FilterCursor

- **`FilterCursor`** — 支持节点过滤的游标，可跳过不满足条件的子树

### TreeMap 和 TreeSet

- **`TreeMap<K, V>`** — 基于 SumTree 的有序映射
  - `insert()` / `insert_or_replace()` — 插入/更新
  - `get()` / `contains_key()` — 查询
  - `remove()` / `remove_range()` — 删除
  - `closest(key)` — 查找最接近的键
  - `iter_from(key)` — 从指定键开始迭代
  - `update(key, f)` — 更新值
  - `retain(predicate)` — 保留满足条件的条目

- **`TreeSet<K>`** — 基于 TreeMap 的有序集合

### 其他类型

- **`Bias`** — 定位偏差（`Left` / `Right`），用于处理边界情况
- **`Dimensions<D1, D2, D3>`** — 组合多个维度
- **`NoSummary`** — 空摘要类型
- **`Edit<T>`** — 编辑操作（`Insert` / `Remove`）

## 使用示例

```rust
use sum_tree::{SumTree, Item, Summary, ContextLessSummary, Cursor, Bias};

// 定义简单的整数条目
impl Item for u32 {
    type Summary = u32;
    
    fn summary(&self, _: ()) -> Self::Summary {
        *self
    }
}

// 创建树
let mut tree = SumTree::default();
tree.extend(1..10, ());

// 查询
assert_eq!(tree.summary(), &45); // 1+2+...+9

// 使用游标切片
let mut cursor = tree.cursor::<u32>(());
let left = cursor.slice(&6, Bias::Left); // 1+2+3 = 6
let right = cursor.suffix(); // 剩余部分

// TreeMap
use sum_tree::TreeMap;
let mut map = TreeMap::default();
map.insert("a", 1);
map.insert("b", 2);
assert_eq!(map.get(&"a"), Some(&1));
```

## 依赖关系

- `heapless` — 无堆分配的固定大小容器
- `rayon` — 并行迭代支持
- `log` — 日志记录
- `ztracing` — 性能追踪
- `tracing` — 追踪基础设施

### 开发依赖

- `proptest` — 属性测试（启用 `test-support` 特性）
- `rand` — 随机数生成
- `ctor` — 测试初始化
