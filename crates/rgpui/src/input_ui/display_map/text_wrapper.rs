use crate::Half;
use std::ops::Range;

use crate::{
    App, Font, LineFragment, Pixels, Point, ShapedLine, Size, TextAlign, Window, point, px, size,
};
use ropey::Rope;
use smallvec::SmallVec;
use crate::sum_tree::{Bias, Dimensions, SumTree};

use super::super::{LastLayout, RopeExt, WhitespaceIndicators};
use super::super::Point as TreeSitterPoint;

/// 控制软换行续行如何缩进。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WrappingIndent {
    /// 续行在编辑器的全宽处左对齐开始。
    None,
    /// 续行保持与第一行相同的缩进。
    #[default]
    Same,
}

/// 一条带软换行信息的行。
#[derive(Debug, Clone)]
pub(crate) struct LineItem {
    /// 行的字节长度，不包含末尾的 `\n`。
    len: usize,
    /// 当使用 [`WrappingIndent::Same`] 时，行首保留的续行缩进字符数。
    ///
    /// 使用 [`WrappingIndent::None`] 或该行未换行时为零。
    pub(crate) indent: u32,
    /// 该行的软换行相对字节范围（0..len）（包含第一行）。
    ///
    /// 不包含行末尾的 `\n`。
    pub(crate) wrapped_lines: SmallVec<[Range<usize>; 1]>,
}

impl LineItem {
    /// 获取此行的字节长度。
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// 获取此行的软换行行数（包含第一行）。
    #[inline]
    pub(crate) fn lines_len(&self) -> usize {
        self.wrapped_lines.len()
    }
}

/// [`LineItem`] 子树摘要，由 [`SumTree`] 增量维护。
#[derive(Debug, Clone)]
pub(crate) struct LineSummary {
    /// buffer 行数。
    buffer_rows: usize,
    /// wrap 行数（每个行 `lines_len()` 之和）。
    wrap_rows: usize,
    /// buffer 行的字节长度之和（不包含末尾 `\n`）。
    bytes: usize,
    /// 此子树中最长行的字节长度。
    max_line_len: usize,
    /// 达到 `max_line_len` 的第一行的 buffer 行（相对此子树）。
    longest_row: usize,
}

impl crate::sum_tree::Summary for LineSummary {
    type Context<'a> = &'a ();

    fn zero(_: &()) -> Self {
        LineSummary {
            buffer_rows: 0,
            wrap_rows: 0,
            bytes: 0,
            max_line_len: 0,
            longest_row: 0,
        }
    }

    fn add_summary(&mut self, other: &Self, _: &()) {
        // 保持严格更大长度的最左侧行
        if other.max_line_len > self.max_line_len {
            self.longest_row = self.buffer_rows + other.longest_row;
            self.max_line_len = other.max_line_len;
        }
        self.buffer_rows += other.buffer_rows;
        self.wrap_rows += other.wrap_rows;
        self.bytes += other.bytes;
    }
}

impl crate::sum_tree::Item for LineItem {
    type Summary = LineSummary;

    fn summary(&self, _: &()) -> LineSummary {
        LineSummary {
            buffer_rows: 1,
            wrap_rows: self.lines_len(),
            bytes: self.len(),
            max_line_len: self.len(),
            longest_row: 0,
        }
    }
}

/// 计数 buffer 行的游标维度。
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct BufferRows(pub usize);

impl<'a> crate::sum_tree::Dimension<'a, LineSummary> for BufferRows {
    fn zero(_: &()) -> Self {
        BufferRows(0)
    }

    fn add_summary(&mut self, summary: &'a LineSummary, _: &()) {
        self.0 += summary.buffer_rows;
    }
}

/// 计数 wrap 行的游标维度。
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct WrapRows(pub usize);

impl<'a> crate::sum_tree::Dimension<'a, LineSummary> for WrapRows {
    fn zero(_: &()) -> Self {
        WrapRows(0)
    }

    fn add_summary(&mut self, summary: &'a LineSummary, _: &()) {
        self.0 += summary.wrap_rows;
    }
}

/// 用于准备带软换行的文本，以便获取 Editor 中显示的行。
///
/// 之后使用行数计算 Editor 的滚动尺寸。
pub(crate) struct TextWrapper {
    text: Rope,
    font: Font,
    font_size: Pixels,
    /// 如果为 None，表示文本不换行
    wrap_width: Option<Pixels>,
    wrapping_indent: WrappingIndent,
    /// 按 `\n` 分割的行
    pub(crate) lines: SumTree<LineItem>,

    _initialized: bool,
}

#[allow(unused)]
impl TextWrapper {
    /// 创建新的文本包装器。
    pub(crate) fn new(font: Font, font_size: Pixels, wrap_width: Option<Pixels>) -> Self {
        Self {
            text: Rope::new(),
            font,
            font_size,
            wrap_width,
            wrapping_indent: WrappingIndent::default(),
            lines: SumTree::new(&()),
            _initialized: false,
        }
    }

    /// 设置默认文本。
    #[inline]
    pub(crate) fn set_default_text(&mut self, text: &Rope) {
        self.text = text.clone();
    }

