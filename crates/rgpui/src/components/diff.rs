//! 文本差异查看器：对比两段文本并以颜色高亮显示增删改。
//!
//! 基于 `similar` crate 计算行级差异，以统一视图（unified diff）形式展示。
//! 状态与元素分离：父组件 `cx.new(|cx| DiffState::new(old, new))` 持有状态，
//! `render` 里放 `DiffView::new(state)` 渲染差异。

use crate::{prelude::*, *};
use similar::{ChangeTag, TextDiff};

/// 差异行类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    /// 未变更行。
    Unchanged,
    /// 新增行。
    Added,
    /// 删除行。
    Removed,
}

/// 差异行。
#[derive(Debug, Clone)]
pub struct DiffLine {
    /// 行内容（不含换行符）。
    pub text: SharedString,
    /// 行类型。
    pub kind: DiffLineKind,
    /// 旧文件行号（从 1 开始，`None` 表示新增行）。
    pub old_line: Option<usize>,
    /// 新文件行号（从 1 开始，`None` 表示删除行）。
    pub new_line: Option<usize>,
}

/// 差异状态实体。
pub struct DiffState {
    /// 差异行列表。
    lines: Vec<DiffLine>,
    /// 是否显示行号。
    show_line_numbers: bool,
}

impl DiffState {
    /// 创建差异状态，计算两段文本的行级差异。
    pub fn new(old: impl Into<String>, new: impl Into<String>) -> Self {
        let old_text = old.into();
        let new_text = new.into();
        let diff = TextDiff::from_lines(&old_text, &new_text);

        let mut lines = Vec::new();
        let mut old_line = 1;
        let mut new_line = 1;

        for change in diff.iter_all_changes() {
            let tag = change.tag();
            // 逐行输入可能带 CRLF：去掉行尾的 `\n` 与紧随的 `\r`，
            // 否则视图里会残留一个不可见的回车（行号对齐也会被撑开）。
            let raw = change.as_str().unwrap_or("");
            let text = raw.strip_suffix('\n').unwrap_or(raw);
            let text = text.strip_suffix('\r').unwrap_or(text).to_string();

            match tag {
                ChangeTag::Equal => {
                    lines.push(DiffLine {
                        text: text.into(),
                        kind: DiffLineKind::Unchanged,
                        old_line: Some(old_line),
                        new_line: Some(new_line),
                    });
                    old_line += 1;
                    new_line += 1;
                }
                ChangeTag::Delete => {
                    lines.push(DiffLine {
                        text: text.into(),
                        kind: DiffLineKind::Removed,
                        old_line: Some(old_line),
                        new_line: None,
                    });
                    old_line += 1;
                }
                ChangeTag::Insert => {
                    lines.push(DiffLine {
                        text: text.into(),
                        kind: DiffLineKind::Added,
                        old_line: None,
                        new_line: Some(new_line),
                    });
                    new_line += 1;
                }
            }
        }

        Self {
            lines,
            show_line_numbers: true,
        }
    }

    /// 设置是否显示行号。
    pub fn show_line_numbers(mut self, show: bool) -> Self {
        self.show_line_numbers = show;
        self
    }

    /// 差异行列表。
    pub fn lines(&self) -> &[DiffLine] {
        &self.lines
    }

    /// 是否显示行号。
    pub fn is_show_line_numbers(&self) -> bool {
        self.show_line_numbers
    }

    /// 统计：新增行数。
    pub fn added_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.kind == DiffLineKind::Added)
            .count()
    }

    /// 统计：删除行数。
    pub fn removed_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.kind == DiffLineKind::Removed)
            .count()
    }

    /// 统计：未变更行数。
    pub fn unchanged_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.kind == DiffLineKind::Unchanged)
            .count()
    }
}

impl Render for DiffState {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// 差异查看器元素。
#[derive(IntoElement)]
pub struct DiffView {
    /// 差异状态实体。
    state: Entity<DiffState>,
    /// 行高。
    line_height: Pixels,
}

impl DiffView {
    /// 创建差异查看器。
    pub fn new(state: &Entity<DiffState>) -> Self {
        Self {
            state: state.clone(),
            line_height: px(20.0),
        }
    }

