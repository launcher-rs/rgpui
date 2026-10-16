//! 调试帧覆盖层：把帧耗时统计直接画进场景里。
//!
//! 它绕过布局、文本与视图失效机制 —— 不参与元素树、也不触发新的绘制请求，
//! 因此可以常驻显示而不会把自己变成一个新的绘制来源（这正是它不会导致无限帧
//! 循环的原因）。面板位于窗口右上角，用一套 5x7 点阵字形绘制，显示最近一帧的
//! 耗时，并可切换显示百分位统计与累计帧数。
//!
//! 由 [`Window::cycle_debug_overlay`](crate::Window::cycle_debug_overlay) 在
//! 「隐藏 → 最小 → 完整」之间循环切换。

use std::{collections::VecDeque, time::Duration};

use crate::{
    BorderStyle, Bounds, ContentMask, Corners, Edges, Hsla, Pixels, Quad, ScaledPixels, Scene,
    Size, point, rgba, size,
};

/// 调试帧覆盖层的显示模式。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum DebugFrameOverlayMode {
    /// 隐藏（默认），零开销。
    #[default]
    Hidden,
    /// 只显示最近一帧的耗时。
    Minimal,
    /// 显示最近一帧耗时、百分位统计与累计帧数。
    Full,
}

impl DebugFrameOverlayMode {
    /// 返回「隐藏 → 最小 → 完整」循环中的下一个模式。
    pub fn next(self) -> Self {
        match self {
            Self::Hidden => Self::Minimal,
            Self::Minimal => Self::Full,
            Self::Full => Self::Hidden,
        }
    }
}

/// 用于百分位统计而保留的最近绘制耗时样本数。
const MAX_SAMPLES: usize = 1000;

/// 点阵字形的宽（列数）。
const GLYPH_WIDTH: usize = 5;
/// 点阵字形的高（行数）。
const GLYPH_HEIGHT: usize = 7;
/// 字符步进与行步进，单位为「单元格」。
const CHAR_ADVANCE: f32 = (GLYPH_WIDTH + 1) as f32;
const LINE_ADVANCE: f32 = (GLYPH_HEIGHT + 2) as f32;
/// 面板边缘与文字之间的内边距，单位为单元格。
const PANEL_PADDING: f32 = 2.0;
/// 面板与窗口边缘之间的外边距，单位为单元格。
const PANEL_MARGIN: f32 = 4.0;
/// 单个字形单元格的边长（逻辑像素）。
const CELL_SIZE: f32 = 2.0;

/// 文字颜色（亮绿色）。
fn text_color() -> Hsla {
    rgba(0x33ff33ff).into()
}

/// 面板底色（半透明黑）。
fn panel_color() -> Hsla {
    rgba(0x000000aa).into()
}

/// 帧覆盖层状态：模式、样本与累计帧数。
pub(crate) struct DebugFrameOverlay {
    /// 当前显示模式。
    mode: DebugFrameOverlayMode,
    /// 最近的绘制耗时样本。
    draw_durations: VecDeque<Duration>,
    /// 自创建以来记录的帧数（不受模式切换与 [`reset_stats`](Self::reset_stats) 影响）。
    total_frame_count: u64,
}

impl DebugFrameOverlay {
    /// 创建处于隐藏模式的覆盖层。
    pub(crate) fn new() -> Self {
        Self {
            mode: DebugFrameOverlayMode::default(),
            draw_durations: VecDeque::new(),
            total_frame_count: 0,
        }
    }

    /// 返回当前显示模式。
    pub(crate) fn mode(&self) -> DebugFrameOverlayMode {
        self.mode
    }

    /// 设置显示模式。
    pub(crate) fn set_mode(&mut self, mode: DebugFrameOverlayMode) {
        self.mode = mode;
    }

    /// 清空耗时样本，重新开始百分位统计窗口；累计帧数保持不变。
    pub(crate) fn reset_stats(&mut self) {
        self.draw_durations.clear();
    }

    /// 覆盖层当前是否可见（非隐藏模式）。
    pub(crate) fn is_enabled(&self) -> bool {
        self.mode != DebugFrameOverlayMode::Hidden
    }

    /// 记录一帧的绘制耗时。
    pub(crate) fn record_frame(&mut self, draw_duration: Duration) {
        self.total_frame_count += 1;
        if self.draw_durations.len() >= MAX_SAMPLES {
            self.draw_durations.pop_front();
        }
        self.draw_durations.push_back(draw_duration);
    }