    /// 获取 rope 文本的引用。
    #[inline]
    pub(crate) fn text(&self) -> &Rope {
        &self.text
    }

    /// 获取包含换行行的总行数。
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.lines.summary().wrap_rows
    }

    /// 获取 buffer 行总数。
    #[inline]
    pub(crate) fn lines_count(&self) -> usize {
        self.lines.summary().buffer_rows
    }

    /// 获取最长行（按字节长度）的 0 起始行索引。
    #[inline]
    pub(crate) fn longest_row(&self) -> usize {
        self.lines.summary().longest_row
    }

    /// 按 buffer 行索引获取行项。
    #[inline]
    pub(crate) fn line(&self, row: usize) -> Option<&LineItem> {
        let mut cursor = self.lines.cursor::<BufferRows>(&());
        cursor.seek(&BufferRows(row), Bias::Right);
        cursor.item()
    }

    /// 按顺序迭代 buffer 行。
    #[inline]
    pub(crate) fn iter_lines(&self) -> impl Iterator<Item = &LineItem> {
        self.lines.iter()
    }

    /// buffer 行 `row` 的第一个 wrap 行。如果 `row` 超出范围则返回总 wrap 行数。
    pub(crate) fn buffer_line_to_first_wrap_row(&self, row: usize) -> usize {
        let mut cursor = self.lines.cursor::<Dimensions<BufferRows, WrapRows>>(&());
        cursor.seek(&BufferRows(row), Bias::Right);
        cursor.start().1.0
    }

    /// buffer 行 `row` 的 wrap 行范围。
    pub(crate) fn buffer_line_to_wrap_row_range(&self, row: usize) -> Range<usize> {
        let mut cursor = self.lines.cursor::<Dimensions<BufferRows, WrapRows>>(&());
        cursor.seek(&BufferRows(row), Bias::Right);
        let start = cursor.start().1.0;
        let len = cursor.item().map(|l| l.lines_len()).unwrap_or(0);
        start..start + len
    }

    /// 包含 wrap 行 `wrap_row` 的 buffer 行，裁剪到最后一行。
    pub(crate) fn wrap_row_to_buffer_line(&self, wrap_row: usize) -> usize {
        let mut cursor = self.lines.cursor::<Dimensions<WrapRows, BufferRows>>(&());
        cursor.seek(&WrapRows(wrap_row), Bias::Right);
        match cursor.item() {
            Some(_) => cursor.start().1.0,
            None => self.lines_count().saturating_sub(1),
        }
    }

    /// 设置换行宽度。
    pub(crate) fn set_wrap_width(&mut self, wrap_width: Option<Pixels>, cx: &mut App) {
        if wrap_width == self.wrap_width {
            return;
        }

        self.wrap_width = wrap_width;
        self.update_all(&self.text.clone(), cx);
    }

    /// 设置续行缩进模式。
    pub(crate) fn set_wrapping_indent(&mut self, wrapping_indent: WrappingIndent, cx: &mut App) {
        if wrapping_indent == self.wrapping_indent {
            return;
        }

        self.wrapping_indent = wrapping_indent;
        self.update_all(&self.text.clone(), cx);
    }

    /// 设置字体参数。
    pub(crate) fn set_font(&mut self, font: Font, font_size: Pixels, cx: &mut App) {
        if self.font.eq(&font) && self.font_size == font_size {
            return;
        }

        self.font = font;
        self.font_size = font_size;
        self.update_all(&self.text.clone(), cx);
    }

    /// 如需要则准备文本。
    pub(crate) fn prepare_if_need(&mut self, text: &Rope, cx: &mut App) -> bool {
        if self._initialized {
            return false;
        }
        self._initialized = true;
        self.update_all(text, cx);
        true
    }

    /// 更新文本包装器并重新计算换行行。
    ///
    /// 如果 `text` 与当前文本相同，则不做任何事。
    ///
    /// - `changed_text`：已变更的文本 [`Rope`]。
    /// - `range`：变更前的 `selected_range`。
    /// - `new_text`：插入的文本。
    /// - `force`：是否强制更新，如果为 false，当文本相同时将跳过更新。
    /// - `cx`：应用上下文。
    pub(crate) fn update(
        &mut self,
        changed_text: &Rope,
        range: &Range<usize>,
        new_text: &Rope,
        cx: &mut App,
    ) {
        let mut line_wrapper = cx
            .text_system()
            .line_wrapper(self.font.clone(), self.font_size);
        self._update(
            changed_text,
            range,
            new_text,
            &mut |line_str, wrap_width| {
                line_wrapper
                    .wrap_line(&[LineFragment::text(line_str)], wrap_width)
                    .collect()
            },
        );
    }

    /// 内部更新方法。
    fn _update<F>(
        &mut self,
        changed_text: &Rope,
        range: &Range<usize>,
        new_text: &Rope,
        wrap_line: &mut F,
    ) where
        F: FnMut(&str, Pixels) -> Vec<crate::Boundary>,
    {
        // 移除旧的变更行。
        let buffer_line_count = self.lines_count();
        let start_row = self.text.offset_to_point(range.start).row;
        let start_row = start_row.min(buffer_line_count.saturating_sub(1));
        let end_row = self.text.offset_to_point(range.end).row;
        let end_row = end_row.min(buffer_line_count.saturating_sub(1));

        // 添加新行。
        let new_start_row = changed_text.offset_to_point(range.start).row;
        let new_start_offset = changed_text.line_start_offset(new_start_row);
        let new_end_row = changed_text
            .offset_to_point(range.start + new_text.len())
            .row;
        let new_end_offset = changed_text.line_end_offset(new_end_row);
        let new_range = new_start_offset..new_end_offset;

        let mut new_lines = vec![];
        let wrap_width = self.wrap_width;

        // 行不包含 `\n`。
        for line in Rope::from(changed_text.slice(new_range)).iter_lines() {
            let line_str = line.to_string();
            let mut wrapped_lines = SmallVec::<[Range<usize>; 1]>::new();
            let mut prev_boundary_ix = 0;
            let mut indent_chars = 0;

            // 如果 wrap_width 为 Pixels::MAX，跳过换行以禁用自动换行
            if let Some(wrap_width) = wrap_width {
                match self.wrapping_indent {
                    WrappingIndent::Same => {
                        // 这里只有换行行，如果没有换行，`line_wraps` 结果为空。
                        for boundary in wrap_line(&line_str, wrap_width) {
                            wrapped_lines.push(prev_boundary_ix..boundary.ix);
                            prev_boundary_ix = boundary.ix;
                            indent_chars = boundary.next_indent;
                        }
                    }
                    WrappingIndent::None => {
                        // 第一个视觉行保持该行的前导缩进，因此原样换行。
                        let bondaries = wrap_line(&line_str, wrap_width);
                        if let Some(first) = bondaries.first() {
                            let first_ix = first.ix;
                            wrapped_lines.push(prev_boundary_ix..first_ix);
                            prev_boundary_ix = first_ix;

                            for boundary in wrap_line(&line_str[first_ix..], wrap_width) {
                                let ix = first_ix + boundary.ix;
                                wrapped_lines.push(prev_boundary_ix..ix);
                                prev_boundary_ix = ix;
                            }
                        }
                    }
                }
            }

            // 行的剩余部分
            if !line_str[prev_boundary_ix..].is_empty() || prev_boundary_ix == 0 {
                wrapped_lines.push(prev_boundary_ix..line.len());
            }

            new_lines.push(LineItem {
                len: line.len(),
                indent: indent_chars,
                wrapped_lines,
            });
        }

        if self.lines.is_empty() {
            self.lines = SumTree::from_iter(new_lines, &());
        } else {
            let mut cursor = self.lines.cursor::<BufferRows>(&());
            let mut new_tree = cursor.slice(&BufferRows(start_row), Bias::Right);
            // 跳过被替换的行
            cursor.seek_forward(&BufferRows(end_row + 1), Bias::Right);
            new_tree.extend(new_lines, &());
            // 编辑后未触及的行
            new_tree.append(cursor.suffix(), &());
            drop(cursor);
            self.lines = new_tree;
        }

        self.text = changed_text.clone();
    }

    /// 更新文本包装器并重新计算换行行。
    ///
    /// 如果 `text` 与当前文本相同，则不做任何事。
    fn update_all(&mut self, text: &Rope, cx: &mut App) {
        self.update(text, &(0..text.len()), &text, cx);
    }

    /// 从文本中的给定字节偏移返回 display 点（带软换行）。
    ///
    /// 如果 `offset` 超出边界则 panic。
    pub(crate) fn offset_to_display_point(&self, offset: usize) -> WrapDisplayPoint {
        let row = self.text.offset_to_point(offset).row;
        let start = self.text.line_start_offset(row);

        // 定位到 buffer 行
        let mut cursor = self.lines.cursor::<Dimensions<BufferRows, WrapRows>>(&());
        cursor.seek(&BufferRows(row), Bias::Right);
        let wrapped_row = cursor.start().1.0;
        let Some(line) = cursor.item() else {
            return WrapDisplayPoint::new(wrapped_row, 0, 0);
        };

        let local_offset = offset.saturating_sub(start);
        for (ix, range) in line.wrapped_lines.iter().enumerate() {
            if range.contains(&local_offset) {
                return WrapDisplayPoint::new(
                    wrapped_row + ix,
                    ix,
                    local_offset.saturating_sub(range.start),
                );
            }
        }

        // 否则返回该行的末尾。
        let last_range = line.wrapped_lines.last().unwrap_or(&(0..0));
        let ix = line.lines_len().saturating_sub(1);
        return WrapDisplayPoint::new(wrapped_row + ix, ix, last_range.len());
    }

    /// 从给定的 display 点（带软换行）返回文本中的字节偏移。
    ///
    /// 如果 `point.row` 超出边界则 panic。
    pub(crate) fn display_point_to_offset(&self, point: WrapDisplayPoint) -> usize {
        // 定位到 wrap 行 `point.row`
        let mut cursor = self.lines.cursor::<Dimensions<WrapRows, BufferRows>>(&());
        cursor.seek(&WrapRows(point.row), Bias::Right);
        let Some(line) = cursor.item() else {
            return self.text.len();
        };
        let wrapped_row = cursor.start().0.0;
        let row = cursor.start().1.0;

        let line_start = self.text.line_start_offset(row);
        let local_row = point.row.saturating_sub(wrapped_row);
        if let Some(range) = line.wrapped_lines.get(local_row) {
            line_start + (range.start + point.column).min(range.end)
        } else {
            // 如果未找到，返回行的末尾。
            line_start + line.len()
        }
    }

    /// 将 display 点转换为树点（buffer 行/列）。
    pub(crate) fn display_point_to_point(&self, point: WrapDisplayPoint) -> TreeSitterPoint {
        let offset = self.display_point_to_offset(point);
        self.text.offset_to_point(offset)
    }

    /// 将树点（buffer 行/列）转换为 display 点。
    pub(crate) fn point_to_display_point(&self, point: TreeSitterPoint) -> WrapDisplayPoint {
        let offset = self.text.point_to_offset(point);
        self.offset_to_display_point(offset)
    }
}

