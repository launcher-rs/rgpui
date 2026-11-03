use std::rc::Rc;

use crate::{App, Pixels, Size};

use crate::IndexPath;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowEntry {
    Entry(IndexPath),
    SectionHeader(usize),
    SectionFooter(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct MeasuredEntrySize {
    pub(crate) item_size: Size<Pixels>,
    pub(crate) section_header_size: Size<Pixels>,
    pub(crate) section_footer_size: Size<Pixels>,
}

impl RowEntry {
    pub(crate) fn eq_index_path(&self, path: &IndexPath) -> bool {
        match self {
            RowEntry::Entry(index_path) => index_path == path,
            RowEntry::SectionHeader(_) | RowEntry::SectionFooter(_) => false,
        }
    }

    pub(crate) fn index(&self) -> IndexPath {
        match self {
            RowEntry::Entry(index_path) => *index_path,
            RowEntry::SectionHeader(ix) => IndexPath::default().section(*ix),
            RowEntry::SectionFooter(ix) => IndexPath::default().section(*ix),
        }
    }

    #[inline]
    pub(crate) fn is_entry(&self) -> bool {
        matches!(self, RowEntry::Entry(_))
    }
}

#[derive(Default, Clone)]
pub(crate) struct RowsCache {
    /// 仅包含有行的节。
    pub(crate) entities: Rc<Vec<RowEntry>>,
    pub(crate) items_count: usize,
    /// 各节的行数。
    pub(crate) sections: Rc<Vec<usize>>,
    pub(crate) entries_sizes: Rc<Vec<Size<Pixels>>>,
    measured_size: MeasuredEntrySize,
}

impl RowsCache {
    pub(crate) fn get(&self, flatten_ix: usize) -> Option<RowEntry> {
        self.entities.get(flatten_ix).cloned()
    }

    /// 返回展平行的数量（包含节头、条目、节尾）。
    pub(crate) fn len(&self) -> usize {
        self.entities.len()
    }

    /// 返回缓存中的条目数量。
    pub(crate) fn items_count(&self) -> usize {
        self.items_count
    }

    /// 返回给定路径的条目在展平行中的索引。
    pub(crate) fn position_of(&self, path: &IndexPath) -> Option<usize> {
        self.entities
            .iter()
            .position(|p| p.is_entry() && p.eq_index_path(path))
    }

    /// 返回第一个条目行的展平位置，跳过节头和节尾。
    pub(crate) fn first_entry_position(&self) -> Option<usize> {
        self.entities.iter().position(RowEntry::is_entry)
    }

    /// 返回展平位置 `flatten_ix` **之前**的条目行数（节头、节尾不计）。
    ///
    /// 用于把「已滚动到第几行」换算成「已渲染几个条目」：分页阈值与
    /// `aria-posinset` 都以条目为单位，直接用行数会在多节列表里失真。
    pub(crate) fn items_before(&self, flatten_ix: usize) -> usize {
        self.entities
            .iter()
            .take(flatten_ix)
            .filter(|entry| entry.is_entry())
            .count()
    }

    /// 返回条目在展平行中的全局序号（从 1 开始）；该路径不在缓存里时返回 `None`。
    pub(crate) fn item_position_of(&self, path: &IndexPath) -> Option<usize> {
        self.position_of(path)
            .map(|flatten_ix| self.items_before(flatten_ix) + 1)
    }

    /// 返回上一行；如果该行是第一节的第一行，则跳到最后一行。
    ///
    /// 空行节会被跳过。
    pub(crate) fn prev(&self, path: Option<IndexPath>) -> IndexPath {
        let path = path.unwrap_or_default();
        let Some(pos) = self.position_of(&path) else {
            return self
                .entities
                .iter()
                .rfind(|entry| entry.is_entry())
                .map(|entry| entry.index())
                .unwrap_or_default();
        };

        if let Some(path) = self
            .entities
            .iter()
            .take(pos)
            .rev()
            .find(|entry| entry.is_entry())
            .map(|entry| entry.index())
        {
            path
        } else {
            self.entities
                .iter()
                .rfind(|entry| entry.is_entry())
                .map(|entry| entry.index())
                .unwrap_or_default()
        }
    }

    /// 返回下一行；如果该行是最后一节的最后一行，则回到第一行。
    ///
    /// 空行节会被跳过。
    pub(crate) fn next(&self, path: Option<IndexPath>) -> IndexPath {
        let Some(mut path) = path else {
            return IndexPath::default();
        };

        let Some(pos) = self.position_of(&path) else {
            return self
                .entities
                .iter()
                .find(|entry| entry.is_entry())
                .map(|entry| entry.index())
                .unwrap_or_default();
        };

        if let Some(next_path) = self
            .entities
            .iter()
            .skip(pos + 1)
            .find(|entry| entry.is_entry())
            .map(|entry| entry.index())
        {
            path = next_path;
        } else {
            path = self
                .entities
                .iter()
                .find(|entry| entry.is_entry())
                .map(|entry| entry.index())
                .unwrap_or_default()
        }

        path
    }

    pub(crate) fn prepare_if_needed<F>(
        &mut self,
        sections_count: usize,
        measured_size: MeasuredEntrySize,
        cx: &App,
        rows_count_f: F,
    ) where
        F: Fn(usize, &App) -> usize,
    {
        let mut new_sections = vec![];
        for section_ix in 0..sections_count {
            new_sections.push(rows_count_f(section_ix, cx));
        }

        let need_update = new_sections != *self.sections || self.measured_size != measured_size;

        if !need_update {
            return;
        }

        let mut entries_sizes = vec![];
        let mut total_items_count = 0;
        self.measured_size = measured_size;
        self.sections = Rc::new(new_sections);
        self.entities = Rc::new(
            self.sections
                .iter()
                .enumerate()
                .flat_map(|(section, items_count)| {
                    total_items_count += items_count;
                    let mut children = vec![];
                    if *items_count == 0 {
                        return children;
                    }

                    children.push(RowEntry::SectionHeader(section));
                    entries_sizes.push(measured_size.section_header_size);
                    for row in 0..*items_count {
                        children.push(RowEntry::Entry(IndexPath {
                            section,
                            row,
                            ..Default::default()
                        }));
                        entries_sizes.push(measured_size.item_size);
                    }
                    children.push(RowEntry::SectionFooter(section));
                    entries_sizes.push(measured_size.section_footer_size);
                    children
                })
                .collect(),
        );
        self.entries_sizes = Rc::new(entries_sizes);
        self.items_count = total_items_count;
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use crate::{
        IndexPath,
        list::cache::{RowEntry, RowsCache},
    };

    fn build_entities(sections: &[usize]) -> Vec<RowEntry> {
        sections
            .iter()
            .enumerate()
            .flat_map(|(section, items_count)| {
                let mut children = vec![];
                if *items_count == 0 {
                    return children;
                }

                children.push(RowEntry::SectionHeader(section));
                for row in 0..*items_count {
                    children.push(RowEntry::Entry(IndexPath {
                        section,
                        row,
                        ..Default::default()
                    }));
                }
                children.push(RowEntry::SectionFooter(section));
                children
            })
            .collect()
    }

    #[test]
    fn first_entry_position_skips_section_headers() {
        let mut row_cache = RowsCache::default();
        row_cache.entities = Rc::new(build_entities(&[2]));

        assert_eq!(row_cache.first_entry_position(), Some(1));
    }

    /// 展平行含节头/节尾，换算到条目单位时必须把它们剔除。
    #[test]
    fn items_before_and_item_position_count_entries_only() {
        let mut row_cache = RowsCache::default();
        // section 0: 2 条  -> 行 0..4（头、条目×2、尾）
        // section 1: 3 条  -> 行 4..9
        // section 2: 1 条  -> 行 9..12
        row_cache.sections = Rc::new(vec![2, 3, 1]);
        row_cache.entities = Rc::new(build_entities(&[2, 3, 1]));
        row_cache.items_count = 6;

        assert_eq!(row_cache.items_before(0), 0);
        // 第 4 行之前是 section 0 的 2 个条目（行 0 是节头、行 3 是节尾）。
        assert_eq!(row_cache.items_before(4), 2);
        // 行 4 是 section 1 的节头，本身不计入条目。
        assert_eq!(row_cache.items_before(5), 2);
        assert_eq!(row_cache.items_before(9), 5);
        assert_eq!(row_cache.items_before(12), 6);

        // 全局序号跨节连续：section 1 的第一条是第 3 条，而不是第 1 条。
        assert_eq!(
            row_cache.item_position_of(&IndexPath::new(0).section(1)),
            Some(3)
        );
        assert_eq!(
            row_cache.item_position_of(&IndexPath::new(2).section(1)),
            Some(5)
        );
        assert_eq!(
            row_cache.item_position_of(&IndexPath::new(0).section(2)),
            Some(6)
        );
    }

    #[test]
    fn test_prev_next() {
        let mut row_cache = RowsCache::default();
        // section 0
        //  row 0
        //  row 1
        // section 1
        //  row 0
        //  row 1
        //  row 2
        //  row 3
        // section 2
        //  row 0
        //  row 1
        //  row 2
        row_cache.sections = Rc::new(vec![2, 4, 3]);
        row_cache.entities = Rc::new(build_entities(&[2, 4, 3]));

        assert_eq!(
            row_cache.next(Some(IndexPath::new(0).section(0))),
            IndexPath::new(1).section(0)
        );
        assert_eq!(
            row_cache.next(Some(IndexPath::new(1).section(0))),
            IndexPath::new(0).section(1)
        );
        assert_eq!(
            row_cache.next(Some(IndexPath::new(0).section(1))),
            IndexPath::new(1).section(1)
        );
        assert_eq!(
            row_cache.next(Some(IndexPath::new(3).section(1))),
            IndexPath::new(0).section(2)
        );
        assert_eq!(
            row_cache.next(Some(IndexPath::new(0).section(2))),
            IndexPath::new(1).section(2)
        );
        assert_eq!(
            row_cache.next(Some(IndexPath::new(1).section(2))),
            IndexPath::new(2).section(2)
        );
        assert_eq!(
            row_cache.next(Some(IndexPath::new(2).section(2))),
            IndexPath::new(0).section(0)
        );

        assert_eq!(
            row_cache.prev(Some(IndexPath::new(0).section(0))),
            IndexPath::new(2).section(2)
        );
        assert_eq!(
            row_cache.prev(Some(IndexPath::new(1).section(0))),
            IndexPath::new(0).section(0)
        );
        assert_eq!(
            row_cache.prev(Some(IndexPath::new(0).section(1))),
            IndexPath::new(1).section(0)
        );
        assert_eq!(
            row_cache.prev(Some(IndexPath::new(1).section(1))),
            IndexPath::new(0).section(1)
        );
        assert_eq!(
            row_cache.prev(Some(IndexPath::new(3).section(1))),
            IndexPath::new(2).section(1)
        );
        assert_eq!(
            row_cache.prev(Some(IndexPath::new(0).section(2))),
            IndexPath::new(3).section(1)
        );
    }

    #[test]
    fn test_prev_next_with_empty_sections() {
        let mut row_cache = RowsCache::default();
        // section 0: 2 items
        // section 1: 0 items (empty, should be skipped)
        // section 2: 3 items
        // section 3: 0 items (empty, should be skipped)
        // section 4: 1 item
        row_cache.sections = Rc::new(vec![2, 0, 3, 0, 1]);
        row_cache.entities = Rc::new(build_entities(&[2, 0, 3, 0, 1]));

        // Test next: should skip empty sections
        assert_eq!(
            row_cache.next(Some(IndexPath::new(0).section(0))),
            IndexPath::new(1).section(0)
        );
        assert_eq!(
            row_cache.next(Some(IndexPath::new(1).section(0))),
            IndexPath::new(0).section(2) // Skip section 1 (empty)
        );
        assert_eq!(
            row_cache.next(Some(IndexPath::new(0).section(2))),
            IndexPath::new(1).section(2)
        );
        assert_eq!(
            row_cache.next(Some(IndexPath::new(2).section(2))),
            IndexPath::new(0).section(4) // Skip section 3 (empty)
        );
        assert_eq!(
            row_cache.next(Some(IndexPath::new(0).section(4))),
            IndexPath::new(0).section(0) // Wrap around to first item
        );

        // Test prev: should skip empty sections
        assert_eq!(
            row_cache.prev(Some(IndexPath::new(0).section(0))),
            IndexPath::new(0).section(4) // Wrap around to last item, skip empty sections
        );
        assert_eq!(
            row_cache.prev(Some(IndexPath::new(0).section(2))),
            IndexPath::new(1).section(0) // Skip section 1 (empty)
        );
        assert_eq!(
            row_cache.prev(Some(IndexPath::new(0).section(4))),
            IndexPath::new(2).section(2) // Skip section 3 (empty)
        );
        assert_eq!(
            row_cache.prev(Some(IndexPath::new(1).section(2))),
            IndexPath::new(0).section(2)
        );
        assert_eq!(
            row_cache.prev(Some(IndexPath::new(2).section(2))),
            IndexPath::new(1).section(2)
        );
    }
}
