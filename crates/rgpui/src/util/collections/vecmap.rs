/// 提供映射接口但由向量支持的集合。
///
/// 适用于键值对数量不足以克服更复杂算法
/// 开销的小型存储场景。
///
/// 如果满足你的使用场景，[`VecMap`] 应该是 [`std::collections::HashMap`]
/// 或 [`super::HashMap`] 的直接替代品。注意我们按需添加 API，
/// 如果你需要的 API 尚不存在，请添加它！
///
/// 由于使用向量作为底层存储，该映射也按插入顺序迭代，
/// 类似于 [`super::IndexMap`]。
///
/// 此结构体使用结构体数组（SoA）表示，通常具有更好的缓存效率
/// 并在使用简单键或值类型时促进自动向量化。
#[derive(Default)]
pub struct VecMap<K, V> {
    keys: Vec<K>,
    values: Vec<V>,
}

impl<K, V> VecMap<K, V> {
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            values: Vec::new(),
        }
    }

    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            iter: self.keys.iter().zip(self.values.iter()),
        }
    }
}

impl<K: Eq, V> VecMap<K, V> {
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        match self.keys.iter().position(|k| k == &key) {
            Some(index) => Entry::Occupied(OccupiedEntry {
                key: &self.keys[index],
                value: &mut self.values[index],
            }),
            None => Entry::Vacant(VacantEntry { map: self, key }),
        }
    }

    /// 类似于 [`Self::entry`]，但通过引用而非值获取键。
    ///
    /// 当克隆键的开销较大时，此方法很有用，因为我们
    /// 可以等到在该条目下插入值时才克隆键。
    pub fn entry_ref<'a, 'k>(&'a mut self, key: &'k K) -> EntryRef<'k, 'a, K, V> {
        match self.keys.iter().position(|k| k == key) {
            Some(index) => EntryRef::Occupied(OccupiedEntry {
                key: &self.keys[index],
                value: &mut self.values[index],
            }),
            None => EntryRef::Vacant(VacantEntryRef { map: self, key }),
        }
    }
}

pub struct Iter<'a, K, V> {
    iter: std::iter::Zip<std::slice::Iter<'a, K>, std::slice::Iter<'a, V>>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

pub enum Entry<'a, K, V> {
    Occupied(OccupiedEntry<'a, K, V>),
    Vacant(VacantEntry<'a, K, V>),
}

impl<'a, K, V> Entry<'a, K, V> {
    pub fn key(&self) -> &K {
        match self {
            Entry::Occupied(entry) => entry.key,
            Entry::Vacant(entry) => &entry.key,
        }
    }

    pub fn or_insert_with_key<F>(self, default: F) -> &'a mut V
    where
        F: FnOnce(&K) -> V,
    {
        match self {
            Entry::Occupied(entry) => entry.value,
            Entry::Vacant(entry) => {
                entry.map.values.push(default(&entry.key));
                entry.map.keys.push(entry.key);
                match entry.map.values.last_mut() {
                    Some(value) => value,
                    None => unreachable!("vec empty after pushing to it"),
                }
            }
        }
    }

    pub fn or_insert_with<F>(self, default: F) -> &'a mut V
    where
        F: FnOnce() -> V,
    {
        self.or_insert_with_key(|_| default())
    }

    pub fn or_insert(self, value: V) -> &'a mut V {
        self.or_insert_with_key(|_| value)
    }

    pub fn or_insert_default(self) -> &'a mut V
    where
        V: Default,
    {
        self.or_insert_with_key(|_| Default::default())
    }
}

pub struct OccupiedEntry<'a, K, V> {
    key: &'a K,
    value: &'a mut V,
}

pub struct VacantEntry<'a, K, V> {
    map: &'a mut VecMap<K, V>,
    key: K,
}

pub enum EntryRef<'key, 'map, K, V> {
    Occupied(OccupiedEntry<'map, K, V>),
    Vacant(VacantEntryRef<'key, 'map, K, V>),
}

impl<'key, 'map, K, V> EntryRef<'key, 'map, K, V> {
    pub fn key(&self) -> &K {
        match self {
            EntryRef::Occupied(entry) => entry.key,
            EntryRef::Vacant(entry) => entry.key,
        }
    }
}

impl<'key, 'map, K, V> EntryRef<'key, 'map, K, V>
where
    K: Clone,
{
    pub fn or_insert_with_key<F>(self, default: F) -> &'map mut V
    where
        F: FnOnce(&K) -> V,
    {
        match self {
            EntryRef::Occupied(entry) => entry.value,
            EntryRef::Vacant(entry) => {
                entry.map.values.push(default(entry.key));
                entry.map.keys.push(entry.key.clone());
                match entry.map.values.last_mut() {
                    Some(value) => value,
                    None => unreachable!("vec empty after pushing to it"),
                }
            }
        }
    }

    pub fn or_insert_with<F>(self, default: F) -> &'map mut V
    where
        F: FnOnce() -> V,
    {
        self.or_insert_with_key(|_| default())
    }

    pub fn or_insert(self, value: V) -> &'map mut V {
        self.or_insert_with_key(|_| value)
    }

    pub fn or_insert_default(self) -> &'map mut V
    where
        V: Default,
    {
        self.or_insert_with_key(|_| Default::default())
    }
}

pub struct VacantEntryRef<'key, 'map, K, V> {
    map: &'map mut VecMap<K, V>,
    key: &'key K,
}