/// 软换行文本中的 display 点。
///
/// 表示软换行后的文本位置，附加 `local_row` 字段跟踪
/// 原始 buffer 行内的 wrap 行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WrapDisplayPoint {
    /// 文本中 0 起始的软换行行索引。
    pub row: usize,
    /// 本地行（包含第一行）内 0 起始的行索引。
    ///
    /// 该值仅在 [`TextWrapper::offset_to_display_point`] 返回时有效，否则被忽略。
    pub local_row: usize,
    /// display 行（带软换行）中 0 起始的列字节索引。
    pub column: usize,
}

impl WrapDisplayPoint {
    /// 创建新的 display 点。
    pub fn new(row: usize, local_row: usize, column: usize) -> Self {
        Self {
            row,
            local_row,
            column,
        }
    }
}

/// 一条带软换行行的布局信息。
pub(crate) struct LineLayout {
    /// 该行的总字节长度。
    len: usize,
    /// 该行的软换行行（包含第一行）。
    pub(crate) wrapped_lines: SmallVec<[ShapedLine; 1]>,
    /// 应用于续行（wrap 行）的额外左偏移，当使用 [`WrappingIndent::Same`] 时
    /// 用于保留第一行的缩进。
    pub(crate) wrap_indent: Pixels,
    /// 最长行的宽度。
    pub(crate) longest_width: Pixels,
    /// 空白指示符。
    pub(crate) whitespace_indicators: Option<WhitespaceIndicators>,
    /// 空白指示符：(行索引, x 位置, 是否制表符)
    pub(crate) whitespace_chars: Vec<(usize, Pixels, bool)>,
}

