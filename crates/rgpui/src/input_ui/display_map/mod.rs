/// Display 映射系统，用于 Editor/Input。
///
/// 本模块实现分层 display 映射架构：
/// - **WrapMap**：处理软换行（buffer 行 → wrap 行）
/// - **FoldMap**：处理折叠（wrap 行 → display 行）
/// - **DisplayMap**：Editor/Input 的公共门面
///
/// 目标是提供干净统一的 API，Editor 只需了解 `BufferPoint → DisplayPoint` 映射，
/// 无需关心内部 wrap/fold 的复杂性。
mod display_map;
mod fold_map;
mod folding;
pub(crate) mod text_wrapper;
mod wrap_map;

// 重新导出公共 API
pub use self::display_map::{DisplayMap, WrappingIndent};
pub(crate) use self::text_wrapper::LineLayout;

// 重新导出 FoldRange 和 extract_fold_ranges
pub use folding::{FoldRange, Tree, extract_fold_ranges};

/// Buffer 中的位置（逻辑文本）。
///
/// - `line`：0 起始的逻辑行号（按 `\n` 分割）
/// - `col`：0 起始的列偏移（字节偏移）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferPoint {
    /// 0 起始的逻辑行号。
    pub line: usize,
    /// 0 起始的列偏移（字节偏移）。
    pub col: usize,
}

impl BufferPoint {
    /// 创建新的 buffer 位置。
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

/// 软换行后、折叠前的位置（内部）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct WrapPoint {
    /// 0 起始的 wrap 行号。
    pub row: usize,
    /// 0 起始的列号。
    pub col: usize,
}

impl WrapPoint {
    /// 创建新的 wrap 位置。
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

/// 最终的 display 位置（软换行和折叠之后）。
///
/// - `row`：0 起始的 display 行（最终可见行）
/// - `col`：0 起始的 display 列
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DisplayPoint {
    /// 0 起始的 display 行。
    pub row: usize,
    /// 0 起始的 display 列。
    pub col: usize,
}

impl DisplayPoint {
    /// 创建新的 display 位置。
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}