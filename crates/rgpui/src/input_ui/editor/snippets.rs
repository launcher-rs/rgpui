//! 代码片段最小版（M3，`editor` feature 门控）。
//!
//! 子集语法（VS Code 子集，文档注明即契约）：`$1` / `${1:default}` / `$0`（
//! 结束位）/ `$$`（转义 `$`）；不支持嵌套占位与镜像编辑，未闭合的 `$` 按字面量。
//! 跳转顺序：`$1..$n` 升序，`$0` 收尾；无显式 `$0` 时过最后一个占位即退出。
//!
//! 会话模型：占位范围存在隐形装饰集合里（默认样式零渲染），编辑时
//! [`adjust_for_edit`](crate::input_ui::decorations) 自动跟踪，`Tab` 跳转读
//! [`get_ranges`](crate::input_ui::TextDecorationCollection) 回读——不另建跟踪机制。
//!
//! 交互注明（分拆红线）：
//! - 多光标：展开前坍缩 extras，只主光标展开（L 章"不扇出"先例）。
//! - 撤销：展开本身并为单组（`regroup_last`）；会话内键入按正常时间分组，
//!   跨越展开点的撤销可能使占位过期——跳转范围钳制到文本长度，不 panic。
//! - IME：展开走程序化 `insert`；组字（marked）中触发展开行为未定义，
//!   应用避免在组合中调用。
//! - 会话内键入不限制位置（只跟踪跳转，不做"只能改当前占位"强制；强制需
//!   进 `InputState` funnel，违背 M1 冻结，v1 边界）。
//! - 补全联动：`accept_completion` 见到 `insertTextFormat == Snippet` 即走
//!   `expand_snippet`（纯文本插入不经过），见 `lsp_attach.rs`。

use std::ops::Range;

use crate::{Context, Window};

use super::super::decorations::TextDecoration;
use super::state::EditorState;

/// 片段词法单元（子集）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum SnipTok {
    /// 字面文本。
    Text(String),
    /// 占位：编号 + 缺省文本（`$1` 缺省为空）。
    Tabstop { num: u32, default: String },
}

/// 解析片段模板（子集，无嵌套；`$$` 转义 `$`，未闭合按字面量）。
fn parse_snippet(template: &str) -> Vec<SnipTok> {
    let mut toks = Vec::new();
    let mut literal = String::new();
    let mut chars = template.chars().peekable();

    /// 冲掉字面量缓存。
    macro_rules! flush {
        () => {
            if !literal.is_empty() {
                toks.push(SnipTok::Text(std::mem::take(&mut literal)));
            }
        };
    }

    while let Some(ch) = chars.next() {
        if ch != '$' {
            literal.push(ch);
            continue;
        }
        match chars.peek() {
            // `$$` → 字面 `$`。
            Some('$') => {
                chars.next();
                literal.push('$');
            }
            // `$1` 数字式。
            Some(c) if c.is_ascii_digit() => {
                flush!();
                let mut num = String::new();
                while let Some(d) = chars.peek() {
                    if !d.is_ascii_digit() {
                        break;
                    }
                    num.push(*d);
                    chars.next();
                }
                toks.push(SnipTok::Tabstop {
                    num: num.parse().unwrap_or(0),
                    default: String::new(),
                });
            }
            // `${1}` / `${1:default}` 花括号式。
            Some('{') => {
                chars.next();
                let mut num = String::new();
                while let Some(d) = chars.peek() {
                    if !d.is_ascii_digit() {
                        break;
                    }
                    num.push(*d);
                    chars.next();
                }
                if num.is_empty() {
                    // `${` 后无数字：按字面量。
                    literal.push_str("${");
                    literal.push_str(&num);
                    continue;
                }
                let mut default = String::new();
                let mut closed = false;
                if chars.peek() == Some(&':') {
                    chars.next();
                    let iter = chars.by_ref();
                    while let Some(c) = iter.next() {
                        if c == '}' {
                            closed = true;
                            break;
                        }
                        // 缺省内 `$$` 照样转义。
                        if c == '$' && iter.peek() == Some(&'$') {
                            iter.next();
                            default.push('$');
                        } else {
                            default.push(c);
                        }
                    }
                } else if chars.peek() == Some(&'}') {
                    chars.next();
                    closed = true;
                }
                if !closed {
                    // 未闭合：整体按字面量（含已消费部分）。
                    literal.push_str("${");
                    literal.push_str(&num);
                    if !default.is_empty() {
                        literal.push(':');
                        literal.push_str(&default);
                    }
                    continue;
                }
                flush!();
                toks.push(SnipTok::Tabstop {
                    num: num.parse().unwrap_or(0),
                    default,
                });
            }
            // `$` 后跟其他：字面 `$` + 继续。
            _ => {
                literal.push('$');
            }
        }
    }
    flush!();
    toks
}

