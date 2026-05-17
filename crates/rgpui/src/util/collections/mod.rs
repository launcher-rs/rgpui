/// 基于 FxHasher 的高性能 HashMap 类型别名。
pub type HashMap<K, V> = FxHashMap<K, V>;
/// 基于 FxHasher 的高性能 HashSet 类型别名。
pub type HashSet<T> = FxHashSet<T>;
/// 基于 FxHasher 的索引有序 IndexMap 类型别名。
pub type IndexMap<K, V> = indexmap::IndexMap<K, V, rustc_hash::FxBuildHasher>;
/// 基于 FxHasher 的索引有序 IndexSet 类型别名。
pub type IndexSet<T> = indexmap::IndexSet<T, rustc_hash::FxBuildHasher>;

pub use indexmap::Equivalent;
pub use rustc_hash::FxHasher;
pub use rustc_hash::{FxHashMap, FxHashSet};
pub use std::collections::*;

pub mod vecmap;
#[cfg(test)]
mod vecmap_tests;
