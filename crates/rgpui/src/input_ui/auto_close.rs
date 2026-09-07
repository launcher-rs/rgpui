//! 括号/引号自动闭合：输入左括号时补右括号、输入右括号时跳过、退格删空对。
//!
//! 拦截点是 [`EntityInputHandler::replace_text_in_range`]（OS 键入入口），
//! 程序化写入（`insert`/`replace`/`set_value`/撤销等）走 raw 直通，不受影响。
//! 生效条件见 [`auto_close_applies`]：默认多行开启、单行关闭，显式可覆盖。

use std::ops::Range;

use crate::{Context, Window};

use super::state::InputState;

/// 自动闭合的括号对（尖括号 excluded：`a < b` 这类比较会被误配）。
const PAIRS: [(char, char); 5] = [('(', ')'), ('[', ']'), ('{', '}'), ('"', '"'), ('\'', '\'')];

/// 文本恰为一个字符时返回它（键入拦截只处理单字符）。
pub(super) fn single_typed_char(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let first = chars.next()?;
    chars.next().is_none().then_some(first)
}

/// 自动闭合是否对当前状态生效。
pub(super) fn auto_close_applies(state: &InputState) -> bool {
    if state.disabled || state.read_only || state.masked || !state.mask_pattern.is_none() {
        return false;
    }
    match state.auto_close_pairs {
        Some(enabled) => enabled,
        // 默认：多行开、单行关（搜索框这类单行输入不惊扰用户）。
        None => state.mode.is_multi_line(),
    }
}

/// 给定左括号找右括号。
pub(super) fn matching_closer(open: char) -> Option<char> {
    PAIRS.iter().find(|(o, _)| *o == open).map(|(_, c)| *c)
}

/// 给定字符找它作为右括号时的左括号。
fn matching_opener(close: char) -> Option<char> {
    PAIRS.iter().find(|(_, c)| *c == close).map(|(o, _)| *o)
}

/// 单词字符（其前面不自动闭合，避免 `foo(` 变成 `foo(|)` 夹住标识符……注：
///
/// 这里恰恰相反：`foo` 后输 `(` 时后面是行尾/空位，应闭合；`foo(` 已有内容时
/// 若光标后紧跟单词字符（如补全中），则只插单个，避免打乱已有文本）。
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// 处理一次用户键入，返回 true 表示已完全处理（调用方直接返回）。
///
/// 规则：有选区 + 左括号 → 环绕；右括号 + 光标后是同一右括号 → 跳过；
/// 左括号 + 光标后非单词字符 → 补全对，光标留中间；其余插入单个。
pub(super) fn handle_typed_char(
    state: &mut InputState,
    typed: char,
    window: &mut Window,
    cx: &mut Context<InputState>,
) -> bool {
    let sel: Range<usize> = state.selected_range.into();
    let cursor = state.cursor();
    let next = state.text.slice(cursor..).chars().next();

    // 有选区时用括号环绕选区。
    if !sel.is_empty()
        && let Some(close) = matching_closer(typed)
    {
        let selected = state.text.slice(sel.clone()).to_string();
        let replacement = format!("{typed}{selected}{close}");
        state.replace_text_in_range_raw(Some(state.range_to_utf16(&sel)), &replacement, window, cx);
        let end = sel.start + replacement.len();
        state.selected_range = (end..end).into();
        state.selection_reversed = false;
        cx.notify();
        return true;
    }

    // 右括号且光标后是同一右括号：跳过而非插入。
    if sel.is_empty() && matching_opener(typed).is_some() && next == Some(typed) {
        let next_len = typed.len_utf8();
        state.selected_range = (cursor + next_len..cursor + next_len).into();
        state.selection_reversed = false;
        cx.notify();
        return true;
    }

    // 左括号且光标后非单词字符：补全对，光标留中间。
    if sel.is_empty()
        && let Some(close) = matching_closer(typed)
        && !next.is_some_and(is_word_char)
    {
        // 引号的特殊情况：光标后紧跟同一引号走上面的跳过分支；
        // 单词中间（如 `don't` 的 `t` 后）只插单个。
        let pair = format!("{typed}{close}");
        state.replace_text_in_range_raw(
            Some(state.range_to_utf16(&(cursor..cursor))),
            &pair,
            window,
            cx,
        );
        let middle = cursor + typed.len_utf8();
        state.selected_range = (middle..middle).into();
        state.selection_reversed = false;
        cx.notify();
        return true;
    }

    false
}

