use crate::collections::HashMap;
use crate::{FontId, Pixels, SharedString, TextRun, TextSystem, px};
use std::{borrow::Cow, sync::Arc};
use unicode_segmentation::UnicodeSegmentation;

/// 确定从开头、结尾还是中间截断文本。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TruncateFrom {
    /// 从开头截断文本。
    Start,
    /// 从结尾截断文本。
    End,
    /// 从中间截断文本，保留开头和结尾。
    Middle,
}

/// RGPUI 行包装器，用于将文本行包装到给定宽度。
pub struct LineWrapper {
    text_system: Arc<TextSystem>,
    pub(crate) font_id: FontId,
    pub(crate) font_size: Pixels,
    char_widths: CharWidthCache,
    /// 重建的行文本（fragment 拼接，元素占位为等长的 `a`）：UAX #14 断点与
    /// 字素边界都按这份文本的字节偏移计算。跨调用复用以避免重复分配。
    text_buffer: String,
}

/// 单字体单字号下的字符宽度缓存。
///
/// 池里只保存这份缓存而不再保存整个 `LineWrapper`：包装器持有
/// `Arc<TextSystem>`，池又挂在 `TextSystem` 上，放整个包装器会让
/// 文本系统永远无法回收。
pub(crate) struct CharWidthCache {
    ascii: [Option<Pixels>; 128],
    other: HashMap<char, Pixels>,
}

impl Default for CharWidthCache {
    fn default() -> Self {
        Self {
            ascii: [None; 128],
            other: HashMap::default(),
        }
    }
}

impl LineWrapper {
    /// 可应用于一行的最大缩进量。
    pub const MAX_INDENT: u32 = 256;

    pub(crate) fn new(
        font_id: FontId,
        font_size: Pixels,
        text_system: Arc<TextSystem>,
        char_widths: CharWidthCache,
    ) -> Self {
        Self {
            text_system,
            font_id,
            font_size,
            char_widths,
            text_buffer: String::new(),
        }
    }

    /// 取出字符宽度缓存，供包装器池复用。
    pub(crate) fn into_char_widths(self) -> CharWidthCache {
        self.char_widths
    }