impl LineLayout {
    /// 创建新的行布局。
    pub(crate) fn new() -> Self {
        Self {
            len: 0,
            longest_width: px(0.),
            wrapped_lines: SmallVec::new(),
            wrap_indent: px(0.),
            whitespace_chars: Vec::new(),
            whitespace_indicators: None,
        }
    }

    /// 设置续行保留的左偏移。
    pub(crate) fn wrap_indent(mut self, wrap_indent: Pixels) -> Self {
        self.wrap_indent = wrap_indent;
        self
    }

    /// 应用于给定视觉行的像素缩进，相对于该行的前导文本。
    /// 只有续行（索引 > 0）会缩进。
    #[inline]
    fn line_indent(&self, line_index: usize) -> Pixels {
        if line_index == 0 {
            px(0.)
        } else {
            self.wrap_indent
        }
    }

    /// 设置换行行。
    pub(crate) fn lines(mut self, wrapped_lines: SmallVec<[ShapedLine; 1]>) -> Self {
        self.set_wrapped_lines(wrapped_lines);
        self
    }

    /// 设置换行行。
    pub(crate) fn set_wrapped_lines(&mut self, wrapped_lines: SmallVec<[ShapedLine; 1]>) {
        self.len = wrapped_lines.iter().map(|l| l.len).sum();
        let width = wrapped_lines
            .iter()
            .map(|l| l.width)
            .max()
            .unwrap_or_default();
        self.longest_width = width;
        self.wrapped_lines = wrapped_lines;
    }

    /// 设置空白指示符。
    pub(crate) fn with_whitespaces(mut self, indicators: Option<WhitespaceIndicators>) -> Self {
        self.whitespace_indicators = indicators;
        let Some(indicators) = self.whitespace_indicators.as_ref() else {
            return self;
        };

        let space_indicator_offset = indicators.space.width().half();

        for (line_index, wrapped_line) in self.wrapped_lines.iter().enumerate() {
            for (relative_offset, c) in wrapped_line.text.char_indices() {
                if matches!(c, ' ' | '\t') {
                    let is_tab = c == '\t';
                    let start_x = wrapped_line.x_for_index(relative_offset);
                    let end_x = wrapped_line.x_for_index(relative_offset + c.len_utf8());
                    // 将指示符居中在字符的实际空间中
                    let x_position = if c == ' ' {
                        (start_x + end_x).half() - space_indicator_offset
                    } else {
                        start_x
                    };

                    self.whitespace_chars.push((line_index, x_position, is_tab));
                }
            }
        }
        self
    }