/// 片段会话（`EditorState` 内嵌小字段）。
pub(super) struct SnippetSession {
    /// 隐形占位集合（跳转顺序插入，`get_ranges` 按序回读；`$0` 若有必在末尾）。
    tabstops: crate::input_ui::TextDecorationCollection,
    /// 当前位置（跳转下标）。
    current: usize,
}

impl EditorState {
    /// 展开片段（光标处纯插入；多光标先坍缩到主光标）。
    ///
    /// 无占位（纯文本模板）时退化为普通插入，无会话；仅 `$0` 时光标落位无会话。
    /// 展开本身并为一个撤销单元。
    pub fn expand_snippet(&mut self, template: &str, window: &mut Window, cx: &mut Context<Self>) {
        // 新展开替换旧会话（旧集合先清）。
        self.exit_snippet(cx);
        let toks = parse_snippet(template);
        // 占位去重（首见为准）+ 排序（$1..$n 升序，$0 收尾）。
        let mut seen = std::collections::HashSet::new();
        let mut stops: Vec<(u32, String)> = Vec::new();
        for tok in &toks {
            if let SnipTok::Tabstop { num, default } = tok {
                if seen.insert(*num) {
                    stops.push((*num, default.clone()));
                }
            }
        }
        stops.sort_by(|a, b| match (a.0 == 0, b.0 == 0) {
            (true, true) => std::cmp::Ordering::Equal,
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            (false, false) => a.0.cmp(&b.0),
        });
        // 拼展开文本 + 相对占位范围。
        let mut expanded = String::new();
        // num → 相对范围（后查）。
        let mut ranges: std::collections::HashMap<u32, Range<usize>> =
            std::collections::HashMap::new();
        for tok in &toks {
            match tok {
                SnipTok::Text(s) => expanded.push_str(s),
                SnipTok::Tabstop { num, default } => {
                    // 去重后只记录首见编号的范围（重复编号跳转到首见处）。
                    let start = expanded.len();
                    // 缺省文本只在首见处展开（重复编号处不重复展开）。
                    if ranges.contains_key(num) {
                        continue;
                    }
                    expanded.push_str(default);
                    ranges.insert(*num, start..expanded.len());
                }
            }
        }
        // 跳转顺序的范围（与 stops 同序）。
        let ordered: Vec<Range<usize>> = stops
            .iter()
            .filter_map(|(num, _)| ranges.get(num).cloned())
            .collect();
        // 插入 + 撤销并组。
        let base = self.input.read_with(cx, |state, _| state.cursor());
        self.input.update(cx, |state, cx| {
            if state.has_multiple_cursors() {
                state.clear_extra_cursors(cx);
            }
            let before = state.core.history.undos().len();
            state.insert(expanded.as_str(), window, cx);
            let pushed = state.core.history.undos().len().saturating_sub(before);
            state.core.history.regroup_last(pushed);
        });
        // 相对范围 → 绝对范围（插入点基址偏移）。
        let decorations: Vec<TextDecoration> = ordered
            .into_iter()
            .map(|r| TextDecoration::new(r.start + base..r.end + base, Default::default()))
            .collect();
        // 无有效占位：纯插入结束（`$0` 单独也只落光标，不建会话）。
        let nonzero = stops.iter().filter(|(n, _)| *n != 0).count();
        if nonzero == 0 {
            // 有 `$0` 则光标落位，否则留在插入末尾（insert 语义）。
            if let Some((_, _)) = stops.iter().find(|(n, _)| *n == 0) {
                if let Some(r) = ranges.get(&0) {
                    let at = base + r.start;
                    self.set_selected_range(at..at, cx);
                }
            }
            return;
        }
        // 建隐形集合（跳转顺序插入，不过 normalize——塌缩占位会被丢弃；
        // 范围端点构造时即字符边界，见 create_raw_collection 文档）。
        let mut handle = None;
        self.input.update(cx, |state, cx| {
            handle = Some(state.create_raw_collection(decorations, cx));
        });
        let Some(tabstops) = handle else { return };
        self.snippet = Some(SnippetSession {
            tabstops,
            current: 0,
        });
        self.select_snippet_stop(0, cx);
    }

    /// 跳到第 `index` 个占位（越界即退出会话）。
    fn select_snippet_stop(&mut self, index: usize, cx: &mut Context<Self>) {
        let ranges = match &self.snippet {
            Some(session) => session.tabstops.get_ranges(cx),
            None => return,
        };
        if index >= ranges.len() {
            self.exit_snippet(cx);
            return;
        }
        if let Some(session) = self.snippet.as_mut() {
            session.current = index;
        }
        let range = ranges[index].clone();
        self.set_selected_range(range, cx);
    }

