/// WrapMap：软换行层（Buffer → Wrap 行）。
///
/// 本模块包装现有的 TextWrapper 并提供：
/// - BufferPoint → WrapPoint 映射
/// - 通过前缀和缓存的高效 buffer_line → wrap_row 查询
/// - 文本或布局变更时的增量更新
use std::ops::Range;

use crate::{App, Font, Pixels};
use ropey::Rope;

use super::fold_map::FoldMap;
use super::text_wrapper::{LineItem, TextWrapper, WrapDisplayPoint, WrappingIndent};
use super::{BufferPoint, WrapPoint};
use super::super::RopeExt;

/// WrapMap 管理软换行并提供 buffer → wrap 坐标映射。
///
/// buffer 行 → wrap 行映射由 [`TextWrapper`] 的 `SumTree` 支持。
pub struct WrapMap {
    /// 底层文本包装器（复用现有实现）
    wrapper: TextWrapper,
}

impl WrapMap {
    /// 创建新的 WrapMap。
    pub fn new(font: Font, font_size: Pixels, wrap_width: Option<Pixels>) -> Self {
        Self {
            wrapper: TextWrapper::new(font, font_size, wrap_width),
        }
    }

    /// 获取总 wrap 行数（软换行后的视觉行数）
    #[inline]
    pub fn wrap_row_count(&self) -> usize {
        self.wrapper.len()
    }

    /// 获取总 buffer 行数（逻辑行数）
    #[inline]
    pub fn buffer_line_count(&self) -> usize {
        self.wrapper.lines_count()
    }

    /// 将 buffer 位置转换为 wrap 位置。
    pub(super) fn buffer_pos_to_wrap_pos(&self, pos: BufferPoint) -> WrapPoint {
        let BufferPoint { line, col } = pos;

        // 裁剪到有效范围
        let line = line.min(self.buffer_line_count().saturating_sub(1));
        let line_item = self.wrapper.line(line);

        let col = if let Some(line_item) = line_item {
            col.min(line_item.len())
        } else {
            0
        };

        // 计算 rope 中的偏移
        let line_start_offset = self.wrapper.text().line_start_offset(line);
        let offset = line_start_offset + col;

        // 使用 TextWrapper 的现有转换
        let display_point = self.wrapper.offset_to_display_point(offset);

        WrapPoint::new(display_point.row, display_point.column)
    }

    /// 将 wrap 位置转换为 buffer 位置。
    pub(super) fn wrap_pos_to_buffer_pos(&self, pos: WrapPoint) -> BufferPoint {
        let WrapPoint { row, col } = pos;

        // 裁剪 wrap_row 到有效范围
        let row = row.min(self.wrap_row_count().saturating_sub(1));

        // 使用 TextWrapper 的现有转换
        let display_point = WrapDisplayPoint::new(row, 0, col);
        let offset = self.wrapper.display_point_to_offset(display_point);

        // 将偏移转换为 buffer 位置
        let point = self.wrapper.text().offset_to_point(offset);
        let line_start = self.wrapper.text().line_start_offset(point.row);
        let col = offset.saturating_sub(line_start);

        BufferPoint::new(point.row, col)
    }

    /// 获取给定 wrap 行的 buffer 行。
    pub fn wrap_row_to_buffer_line(&self, wrap_row: usize) -> usize {
        self.wrapper.wrap_row_to_buffer_line(wrap_row)
    }

    /// 获取给定 buffer 行的第一个 wrap 行。
    pub fn buffer_line_to_first_wrap_row(&self, line: usize) -> usize {
        self.wrapper.buffer_line_to_first_wrap_row(line)
    }

    /// 获取 buffer 行的 wrap 行范围：[start, end)。
    pub fn buffer_line_to_wrap_row_range(&self, line: usize) -> Range<usize> {
        self.wrapper.buffer_line_to_wrap_row_range(line)
    }

    /// 更新文本（增量或全量）。
    pub fn on_text_changed(
        &mut self,
        changed_text: &Rope,
        range: &Range<usize>,
        new_text: &Rope,
        cx: &mut App,
    ) {
        self.wrapper.update(changed_text, range, new_text, cx);
    }

    /// 更新布局参数（换行宽度或字体）。
    pub fn on_layout_changed(&mut self, wrap_width: Option<Pixels>, cx: &mut App) {
        self.wrapper.set_wrap_width(wrap_width, cx);
    }

    /// 设置续行的缩进模式。
    pub fn set_wrapping_indent(&mut self, wrapping_indent: WrappingIndent, cx: &mut App) {
        self.wrapper.set_wrapping_indent(wrapping_indent, cx);
    }

    /// 设置字体参数。
    pub fn set_font(&mut self, font: Font, font_size: Pixels, cx: &mut App) {
        self.wrapper.set_font(font, font_size, cx);
    }

    /// 确保文本已准备（如需要则初始化包装器）。
    pub fn ensure_text_prepared(&mut self, text: &Rope, cx: &mut App) -> bool {
        self.wrapper.prepare_if_need(text, cx)
    }

    /// 用文本初始化。
    pub fn set_text(&mut self, text: &Rope, cx: &mut App) {
        self.wrapper.set_default_text(text);
        self.wrapper.prepare_if_need(text, cx);
    }

    /// 访问底层包装器（用于渲染/命中测试）。
    pub(crate) fn wrapper(&self) -> &TextWrapper {
        &self.wrapper
    }

    /// 按 buffer 行索引获取行项。
    #[inline]
    pub(crate) fn line(&self, row: usize) -> Option<&LineItem> {
        self.wrapper.line(row)
    }

    /// 获取 rope 文本。
    pub fn text(&self) -> &Rope {
        self.wrapper.text()
    }

    /// 计算 buffer 行的可见（未折叠）wrap 行数。
    pub fn visible_wrap_row_count_for_line(&self, line: usize, fold_map: &FoldMap) -> usize {
        let wrap_range = self.buffer_line_to_wrap_row_range(line);
        wrap_range
            .filter(|&wr| fold_map.wrap_row_to_display_row(wr).is_some())
            .count()
    }
}