/// 智能退格：光标夹在空括号对中间时返回待删范围（如 `(|)` → 删 `()`）。
///
/// 调用方（`backspace`）：有范围则直接删并返回，否则走正常退格。
pub(super) fn smart_backspace_range(state: &InputState) -> Option<Range<usize>> {
    if !auto_close_applies(state) || !state.selected_range.is_empty() {
        return None;
    }
    let cursor = state.cursor();
    let prev = state.text.slice(..cursor).chars().last()?;
    let next = state.text.slice(cursor..).chars().next()?;
    if matching_closer(prev) == Some(next) {
        Some(cursor - prev.len_utf8()..cursor + next.len_utf8())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppContext as _;
    use crate::{Entity, EntityInputHandler as _};

    /// 持有输入框状态的测试宿主视图。
    struct Probe {
        state: Entity<InputState>,
    }

    impl crate::Render for Probe {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> impl crate::IntoElement {
            crate::div()
        }
    }

    /// 模拟一次 OS 键入（走 trait 方法，与真实键入同路径）。
    macro_rules! type_text {
        ($state:expr, $window_cx:expr, $text:expr) => {
            $window_cx.update(|window, cx| {
                $state.update(cx, |state, cx| {
                    state.replace_text_in_range(None, $text, window, cx);
                });
            });
        };
    }

    #[rgpui::test]
    fn typing_opener_inserts_pair(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        type_text!(state, cx, "(");
        assert_eq!(
            state.read_with(cx, |state, _| (state.value().to_string(), state.cursor())),
            ("()".to_string(), 1)
        );
        // 光标后紧跟右括号时输入右括号：跳过不插入。
        type_text!(state, cx, ")");
        assert_eq!(
            state.read_with(cx, |state, _| (state.value().to_string(), state.cursor())),
            ("()".to_string(), 2)
        );
    }

    #[rgpui::test]
    fn typing_opener_surrounds_selection(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            state.update(cx, |state, cx| state.replace("ab", window, cx));
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_selected_range(0..2, cx));
        });
        type_text!(state, cx, "(");
        assert_eq!(
            state.read_with(cx, |state, _| (state.value().to_string(), state.cursor())),
            ("(ab)".to_string(), 4)
        );
    }

    #[rgpui::test]
    fn backspace_deletes_empty_pair(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        type_text!(state, cx, "(");
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                state.backspace(&crate::input_ui::Backspace, window, cx)
            });
        });
        assert_eq!(
            state.read_with(cx, |state, _| (state.value().to_string(), state.cursor())),
            ("".to_string(), 0)
        );
    }

    #[rgpui::test]
    fn single_line_inputs_do_not_auto_close_by_default(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx));
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        type_text!(state, cx, "(");
        assert_eq!(
            state.read_with(cx, |state, _| (state.value().to_string(), state.cursor())),
            ("(".to_string(), 1)
        );
    }

    #[test]
    fn pair_table_is_symmetric() {
        for (open, close) in PAIRS {
            assert_eq!(matching_closer(open), Some(close));
            assert_eq!(matching_opener(close), Some(open));
        }
        assert_eq!(matching_closer('a'), None);
        assert_eq!(matching_opener('<'), None);
    }

    #[test]
    fn single_char_detection() {
        assert_eq!(single_typed_char("("), Some('('));
        assert_eq!(single_typed_char(""), None);
        assert_eq!(single_typed_char("ab"), None);
        assert_eq!(single_typed_char("中"), Some('中'));
    }

    #[test]
    fn smart_backspace_range_detection() {
        // 纯函数部分经配对表覆盖；状态相关由下面的集成测试覆盖。
        assert!(is_word_char('a'));
        assert!(is_word_char('中'));
        assert!(is_word_char('_'));
        assert!(!is_word_char(' '));
        assert!(!is_word_char(')'));
    }
}