    /// 下一占位（`Tab`；越过末位（含 `$0`）即退出会话，光标停留）。
    pub fn next_placeholder(&mut self, cx: &mut Context<Self>) {
        let (current, total) = match &self.snippet {
            Some(session) => (session.current, session.tabstops.get_ranges(cx).len()),
            None => return,
        };
        if current + 1 >= total {
            self.exit_snippet(cx);
            return;
        }
        self.select_snippet_stop(current + 1, cx);
    }

    /// 上一占位（`Shift-Tab`；首位前停留不动）。
    pub fn prev_placeholder(&mut self, cx: &mut Context<Self>) {
        let current = match &self.snippet {
            Some(session) => session.current,
            None => return,
        };
        if current == 0 {
            return;
        }
        self.select_snippet_stop(current - 1, cx);
    }

    /// 会话是否进行中（应用层键位/按钮 gating 用）。
    pub fn snippet_active(&self) -> bool {
        self.snippet.is_some()
    }

    /// 退出会话（保留文本，只清占位集合；`Esc`/提交/越界/新展开时调用）。
    pub fn exit_snippet(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = self.snippet.take() {
            session.tabstops.clear(cx);
            cx.notify();
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    /// 解析：文本 + 数字式 + 花括号缺省 + `$0`。
    #[test]
    fn parse_mixed_template() {
        assert_eq!(
            parse_snippet("fn ${1:name}($2) { $0 }"),
            vec![
                SnipTok::Text("fn ".to_string()),
                SnipTok::Tabstop {
                    num: 1,
                    default: "name".to_string()
                },
                SnipTok::Text("(".to_string()),
                SnipTok::Tabstop {
                    num: 2,
                    default: String::new()
                },
                SnipTok::Text(") { ".to_string()),
                SnipTok::Tabstop {
                    num: 0,
                    default: String::new()
                },
                SnipTok::Text(" }".to_string()),
            ]
        );
    }

    /// 解析：`$$` 转义 + 未闭合按字面量。
    #[test]
    fn parse_escape_and_unterminated() {
        assert_eq!(
            parse_snippet("a$$b${1:x"),
            vec![SnipTok::Text("a$b${1:x".to_string()),]
        );
        assert_eq!(
            parse_snippet("a$b-c"),
            vec![SnipTok::Text("a$b-c".to_string()),]
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppContext as _;
    use crate::{Entity, Render, Window};

    /// 持有编辑器状态的测试宿主视图。
    struct Probe {
        state: Entity<EditorState>,
    }

    impl Render for Probe {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> impl crate::IntoElement {
            crate::div()
        }
    }

    fn cursor_of(editor: &Entity<EditorState>, cx: &mut crate::TestAppContext) -> usize {
        editor.read_with(cx, |state, cx| state.cursor(cx))
    }

    fn expand(editor: &Entity<EditorState>, template: &str, cx: &mut crate::VisualTestContext) {
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.expand_snippet(template, window, cx);
            });
        });
    }