    /// 设置行高。
    pub fn line_height(mut self, height: impl Into<Pixels>) -> Self {
        self.line_height = height.into();
        self
    }
}

impl RenderOnce for DiffView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state;
        let entity_id = state.entity_id();
        let line_count = state.read(cx).lines().len();
        let show_line_numbers = state.read(cx).is_show_line_numbers();
        let line_height = self.line_height;

        let theme = cx.theme();
        let added_bg = theme.tokens.success.color.opacity(0.15);
        let removed_bg = theme.tokens.danger.color.opacity(0.15);
        let added_fg = theme.tokens.success.color;
        let removed_fg = theme.tokens.danger.color;
        let text_color = theme.tokens.foreground.color;
        let line_number_color = theme.tokens.muted_foreground.color;

        let line_number_width = if show_line_numbers { px(60.0) } else { px(0.0) };

        // 差异动辄上千行，逐行 `div` 会把整份文本都排进布局；交给 `uniform_list`
        // 只渲染视口内的行。横向不约束，长行可横向滚动而不是折行破坏「一行一高」。
        uniform_list(
            ("diff-view", entity_id),
            line_count,
            move |range, _window, cx| {
                let state = state.read(cx);
                let lines = state.lines();

                range
                    .map(|ix| {
                        let line = &lines[ix];
                        let (bg, fg) = match line.kind {
                            DiffLineKind::Unchanged => (None, text_color),
                            DiffLineKind::Added => (Some(added_bg), added_fg),
                            DiffLineKind::Removed => (Some(removed_bg), removed_fg),
                        };

                        let prefix = match line.kind {
                            DiffLineKind::Unchanged => "  ",
                            DiffLineKind::Added => "+ ",
                            DiffLineKind::Removed => "- ",
                        };

                        let line_number_text = if show_line_numbers {
                            match (line.old_line, line.new_line) {
                                (Some(old), Some(new)) => format!("{old:>3} {new:>3} "),
                                (Some(old), None) => format!("{old:>3}     "),
                                (None, Some(new)) => format!("    {new:>3} "),
                                (None, None) => "        ".to_string(),
                            }
                        } else {
                            String::new()
                        };

                        let content_text = format!("{prefix}{}", line.text);

                        h_flex()
                            .h(line_height)
                            .w_full()
                            .when_some(bg, |el, bg| el.bg(bg))
                            .child(
                                div()
                                    .w(line_number_width)
                                    .text_color(line_number_color)
                                    .child(line_number_text),
                            )
                            .child(
                                div()
                                    .flex_grow(1.0)
                                    .whitespace_nowrap()
                                    .text_color(fg)
                                    .child(content_text),
                            )
                    })
                    .collect::<Vec<_>>()
            },
        )
        .size_full()
        .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
    }
}

#[cfg(test)]
mod tests {
    use super::{DiffLineKind, DiffState, DiffView};
    use crate::{
        AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
        Render, Styled as _, TestAppContext, Window, div, px,
    };

    /// 创建测试用的 DiffState。
    fn setup(cx: &mut TestAppContext) -> crate::Entity<DiffState> {
        let old = "line 1\nline 2\nline 3\n";
        let new = "line 1\nline 2 modified\nline 3\nline 4\n";
        cx.update(|cx| cx.new(|_| DiffState::new(old, new)))
    }

    /// 基本差异计算：新增、删除、未变更行。
    #[rgpui::test]
    fn diff_computes_changes(cx: &mut TestAppContext) {
        let state = setup(cx);

        let line_count = state.read_with(cx, |state, _| state.lines().len());
        assert_eq!(line_count, 5);

        let line_0 = state.read_with(cx, |state, _| {
            (state.lines()[0].kind, state.lines()[0].text.clone())
        });
        assert_eq!(line_0.0, DiffLineKind::Unchanged);
        assert_eq!(line_0.1.as_ref(), "line 1");

        let line_1 = state.read_with(cx, |state, _| {
            (state.lines()[1].kind, state.lines()[1].text.clone())
        });
        assert_eq!(line_1.0, DiffLineKind::Removed);
        assert_eq!(line_1.1.as_ref(), "line 2");

        let line_2 = state.read_with(cx, |state, _| {
            (state.lines()[2].kind, state.lines()[2].text.clone())
        });
        assert_eq!(line_2.0, DiffLineKind::Added);
        assert_eq!(line_2.1.as_ref(), "line 2 modified");
    }

