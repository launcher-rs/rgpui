use std::ops::Range;
use std::rc::Rc;

use crate::{Bounds, Half, Pixels, ShapedLine, TextAlign, px};

use super::display_map::WrappingIndent;
use super::display_map::text_wrapper::LineLayout;

/// 空白指示符，用于渲染空格和制表符。
#[derive(Clone, Default)]
pub(crate) struct WhitespaceIndicators {
    /// 空格字符指示符（•）的塑形行
    pub(crate) space: ShapedLine,
    /// 制表符字符指示符（→）的塑形行
    pub(crate) tab: ShapedLine,
}

/// 上次布局信息，用于渲染和命中测试。
#[derive(Clone)]
pub(crate) struct LastLayout {
    /// 视口中可见行的范围（无换行），值为行（0 起始）索引。
    /// 这是包含所有可见行的 buffer 行范围。
    pub(super) visible_range: Range<usize>,
    /// 可见 buffer 行索引列表（排除隐藏/折叠的行）。
    /// 与 `lines` 并行：`visible_buffer_lines[i]` 是 `lines[i]` 的 buffer 行索引。
    pub(super) visible_buffer_lines: Vec<usize>,
    /// 每个可见 buffer 行在 Rope 中的字节偏移（与 visible_buffer_lines/lines 并行）。
    pub(super) visible_line_byte_offsets: Vec<usize>,
    /// 滚动视口中第一个可见行的顶部位置。
    pub(super) visible_top: Pixels,
    /// 可见行的字节偏移范围。
    pub(super) visible_range_offset: Range<usize>,
    /// 最后的布局行（仅可见行，隐藏行无空项）。
    pub(super) lines: Rc<Vec<LineLayout>>,
    /// 文本布局的行高，可能在 InputElement 绘制时改变。
    pub(super) line_height: Pixels,
    /// 文本布局的换行宽度，可能在 InputElement 绘制时改变。
    pub(super) wrap_width: Option<Pixels>,
    /// 文本布局的续行缩进模式，可能在 InputElement 绘制时改变。
    pub(super) wrapping_indent: WrappingIndent,
    /// 文本布局的行号区域宽度，如果无行号则为 0px。
    pub(super) line_number_width: Pixels,
    /// 光标的像素位置（顶部、左侧）。
    pub(super) cursor_bounds: Option<Bounds<Pixels>>,
    /// 文本布局的对齐方式。
    pub(super) text_align: TextAlign,
    /// 文本布局的内容宽度。
    pub(super) content_width: Pixels,
}

impl LastLayout {
    /// 获取给定 buffer 行（0 起始）的行布局。
    ///
    /// 对 `visible_buffer_lines` 使用二分查找。
    /// 如果该行不可见（超出范围或折叠）则返回 None。
    pub(crate) fn line(&self, row: usize) -> Option<&LineLayout> {
        let pos = self.visible_buffer_lines.binary_search(&row).ok()?;
        self.lines.get(pos)
    }

    /// 获取给定行宽的对齐偏移。
    pub(super) fn alignment_offset(&self, line_width: Pixels) -> Pixels {
        match self.text_align {
            TextAlign::Left => px(0.),
            TextAlign::Center => (self.content_width - line_width).half().max(px(0.)),
            TextAlign::Right => (self.content_width - line_width).max(px(0.)),
        }
    }
}