    /// 使用此包装器的字体和字体大小将文本行包装到给定宽度。
    pub fn wrap_line<'a>(
        &'a mut self,
        fragments: &'a [LineFragment],
        wrap_width: Pixels,
    ) -> impl Iterator<Item = Boundary> + 'a {
        self.wrap_line_measured(fragments, wrap_width)
            .segments
            .into_iter()
            .map(|segment| segment.boundary)
    }

    /// 与 [`LineWrapper::wrap_line`] 同算法，但一次性收集全部断点，并给出
    /// 该行换行结构保持稳定的 `wrap_width` 半开区间 `[valid_from, valid_to)`：
    /// `wrap_width` 落在区间内时，断点的位置与数量完全不变（增量重换行的依据）。
    ///
    /// 每个断点的下界 = 断点字符**计入前**的累计宽度（低于它断点会前移），
    /// 上界 = 触发超宽的字符计入后的累计宽度（达到它断点后移）；
    /// 行级下界再并入最后一个断点之后尾段的累计宽度（防止尾段冒出新增断点）；
    /// 未换行的行下界即整行总宽、上界为 [`Pixels::MAX`]。
    pub fn wrap_line_measured(
        &mut self,
        fragments: &[LineFragment],
        wrap_width: Pixels,
    ) -> WrapMeasurement {
        // 重建行文本：元素占位为等长的 `a`（词字符），使内联元素与相邻单词
        // 在 UAX #14 眼中粘结成段（对齐上游 f0210f1621）。
        // 文本缓冲临时取出：`width_for_char` 需要 `&mut self`，而扫描期间
        // 断点 / 字素游标一直借 `text`，两者不能同时挂在 self 上。
        let mut text = std::mem::take(&mut self.text_buffer);
        text.clear();
        for fragment in fragments {
            match fragment {
                LineFragment::Text {
                    text: fragment_text,
                } => text.push_str(fragment_text),
                LineFragment::Element { len_utf8, .. } => {
                    text.extend(std::iter::repeat_n('a', *len_utf8));
                }
            }
        }
        let result = self.wrap_line_measured_with_text(fragments, &text, wrap_width);
        self.text_buffer = text;
        result
    }

    /// [`Self::wrap_line_measured`] 的扫描本体：`text` 是已重建好的行文本
    /// （由调用方持有，避免与 `&mut self` 的宽度缓存借用冲突）。
    fn wrap_line_measured_with_text(
        &mut self,
        fragments: &[LineFragment],
        text: &str,
        wrap_width: Pixels,
    ) -> WrapMeasurement {
        // 候选断点改为预计算的 UAX #14 断点集合查询（B1 API 同款分段器）。
        let mut break_indices = self.break_indices(text).peekable();
        let mut grapheme_indices = text
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .peekable();
        let mut width = px(0.);
        let mut first_non_whitespace_ix = None;
        let mut indent = None;
        let mut last_candidate_ix = 0;
        let mut last_candidate_width = px(0.);
        let mut last_wrap_ix = 0;
        let mut grapheme_index = 0;
        let mut width_before_grapheme = px(0.);
        let mut index = 0;
        let mut segments = Vec::new();
        let mut valid_from = px(0.);
        let mut valid_to = Pixels::MAX;
        for candidate in fragments
            .iter()
            .flat_map(|fragment| fragment.wrap_boundary_candidates())
        {
            let ix = index;
            index += candidate.len_utf8();
            // 推进字素游标到当前候选之前；候选若落在字素边界上（普通字符
            // 恰是边界，组合标记不是），记录该字素的起点与前方累计宽度——
            // 超宽字素换行时整体保留、缩进计入后随新行起算。
            while grapheme_indices
                .next_if(|position| *position < ix)
                .is_some()
            {}
            let is_grapheme_boundary = grapheme_indices.next_if_eq(&ix).is_some();
            if is_grapheme_boundary {
                grapheme_index = ix;
                width_before_grapheme = width;
            }
            // 候选断点 = UAX #14 允许断行的位置（含 CJK 逐字与禁则，分段器
            // 口径），且必须落在字素边界上。对所有候选（含内联元素）生效。
            if break_indices.next_if_eq(&ix).is_some()
                && is_grapheme_boundary
                && first_non_whitespace_ix.is_some()
            {
                last_candidate_ix = ix;
                last_candidate_width = width;
            }
            let item_width = match candidate {
                WrapBoundaryCandidate::Char { character: c } => {
                    if c == '\n' {
                        continue;
                    }

                    if c != ' ' && first_non_whitespace_ix.is_none() {
                        first_non_whitespace_ix = Some(ix);
                    }

                    self.width_for_char(c)
                }
                WrapBoundaryCandidate::Element {
                    width: element_width,
                    ..
                } => {
                    if first_non_whitespace_ix.is_none() {
                        first_non_whitespace_ix = Some(ix);
                    }

                    element_width
                }
            };

            width += item_width;
            if width > wrap_width && grapheme_index > last_wrap_ix {
                // 断点结构稳定的边界：有候选断点时下界是候选字符计入前的宽度
                // （低于它触发点会退到候选之前），无候选（超宽字素整体换行）时
                // 下界是当前字素计入前的宽度（低于它断点会落在更早的字符上）。
                let wrap_at_candidate =
                    last_candidate_ix > last_wrap_ix && width - last_candidate_width <= wrap_width;
                let segment_from = if wrap_at_candidate {
                    last_candidate_width
                } else {
                    width_before_grapheme
                };
                let segment_to = width;

                if let (None, Some(first_non_whitespace_ix)) = (indent, first_non_whitespace_ix) {
                    indent =
                        Some(Self::MAX_INDENT.min((first_non_whitespace_ix - last_wrap_ix) as u32));
                }

                if wrap_at_candidate {
                    last_wrap_ix = last_candidate_ix;
                    width -= last_candidate_width;
                    width_before_grapheme -= last_candidate_width;
                } else {
                    // 超宽字素整体保留，即使超出行宽。
                    last_wrap_ix = grapheme_index;
                    width -= width_before_grapheme;
                    width_before_grapheme = px(0.);
                }
                last_candidate_ix = 0;

                if let Some(indent) = indent {
                    width += self.width_for_char(' ') * indent as f32;
                    width_before_grapheme += self.width_for_char(' ') * indent as f32;
                }

                segments.push(WrapSegment {
                    boundary: Boundary::new(last_wrap_ix, indent.unwrap_or(0)),
                    valid_from: segment_from,
                    valid_to: segment_to,
                });
                valid_from = valid_from.max(segment_from);
                valid_to = valid_to.min(segment_to);
            }
        }

        // 扫描结束没有再触发断点：尾段累计宽度也是下界的一部分
        // （未换行的行此时尾段就是整行，下界即总宽、上界保持 MAX）。
        valid_from = valid_from.max(width);
        WrapMeasurement {
            segments,
            tail_width: width,
            valid_from,
            valid_to,
        }
    }

    /// 根据行的宽度确定是否应截断该行。
    ///
    /// 返回 `line` 中的截断索引。
    pub fn should_truncate_line(
        &mut self,
        line: &str,
        truncate_width: Pixels,
        truncation_affix: &str,
        truncate_from: TruncateFrom,
    ) -> Option<usize> {
        let mut width = px(0.);
        let suffix_width = truncation_affix
            .chars()
            .map(|c| self.width_for_char(c))
            .fold(px(0.0), |a, x| a + x);
        let mut truncate_ix = 0;

        match truncate_from {
            TruncateFrom::Start => {
                for (ix, c) in line.char_indices().rev() {
                    if width + suffix_width < truncate_width {
                        truncate_ix = ix;
                    }

                    let char_width = self.width_for_char(c);
                    width += char_width;

                    if width.floor() > truncate_width {
                        return Some(truncate_ix);
                    }
                }
            }
            TruncateFrom::End => {
                for (ix, c) in line.char_indices() {
                    if width + suffix_width < truncate_width {
                        truncate_ix = ix;
                    }

                    let char_width = self.width_for_char(c);
                    width += char_width;

                    if width.floor() > truncate_width {
                        return Some(truncate_ix);
                    }
                }
            }
            TruncateFrom::Middle => {}
        }

        None
    }

    fn should_truncate_line_middle(
        &mut self,
        line: &str,
        truncate_width: Pixels,
        truncation_affix: &str,
    ) -> Option<(usize, usize)> {
        let suffix_width = truncation_affix
            .chars()
            .map(|c| self.width_for_char(c))
            .fold(px(0.0), |a, x| a + x);

        let total_width: Pixels = line
            .chars()
            .map(|c| self.width_for_char(c))
            .fold(px(0.0), |a, x| a + x);

        if total_width <= truncate_width {
            return None;
        }

        let content_budget = truncate_width - suffix_width;
        if content_budget <= px(0.) {
            return Some((0, line.len()));
        }

        let front_budget = content_budget * (2.0 / 3.0);
        let back_budget = content_budget - front_budget;

        let mut front_width = px(0.);
        let mut front_end_ix = 0usize;
        for (ix, c) in line.char_indices() {
            let char_width = self.width_for_char(c);
            if front_width + char_width > front_budget {
                break;
            }
            front_width += char_width;
            front_end_ix = ix + c.len_utf8();
        }

        let mut back_width = px(0.);
        let mut back_start_ix = line.len();
        for (ix, c) in line.char_indices().rev() {
            let char_width = self.width_for_char(c);
            if back_width + char_width > back_budget {
                break;
            }
            back_width += char_width;
            back_start_ix = ix;
        }

        if front_end_ix >= back_start_ix {
            return Some((0, line.len()));
        }

        Some((front_end_ix, back_start_ix))
    }

    /// 使用此包装器的字体和字体大小将文本行截断到给定宽度。
    pub fn truncate_line<'a>(
        &mut self,
        line: SharedString,
        truncate_width: Pixels,
        truncation_affix: &str,
        runs: &'a [TextRun],
        truncate_from: TruncateFrom,
    ) -> (SharedString, Cow<'a, [TextRun]>) {
        if truncate_from == TruncateFrom::Middle {
            if let Some((front_end_ix, back_start_ix)) =
                self.should_truncate_line_middle(&line, truncate_width, truncation_affix)
            {
                let result = SharedString::from(format!(
                    "{}{truncation_affix}{}",
                    &line[..front_end_ix],
                    &line[back_start_ix..]
                ));
                let mut runs = runs.to_vec();
                update_runs_after_middle_truncation(
                    truncation_affix,
                    &mut runs,
                    front_end_ix,
                    back_start_ix,
                );
                return (result, Cow::Owned(runs));
            } else {
                return (line, Cow::Borrowed(runs));
            }
        }

        if let Some(truncate_ix) =
            self.should_truncate_line(&line, truncate_width, truncation_affix, truncate_from)
        {
            let result = match truncate_from {
                TruncateFrom::Start => SharedString::from(format!(
                    "{truncation_affix}{}",
                    &line[line.ceil_char_boundary(truncate_ix + 1)..]
                )),
                TruncateFrom::End => SharedString::from(format!(
                    "{}{truncation_affix}",
                    line[..truncate_ix]
                        .trim_end_matches(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
                )),
                TruncateFrom::Middle => unreachable!("Middle truncation is handled above"),
            };
            let mut runs = runs.to_vec();
            update_runs_after_truncation(&result, truncation_affix, &mut runs, truncate_from);
            (result, Cow::Owned(runs))
        } else {
            (line, Cow::Borrowed(runs))
        }
    }

    /// 将文本截断以适合指定数量的换行行。
    ///
    /// 与 `truncate_line`（将文本视为扁平宽度预算 `width * max_lines`）不同，
    /// 此方法考虑单词边界换行：它一次遍历字符，同时跟踪换行边界和截断点。
    /// 当文本在最后允许的行上溢出时，在此处截断并追加后缀。
    ///
    /// 对于 `max_lines == 1`，委托给 `truncate_line`。
    pub fn truncate_wrapped_line<'a>(
        &mut self,
        text: SharedString,
        wrap_width: Pixels,
        max_lines: usize,
        truncation_affix: &str,
        runs: &'a [TextRun],
        truncate_from: TruncateFrom,
    ) -> (SharedString, Cow<'a, [TextRun]>) {
        if max_lines <= 1
            || truncate_from == TruncateFrom::Start
            || truncate_from == TruncateFrom::Middle
        {
            return self.truncate_line(
                text,
                wrap_width * max_lines,
                truncation_affix,
                runs,
                truncate_from,
            );
        }

        let affix_width: Pixels = truncation_affix
            .chars()
            .map(|c| self.width_for_char(c))
            .sum();

        let mut width = px(0.);
        let mut line = 0usize;
        let mut first_non_whitespace_ix = None;
        let mut last_candidate_ix = 0usize;
        let mut last_candidate_width = px(0.);
        let mut last_wrap_ix = 0usize;
        let mut indent: Option<u32> = None;
        let mut truncate_ix = 0usize;

        // 候选断点改为 UAX #14 断点集合查询（与 wrap_line_measured 同口径）。
        let mut break_indices = self.break_indices(&text).peekable();
        for (ix, c) in text.char_indices() {
            // 换行符位置也消费断点：跳过空行时不让迭代器滞留在后续文本之前。
            let is_break = break_indices.next_if_eq(&ix).is_some();
            if c == '\n' {
                if line >= max_lines - 1 && !text[ix + 1..].trim().is_empty() {
                    // 最后允许的行上遇到换行符，且下方有真实内容，在此处截断。
                    let truncated = text[..truncate_ix]
                        .trim_end_matches(|c: char| c.is_whitespace() || c.is_ascii_punctuation());
                    let result = SharedString::from(format!("{truncated}{truncation_affix}"));
                    let mut runs = runs.to_vec();
                    update_runs_after_truncation(
                        &result,
                        truncation_affix,
                        &mut runs,
                        TruncateFrom::End,
                    );
                    return (result, Cow::Owned(runs));
                }

                // 最后一行之前的换行符消耗一行。
                line += 1;
                width = px(0.);
                first_non_whitespace_ix = None;
                last_candidate_ix = 0;
                last_candidate_width = px(0.);
                last_wrap_ix = ix + 1;
                indent = None;
                truncate_ix = ix + 1;
                continue;
            }

            let char_width = self.width_for_char(c);

            if is_break && first_non_whitespace_ix.is_some() {
                last_candidate_ix = ix;
                last_candidate_width = width;
            }

            if c != ' ' && first_non_whitespace_ix.is_none() {
                first_non_whitespace_ix = Some(ix);
            }

            width += char_width;

            if line < max_lines - 1 {
                // 最后一行之前：复制 wrap_line 的边界逻辑。
                if width > wrap_width && ix > last_wrap_ix {
                    if let (None, Some(first_nw)) = (indent, first_non_whitespace_ix) {
                        indent = Some(Self::MAX_INDENT.min((first_nw - last_wrap_ix) as u32));
                    }

                    if last_candidate_ix > last_wrap_ix {
                        last_wrap_ix = last_candidate_ix;
                        width -= last_candidate_width;
                        last_candidate_ix = 0;
                    } else {
                        last_wrap_ix = ix;
                        width = char_width;
                    }

                    if let Some(ind) = indent {
                        width += self.width_for_char(' ') * ind as f32;
                    }

                    line += 1;
                    truncate_ix = last_wrap_ix;
                }
            } else {
                // 在最后一行上：跟踪后缀仍能容纳的最远位置，
                // 一旦行溢出就停止。
                if width + affix_width <= wrap_width {
                    truncate_ix = ix + c.len_utf8();
                }

                if width > wrap_width {
                    let truncated = text[..truncate_ix]
                        .trim_end_matches(|c: char| c.is_whitespace() || c.is_ascii_punctuation());
                    let result = SharedString::from(format!("{truncated}{truncation_affix}"));
                    let mut runs = runs.to_vec();
                    update_runs_after_truncation(
                        &result,
                        truncation_affix,
                        &mut runs,
                        TruncateFrom::End,
                    );
                    return (result, Cow::Owned(runs));
                }
            }
        }
        drop(break_indices);

        // 文本在 max_lines 内无需截断。
        (text, Cow::Borrowed(runs))
    }
    /// UAX #14 允许断行的字节偏移（分段器口径，与 wrap_line_measured 同源）。
    pub(crate) fn break_indices<'a>(&self, text: &'a str) -> impl Iterator<Item = usize> + use<'a> {
        icu_segmenter::LineSegmenter::new_for_non_complex_scripts(Default::default())
            .segment_str(text)
    }

    #[inline(always)]
    fn width_for_char(&mut self, c: char) -> Pixels {
        if (c as u32) < 128 {
            if let Some(cached_width) = self.char_widths.ascii[c as usize] {
                cached_width
            } else {
                let width = self
                    .text_system
                    .layout_width(self.font_id, self.font_size, c);
                self.char_widths.ascii[c as usize] = Some(width);
                width
            }
        } else if let Some(cached_width) = self.char_widths.other.get(&c) {
            *cached_width
        } else {
            let width = self
                .text_system
                .layout_width(self.font_id, self.font_size, c);
            self.char_widths.other.insert(c, width);
            width
        }
    }
}