    /// 统计方法正确计数。
    #[rgpui::test]
    fn diff_counts(cx: &mut TestAppContext) {
        let state = setup(cx);

        let (added, removed, unchanged) = state.read_with(cx, |state, _| {
            (
                state.added_count(),
                state.removed_count(),
                state.unchanged_count(),
            )
        });
        assert_eq!(added, 2);
        assert_eq!(removed, 1);
        assert_eq!(unchanged, 2);
    }

    /// 行号正确分配。
    #[rgpui::test]
    fn diff_line_numbers(cx: &mut TestAppContext) {
        let state = setup(cx);

        let line_0 = state.read_with(cx, |state, _| {
            (state.lines()[0].old_line, state.lines()[0].new_line)
        });
        assert_eq!(line_0.0, Some(1));
        assert_eq!(line_0.1, Some(1));

        let line_1 = state.read_with(cx, |state, _| {
            (state.lines()[1].old_line, state.lines()[1].new_line)
        });
        assert_eq!(line_1.0, Some(2));
        assert_eq!(line_1.1, None);

        let line_2 = state.read_with(cx, |state, _| {
            (state.lines()[2].old_line, state.lines()[2].new_line)
        });
        assert_eq!(line_2.0, None);
        assert_eq!(line_2.1, Some(2));
    }

    /// show_line_numbers 开关生效。
    #[rgpui::test]
    fn show_line_numbers_toggle(cx: &mut TestAppContext) {
        let old = "a\n";
        let new = "b\n";
        let state = cx.update(|cx| cx.new(|_| DiffState::new(old, new).show_line_numbers(false)));

        let show = state.read_with(cx, |state, _| state.is_show_line_numbers());
        assert!(!show);
    }

    /// 空文本差异。
    #[rgpui::test]
    fn empty_diff(cx: &mut TestAppContext) {
        let state = cx.update(|cx| cx.new(|_| DiffState::new("", "")));
        let len = state.read_with(cx, |state, _| state.lines().len());
        assert_eq!(len, 0);
    }

    /// 相同文本无差异。
    #[rgpui::test]
    fn identical_text(cx: &mut TestAppContext) {
        let text = "line 1\nline 2\n";
        let state = cx.update(|cx| cx.new(|_| DiffState::new(text, text)));

        let (len, all_unchanged) = state.read_with(cx, |state, _| {
            let len = state.lines().len();
            let all_unchanged = state
                .lines()
                .iter()
                .all(|l| l.kind == DiffLineKind::Unchanged);
            (len, all_unchanged)
        });
        assert_eq!(len, 2);
        assert!(all_unchanged);
    }

    /// CRLF 文本的行尾 `\r` 不应留在行内容里。
    #[rgpui::test]
    fn diff_strips_carriage_return(cx: &mut TestAppContext) {
        let state = cx.update(|cx| cx.new(|_| DiffState::new("a\r\nb\r\n", "a\r\nc\r\n")));
        let texts = state.read_with(cx, |state, _| {
            state
                .lines()
                .iter()
                .map(|line| line.text.to_string())
                .collect::<Vec<_>>()
        });
        assert_eq!(
            texts,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    /// 差异视图能在确定高度的父容器里完成布局与绘制（`uniform_list` 的前置条件）。
    #[rgpui::test]
    fn diff_view_draws_in_bounded_container(cx: &mut TestAppContext) {
        /// 只提供固定高度父容器的宿主视图。
        struct Host {
            state: Entity<DiffState>,
        }

        impl Render for Host {
            fn render(
                &mut self,
                _window: &mut Window,
                _cx: &mut Context<Self>,
            ) -> impl IntoElement {
                div()
                    .w(px(400.))
                    .h(px(200.))
                    .debug_selector(|| "diff-host".to_string())
                    .child(DiffView::new(&self.state))
            }
        }

        cx.update(crate::theme::init);
        let (_view, cx) = cx.add_window_view(|_window, cx| {
            let state = cx.new(|_| {
                let old = (0..200)
                    .map(|ix| format!("old line {ix}\n"))
                    .collect::<String>();
                let new = (0..200)
                    .map(|ix| format!("new line {ix}\n"))
                    .collect::<String>();
                DiffState::new(old, new)
            });
            Host { state }
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });

        let bounds = cx.debug_bounds("diff-host").expect("宿主视图应完成绘制");
        assert!(bounds.size.width > px(0.));
        assert_eq!(bounds.size.height, px(200.));
    }
}