    /// 获取行布局的长度（字节数）。
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// 获取此布局中给定索引的位置 (x, y)。
    ///
    /// - `offset` 是此布局中的本地字节索引。
    /// - 当 `line_end_affinity` 为 true 时，软换行边界处的偏移放置在
    ///   当前视觉行的末尾而不是下一行的开头。
    /// - 返回值相对于此布局的左上角，从 (0, 0) 开始。
    pub(crate) fn position_for_index(
        &self,
        offset: usize,
        last_layout: &LastLayout,
        line_end_affinity: bool,
    ) -> Option<Point<Pixels>> {
        let mut acc_len = 0;
        let mut offset_y = px(0.);

        let x_offset = last_layout.alignment_offset(self.longest_width);

        for (i, line) in self.wrapped_lines.iter().enumerate() {
            let is_last = i + 1 == self.wrapped_lines.len();

            let matches = if line.len == 0 {
                // 空视觉行仍拥有其边界偏移。
                offset == acc_len
            } else if is_last || line_end_affinity {
                // 包含：光标可以位于此视觉行的末尾。
                offset >= acc_len && offset <= acc_len + line.len
            } else {
                // 排除：边界偏移属于下一视觉行。
                offset >= acc_len && offset < acc_len + line.len
            };

            if matches {
                let x = line.x_for_index(offset.saturating_sub(acc_len))
                    + x_offset
                    + self.line_indent(i);
                return Some(point(x, offset_y));
            }

            // 总是按实际行长推进。最后一行加 1，使光标可以放在最后一个字符之后。
            acc_len += if is_last { line.len + 1 } else { line.len };
            offset_y += last_layout.line_height;
        }

        None
    }

    /// 获取此布局中给定 x 的最近索引。
    pub(crate) fn closest_index_for_x(&self, x: Pixels, last_layout: &LastLayout) -> usize {
        let mut acc_len = 0;
        let x_offset = last_layout.alignment_offset(self.longest_width);
        let x = x - x_offset;

        for (i, line) in self.wrapped_lines.iter().enumerate() {
            let is_last = i + 1 == self.wrapped_lines.len();
            let line_indent = self.line_indent(i);
            if x <= line_indent + line.width() {
                let mut ix = line.closest_index_for_x(x - line_indent);
                if !is_last && ix == line.text.len() {
                    // 对于软换行行，不能将光标放在行末尾。
                    let c_len = line.text.chars().last().map(|c| c.len_utf8()).unwrap_or(0);
                    ix = ix.saturating_sub(c_len);
                }

                return acc_len + ix;
            }
            acc_len += line.text.len();
        }

        acc_len
    }

    /// 获取此布局中给定位置 (x, y) 的索引。
    ///
    /// `pos` 相对于此布局的左上角，从 (0, 0) 开始。
    /// 返回值是此布局中的本地字节索引，从 0 开始。
    pub(crate) fn closest_index_for_position(
        &self,
        pos: Point<Pixels>,
        last_layout: &LastLayout,
    ) -> Option<usize> {
        let mut offset = 0;
        let mut line_top = px(0.);
        let x_offset = last_layout.alignment_offset(self.longest_width);
        for (i, line) in self.wrapped_lines.iter().enumerate() {
            let is_last = i + 1 == self.wrapped_lines.len();
            let line_bottom = line_top + last_layout.line_height;
            if pos.y >= line_top && pos.y < line_bottom {
                let mut ix = line.closest_index_for_x(pos.x - x_offset - self.line_indent(i));
                if !is_last && ix == line.text.len() {
                    // 对于软换行行，不能将光标放在行末尾。
                    let c_len = line.text.chars().last().map(|c| c.len_utf8()).unwrap_or(0);
                    ix = ix.saturating_sub(c_len);
                }
                return Some(offset + ix);
            }

            offset += line.text.len();
            line_top = line_bottom;
        }

        None
    }

    /// 获取此布局中给定位置 (x, y) 的索引。
    pub(crate) fn index_for_position(
        &self,
        pos: Point<Pixels>,
        last_layout: &LastLayout,
    ) -> Option<usize> {
        let mut offset = 0;
        let mut line_top = px(0.);
        let x_offset = last_layout.alignment_offset(self.longest_width);
        for (i, line) in self.wrapped_lines.iter().enumerate() {
            let line_bottom = line_top + last_layout.line_height;
            if pos.y >= line_top && pos.y < line_bottom {
                let ix = line.index_for_x(pos.x - x_offset - self.line_indent(i))?;
                return Some(offset + ix);
            }

            offset += line.text.len();
            line_top = line_bottom;
        }

        None
    }

    /// 获取行布局的尺寸。
    pub(crate) fn size(&self, line_height: Pixels) -> Size<Pixels> {
        let width = self
            .wrapped_lines
            .iter()
            .enumerate()
            .map(|(ix, line)| line.width() + self.line_indent(ix))
            .max()
            .unwrap_or(self.longest_width);
        size(width, self.wrapped_lines.len() * line_height)
    }

