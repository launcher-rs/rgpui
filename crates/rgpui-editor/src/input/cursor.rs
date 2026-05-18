use std::ops::{Range, RangeBounds};

/// 文本中的选区，由起始和结束字节索引表示
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Selection {
    /// 起始位置
    pub start: usize,
    /// 结束位置
    pub end: usize,
}

impl Selection {
    /// 创建新的选区
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// 获取选区长度
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// 检查选区是否为空
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// 清除选区，将起始和结束位置设为 0
    pub fn clear(&mut self) {
        self.start = 0;
        self.end = 0;
    }

    /// 检查给定偏移量是否在选区范围内
    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.start && offset < self.end
    }
}

impl From<Range<usize>> for Selection {
    fn from(value: Range<usize>) -> Self {
        Self::new(value.start, value.end)
    }
}
impl From<Selection> for Range<usize> {
    fn from(value: Selection) -> Self {
        value.start..value.end
    }
}
impl RangeBounds<usize> for Selection {
    fn start_bound(&self) -> std::ops::Bound<&usize> {
        std::ops::Bound::Included(&self.start)
    }

    fn end_bound(&self) -> std::ops::Bound<&usize> {
        std::ops::Bound::Excluded(&self.end)
    }
}

#[cfg(test)]
mod tests {
    use crate::input::Position;

    #[test]
    fn test_line_column_from_to() {
        assert_eq!(
            Position::new(1, 2),
            Position {
                line: 1,
                character: 2
            }
        );
    }
}
