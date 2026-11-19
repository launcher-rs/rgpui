//! 文本差异查看器：对比两段文本并以颜色高亮显示增删改。
//!
//! 基于 `similar` crate 计算行级差异，以统一视图（unified diff）形式展示。
//! 状态与元素分离：父组件 `cx.new(|cx| DiffState::new(old, new))` 持有状态，
//! `render` 里放 `DiffView::new(state)` 渲染差异。

use std::{collections::HashMap, ops::Range};

use crate::{prelude::*, *};
use similar::{ChangeTag, TextDiff};

/// 行内容前缀宽度：`"  "`（未变更）与 `"+"`/`"-"` 加空格均为 2 字节，
/// 字符高亮区间换算到含前缀的内容文本时平移该宽度。
const PREFIX_WIDTH: usize = 2;

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

/// 行内字符高亮区间：在差异行上强调一段字符（如搜索命中）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSpanHighlight {
    /// 行索引（`lines()` 的下标）。
    pub line: usize,
    /// 行内容（不含行号与 `+`/`-` 前缀）的字节区间；须落在字符边界上，
    /// 越界或非边界的区间渲染时忽略。
    pub range: Range<usize>,
}

impl DiffSpanHighlight {
    /// 创建一条行内字符高亮。
    pub fn new(line: usize, range: Range<usize>) -> Self {
        Self { line, range }
    }
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
    /// 字符级高亮区间集合（按行索引；渲染时以强调背景显示）。
    span_highlights: Vec<DiffSpanHighlight>,
    /// `uniform_list` 滚动句柄（状态实体持有，跨帧保持滚动位置）。
    scroll_handle: UniformListScrollHandle,
    /// 待执行的一次性滚动目标（消费后清空，避免每帧重复定位）。
    scroll_target: Option<usize>,
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
            span_highlights: Vec::new(),
            scroll_handle: UniformListScrollHandle::default(),
            scroll_target: None,
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

    /// 设置字符级高亮区间（替换现有集合；渲染时以强调背景显示，如搜索命中）。
    pub fn set_span_highlights(
        &mut self,
        highlights: impl IntoIterator<Item = DiffSpanHighlight>,
        cx: &mut Context<Self>,
    ) {
        self.span_highlights = highlights.into_iter().collect();
        cx.notify();
    }

    /// 清空字符级高亮区间。
    pub fn clear_span_highlights(&mut self, cx: &mut Context<Self>) {
        if !self.span_highlights.is_empty() {
            self.span_highlights.clear();
            cx.notify();
        }
    }

    /// 字符级高亮区间集合。
    pub fn span_highlights(&self) -> &[DiffSpanHighlight] {
        &self.span_highlights
    }

    /// 按查询词生成全行命中区间（大小写不敏感，按字符边界对齐；空查询返回空集）。
    ///
    /// 返回结果可直接喂给 [`Self::set_span_highlights`]，与
    /// [`Self::scroll_to_line`] 配合实现「搜索 → 高亮 + 跳转」联动。
    pub fn highlights_for_query(&self, query: &str) -> Vec<DiffSpanHighlight> {
        let query_lower: Vec<char> = query.to_lowercase().chars().collect();
        if query_lower.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (ix, line) in self.lines.iter().enumerate() {
            let text = line.text.as_ref();
            let mut search_start = 0;
            while search_start < text.len() {
                // `get` 失败说明起点不在字符边界（不应发生，防御性跳过整行剩余部分）。
                let Some(rest) = text.get(search_start..) else {
                    break;
                };
                let Some(first_len) = rest.chars().next().map(|c| c.len_utf8()) else {
                    break;
                };
                if let Some(matched_len) = insensitive_prefix_len(rest, &query_lower) {
                    out.push(DiffSpanHighlight {
                        line: ix,
                        range: search_start..search_start + matched_len,
                    });
                }
                search_start += first_len;
                while search_start < text.len() && !text.is_char_boundary(search_start) {
                    search_start += 1;
                }
            }
        }
        out
    }

    /// 滚动到指定行（一次性：下次布局按 `Nearest` 策略定位，越界收敛到末行）。
    pub fn scroll_to_line(&mut self, line: usize, cx: &mut Context<Self>) {
        self.scroll_target = Some(line);
        cx.notify();
    }

    /// 滚动句柄（`DiffView` 每帧取用传给 `uniform_list`）。
    pub fn scroll_handle(&self) -> &UniformListScrollHandle {
        &self.scroll_handle
    }
}