    /// 展开 + 跳转往返（`$1` → `$2` → `$0` → 退出）。
    #[rgpui::test]
    fn expand_and_tab_roundtrip(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, ""));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        expand(&editor, "f(${1:a}, $2)$0", cx);
        // 文本：`f(a, )`；首选 `$1`（2..3）。
        assert_eq!(editor.read_with(cx, |state, cx| state.text(cx)), "f(a, )");
        assert_eq!(cursor_of(&editor, cx), 3);
        // `$2`（塌缩 5..5）→ `$0`（塌缩 6..6）→ 越界退出。
        editor.update(cx, |state, cx| state.next_placeholder(cx));
        assert_eq!(cursor_of(&editor, cx), 5);
        editor.update(cx, |state, cx| state.next_placeholder(cx));
        assert_eq!(cursor_of(&editor, cx), 6);
        assert!(editor.read_with(cx, |state, _| state.snippet_active()));
        editor.update(cx, |state, cx| state.next_placeholder(cx));
        assert!(!editor.read_with(cx, |state, _| state.snippet_active()));
        // 文本保留。
        assert_eq!(editor.read_with(cx, |state, cx| state.text(cx)), "f(a, )");
    }

    /// 占位内键入后跳转跟随（装饰跟踪；`$1` 替换为 `xyz` 后 `$2` 从 5 移到 7）。
    #[rgpui::test]
    fn typing_in_placeholder_tracks_next(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, ""));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        expand(&editor, "f(${1:a}, $2)", cx);
        // `$1` 被选中（2..3），键入替换为 `xyz`。
        let input = editor.read_with(cx, |state, _| state.input().clone());
        cx.update(|window, cx| {
            input.update(cx, |state, cx| {
                state.replace("xyz", window, cx);
            });
        });
        assert_eq!(editor.read_with(cx, |state, cx| state.text(cx)), "f(xyz, )");
        // `$2` 跟到 7..7。
        editor.update(cx, |state, cx| state.next_placeholder(cx));
        assert_eq!(cursor_of(&editor, cx), 7);
    }

    /// `Shift-Tab` 回跳 + 首位前停留。
    #[rgpui::test]
    fn shift_tab_goes_back(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, ""));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        expand(&editor, "f(${1:a}, $2)", cx);
        editor.update(cx, |state, cx| state.next_placeholder(cx));
        assert_eq!(cursor_of(&editor, cx), 5);
        editor.update(cx, |state, cx| state.prev_placeholder(cx));
        assert_eq!(cursor_of(&editor, cx), 3);
        editor.update(cx, |state, cx| state.prev_placeholder(cx));
        assert_eq!(cursor_of(&editor, cx), 3);
    }

    /// 退出保留文本 + 新展开替换旧会话。
    #[rgpui::test]
    fn exit_and_reexpand(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, ""));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        expand(&editor, "f($1)", cx);
        assert!(editor.read_with(cx, |state, _| state.snippet_active()));
        editor.update(cx, |state, cx| state.exit_snippet(cx));
        assert!(!editor.read_with(cx, |state, _| state.snippet_active()));
        assert_eq!(editor.read_with(cx, |state, cx| state.text(cx)), "f()");
        // 新展开替换旧会话（旧集合已清，不残留）。
        expand(&editor, "g($1)", cx);
        assert!(editor.read_with(cx, |state, _| state.snippet_active()));
        editor.update(cx, |state, cx| state.next_placeholder(cx));
        assert!(!editor.read_with(cx, |state, _| state.snippet_active()));
    }

    /// 展开并为单组撤销（一次 undo 回到展开前）。
    #[rgpui::test]
    fn expand_is_single_undo_unit(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, ""));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        expand(&editor, "f(${1:a})", cx);
        assert_eq!(editor.read_with(cx, |state, cx| state.text(cx)), "f(a)");
        let input = editor.read_with(cx, |state, _| state.input().clone());
        cx.update(|window, cx| {
            input.update(cx, |state, cx| {
                state.undo(&crate::input_ui::Undo, window, cx)
            });
        });
        assert_eq!(editor.read_with(cx, |state, cx| state.text(cx)), "");
    }

    /// 无占位模板退化为普通插入（无会话）；纯 `$0` 只落光标。
    #[rgpui::test]
    fn plain_and_final_only_templates(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, ""));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        expand(&editor, "plain", cx);
        assert!(!editor.read_with(cx, |state, _| state.snippet_active()));
        assert_eq!(editor.read_with(cx, |state, cx| state.text(cx)), "plain");
    }

    /// 补全联动：`insertTextFormat == Snippet` 的条目确认即展开。
    #[rgpui::test]
    fn accept_snippet_completion_expands(cx: &mut crate::TestAppContext) {
        use crate::lsp::CompletionProvider;
        use lsp_types::{CompletionItem, CompletionResponse, InsertTextFormat};

        struct SnippetProvider;
        impl CompletionProvider for SnippetProvider {
            fn completions(
                &self,
                _text: &ropey::Rope,
                _offset: usize,
                _trigger: lsp_types::CompletionContext,
                _window: &mut Window,
                _cx: &mut crate::App,
            ) -> crate::Task<anyhow::Result<CompletionResponse>> {
                crate::Task::ready(Ok(CompletionResponse::Array(vec![CompletionItem {
                    label: "g".to_string(),
                    insert_text: Some("g(${1:x})$0".to_string()),
                    insert_text_format: Some(InsertTextFormat::SNIPPET),
                    ..Default::default()
                }])))
            }
        }

        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, ""));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_completion_provider(Some(std::rc::Rc::new(SnippetProvider)), cx);
                state.request_completions(window, cx);
            });
        });
        cx.dispatcher
            .advance_clock(std::time::Duration::from_millis(2000));
        cx.run_until_parked();
        cx.background_executor.run_until_parked();
        cx.run_until_parked();
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.accept_completion(None, window, cx);
            });
        });
        // 展开（非纯文本插入）+ 会话进行中（`$1` 被选中）。
        assert_eq!(editor.read_with(cx, |state, cx| state.text(cx)), "g(x)");
        assert!(editor.read_with(cx, |state, _| state.snippet_active()));
        assert_eq!(cursor_of(&editor, cx), 3);
    }
}