    /// 把读面板绘制进场景。绕过布局与文本系统，只插入四边形图元。
    pub(crate) fn paint(&self, scene: &mut Scene, viewport_size: Size<Pixels>, scale_factor: f32) {
        if !self.is_enabled() {
            return;
        }

        let lines = self.lines();
        let max_line_chars = lines.iter().map(|line| line.len()).max().unwrap_or(0);
        // 保证每个单元格至少一个物理像素，否则在小数缩放因子下文字会糊成一团。
        let cell = (CELL_SIZE * scale_factor).max(1.0);

        let panel_width = cell * (max_line_chars as f32 * CHAR_ADVANCE + 2.0 * PANEL_PADDING);
        let panel_height = cell * (lines.len() as f32 * LINE_ADVANCE + 2.0 * PANEL_PADDING);
        let viewport = viewport_size.scale(scale_factor);
        let panel_left = viewport.width.0 - panel_width - cell * PANEL_MARGIN;
        let panel_top = cell * PANEL_MARGIN;

        let content_mask = ContentMask {
            bounds: Bounds {
                origin: point(ScaledPixels(0.), ScaledPixels(0.)),
                size: viewport,
            },
        };

        scene.insert_primitive(solid_quad(
            scaled_bounds(panel_left, panel_top, panel_width, panel_height),
            &content_mask,
            panel_color(),
        ));

        let text_color = text_color();
        for (line_index, line) in lines.iter().enumerate() {
            let line_top = panel_top + cell * (PANEL_PADDING + line_index as f32 * LINE_ADVANCE);
            for (char_index, character) in line.chars().enumerate() {
                let Some(rows) = glyph(character) else {
                    continue;
                };
                let glyph_left =
                    panel_left + cell * (PANEL_PADDING + char_index as f32 * CHAR_ADVANCE);
                for (row_index, row) in rows.iter().enumerate() {
                    let row_top = line_top + cell * row_index as f32;
                    // 把每一行里连续点亮的单元格合并成单个四边形，减少图元数量。
                    let mut column = 0;
                    while column < GLYPH_WIDTH {
                        if row & (1 << (GLYPH_WIDTH - 1 - column)) == 0 {
                            column += 1;
                            continue;
                        }
                        let run_start = column;
                        while column < GLYPH_WIDTH && row & (1 << (GLYPH_WIDTH - 1 - column)) != 0 {
                            column += 1;
                        }
                        scene.insert_primitive(solid_quad(
                            scaled_bounds(
                                glyph_left + cell * run_start as f32,
                                row_top,
                                cell * (column - run_start) as f32,
                                cell,
                            ),
                            &content_mask,
                            text_color,
                        ));
                    }
                }
            }
        }
    }

    /// 按当前模式生成要显示的各行文本。
    fn lines(&self) -> Vec<String> {
        let current = self.draw_durations.back().copied();
        match self.mode {
            DebugFrameOverlayMode::Hidden => Vec::new(),
            DebugFrameOverlayMode::Minimal => vec![format_ms(current)],
            DebugFrameOverlayMode::Full => {
                let mut sorted: Vec<Duration> = self.draw_durations.iter().copied().collect();
                sorted.sort_unstable();
                let percentile = |numerator: usize| {
                    (!sorted.is_empty()).then(|| sorted[(sorted.len() - 1) * numerator / 100])
                };
                // 超过五位数会把列对齐撑坏，所以饱和显示为 `LOTS`。
                let frame_count = if self.total_frame_count > 99_999 {
                    "LOTS".to_string()
                } else {
                    self.total_frame_count.to_string()
                };
                // 标签补齐到统一宽度，让定宽的耗时值每行都从同一列开始。
                vec![
                    format!("CUR {}", format_ms(current)),
                    format!("1%  {}", format_ms(percentile(99))),
                    format!("10% {}", format_ms(percentile(90))),
                    format!("MAX {}", format_ms(sorted.last().copied())),
                    format!("FRAMES {frame_count:>5}"),
                ]
            }
        }
    }
}

/// 格式化为 `abc.d MS`：整数部分最多三位、保留一位小数，向右对齐（用空格而非
/// 零补齐），这样堆叠的多行读数能对齐到同一列。
fn format_ms(duration: Option<Duration>) -> String {
    match duration {
        Some(duration) => {
            let ms = duration.as_secs_f32() * 1000.0;
            format!("{ms:>5.1} MS")
        }
        None => "   -- MS".into(),
    }
}

