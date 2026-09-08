use super::super::TabSize;
use super::super::display_map::DisplayMap;

/// 输入模式枚举：纯文本（单行/多行 + 可选行号/折叠/引导线）与自动增长。
#[derive(Clone)]
pub(crate) enum InputMode {
    /// 纯文本输入模式（多行时可选行号/折叠/缩进引导线）。
    PlainText {
        multi_line: bool,
        tab: TabSize,
        rows: usize,
        /// 是否显示行号（仅多行生效）。
        line_number: bool,
        /// 是否启用代码折叠（仅多行生效）。
        folding: bool,
        /// 是否显示缩进引导线（仅多行生效）。
        indent_guides: bool,
    },
    /// 自动增长输入模式。
    AutoGrow {
        rows: usize,
        min_rows: usize,
        max_rows: usize,
    },
}

impl Default for InputMode {
    fn default() -> Self {
        InputMode::plain_text()
    }
}

impl InputMode {
    /// 创建带默认设置的纯文本输入模式。
    pub(super) fn plain_text() -> Self {
        InputMode::PlainText {
            multi_line: false,
            tab: TabSize::default(),
            rows: 1,
            line_number: false,
            folding: false,
            indent_guides: false,
        }
    }

    /// 创建给定最小和最大行数的自动增长输入模式。
    pub(super) fn auto_grow(min_rows: usize, max_rows: usize) -> Self {
        InputMode::AutoGrow {
            rows: min_rows,
            min_rows,
            max_rows,
        }
    }

    pub(super) fn multi_line(mut self, multi_line: bool) -> Self {
        match &mut self {
            InputMode::PlainText { multi_line: ml, .. } => *ml = multi_line,
            InputMode::AutoGrow { .. } => {}
        }
        self
    }

    #[inline]
    pub(crate) fn is_single_line(&self) -> bool {
        !self.is_multi_line()
    }

    /// 当多行且 `folding: true` 时返回 true。
    #[inline]
    pub(crate) fn is_folding(&self) -> bool {
        matches!(
            self,
            InputMode::PlainText {
                folding: true,
                multi_line: true,
                ..
            }
        )
    }

    #[inline]
    pub(super) fn is_auto_grow(&self) -> bool {
        matches!(self, InputMode::AutoGrow { .. })
    }

    #[inline]
    pub(crate) fn is_multi_line(&self) -> bool {
        match self {
            InputMode::PlainText { multi_line, .. } => *multi_line,
            InputMode::AutoGrow { max_rows, .. } => *max_rows > 1,
        }
    }

    pub(super) fn set_rows(&mut self, new_rows: usize) {
        match self {
            InputMode::PlainText { rows, .. } => {
                *rows = new_rows;
            }
            InputMode::AutoGrow {
                rows,
                min_rows,
                max_rows,
            } => {
                *rows = new_rows.clamp(*min_rows, *max_rows);
            }
        }
    }

    pub(super) fn update_auto_grow(&mut self, display_map: &DisplayMap) {
        if self.is_single_line() {
            return;
        }

        let wrapped_lines = display_map.wrap_row_count();
        self.set_rows(wrapped_lines);
    }

    /// 至少返回 1 行。
    pub(super) fn rows(&self) -> usize {
        if !self.is_multi_line() {
            return 1;
        }

        match self {
            InputMode::PlainText { rows, .. } => *rows,
            InputMode::AutoGrow { rows, .. } => *rows,
        }
        .max(1)
    }

    pub(super) fn max_rows(&self) -> usize {
        if !self.is_multi_line() {
            return 1;
        }

        match self {
            InputMode::AutoGrow { max_rows, .. } => *max_rows,
            _ => usize::MAX,
        }
    }

    /// 当多行且 `line_number: true` 时返回 true（单行/自动增长恒 false）。
    #[inline]
    pub(super) fn line_number(&self) -> bool {
        match self {
            InputMode::PlainText {
                line_number,
                multi_line,
                ..
            } => *line_number && *multi_line,
            _ => false,
        }
    }

    /// 当多行且 `indent_guides: true` 时返回 true。
    #[inline]
    pub(crate) fn has_indent_guides(&self) -> bool {
        match self {
            InputMode::PlainText {
                indent_guides,
                multi_line,
                ..
            } => *indent_guides && *multi_line,
            _ => false,
        }
    }

    /// 是否带编辑器铬（行号或折叠）：编辑器背景 + 底部留白 + 滚动边距都认它。
    #[inline]
    pub(super) fn has_editor_chrome(&self) -> bool {
        self.line_number() || self.is_folding()
    }

    #[inline]
    pub(crate) fn tab_size(&self) -> TabSize {
        match self {
            InputMode::PlainText { tab, .. } => *tab,
            _ => TabSize::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::InputMode;
    use crate::input_ui::TabSize;

    #[test]
    fn test_multiline_options() {
        // 多行 + 行号/折叠/引导线全开：编辑器铬齐备。
        let mode = InputMode::PlainText {
            multi_line: true,
            tab: TabSize::default(),
            rows: 2,
            line_number: true,
            folding: true,
            indent_guides: true,
        };
        assert_eq!(mode.is_multi_line(), true);
        assert_eq!(mode.is_single_line(), false);
        assert_eq!(mode.line_number(), true);
        assert_eq!(mode.has_indent_guides(), true);
        assert_eq!(mode.is_folding(), true);
        assert_eq!(mode.has_editor_chrome(), true);
        assert_eq!(mode.max_rows(), usize::MAX);

        // 单行下选项全部失效（防呆：构建器只在多行写，读侧同步门控）。
        let mode = InputMode::PlainText {
            multi_line: false,
            line_number: true,
            indent_guides: true,
            folding: true,
            rows: 0,
            tab: Default::default(),
        };
        assert_eq!(mode.is_multi_line(), false);
        assert_eq!(mode.is_single_line(), true);
        assert_eq!(mode.line_number(), false);
        assert_eq!(mode.has_indent_guides(), false);
        assert_eq!(mode.max_rows(), 1);
        assert_eq!(mode.is_folding(), false);
        assert_eq!(mode.has_editor_chrome(), false);
    }

    #[test]
    fn test_plain() {
        let mode = InputMode::PlainText {
            multi_line: true,
            tab: TabSize::default(),
            rows: 5,
            line_number: false,
            folding: false,
            indent_guides: false,
        };
        assert_eq!(mode.is_multi_line(), true);
        assert_eq!(mode.is_single_line(), false);
        assert_eq!(mode.line_number(), false);
        assert_eq!(mode.has_editor_chrome(), false);
        assert_eq!(mode.rows(), 5);
        assert_eq!(mode.max_rows(), usize::MAX);

        let mode = InputMode::plain_text();
        assert_eq!(mode.is_multi_line(), false);
        assert_eq!(mode.is_single_line(), true);
        assert_eq!(mode.line_number(), false);
        assert_eq!(mode.max_rows(), 1);
    }

    #[test]
    fn test_auto_grow() {
        let mut mode = InputMode::auto_grow(2, 5);
        assert_eq!(mode.is_multi_line(), true);
        assert_eq!(mode.is_single_line(), false);
        assert_eq!(mode.line_number(), false);
        assert_eq!(mode.rows(), 2);
        assert_eq!(mode.max_rows(), 5);

        mode.set_rows(4);
        assert_eq!(mode.rows(), 4);

        mode.set_rows(1);
        assert_eq!(mode.rows(), 2);

        mode.set_rows(10);
        assert_eq!(mode.rows(), 5);
    }
}