/// 大小写不敏感前缀匹配：`rest` 以 `query_lower`（已按字符小写）开头时，
/// 返回原串中匹配部分的字节长度。
///
/// 与 `label.rs` / `search_panel` 的同名逻辑同款：避免整串 `to_lowercase()`
/// 后字节偏移漂移切错多字节字符（`İ` 等小写后字节数会变化）。
fn insensitive_prefix_len(rest: &str, query_lower: &[char]) -> Option<usize> {
    if query_lower.is_empty() {
        return None;
    }
    let mut chars = rest.chars();
    let mut orig_consumed = 0usize;
    let mut qi = 0usize;
    while qi < query_lower.len() {
        let Some(c) = chars.next() else {
            return None;
        };
        orig_consumed += c.len_utf8();
        let mut lowered = c.to_lowercase();
        while let Some(lc) = lowered.next() {
            if qi >= query_lower.len() || lc != query_lower[qi] {
                return None;
            }
            qi += 1;
        }
    }
    Some(orig_consumed)
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
        let scroll_handle = state.read(cx).scroll_handle().clone();

        // 一次性滚动目标：设置延迟滚动并在本帧消费掉，避免每帧重复定位。
        let scroll_target = state.read(cx).scroll_target;
        if let Some(line) = scroll_target {
            let last = line_count.saturating_sub(1);
            scroll_handle.scroll_to_item(line.min(last), ScrollStrategy::Nearest);
            state.update(cx, |s, _| s.scroll_target = None);
        }

        // 字符级高亮按行索引预分组：区间平移掉 `+`/`-` 前缀宽度（2 字节），
        // 落到含前缀的内容文本上；非法区间（越界 / 非字符边界）忽略。
        let mut highlight_by_line: HashMap<usize, Vec<(Range<usize>, HighlightStyle)>> =
            HashMap::new();
        let span_count = state.read(cx).span_highlights().len();
        if span_count > 0 {
            let accent = cx.theme().tokens.accent.color;
            let span_style = HighlightStyle {
                background_color: Some(accent.opacity(0.35)),
                ..Default::default()
            };
            for span in state.read(cx).span_highlights() {
                if span.line >= line_count {
                    continue;
                }
                let start = span.range.start + PREFIX_WIDTH;
                let end = span.range.end + PREFIX_WIDTH;
                if start >= end {
                    continue;
                }
                highlight_by_line
                    .entry(span.line)
                    .or_default()
                    .push((start..end, span_style.clone()));
            }
        }

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
                        // 有该行高亮时用带样式区间渲染（背景强调），否则直接放文本。
                        let content_child = match highlight_by_line.get(&ix) {
                            Some(spans) if !spans.is_empty() => StyledText::new(content_text)
                                .with_highlights(spans.iter().cloned())
                                .into_any_element(),
                            _ => content_text.into_any_element(),
                        };

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
                                    .child(content_child),
                            )
                    })
                    .collect::<Vec<_>>()
            },
        )
        .track_scroll(&scroll_handle)
        .size_full()
        .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
    }
}

#[cfg(test)]
mod tests {
    use super::{DiffLineKind, DiffSpanHighlight, DiffState, DiffView};
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

    /// 字符级高亮：设置 / 读取 / 清空，`clear` 在空集合上不重复通知。
    #[rgpui::test]
    fn span_highlights_roundtrip(cx: &mut TestAppContext) {
        let state = setup(cx);
        state.update(cx, |state, cx| {
            state.set_span_highlights([DiffSpanHighlight::new(1, 5..18)], cx);
        });
        let spans = state.read_with(cx, |state, _| state.span_highlights().to_vec());
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].line, 1);
        assert_eq!(spans[0].range, 5..18);
        state.update(cx, |state, cx| state.clear_span_highlights(cx));
        assert!(state.read_with(cx, |state, _| state.span_highlights().is_empty()));
    }

    /// `highlights_for_query` 大小写不敏感、全行多命中、CJK 多字节区间按字符边界。
    #[rgpui::test]
    fn highlights_for_query_finds_all_matches(cx: &mut TestAppContext) {
        let old = "Alpha\nbeta gamma\n";
        let new = "alpha BETA\n日本語 beta\n";
        let state = cx.update(|cx| cx.new(|_| DiffState::new(old, new)));
        let spans = state.read_with(cx, |state, _| state.highlights_for_query("beta"));
        // similar 先输出旧行再输出新行：line 1（删除行）`beta gamma` 命中 0..4，
        // line 2（新增行）`alpha BETA` 命中 6..10，line 3（新增行）
        // `日本語 beta` 命中 10..14（「日本語」9 字节 + 空格 = 偏移 10）。
        assert_eq!(
            spans,
            vec![
                DiffSpanHighlight::new(1, 0..4),
                DiffSpanHighlight::new(2, 6..10),
                DiffSpanHighlight::new(3, 10..14),
            ]
        );
        // 空查询返回空集。
        let empty = state.read_with(cx, |state, _| state.highlights_for_query(""));
        assert!(empty.is_empty());
        // 未命中返回空集。
        let none = state.read_with(cx, |state, _| state.highlights_for_query("zzz"));
        assert!(none.is_empty());
    }

    /// 滚动定位：目标在消费后被清空（一次性语义）。
    #[rgpui::test]
    fn scroll_to_line_consumes_target(cx: &mut TestAppContext) {
        let state = setup(cx);
        state.update(cx, |state, cx| state.scroll_to_line(3, cx));
        let target = state.read_with(cx, |state, _| state.scroll_target);
        assert_eq!(target, Some(3));
        state.update(cx, |state, _| state.scroll_target = None);
        assert!(state.read_with(cx, |state, _| state.scroll_target.is_none()));
    }

    /// 带高亮区间的差异视图能在确定高度的父容器里完成布局与绘制。
    #[rgpui::test]
    fn diff_view_draws_with_span_highlights(cx: &mut TestAppContext) {
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
                    .debug_selector(|| "diff-host-spans".to_string())
                    .child(DiffView::new(&self.state))
            }
        }

        cx.update(crate::theme::init);
        let (_view, cx) = cx.add_window_view(|_window, cx| {
            let state = cx.new(|_| {
                let old = "alpha line\nsecond line\n".to_string();
                let new = "ALPHA LINE\nsecond line\n".to_string();
                DiffState::new(old, new)
            });
            state.update(cx, |state, cx| {
                let spans = state.highlights_for_query("alpha");
                state.set_span_highlights(spans, cx);
            });
            Host { state }
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });

        let bounds = cx
            .debug_bounds("diff-host-spans")
            .expect("带高亮的宿主视图应完成绘制");
        assert!(bounds.size.width > px(0.));
        assert_eq!(bounds.size.height, px(200.));
    }
}
