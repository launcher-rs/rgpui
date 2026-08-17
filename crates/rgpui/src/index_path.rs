use std::fmt::{Debug, Display};

use crate::ElementId;

/// 表示列表中的索引路径，由节索引、行索引和列索引组成。
///
/// section、row、column 的默认值均为 0。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IndexPath {
    /// 节索引。
    pub section: usize,
    /// 节内的项索引。
    pub row: usize,
    /// 列索引。
    pub column: usize,
}

impl From<IndexPath> for ElementId {
    fn from(path: IndexPath) -> Self {
        ElementId::Name(format!("index-path({},{},{})", path.section, path.row, path.column).into())
    }
}

impl Display for IndexPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "IndexPath(section: {}, row: {}, column: {})",
            self.section, self.row, self.column
        )
    }
}

impl IndexPath {
    /// 使用指定的行索引创建新的索引路径。
    ///
    /// section 默认为 0，column 默认为 0。
    pub fn new(row: usize) -> Self {
        IndexPath {
            section: 0,
            row,
            ..Default::default()
        }
    }

    /// 设置索引路径的节索引。
    pub fn section(mut self, section: usize) -> Self {
        self.section = section;
        self
    }

    /// 设置索引路径的行索引。
    pub fn row(mut self, row: usize) -> Self {
        self.row = row;
        self
    }

    /// 设置索引路径的列索引。
    pub fn column(mut self, column: usize) -> Self {
        self.column = column;
        self
    }

    /// 检查自身是否与给定的索引路径相等（section 和 row 相同）。
    pub fn eq_row(&self, index: IndexPath) -> bool {
        self.section == index.section && self.row == index.row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_into_element_id() {
        let index_path = IndexPath::new(2).section(1).column(3);
        let element_id: ElementId = index_path.into();
        assert_eq!(element_id.to_string(), "index-path(1,2,3)");
    }

    #[test]
    fn test_display() {
        assert_eq!(
            format!("{}", IndexPath::new(2).section(1).column(3)),
            "IndexPath(section: 1, row: 2, column: 3)"
        );
    }

    #[test]
    fn test_index_path() {
        let mut index_path = IndexPath::default();
        assert_eq!(index_path.section, 0);
        assert_eq!(index_path.row, 0);
        assert_eq!(index_path.column, 0);

        index_path = index_path.section(1).row(2).column(3);
        assert_eq!(index_path.section, 1);
        assert_eq!(index_path.row, 2);
        assert_eq!(index_path.column, 3);
    }
}