    /// 绘制行布局。
    pub(crate) fn paint(
        &self,
        pos: Point<Pixels>,
        line_height: Pixels,
        text_align: TextAlign,
        align_width: Option<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        for (ix, line) in self.wrapped_lines.iter().enumerate() {
            _ = line.paint(
                pos + point(self.line_indent(ix), ix * line_height),
                line_height,
                text_align,
                align_width,
                window,
                cx,
            );
        }

        // 绘制空白指示符
        if let Some(indicators) = self.whitespace_indicators.as_ref() {
            for (line_index, x_position, is_tab) in &self.whitespace_chars {
                let invisible = if *is_tab {
                    indicators.tab.clone()
                } else {
                    indicators.space.clone()
                };

                let origin = point(
                    pos.x + *x_position + self.line_indent(*line_index),
                    pos.y + *line_index as f32 * line_height,
                );

                _ = invisible.paint(origin, line_height, text_align, align_width, window, cx);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    use crate::{Boundary, FontFeatures, FontStyle, FontWeight, px};

    #[test]
    fn test_update() {
        let font = crate::Font {
            family: "Arial".into(),
            weight: FontWeight::default(),
            style: FontStyle::Normal,
            features: FontFeatures::default(),
            fallbacks: None,
        };

        let mut wrapper = TextWrapper::new(font, px(14.), None);
        let mut text = Rope::from(
            "Hello, 世界!\r\nThis is second line.\nThis is third line.\n这里是第 4 行。",
        );

        fn fake_wrap_line(_line: &str, _wrap_width: Pixels) -> Vec<Boundary> {
            vec![]
        }

        #[track_caller]
        fn assert_wrapper_lines(text: &Rope, wrapper: &TextWrapper, expected_lines: &[&[&str]]) {
            let mut actual_lines = vec![];
            let mut offset = 0;
            for line in wrapper.iter_lines() {
                actual_lines.push(
                    line.wrapped_lines
                        .iter()
                        .map(|range| text.slice(offset + range.start..offset + range.end))
                        .collect::<Vec<_>>(),
                );
                // +1 \n
                offset += line.len() + 1;
            }
            assert_eq!(actual_lines, expected_lines);
        }

        wrapper._update(&text, &(0..text.len()), &text, &mut fake_wrap_line);
        assert_eq!(wrapper.lines_count(), 4);
        assert_wrapper_lines(
            &text,
            &wrapper,
            &[
                &["Hello, 世界!\r"],
                &["This is second line."],
                &["This is third line."],
                &["这里是第 4 行。"],
            ],
        );

        // 在末尾添加新文本
        let range = text.len()..text.len();
        let new_text = "New text";
        text.replace(range.clone(), new_text);
        wrapper._update(&text, &range, &Rope::from(new_text), &mut fake_wrap_line);
        assert_eq!(
            text.to_string(),
            "Hello, 世界!\r\nThis is second line.\nThis is third line.\n这里是第 4 行。New text"
        );
        assert_eq!(wrapper.lines_count(), 4);
        assert_wrapper_lines(
            &text,
            &wrapper,
            &[
                &["Hello, 世界!\r"],
                &["This is second line."],
                &["This is third line."],
                &["这里是第 4 行。New text"],
            ],
        );

        // 将第一行 `Hello` 替换为 `AAA`
        let range = 0..5;
        let new_text = "AAA";
        text.replace(range.clone(), new_text);
        wrapper._update(&text, &range, &Rope::from(new_text), &mut fake_wrap_line);
        assert_eq!(
            text.to_string(),
            "AAA, 世界!\r\nThis is second line.\nThis is third line.\n这里是第 4 行。New text"
        );
        assert_eq!(wrapper.lines_count(), 4);
        assert_wrapper_lines(
            &text,
            &wrapper,
            &[
                &["AAA, 世界!\r"],
                &["This is second line."],
                &["This is third line."],
                &["这里是第 4 行。New text"],
            ],
        );

        // 移除第二行
        let start_offset = text.line_start_offset(1);
        let end_offset = text.line_end_offset(1);
        let range = start_offset..end_offset + 1;
        text.replace(range.clone(), "");
        wrapper._update(&text, &range, &Rope::from(""), &mut fake_wrap_line);
        assert_eq!(
            text.to_string(),
            "AAA, 世界!\r\nThis is third line.\n这里是第 4 行。New text"
        );
        assert_eq!(wrapper.lines_count(), 3);
        assert_wrapper_lines(
            &text,
            &wrapper,
            &[
                &["AAA, 世界!\r"],
                &["This is third line."],
                &["这里是第 4 行。New text"],
            ],
        );

        // 将前 2 行替换为新行
        let range = text.line_start_offset(0)..text.line_end_offset(1) + 1;
        let new_text = "This is a new line.\nThis is new line 2.\n";
        text.replace(range.clone(), new_text);
        wrapper._update(&text, &range, &Rope::from(new_text), &mut fake_wrap_line);
        assert_eq!(
            text.to_string(),
            "This is a new line.\nThis is new line 2.\n这里是第 4 行。New text"
        );
        assert_eq!(wrapper.lines_count(), 3);
        assert_wrapper_lines(
            &text,
            &wrapper,
            &[
                &["This is a new line."],
                &["This is new line 2."],
                &["这里是第 4 行。New text"],
            ],
        );

        // 在末尾添加新行
        let range = text.len()..text.len();
        let new_text = "\nThis is a new line at the end.";
        text.replace(range.clone(), new_text);
        wrapper._update(&text, &range, &Rope::from(new_text), &mut fake_wrap_line);
        assert_eq!(
            text.to_string(),
            "This is a new line.\nThis is new line 2.\n这里是第 4 行。New text\nThis is a new line at the end."
        );
        assert_eq!(wrapper.lines_count(), 4);
        assert_wrapper_lines(
            &text,
            &wrapper,
            &[
                &["This is a new line."],
                &["This is new line 2."],
                &["这里是第 4 行。New text"],
                &["This is a new line at the end."],
            ],
        );

        // 在开头添加新行
        let range = 0..0;
        let new_text = "This is a new line at the beginning.\n";
        text.replace(range.clone(), new_text);
        wrapper._update(&text, &range, &Rope::from(new_text), &mut fake_wrap_line);
        assert_eq!(
            text.to_string(),
            "This is a new line at the beginning.\nThis is a new line.\nThis is new line 2.\n这里是第 4 行。New text\nThis is a new line at the end."
        );
        assert_eq!(wrapper.lines_count(), 5);
        assert_wrapper_lines(
            &text,
            &wrapper,
            &[
                &["This is a new line at the beginning."],
                &["This is a new line."],
                &["This is new line 2."],
                &["这里是第 4 行。New text"],
                &["This is a new line at the end."],
            ],
        );

        // 移除所有到至少一行在 `lines` 中。
        let range = 0..text.len();
        let new_text = "";
        text.replace(range.clone(), new_text);
        wrapper._update(&text, &range, &Rope::from(new_text), &mut fake_wrap_line);
        assert_eq!(text.to_string(), "");
        assert_eq!(wrapper.lines_count(), 1);
        assert_eq!(wrapper.line(0).unwrap().wrapped_lines.as_slice(), [0..0]);

        // 测试 update_all
        let range = 0..text.len();
        let new_text = "This is a full text.\nThis is a second line.";
        text.replace(range.clone(), new_text);
        wrapper._update(&text, &range, &text, &mut fake_wrap_line);
        assert_eq!(
            text.to_string(),
            "This is a full text.\nThis is a second line."
        );
        assert_eq!(wrapper.lines_count(), 2);
    }

    fn test_font() -> crate::Font {
        crate::Font {
            family: "Arial".into(),
            weight: FontWeight::default(),
            style: FontStyle::Normal,
            features: FontFeatures::default(),
            fallbacks: None,
        }
    }

    /// 当先前最长的行缩小时，最长行摘要保持精确。
    #[test]
    fn test_longest_row_after_shrink() {
        let mut wrapper = TextWrapper::new(test_font(), px(14.), None);
        let mut text = Rope::from("aa\nthis is the longest line\nbb");
        wrapper._update(&text, &(0..text.len()), &text, &mut |_, _| vec![]);
        assert_eq!(wrapper.longest_row(), 1);

        // 缩小第 1 行，使第 0 行现在最长。
        let start = text.line_start_offset(0);
        let end = text.line_end_offset(0);
        let range = start..end;
        let new_text = "a very very long first line now";
        text.replace(range.clone(), new_text);
        wrapper._update(&text, &range, &Rope::from(new_text), &mut |_, _| vec![]);
        assert_eq!(wrapper.longest_row(), 0);
    }

    /// 编辑最后一行并删除所有内容必须保持树一致。
    #[test]
    fn test_edit_last_line_and_full_delete() {
        let mut wrapper = TextWrapper::new(test_font(), px(14.), None);
        let mut text = Rope::from("one\ntwo\nthree");
        wrapper._update(&text, &(0..text.len()), &text, &mut |_, _| vec![]);
        assert_eq!(wrapper.lines_count(), 3);

        // 仅替换最后一行。
        let start = text.line_start_offset(2);
        let range = start..text.len();
        let new_text = "THREE EDITED";
        text.replace(range.clone(), new_text);
        wrapper._update(&text, &range, &Rope::from(new_text), &mut |_, _| vec![]);
        assert_eq!(wrapper.lines_count(), 3);
        assert_eq!(wrapper.line(2).unwrap().len(), "THREE EDITED".len());

        // 删除所有内容。
        let range = 0..text.len();
        text.replace(range.clone(), "");
        wrapper._update(&text, &range, &Rope::from(""), &mut |_, _| vec![]);
        assert_eq!(wrapper.lines_count(), 1);
        assert_eq!(wrapper.len(), 1);
        assert_eq!(wrapper.line(0).unwrap().wrapped_lines.as_slice(), [0..0]);
    }

    #[test]
    fn test_wrap_row_buffer_line_boundaries() {
        let mut wrapper = TextWrapper::new(test_font(), px(14.), None);
        wrapper.text = Rope::from("aa\nbbbb\nc");
        wrapper.lines = SumTree::from_iter(
            vec![
                LineItem {
                    len: 2,
                    indent: 0,
                    wrapped_lines: smallvec::smallvec![0..2],
                },
                LineItem {
                    len: 4,
                    indent: 0,
                    wrapped_lines: smallvec::smallvec![0..2, 2..4],
                },
                LineItem {
                    len: 1,
                    indent: 0,
                    wrapped_lines: smallvec::smallvec![0..1],
                },
            ],
            &(),
        );

        assert_eq!(wrapper.lines_count(), 3);
        assert_eq!(wrapper.len(), 4);

        assert_eq!(wrapper.buffer_line_to_first_wrap_row(0), 0);
        assert_eq!(wrapper.buffer_line_to_first_wrap_row(1), 1);
        assert_eq!(wrapper.buffer_line_to_first_wrap_row(2), 3);
        assert_eq!(wrapper.buffer_line_to_first_wrap_row(3), 4);

        assert_eq!(wrapper.buffer_line_to_wrap_row_range(0), 0..1);
        assert_eq!(wrapper.buffer_line_to_wrap_row_range(1), 1..3);
        assert_eq!(wrapper.buffer_line_to_wrap_row_range(2), 3..4);
        assert_eq!(wrapper.buffer_line_to_wrap_row_range(3), 4..4);

        assert_eq!(wrapper.wrap_row_to_buffer_line(0), 0);
        assert_eq!(wrapper.wrap_row_to_buffer_line(1), 1);
        assert_eq!(wrapper.wrap_row_to_buffer_line(2), 1);
        assert_eq!(wrapper.wrap_row_to_buffer_line(3), 2);
        assert_eq!(wrapper.wrap_row_to_buffer_line(4), 2);
    }

    #[test]
    fn test_wrap_row_queries_after_incremental_splice() {
        let mut wrapper = TextWrapper::new(test_font(), px(14.), Some(px(10.)));
        let mut text = Rope::from("aa\nbbbb\nc");
        let mut fake_wrap_line = |line: &str, _wrap_width: Pixels| {
            if line.len() > 2 {
                vec![Boundary {
                    ix: 2,
                    next_indent: 0,
                }]
            } else {
                vec![]
            }
        };

        wrapper._update(&text, &(0..text.len()), &text, &mut fake_wrap_line);
        assert_eq!(wrapper.buffer_line_to_wrap_row_range(0), 0..1);
        assert_eq!(wrapper.buffer_line_to_wrap_row_range(1), 1..3);
        assert_eq!(wrapper.buffer_line_to_wrap_row_range(2), 3..4);

        let range = text.line_start_offset(1)..text.line_end_offset(1);
        let new_text = "dd\neeee";
        text.replace(range.clone(), new_text);
        wrapper._update(&text, &range, &Rope::from(new_text), &mut fake_wrap_line);

        assert_eq!(wrapper.lines_count(), 4);
        assert_eq!(wrapper.len(), 5);
        assert_eq!(wrapper.buffer_line_to_wrap_row_range(0), 0..1);
        assert_eq!(wrapper.buffer_line_to_wrap_row_range(1), 1..2);
        assert_eq!(wrapper.buffer_line_to_wrap_row_range(2), 2..4);
        assert_eq!(wrapper.buffer_line_to_wrap_row_range(3), 4..5);
        assert_eq!(wrapper.wrap_row_to_buffer_line(0), 0);
        assert_eq!(wrapper.wrap_row_to_buffer_line(1), 1);
        assert_eq!(wrapper.wrap_row_to_buffer_line(2), 2);
        assert_eq!(wrapper.wrap_row_to_buffer_line(3), 2);
        assert_eq!(wrapper.wrap_row_to_buffer_line(4), 3);
    }

    #[test]
    fn test_line_layout() {
        let mut line_layout = LineLayout::new();

        let line1 = ShapedLine::default().with_len(100);
        let line2 = ShapedLine::default().with_len(50);
        let wrapped_lines = smallvec::smallvec![line1, line2];
        line_layout.set_wrapped_lines(wrapped_lines);
        assert_eq!(line_layout.len(), 150);
        assert_eq!(line_layout.wrapped_lines.len(), 2);
    }

    #[test]
    fn test_position_for_index_prefers_first_leading_empty_visual_line() {
        let mut line_layout = LineLayout::new();
        line_layout.set_wrapped_lines(smallvec::smallvec![
            ShapedLine::default(),
            ShapedLine::default(),
            ShapedLine::default().with_len(3),
        ]);

        let last_layout = LastLayout {
            visible_range: 0..1,
            visible_buffer_lines: vec![0],
            visible_line_byte_offsets: vec![0],
            visible_top: px(0.),
            visible_range_offset: 0..0,
            lines: Rc::new(vec![]),
            line_height: px(20.),
            wrap_width: None,
            wrapping_indent: WrappingIndent::Same,
            line_number_width: px(0.),
            cursor_bounds: None,
            text_align: crate::TextAlign::Left,
            content_width: px(0.),
        };

        // 索引 0 应映射到第一个空视觉行。
        let pos = line_layout.position_for_index(0, &last_layout, false).unwrap();
        assert_eq!(pos.y, px(0.));

        // 索引 3（字节 3）应映射到第三行。
        let pos = line_layout.position_for_index(3, &last_layout, false).unwrap();
        assert_eq!(pos.y, px(40.));
    }
}