fn update_runs_after_truncation(
    result: &str,
    ellipsis: &str,
    runs: &mut Vec<TextRun>,
    truncate_from: TruncateFrom,
) {
    let mut truncate_at = result.len() - ellipsis.len();
    match truncate_from {
        TruncateFrom::Start => {
            for (run_index, run) in runs.iter_mut().enumerate().rev() {
                if run.len <= truncate_at {
                    truncate_at -= run.len;
                } else {
                    run.len = truncate_at + ellipsis.len();
                    runs.splice(..run_index, std::iter::empty());
                    break;
                }
            }
        }
        TruncateFrom::End => {
            for (run_index, run) in runs.iter_mut().enumerate() {
                if run.len <= truncate_at {
                    truncate_at -= run.len;
                } else {
                    run.len = truncate_at + ellipsis.len();
                    runs.truncate(run_index + 1);
                    break;
                }
            }
        }
        TruncateFrom::Middle => {
            unreachable!("Middle truncation calls this function with TruncateFrom::End directly")
        }
    }
}

fn update_runs_after_middle_truncation(
    ellipsis: &str,
    runs: &mut Vec<TextRun>,
    front_end_ix: usize,
    back_start_ix: usize,
) {
    let original_runs = std::mem::take(runs);
    let mut result_runs: Vec<TextRun> = Vec::with_capacity(original_runs.len());

    let mut front_remaining = front_end_ix;
    let mut front_done = false;
    for run in &original_runs {
        if front_done {
            break;
        }
        if run.len <= front_remaining {
            result_runs.push(run.clone());
            front_remaining -= run.len;
        } else {
            let mut partial = run.clone();
            partial.len = front_remaining + ellipsis.len();
            result_runs.push(partial);
            front_done = true;
        }
    }
    if !front_done {
        if let Some(last) = result_runs.last_mut() {
            last.len += ellipsis.len();
        } else if let Some(first) = original_runs.first() {
            let mut affix_run = first.clone();
            affix_run.len = ellipsis.len();
            result_runs.push(affix_run);
        }
    }

    let mut byte_pos = 0usize;
    for run in &original_runs {
        let run_end = byte_pos + run.len;
        if run_end > back_start_ix {
            if byte_pos < back_start_ix {
                let mut partial = run.clone();
                partial.len = run_end - back_start_ix;
                result_runs.push(partial);
            } else {
                result_runs.push(run.clone());
            }
        }
        byte_pos = run_end;
    }

    *runs = result_runs;
}

