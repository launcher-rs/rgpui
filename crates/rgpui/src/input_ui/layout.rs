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

/// 行顶纵向坐标累加器（f64 累加，对标 gpui-kit `e028806a` 的 f64 垂直坐标）。
///
/// `Pixels` 是 f32：超过 2^24 px（约 1670 万）后整数精度不再连续，行顶 y 逐行
/// 累加会漂移。editor 内部（文本/装饰背景/选区/行号/活动行/折叠图标/gutter
/// 标记/行内提示）的行顶 y 统一用它累加，绘制输出时才转回 `Pixels`（单次
/// 舍入），长文档下逐行坐标不再跨行累计误差。起步用 `visible_top`（单次乘法，
/// 误差只有 1 ulp）或 `px(0.)`；滚动偏移与行长度不经这里，维持原口径。
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RowTopY(f64);

impl RowTopY {
    /// 从给定行顶开始累加。
    pub(crate) fn new(top: Pixels) -> Self {
        Self(f64::from(top.0))
    }

    /// 累加一行的高度。
    pub(crate) fn advance(&mut self, height: Pixels) {
        self.0 += f64::from(height.0);
    }

    /// 累加 `rows` 行等高行高（折叠行组、行号行组一次过）。
    pub(crate) fn advance_rows(&mut self, rows: usize, line_height: Pixels) {
        self.0 += rows as f64 * f64::from(line_height.0);
    }

    /// 读出当前行顶作为绘制坐标（转回 f32，单次舍入）。
    pub(crate) fn pixels(&self) -> Pixels {
        px(self.0 as f32)
    }
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
            // 输入框不做段落方向重排，`Start` / `End` 按 LTR 处理。
            TextAlign::Left | TextAlign::Start => px(0.),
            TextAlign::Center => (self.content_width - line_width).half().max(px(0.)),
            TextAlign::Right | TextAlign::End => (self.content_width - line_width).max(px(0.)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `RowTopY` 超长文档精度（对标 gpui-kit `e028806a`）：
    /// 100 万行 × 17.3px ≈ 1730 万 px，越过 2^24 的 f32 整数精度界。
    /// f64 累加输出与精确乘积只差一次 f32 舍入（该量级 ulp = 2）；
    /// 朴素 f32 逐行累加因 17.3 不是 ulp 的整数倍、每步都引入舍入误差，
    /// 跨行累计后明显更差。
    #[test]
    fn test_row_top_y_precision_beyond_f32_integer() {
        let line_height = px(17.3);
        let rows: usize = 1_000_000;
        let mut acc = RowTopY::new(px(0.));
        let mut naive = px(0.);
        for _ in 0..rows {
            acc.advance(line_height);
            naive += line_height;
        }
        let exact = f64::from(line_height.0) * rows as f64;
        let acc_err = (f64::from(acc.pixels().0) - exact).abs();
        let naive_err = (f64::from(naive.0) - exact).abs();
        assert!(
            acc_err <= 2.0,
            "f64 累加器误差 {acc_err} 不应超过一个 f32 ulp"
        );
        assert!(
            naive_err > acc_err,
            "朴素 f32 累加误差 {naive_err} 应大于累加器误差 {acc_err}"
        );
        // 整组累加与逐行累加等价。
        let mut group = RowTopY::new(px(0.));
        group.advance_rows(rows, line_height);
        assert!(
            (f64::from(group.pixels().0) - exact).abs() <= 2.0,
            "advance_rows 与逐行 advance 应等价"
        );
    }
}
