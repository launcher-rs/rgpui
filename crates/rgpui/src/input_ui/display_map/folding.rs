use std::ops::Range;

/// 树类型 stub，替代 tree-sitter 的 Tree（rgpui 不引入 tree-sitter）。
pub struct Tree;

/// 可折叠区域的折叠范围。
///
/// 折叠范围从 start_line 到 end_line（包含）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FoldRange {
    /// 起始行（包含）
    pub start_line: usize,
    /// 结束行（包含）
    pub end_line: usize,
}

impl FoldRange {
    /// 创建新的折叠范围。
    pub fn new(start_line: usize, end_line: usize) -> Self {
        assert!(
            start_line <= end_line,
            "fold start_line must be <= end_line"
        );
        Self {
            start_line,
            end_line,
        }
    }
}

/// 提取折叠范围 - stub 实现（无 tree-sitter，返回空）。
pub fn extract_fold_ranges(_tree: &Tree) -> Vec<FoldRange> {
    Vec::new()
}

/// 提取指定字节范围内的折叠范围 - stub 实现（无 tree-sitter，返回空）。
pub fn extract_fold_ranges_in_range(_tree: &Tree, _byte_range: Range<usize>) -> Vec<FoldRange> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fold_range_ordering() {
        let mut ranges = vec![
            FoldRange {
                start_line: 10,
                end_line: 20,
            },
            FoldRange {
                start_line: 5,
                end_line: 15,
            },
            FoldRange {
                start_line: 5,
                end_line: 15,
            },
            FoldRange {
                start_line: 1,
                end_line: 30,
            },
        ];

        ranges.sort_by_key(|r| r.start_line);
        ranges.dedup_by_key(|r| r.start_line);

        assert_eq!(ranges.len(), 3);
        assert_eq!(ranges[0].start_line, 1);
        assert_eq!(ranges[1].start_line, 5);
        assert_eq!(ranges[2].start_line, 10);
    }
}