/// 可以包装的行片段。
pub enum LineFragment<'a> {
    /// 由字符组成的文本片段。
    Text {
        /// 片段的文本内容。
        text: &'a str,
    },
    /// 具有固定宽度的非文本元素。
    Element {
        /// 元素的宽度（像素）。
        width: Pixels,
        /// 元素的 UTF-8 编码长度。
        len_utf8: usize,
    },
}

impl<'a> LineFragment<'a> {
    /// 从给定文本创建新的文本片段。
    pub fn text(text: &'a str) -> Self {
        LineFragment::Text { text }
    }

    /// 创建具有给定宽度和 UTF-8 编码长度的新非文本元素。
    pub fn element(width: Pixels, len_utf8: usize) -> Self {
        LineFragment::Element { width, len_utf8 }
    }

    fn wrap_boundary_candidates(&self) -> impl Iterator<Item = WrapBoundaryCandidate> {
        let text = match self {
            LineFragment::Text { text } => text,
            LineFragment::Element { .. } => "\0",
        };
        text.chars().map(move |character| {
            if let LineFragment::Element { width, len_utf8 } = self {
                WrapBoundaryCandidate::Element {
                    width: *width,
                    len_utf8: *len_utf8,
                }
            } else {
                WrapBoundaryCandidate::Char { character }
            }
        })
    }
}

enum WrapBoundaryCandidate {
    Char { character: char },
    Element { width: Pixels, len_utf8: usize },
}

impl WrapBoundaryCandidate {
    pub fn len_utf8(&self) -> usize {
        match self {
            WrapBoundaryCandidate::Char { character } => character.len_utf8(),
            WrapBoundaryCandidate::Element { len_utf8: len, .. } => *len,
        }
    }
}

/// 两行文本之间的边界。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Boundary {
    /// 行中最后一个字符的索引
    pub ix: usize,
    /// 下一行的缩进。
    pub next_indent: u32,
}

impl Boundary {
    fn new(ix: usize, next_indent: u32) -> Self {
        Self { ix, next_indent }
    }
}

/// 一个软换行断点，附带该断点保持稳定的 `wrap_width` 半开区间。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WrapSegment {
    /// 断点。
    pub boundary: Boundary,
    /// 区间下界（闭）：`wrap_width` 低于它，该断点会前移。
    pub valid_from: Pixels,
    /// 区间上界（开）：`wrap_width` 达到它，该断点会后移。
    pub valid_to: Pixels,
}