/// 以缩放像素构造边界矩形。
fn scaled_bounds(left: f32, top: f32, width: f32, height: f32) -> Bounds<ScaledPixels> {
    Bounds {
        origin: point(ScaledPixels(left), ScaledPixels(top)),
        size: size(ScaledPixels(width), ScaledPixels(height)),
    }
}

/// 构造一个纯色无边框的四边形图元。
fn solid_quad(
    bounds: Bounds<ScaledPixels>,
    content_mask: &ContentMask<ScaledPixels>,
    color: Hsla,
) -> Quad {
    Quad {
        order: 0,
        border_style: BorderStyle::Solid,
        bounds,
        content_mask: *content_mask,
        background: color.into(),
        border_color: Hsla::default(),
        corner_radii: Corners::default(),
        border_widths: Edges::default(),
        ..Default::default()
    }
}

/// 返回给定字符的 5x7 点阵：每个 `u8` 表示一行，最高有效位在最左。
/// 只定义了覆盖层读数实际会用到的字符。
fn glyph(character: char) -> Option<[u8; GLYPH_HEIGHT]> {
    Some(match character {
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '%' => [
            0b11001, 0b11001, 0b00010, 0b00100, 0b01000, 0b10011, 0b10011,
        ],
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::px;

    /// 读数里可能出现的字符都必须有点阵，否则会静默渲染成空白。
    #[test]
    fn all_rendered_characters_have_glyphs() {
        let mut overlay = DebugFrameOverlay::new();
        overlay.set_mode(DebugFrameOverlayMode::Full);

        let mut lines = Vec::new();
        for duration in [
            Duration::ZERO,
            Duration::from_micros(1),
            Duration::from_micros(8_333),
            Duration::from_millis(123),
            Duration::from_secs(2),
        ] {
            overlay.record_frame(duration);
            lines.extend(overlay.lines());
        }
        // 超过五位数的帧数显示为 `LOTS`。
        overlay.total_frame_count = 100_000;
        lines.extend(overlay.lines());

        // 已启用但还没有样本时显示占位符。
        let mut empty = DebugFrameOverlay::new();
        empty.set_mode(DebugFrameOverlayMode::Full);
        lines.extend(empty.lines());

        for line in lines {
            for character in line.chars() {
                assert!(
                    character == ' ' || glyph(character).is_some(),
                    "缺少字形：{character:?}（出现在 {line:?} 中）"
                );
            }
        }
    }

    /// 百分位按耗时如实上报。
    #[test]
    fn percentile_lows_are_reported_as_times() {
        let mut overlay = DebugFrameOverlay::new();
        overlay.set_mode(DebugFrameOverlayMode::Full);
        for milliseconds in 1..=100 {
            overlay.record_frame(Duration::from_millis(milliseconds));
        }
        let lines = overlay.lines();
        assert_eq!(lines[0], "CUR 100.0 MS");
        assert_eq!(lines[1], "1%   99.0 MS");
        assert_eq!(lines[2], "10%  90.0 MS");
        assert_eq!(lines[3], "MAX 100.0 MS");
        assert_eq!(lines[4], "FRAMES   100");
    }

    /// 重置只清样本，累计帧数保留。
    #[test]
    fn reset_clears_durations_but_keeps_frame_count() {
        let mut overlay = DebugFrameOverlay::new();
        overlay.set_mode(DebugFrameOverlayMode::Full);
        for _ in 0..10 {
            overlay.record_frame(Duration::from_millis(10));
        }
        overlay.reset_stats();
        let lines = overlay.lines();
        assert_eq!(lines[0], "CUR    -- MS");
        assert_eq!(lines[3], "MAX    -- MS");
        assert_eq!(lines[4], "FRAMES    10");
        overlay.record_frame(Duration::from_millis(20));
        let lines = overlay.lines();
        assert_eq!(lines[0], "CUR  20.0 MS");
        assert_eq!(lines[4], "FRAMES    11");
    }

    /// 帧数跨模式切换持续累加。
    #[test]
    fn frame_count_accumulates_across_mode_changes() {
        let mut overlay = DebugFrameOverlay::new();
        for _ in 0..3 {
            overlay.record_frame(Duration::from_millis(10));
        }
        overlay.set_mode(DebugFrameOverlayMode::Minimal);
        assert_eq!(overlay.lines(), vec![" 10.0 MS".to_string()]);
        overlay.set_mode(DebugFrameOverlayMode::Full);
        overlay.record_frame(Duration::from_millis(10));
        assert_eq!(overlay.lines()[4], "FRAMES     4");
        overlay.set_mode(DebugFrameOverlayMode::Hidden);
        overlay.record_frame(Duration::from_millis(10));
        overlay.set_mode(DebugFrameOverlayMode::Full);
        assert_eq!(overlay.lines()[4], "FRAMES     5");
    }

    /// 帧数右对齐，超过五位数后饱和为 `LOTS`。
    #[test]
    fn frame_count_is_right_aligned_and_saturates() {
        let mut overlay = DebugFrameOverlay::new();
        overlay.set_mode(DebugFrameOverlayMode::Full);
        overlay.record_frame(Duration::from_millis(10));
        assert_eq!(overlay.lines()[4], "FRAMES     1");
        overlay.total_frame_count = 99_999;
        assert_eq!(overlay.lines()[4], "FRAMES 99999");
        overlay.total_frame_count = 100_000;
        assert_eq!(overlay.lines()[4], "FRAMES  LOTS");
    }

    /// 打开时立刻显示上一帧的耗时。
    #[test]
    fn toggling_on_shows_previous_frame_immediately() {
        let mut overlay = DebugFrameOverlay::new();
        overlay.record_frame(Duration::from_millis(10));
        overlay.set_mode(DebugFrameOverlayMode::Minimal);
        assert_eq!(overlay.lines(), vec![" 10.0 MS".to_string()]);
    }

    /// 模式循环：隐藏 → 最小 → 完整 → 隐藏。
    #[test]
    fn mode_cycles_and_default_is_hidden() {
        assert_eq!(
            DebugFrameOverlayMode::default(),
            DebugFrameOverlayMode::Hidden
        );
        let hidden = DebugFrameOverlayMode::Hidden;
        assert_eq!(hidden.next(), DebugFrameOverlayMode::Minimal);
        assert_eq!(hidden.next().next(), DebugFrameOverlayMode::Full);
        assert_eq!(hidden.next().next().next(), DebugFrameOverlayMode::Hidden);
        assert!(!DebugFrameOverlay::new().is_enabled());
    }

    /// 样本上限：超过后丢弃最旧的。
    #[test]
    fn samples_are_capped() {
        let mut overlay = DebugFrameOverlay::new();
        for i in 0..(MAX_SAMPLES + 10) {
            overlay.record_frame(Duration::from_micros(i as u64));
        }
        assert_eq!(overlay.draw_durations.len(), MAX_SAMPLES);
    }

    /// 绘制：隐藏时不插入任何图元；启用后插入面板底色与点阵字形四边形。
    ///
    /// 这是「绕过布局/文本系统」的落地校验：覆盖层只往场景里塞四边形，
    /// 不产生任何文本图元或元素树节点。
    #[test]
    fn paint_inserts_quads_only_when_enabled() {
        let viewport = size(px(800.0), px(600.0));

        let hidden = DebugFrameOverlay::new();
        let mut scene = Scene::default();
        hidden.paint(&mut scene, viewport, 1.0);
        assert!(scene.quads.is_empty(), "隐藏模式不应插入任何图元");
        assert!(scene.monochrome_sprites.is_empty());
        assert!(scene.polychrome_sprites.is_empty());

        let mut minimal = DebugFrameOverlay::new();
        minimal.set_mode(DebugFrameOverlayMode::Minimal);
        minimal.record_frame(Duration::from_millis(16));
        let mut scene = Scene::default();
        minimal.paint(&mut scene, viewport, 1.0);
        let minimal_quads = scene.quads.len();
        // 至少包含面板底色 + 若干字形四边形。
        assert!(minimal_quads > 1, "得到 {minimal_quads} 个四边形");
        assert!(scene.monochrome_sprites.is_empty(), "不应产生文本精灵");

        let mut full = DebugFrameOverlay::new();
        full.set_mode(DebugFrameOverlayMode::Full);
        full.record_frame(Duration::from_millis(16));
        let mut scene = Scene::default();
        full.paint(&mut scene, viewport, 1.0);
        assert!(
            scene.quads.len() > minimal_quads,
            "完整模式五行读数应比最小模式多：完整 {} vs 最小 {minimal_quads}",
            scene.quads.len()
        );
    }
}
