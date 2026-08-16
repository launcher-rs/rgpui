use instant::{Duration, Instant};
use std::fmt::Debug;

/// 历史记录条目 - 表示历史中的一次变更。
/// 必须实现 Clone 和 PartialEq 才能在 History 中使用。
pub trait HistoryItem: Clone + PartialEq {
    /// 获取版本号。
    fn version(&self) -> usize;
    /// 设置版本号。
    fn set_version(&mut self, version: usize);
}

/// 历史记录 - 用于跟踪模型的变更并支持撤销和重做操作。
///
/// 目前用于 Input 的撤销/重做操作。也可以在自己的模型中使用，
/// 例如跟踪标签页的前进/后退历史。
///
/// ## 使用场景
///
/// - Input 中的撤销/重做操作
/// - 跟踪标签页历史的前进/后退功能
#[derive(Debug)]
pub struct History<I: HistoryItem> {
    undos: Vec<I>,
    redos: Vec<I>,
    last_changed_at: Instant,
    version: usize,
    pub(crate) ignore: bool,
    max_undos: usize,
    group_interval: Option<Duration>,
    grouping: bool,
    unique: bool,
}

impl<I> History<I>
where
    I: HistoryItem,
{
    /// 创建新的历史记录。
    pub fn new() -> Self {
        Self {
            undos: Default::default(),
            redos: Default::default(),
            ignore: false,
            last_changed_at: Instant::now(),
            version: 0,
            max_undos: 1000,
            group_interval: None,
            grouping: false,
            unique: false,
        }
    }

    /// 设置保留的最大撤销步数，默认为 1000。
    pub fn max_undos(mut self, max_undos: usize) -> Self {
        self.max_undos = max_undos;
        self
    }

    /// 设置历史记录为唯一，默认为 false。
    /// 设置为 true 时，历史记录只保留唯一的变更。
    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }

    /// 设置变更分组的间隔毫秒数，默认为 None。
    pub fn group_interval(mut self, group_interval: Duration) -> Self {
        self.group_interval = Some(group_interval);
        self
    }

    /// 开始分组变更，这将阻止版本号递增直到调用 `end_grouping`。
    pub fn start_grouping(&mut self) {
        self.grouping = true;
    }

    /// 结束分组变更，允许版本号再次递增。
    pub fn end_grouping(&mut self) {
        self.grouping = false;
    }

    /// 如果上次变更距离现在超过分组间隔，则递增版本号。
    fn inc_version(&mut self) -> usize {
        let t = Instant::now();
        if !self.grouping && Some(self.last_changed_at.elapsed()) > self.group_interval {
            self.version += 1;
        }

        self.last_changed_at = t;
        self.version
    }

    /// 获取当前版本号。
    pub fn version(&self) -> usize {
        self.version
    }

    /// 向历史记录推入一条新变更。
    pub fn push(&mut self, item: I) {
        let version = self.inc_version();

        if self.undos.len() >= self.max_undos {
            self.undos.remove(0);
        }

        if self.unique {
            self.undos.retain(|c| *c != item);
            self.redos.retain(|c| *c != item);
        }

        let mut item = item;
        item.set_version(version);
        self.undos.push(item);
    }

    /// 获取撤销栈。
    pub fn undos(&self) -> &Vec<I> {
        &self.undos
    }

    /// 获取重做栈。
    pub fn redos(&self) -> &Vec<I> {
        &self.redos
    }

    /// 清空撤销和重做栈。
    pub fn clear(&mut self) {
        self.undos.clear();
        self.redos.clear();
    }

    /// 撤销最后一次变更并返回被撤销的变更列表。
    pub fn undo(&mut self) -> Option<Vec<I>> {
        if let Some(first_change) = self.undos.pop() {
            let mut changes = vec![first_change.clone()];
            // 挑选所有版本号相同的后续变更
            while self
                .undos
                .iter()
                .filter(|c| c.version() == first_change.version())
                .count()
                > 0
            {
                let change = self.undos.pop().unwrap();
                changes.push(change);
            }

            self.redos.extend(changes.clone());
            Some(changes)
        } else {
            None
        }
    }

    /// 重做最后一次被撤销的变更并返回被重做的变更列表。
    pub fn redo(&mut self) -> Option<Vec<I>> {
        if let Some(first_change) = self.redos.pop() {
            let mut changes = vec![first_change.clone()];
            // 挑选所有版本号相同的后续变更
            while self
                .redos
                .iter()
                .filter(|c| c.version() == first_change.version())
                .count()
                > 0
            {
                let change = self.redos.pop().unwrap();
                changes.push(change);
            }
            self.undos.extend(changes.clone());
            Some(changes)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct TabIndex {
        tab_index: usize,
        version: usize,
    }

    impl PartialEq for TabIndex {
        fn eq(&self, other: &Self) -> bool {
            self.tab_index == other.tab_index
        }
    }

    impl From<usize> for TabIndex {
        fn from(value: usize) -> Self {
            TabIndex {
                tab_index: value,
                version: 0,
            }
        }
    }

    impl HistoryItem for TabIndex {
        fn version(&self) -> usize {
            self.version
        }
        fn set_version(&mut self, version: usize) {
            self.version = version;
        }
    }

    #[test]
    fn test_history() {
        let mut history: History<TabIndex> = History::new().max_undos(100);
        history.push(0.into());
        history.push(3.into());
        history.push(2.into());
        history.push(1.into());

        assert_eq!(history.version(), 4);
        let changes = history.undo().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].tab_index, 1);

        let changes = history.undo().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].tab_index, 2);

        history.push(5.into());

        let changes = history.redo().unwrap();
        assert_eq!(changes[0].tab_index, 2);

        let changes = history.redo().unwrap();
        assert_eq!(changes[0].tab_index, 1);

        let changes = history.undo().unwrap();
        assert_eq!(changes[0].tab_index, 1);

        let changes = history.undo().unwrap();
        assert_eq!(changes[0].tab_index, 2);

        let changes = history.undo().unwrap();
        assert_eq!(changes[0].tab_index, 5);

        let changes = history.undo().unwrap();
        assert_eq!(changes[0].tab_index, 3);

        let changes = history.undo().unwrap();
        assert_eq!(changes[0].tab_index, 0);

        assert_eq!(history.undo().is_none(), true);
    }

    #[test]
    fn test_unique_history() {
        let mut history: History<TabIndex> = History::new().max_undos(100).unique();

        // 推入一些条目
        history.push(0.into());
        history.push(1.into());
        history.push(1.into()); // 重复项，应被忽略
        history.push(2.into());
        history.push(1.into()); // 重复项，应移除旧项并添加新项

        // 检查版本号和撤销栈
        assert_eq!(history.version(), 5);
        assert_eq!(history.undos().len(), 3);
        assert_eq!(history.undos().last().unwrap().tab_index, 1);

        // 撤销最后一次变更
        let changes = history.undo().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].tab_index, 1);

        assert_eq!(history.redos().len(), 1);
        // 推入重复项，应被忽略
        history.push(2.into());

        assert_eq!(history.undos().len(), 2);
        assert_eq!(history.redos().len(), 1);

        // 重做最后一次被撤销的变更
        let changes = history.redo().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].tab_index, 1);

        // 推入另一个条目
        history.push(3.into());

        // 检查版本号和撤销栈
        assert_eq!(history.version(), 7);
        assert_eq!(history.undos().len(), 4);

        // 撤销所有变更
        for _ in 0..4 {
            history.undo();
        }

        // 检查撤销栈为空且重做栈包含所有变更
        assert_eq!(history.undos().len(), 0);
        assert_eq!(history.redos().len(), 4);
    }
}