/// 一次软换行测量的完整结果：断点列表 + 整行换行结构稳定的 `wrap_width` 区间。
///
/// 区间判定只保证「不断」的一侧是精确的：区间内结构必然一致，区间外只是
/// 可能需要重测——保守方向只会多换行，不会错换行。
#[derive(Debug, Clone, PartialEq)]
pub struct WrapMeasurement {
    /// 全部断点（含各自的稳定区间）。
    pub segments: Vec<WrapSegment>,
    /// 最后一个断点之后尾段的累计宽度（无断点时为整行总宽）。
    pub tail_width: Pixels,
    /// 整行换行结构（含「尾段不新增断点」）稳定的区间下界（闭）。
    pub valid_from: Pixels,
    /// 区间上界（开）；未换行的行为 [`Pixels::MAX`]。
    pub valid_to: Pixels,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Font, FontFeatures, FontStyle, FontWeight, TestAppContext, TestDispatcher, font};
    #[cfg(target_os = "macos")]
    use crate::{TextRun, WindowTextSystem, WrapBoundary};

    fn build_wrapper() -> LineWrapper {
        let dispatcher = TestDispatcher::new(0);
        let cx = TestAppContext::build(dispatcher, None);
        let id = cx.text_system().resolve_font(&font(".ZedMono"));
        LineWrapper::new(
            id,
            px(16.),
            cx.text_system().clone(),
            CharWidthCache::default(),
        )
    }

    fn generate_test_runs(input_run_len: &[usize]) -> Vec<TextRun> {
        input_run_len
            .iter()
            .map(|run_len| TextRun {
                len: *run_len,
                font: Font {
                    family: "Dummy".into(),
                    features: FontFeatures::default(),
                    fallbacks: None,
                    weight: FontWeight::default(),
                    style: FontStyle::Normal,
                },
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn test_wrap_line() {
        let mut wrapper = build_wrapper();

        assert_eq!(
            wrapper
                .wrap_line(&[LineFragment::text("aa bbb cccc ddddd eeee")], px(72.))
                .collect::<Vec<_>>(),
            &[
                Boundary::new(7, 0),
                Boundary::new(12, 0),
                Boundary::new(18, 0)
            ],
        );
        assert_eq!(
            wrapper
                .wrap_line(&[LineFragment::text("aaa aaaaaaaaaaaaaaaaaa")], px(72.0))
                .collect::<Vec<_>>(),
            &[
                Boundary::new(4, 0),
                Boundary::new(11, 0),
                Boundary::new(18, 0)
            ],
        );
        assert_eq!(
            wrapper
                .wrap_line(&[LineFragment::text("     aaaaaaa")], px(72.))
                .collect::<Vec<_>>(),
            &[
                Boundary::new(7, 5),
                Boundary::new(9, 5),
                Boundary::new(11, 5),
            ]
        );
        assert_eq!(
            wrapper
                .wrap_line(
                    &[LineFragment::text("                            ")],
                    px(72.)
                )
                .collect::<Vec<_>>(),
            &[
                Boundary::new(7, 0),
                Boundary::new(14, 0),
                Boundary::new(21, 0)
            ]
        );
        assert_eq!(
            wrapper
                .wrap_line(&[LineFragment::text("          aaaaaaaaaaaaaa")], px(72.))
                .collect::<Vec<_>>(),
            &[
                Boundary::new(7, 0),
                Boundary::new(14, 3),
                Boundary::new(18, 3),
                Boundary::new(22, 3),
            ]
        );

        // Test wrapping multiple text fragments
        assert_eq!(
            wrapper
                .wrap_line(
                    &[
                        LineFragment::text("aa bbb "),
                        LineFragment::text("cccc ddddd eeee")
                    ],
                    px(72.)
                )
                .collect::<Vec<_>>(),
            &[
                Boundary::new(7, 0),
                Boundary::new(12, 0),
                Boundary::new(18, 0)
            ],
        );

        // Test wrapping with a mix of text and element fragments
        assert_eq!(
            wrapper
                .wrap_line(
                    &[
                        LineFragment::text("aa "),
                        LineFragment::element(px(20.), 1),
                        LineFragment::text(" bbb "),
                        LineFragment::element(px(30.), 1),
                        LineFragment::text(" cccc")
                    ],
                    px(72.)
                )
                .collect::<Vec<_>>(),
            &[
                Boundary::new(5, 0),
                Boundary::new(9, 0),
                Boundary::new(11, 0)
            ],
        );

        // Test with element at the beginning and text afterward
        assert_eq!(
            wrapper
                .wrap_line(
                    &[
                        LineFragment::element(px(50.), 1),
                        LineFragment::text(" aaaa bbbb cccc dddd")
                    ],
                    px(72.)
                )
                .collect::<Vec<_>>(),
            &[
                Boundary::new(2, 0),
                Boundary::new(7, 0),
                Boundary::new(12, 0),
                Boundary::new(17, 0)
            ],
        );

        // Test with a large element that forces wrapping by itself
        assert_eq!(
            wrapper
                .wrap_line(
                    &[
                        LineFragment::text("short text "),
                        LineFragment::element(px(100.), 1),
                        LineFragment::text(" more text")
                    ],
                    px(72.)
                )
                .collect::<Vec<_>>(),
            &[
                Boundary::new(6, 0),
                Boundary::new(11, 0),
                Boundary::new(12, 0),
                Boundary::new(18, 0)
            ],
        );
    }

    /// `wrap_line_measured` 的稳定区间：区间内断点结构不变，达到上界后必变；
    /// 未换行的行上界为 MAX、下界为整行总宽。
    #[test]
    fn test_wrap_line_measured_valid_widths() {
        let mut wrapper = build_wrapper();
        let text = "aa bbb cccc ddddd eeee";

        let ixs = |m: &WrapMeasurement| -> Vec<usize> {
            m.segments.iter().map(|s| s.boundary.ix).collect()
        };
        let measured = wrapper.wrap_line_measured(&[LineFragment::text(text)], px(72.));
        assert_eq!(ixs(&measured), vec![7, 12, 18]);

        // 闭下界、开上界：区间内任意宽度探测，断点结构与宽度 72 时完全一致。
        let probes = [
            measured.valid_from,
            px((measured.valid_from.0 + measured.valid_to.0) / 2.0),
            px(measured.valid_to.0 - 0.5),
        ];
        for probe in probes {
            let again = wrapper.wrap_line_measured(&[LineFragment::text(text)], probe);
            assert_eq!(ixs(&again), vec![7, 12, 18], "probe {probe:?}");
        }

        // 上界是保守口径：恰好达到上界时结构可能恰好不变（触发点后移但没有
        // 新候选），但足够超出（远超整行总宽）必然不同。
        let far =
            wrapper.wrap_line_measured(&[LineFragment::text(text)], measured.valid_to + px(200.));
        assert_ne!(ixs(&far), ixs(&measured));

        // 未换行的行：上界是 MAX，下界是整行总宽；低于下界就出现断点。
        let unwrapped = wrapper.wrap_line_measured(&[LineFragment::text(text)], px(500.));
        assert!(unwrapped.segments.is_empty());
        assert_eq!(unwrapped.valid_to, Pixels::MAX);
        assert!(unwrapped.valid_from > px(0.));
        let shrunk =
            wrapper.wrap_line_measured(&[LineFragment::text(text)], unwrapped.valid_from - px(0.5));
        assert!(!shrunk.segments.is_empty());
    }

    #[test]
    fn test_truncate_line_end() {
        let mut wrapper = build_wrapper();

        fn perform_test(
            wrapper: &mut LineWrapper,
            text: &'static str,
            expected: &'static str,
            ellipsis: &str,
        ) {
            let dummy_run_lens = vec![text.len()];
            let dummy_runs = generate_test_runs(&dummy_run_lens);
            let (result, dummy_runs) = wrapper.truncate_line(
                text.into(),
                px(220.),
                ellipsis,
                &dummy_runs,
                TruncateFrom::End,
            );
            assert_eq!(result, expected);
            assert_eq!(dummy_runs.first().unwrap().len, result.len());
        }

        perform_test(
            &mut wrapper,
            "aa bbb cccc ddddd eeee ffff gggg",
            "aa bbb cccc ddddd eeee",
            "",
        );
        perform_test(
            &mut wrapper,
            "aa bbb cccc ddddd eeee ffff gggg",
            "aa bbb cccc ddddd eee…",
            "…",
        );
        perform_test(
            &mut wrapper,
            "aa bbb cccc ddddd eeee ffff gggg",
            "aa bbb cccc dddd......",
            "......",
        );
        perform_test(
            &mut wrapper,
            "aa bbb cccc 🦀🦀🦀🦀🦀 eeee ffff gggg",
            "aa bbb cccc 🦀🦀🦀🦀…",
            "…",
        );
    }

    #[test]
    fn test_truncate_line_start() {
        let mut wrapper = build_wrapper();

        #[track_caller]
        fn perform_test(
            wrapper: &mut LineWrapper,
            text: &'static str,
            expected: &'static str,
            ellipsis: &str,
        ) {
            let dummy_run_lens = vec![text.len()];
            let dummy_runs = generate_test_runs(&dummy_run_lens);
            let (result, dummy_runs) = wrapper.truncate_line(
                text.into(),
                px(220.),
                ellipsis,
                &dummy_runs,
                TruncateFrom::Start,
            );
            assert_eq!(result, expected);
            assert_eq!(dummy_runs.first().unwrap().len, result.len());
        }

        perform_test(
            &mut wrapper,
            "aaaa bbbb cccc ddddd eeee fff gg",
            "cccc ddddd eeee fff gg",
            "",
        );
        perform_test(
            &mut wrapper,
            "aaaa bbbb cccc ddddd eeee fff gg",
            "…ccc ddddd eeee fff gg",
            "…",
        );
        perform_test(
            &mut wrapper,
            "aaaa bbbb cccc ddddd eeee fff gg",
            "......dddd eeee fff gg",
            "......",
        );
        perform_test(
            &mut wrapper,
            "aaaa bbbb cccc 🦀🦀🦀🦀🦀 eeee fff gg",
            "…🦀🦀🦀🦀 eeee fff gg",
            "…",
        );
    }

    #[test]
    fn test_truncate_multiple_runs_end() {
        let mut wrapper = build_wrapper();

        fn perform_test(
            wrapper: &mut LineWrapper,
            text: &'static str,
            expected: &str,
            run_lens: &[usize],
            result_run_len: &[usize],
            line_width: Pixels,
        ) {
            let dummy_runs = generate_test_runs(run_lens);
            let (result, dummy_runs) =
                wrapper.truncate_line(text.into(), line_width, "…", &dummy_runs, TruncateFrom::End);
            assert_eq!(result, expected);
            for (run, result_len) in dummy_runs.iter().zip(result_run_len) {
                assert_eq!(run.len, *result_len);
            }
        }
        // Case 0: Normal
        // Text: abcdefghijkl
        // Runs: Run0 { len: 12, ... }
        //
        // Truncate res: abcd… (truncate_at = 4)
        // Run res: Run0 { string: abcd…, len: 7, ... }
        perform_test(&mut wrapper, "abcdefghijkl", "abcd…", &[12], &[7], px(50.));
        // Case 1: Drop some runs
        // Text: abcdefghijkl
        // Runs: Run0 { len: 4, ... }, Run1 { len: 4, ... }, Run2 { len: 4, ... }
        //
        // Truncate res: abcdef… (truncate_at = 6)
        // Runs res: Run0 { string: abcd, len: 4, ... }, Run1 { string: ef…, len:
        // 5, ... }
        perform_test(
            &mut wrapper,
            "abcdefghijkl",
            "abcdef…",
            &[4, 4, 4],
            &[4, 5],
            px(70.),
        );
        // Case 2: Truncate at start of some run
        // Text: abcdefghijkl
        // Runs: Run0 { len: 4, ... }, Run1 { len: 4, ... }, Run2 { len: 4, ... }
        //
        // Truncate res: abcdefgh… (truncate_at = 8)
        // Runs res: Run0 { string: abcd, len: 4, ... }, Run1 { string: efgh, len:
        // 4, ... }, Run2 { string: …, len: 3, ... }
        perform_test(
            &mut wrapper,
            "abcdefghijkl",
            "abcdefgh…",
            &[4, 4, 4],
            &[4, 4, 3],
            px(90.),
        );
    }

    #[test]
    fn test_truncate_multiple_runs_start() {
        let mut wrapper = build_wrapper();

        #[track_caller]
        fn perform_test(
            wrapper: &mut LineWrapper,
            text: &'static str,
            expected: &str,
            run_lens: &[usize],
            result_run_len: &[usize],
            line_width: Pixels,
        ) {
            let dummy_runs = generate_test_runs(run_lens);
            let (result, dummy_runs) = wrapper.truncate_line(
                text.into(),
                line_width,
                "…",
                &dummy_runs,
                TruncateFrom::Start,
            );
            assert_eq!(result, expected);
            for (run, result_len) in dummy_runs.iter().zip(result_run_len) {
                assert_eq!(run.len, *result_len);
            }
        }
        // Case 0: Normal
        // Text: abcdefghijkl
        // Runs: Run0 { len: 12, ... }
        //
        // Truncate res: …ijkl (truncate_at = 9)
        // Run res: Run0 { string: …ijkl, len: 7, ... }
        perform_test(&mut wrapper, "abcdefghijkl", "…ijkl", &[12], &[7], px(50.));
        // Case 1: Drop some runs
        // Text: abcdefghijkl
        // Runs: Run0 { len: 4, ... }, Run1 { len: 4, ... }, Run2 { len: 4, ... }
        //
        // Truncate res: …ghijkl (truncate_at = 7)
        // Runs res: Run0 { string: …gh, len: 5, ... }, Run1 { string: ijkl, len:
        // 4, ... }
        perform_test(
            &mut wrapper,
            "abcdefghijkl",
            "…ghijkl",
            &[4, 4, 4],
            &[5, 4],
            px(70.),
        );
        // Case 2: Truncate at start of some run
        // Text: abcdefghijkl
        // Runs: Run0 { len: 4, ... }, Run1 { len: 4, ... }, Run2 { len: 4, ... }
        //
        // Truncate res: abcdefgh… (truncate_at = 3)
        // Runs res: Run0 { string: …, len: 3, ... }, Run1 { string: efgh, len:
        // 4, ... }, Run2 { string: ijkl, len: 4, ... }
        perform_test(
            &mut wrapper,
            "abcdefghijkl",
            "…efghijkl",
            &[4, 4, 4],
            &[3, 4, 4],
            px(90.),
        );
    }

    #[test]
    fn test_update_run_after_truncation_end() {
        fn perform_test(result: &str, run_lens: &[usize], result_run_lens: &[usize]) {
            let mut dummy_runs = generate_test_runs(run_lens);
            update_runs_after_truncation(result, "…", &mut dummy_runs, TruncateFrom::End);
            for (run, result_len) in dummy_runs.iter().zip(result_run_lens) {
                assert_eq!(run.len, *result_len);
            }
        }
        // Case 0: Normal
        // Text: abcdefghijkl
        // Runs: Run0 { len: 12, ... }
        //
        // Truncate res: abcd… (truncate_at = 4)
        // Run res: Run0 { string: abcd…, len: 7, ... }
        perform_test("abcd…", &[12], &[7]);
        // Case 1: Drop some runs
        // Text: abcdefghijkl
        // Runs: Run0 { len: 4, ... }, Run1 { len: 4, ... }, Run2 { len: 4, ... }
        //
        // Truncate res: abcdef… (truncate_at = 6)
        // Runs res: Run0 { string: abcd, len: 4, ... }, Run1 { string: ef…, len:
        // 5, ... }
        perform_test("abcdef…", &[4, 4, 4], &[4, 5]);
        // Case 2: Truncate at start of some run
        // Text: abcdefghijkl
        // Runs: Run0 { len: 4, ... }, Run1 { len: 4, ... }, Run2 { len: 4, ... }
        //
        // Truncate res: abcdefgh… (truncate_at = 8)
        // Runs res: Run0 { string: abcd, len: 4, ... }, Run1 { string: efgh, len:
        // 4, ... }, Run2 { string: …, len: 3, ... }
        perform_test("abcdefgh…", &[4, 4, 4], &[4, 4, 3]);
    }

    /// UAX #14 断点：拉丁空格（LB18）、连字符（LB21）、CJK 逐字、强制断点都给出；
    /// 断点一律落在字素边界上（emoji ZWJ 序列 / 组合标记不被切开）。
    #[test]
    fn test_break_indices_follow_uax14() {
        let mut wrapper = build_wrapper();
        let breaks = |text: &str| -> Vec<usize> { wrapper.break_indices(text).collect() };

        // 拉丁空格（LB18）：`hello world` 在空格后可断（偏移 6 = "hello " 之后）。
        assert!(breaks("hello world").contains(&6));
        // 连字符后允许断行（LB21）：`non-English` 在连字符后可断。
        assert!(breaks("non-English").contains(&4));
        // CJK 逐字可断（ID 类）：`你好世界` 在每个字之间（含首尾哨兵 0 与 len）。
        assert_eq!(breaks("你好世界"), vec![0, 3, 6, 9, 12]);
        // 行首禁则（LB13 类）：`。` 不可起行 —— 与禁则口径一致。
        let cjk = breaks("你好。世界");
        assert!(!cjk.contains(&6), "「。」不可起行: {cjk:?}");

        // 断点必须落在字素边界上：emoji ZWJ 序列（👩‍💻 = 女 + ZWJ + 笔记本）
        // 与肤色修饰（👍🏽）内部不得断开。
        for text in ["{1F469}{200D}{1F4BB}", "{1F44D}{1F3FD}", "e{0301}"] {
            let text = format!("x{text}y");
            let graphemes: std::collections::HashSet<usize> = text
                .grapheme_indices(true)
                .map(|(index, _)| index)
                .chain(std::iter::once(text.len()))
                .collect();
            for index in breaks(&text) {
                assert!(
                    graphemes.contains(&index),
                    "断点 {index} 落在字素内部: {text:?}"
                );
            }
        }
    }

    // For compatibility with the test macro
    #[cfg(target_os = "macos")]
    use crate as rgpui;

    // These seem to vary wildly based on the text system.
    #[cfg(target_os = "macos")]
    #[crate::test]
    fn test_wrap_shaped_line(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let text_system = WindowTextSystem::new(cx.text_system().clone());

            let normal = TextRun {
                len: 0,
                font: font("Helvetica"),
                color: Default::default(),
                underline: Default::default(),
                ..Default::default()
            };
            let bold = TextRun {
                len: 0,
                font: font("Helvetica").bold(),
                ..Default::default()
            };

            let text = "aa bbb cccc ddddd eeee".into();
            let lines = text_system
                .shape_text(
                    text,
                    px(16.),
                    &[
                        normal.with_len(4),
                        bold.with_len(5),
                        normal.with_len(6),
                        bold.with_len(1),
                        normal.with_len(7),
                    ],
                    Some(px(72.)),
                    None,
                )
                .unwrap();

            assert_eq!(
                lines[0].layout.wrap_boundaries(),
                &[
                    WrapBoundary {
                        run_ix: 0,
                        glyph_ix: 7
                    },
                    WrapBoundary {
                        run_ix: 0,
                        glyph_ix: 12
                    },
                    WrapBoundary {
                        run_ix: 0,
                        glyph_ix: 18
                    }
                ],
            );
        });
    }

    #[test]
    fn test_multiline_truncation_fits_within_wrapped_lines() {
        let mut wrapper = build_wrapper();

        // With .ZedMono at 16px, each char is 9.6px wide.
        // wrap_width = 72px fits ~7 chars per line.
        //
        // "aa bbbbbb cccccc dddddd eeee ffff" with wrap_width=72px wraps as:
        //   Line 1: "aa "       (28.8px, wraps because "bbbbbb" won't fit)
        //   Line 2: "bbbbbb "   (67.2px)
        //   Line 3: "cccccc "   (67.2px)
        //   ...
        //
        // truncate_wrapped_line should wrap first to find line 2 starts at
        // "bbbbbb...", then truncate only that line to fit with ellipsis.
        let text: &str = "aa bbbbbb cccccc dddddd eeee ffff";
        let wrap_width = px(72.);
        let max_lines: usize = 2;

        let runs = generate_test_runs(&[text.len()]);
        let (truncated, _) = wrapper.truncate_wrapped_line(
            text.into(),
            wrap_width,
            max_lines,
            "\u{2026}",
            &runs,
            TruncateFrom::End,
        );

        // The truncated text, when wrapped, must fit within max_lines lines.
        let wrap_count = wrapper
            .wrap_line(&[LineFragment::text(&truncated)], wrap_width)
            .count();

        assert!(
            wrap_count < max_lines,
            "Truncated text '{}' wraps into {} visual lines, expected at most {}",
            truncated,
            wrap_count + 1,
            max_lines
        );

        // The truncated text should end with the ellipsis.
        assert!(
            truncated.ends_with('\u{2026}'),
            "Truncated text '{}' should end with ellipsis",
            truncated
        );
    }

    #[test]
    fn test_multiline_truncation_no_truncation_needed() {
        let mut wrapper = build_wrapper();

        // Text that fits in 2 lines shouldn't be truncated.
        // Line 1: "aa bbb " (67.2px), Line 2: "cccccc" (57.6px)
        let text: &str = "aa bbb cccccc";
        let wrap_width = px(72.);
        let max_lines: usize = 2;

        let runs = generate_test_runs(&[text.len()]);
        let (result, _) = wrapper.truncate_wrapped_line(
            text.into(),
            wrap_width,
            max_lines,
            "\u{2026}",
            &runs,
            TruncateFrom::End,
        );

        assert_eq!(
            result.as_ref(),
            text,
            "Text that fits should not be modified"
        );
    }

    #[test]
    fn test_multiline_truncation_three_lines() {
        let mut wrapper = build_wrapper();

        let text: &str = "aa bbb cccc ddddd eeee ffff gggg hhhh iiii jjjj";
        let wrap_width = px(72.);
        let max_lines: usize = 3;

        let runs = generate_test_runs(&[text.len()]);
        let (truncated, _) = wrapper.truncate_wrapped_line(
            text.into(),
            wrap_width,
            max_lines,
            "\u{2026}",
            &runs,
            TruncateFrom::End,
        );

        let wrap_count = wrapper
            .wrap_line(&[LineFragment::text(&truncated)], wrap_width)
            .count();

        assert!(
            wrap_count < max_lines,
            "Truncated text '{}' wraps into {} visual lines, expected at most {}",
            truncated,
            wrap_count + 1,
            max_lines
        );

        assert!(
            truncated.ends_with('\u{2026}'),
            "Truncated text '{}' should end with ellipsis",
            truncated
        );
    }

    #[test]
    fn test_multiline_truncation_with_newlines() {
        let mut wrapper = build_wrapper();

        // "hello\nworld foo bar baz" with line_clamp(2):
        // shape_text splits on \n, giving physical lines "hello" and
        // "world foo bar baz". The newline consumes line 1, so the
        // second physical line should be truncated on line 2.
        let text: &str = "hello\nworld foo bar baz";
        let wrap_width = px(72.);
        let max_lines: usize = 2;

        let runs = generate_test_runs(&[text.len()]);
        let (truncated, _) = wrapper.truncate_wrapped_line(
            text.into(),
            wrap_width,
            max_lines,
            "\u{2026}",
            &runs,
            TruncateFrom::End,
        );

        // The newline should be preserved.
        let parts: Vec<&str> = truncated.splitn(2, '\n').collect();
        assert_eq!(
            parts.len(),
            2,
            "Newline should be preserved: '{}'",
            truncated
        );
        assert_eq!(parts[0], "hello");

        // The second line should fit within wrap_width and end with ellipsis.
        let second_line_width: Pixels = parts[1].chars().map(|c| wrapper.width_for_char(c)).sum();
        assert!(
            second_line_width <= wrap_width,
            "Second line '{}' ({}px) exceeds wrap_width ({}px)",
            parts[1],
            second_line_width,
            wrap_width
        );
        assert!(
            truncated.ends_with('\u{2026}'),
            "Should end with ellipsis: '{}'",
            truncated
        );
    }

    #[test]
    fn test_multiline_truncation_newline_on_last_line() {
        let mut wrapper = build_wrapper();

        // "hello\nworld\nmore" with line_clamp(2):
        // Line 1: "hello", Line 2: "world" — but there's a third line,
        // so line 2 should be truncated with ellipsis.
        let text: &str = "hello\nworld\nmore";
        let wrap_width = px(72.);
        let max_lines: usize = 2;

        let runs = generate_test_runs(&[text.len()]);
        let (truncated, _) = wrapper.truncate_wrapped_line(
            text.into(),
            wrap_width,
            max_lines,
            "\u{2026}",
            &runs,
            TruncateFrom::End,
        );

        let parts: Vec<&str> = truncated.splitn(2, '\n').collect();
        assert_eq!(parts[0], "hello");
        assert!(
            truncated.ends_with('\u{2026}'),
            "Should end with ellipsis since there's more content: '{}'",
            truncated
        );
    }

    #[test]
    fn test_multiline_truncation_trailing_newline() {
        let mut wrapper = build_wrapper();

        // "hello\nworld\n" with line_clamp(2):
        // The trailing newline has no content after it, so no ellipsis.
        let text: &str = "hello\nworld\n";
        let wrap_width = px(72.);
        let max_lines: usize = 2;

        let runs = generate_test_runs(&[text.len()]);
        let (result, _) = wrapper.truncate_wrapped_line(
            text.into(),
            wrap_width,
            max_lines,
            "\u{2026}",
            &runs,
            TruncateFrom::End,
        );

        assert!(
            !result.ends_with('\u{2026}'),
            "Trailing newline with no content should not add ellipsis: '{}'",
            result
        );
    }

    #[test]
    fn test_multiline_truncation_newline_fits_exactly() {
        let mut wrapper = build_wrapper();

        // "hello\nworld" with line_clamp(2):
        // Exactly 2 lines, no truncation needed.
        let text: &str = "hello\nworld";
        let wrap_width = px(72.);
        let max_lines: usize = 2;

        let runs = generate_test_runs(&[text.len()]);
        let (result, _) = wrapper.truncate_wrapped_line(
            text.into(),
            wrap_width,
            max_lines,
            "\u{2026}",
            &runs,
            TruncateFrom::End,
        );

        assert_eq!(
            result.as_ref(),
            text,
            "Text that fits exactly should not be modified: '{}'",
            result
        );
    }

    #[test]
    fn test_truncate_line_middle() {
        let mut wrapper = build_wrapper();

        // 当文本适应非常宽的预算时，不进行截断。
        let short_text = "hello world";
        let runs = generate_test_runs(&[short_text.len()]);
        let (result, result_runs) = wrapper.truncate_line(
            short_text.into(),
            px(10000.),
            "…",
            &runs,
            TruncateFrom::Middle,
        );
        assert_eq!(result.as_ref(), short_text);
        assert_eq!(result_runs.len(), 1);
        assert_eq!(result_runs[0].len, short_text.len());

        // 基本中间截断：在 px(100.) 预算下的长字符串。
        let long_text = "abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz";
        let runs = generate_test_runs(&[long_text.len()]);
        let (result, _result_runs) =
            wrapper.truncate_line(long_text.into(), px(100.), "…", &runs, TruncateFrom::Middle);
        assert!(
            result.contains('…'),
            "Middle-truncated result should contain '…', got: '{}'",
            result
        );
        assert!(
            result.chars().count() < long_text.chars().count(),
            "Middle-truncated result should be shorter than original"
        );
        assert_eq!(
            result.chars().next(),
            long_text.chars().next(),
            "Result should start with the same first character as original"
        );
        assert_eq!(
            result.chars().last(),
            long_text.chars().last(),
            "Result should end with the same last character as original"
        );

        // 特殊情况：预算过于紧张，导致中间截断无法找到有效的分割点。此时仍应显示截断后缀，而不是返回原始的溢出文本。
        let text = "abcdef";
        let runs = generate_test_runs(&[text.len()]);
        let (result, result_runs) =
            wrapper.truncate_line(text.into(), px(1.), "…", &runs, TruncateFrom::Middle);
        assert_eq!(result.as_ref(), "…");
        assert_eq!(result_runs.len(), 1);
        assert_eq!(result_runs[0].len, "…".len());

        // 运行调整正确性：跨字符串的多个运行。
        // 验证返回的 runs 长度之和等于 result.len()。
        let multi_run_text = "abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz";
        let run_lens = [20, 20, multi_run_text.len() - 40];
        let runs = generate_test_runs(&run_lens);
        let (result, result_runs) = wrapper.truncate_line(
            multi_run_text.into(),
            px(100.),
            "…",
            &runs,
            TruncateFrom::Middle,
        );
        let total_run_len: usize = result_runs.iter().map(|r| r.len).sum();
        assert_eq!(
            total_run_len,
            result.len(),
            "Sum of run lengths ({}) should equal result byte length ({})",
            total_run_len,
            result.len()
        );
    }

    // 东亚禁则：CJK 逐字换行时避开行首禁则与行末禁则。
    // 无禁则时会把换行点放在 `。` / `「` 的另一侧，正是中西文混排最常见的排版错误。
    #[test]
    fn test_wrap_line_cjk_kinsoku() {
        let mut wrapper = build_wrapper();
        let char_width = wrapper.width_for_char('你');
        // 3.5 个字符宽：第 4 个字符处必须换行。
        let wrap_width = char_width * 3.5;

        // 行首禁则：`。` 不可起行 —— 换行点被推到 `世` 之前
        // （无禁则时会断在 `。` 之前，让句号跑到行首）。
        assert_eq!(
            wrapper
                .wrap_line(&[LineFragment::text("你好。世界")], wrap_width)
                .collect::<Vec<_>>(),
            &[Boundary::new(9, 0)]
        );

        // 行末禁则：`「` 不可收行 —— 换行点被拉回 `「` 之前
        // （无禁则时会断在 `世` 之前，让开引号留在行末）。
        assert_eq!(
            wrapper
                .wrap_line(&[LineFragment::text("你好「世界")], wrap_width)
                .collect::<Vec<_>>(),
            &[Boundary::new(6, 0)]
        );

        // 汉字之间正常断行不受影响（每行 3 个字符 + 缩进对齐，共两个换行点）。
        assert_eq!(
            wrapper
                .wrap_line(&[LineFragment::text("你好世界你好世界")], wrap_width)
                .collect::<Vec<_>>(),
            &[Boundary::new(9, 0), Boundary::new(18, 0)]
        );
    }

    // 回归：换行器池曾直接存放 `LineWrapper`，而包装器持有 `Arc<TextSystem>`，
    // 于是 TextSystem 永远回收不掉。池里只留字符宽度缓存后应能正常释放。
    #[rgpui::test]
    fn test_wrapper_pool_does_not_retain_text_system() {
        let cx = TestAppContext::build(TestDispatcher::new(0), None);
        let weak = {
            let text_system = cx.text_system().clone();
            let weak = Arc::downgrade(&text_system);
            let handle = text_system.line_wrapper(font(".ZedMono"), px(16.));
            drop(handle);
            weak
        };
        drop(cx);
        assert!(
            weak.upgrade().is_none(),
            "换行器池不应把 TextSystem 留在内存里"
        );
    }
}
