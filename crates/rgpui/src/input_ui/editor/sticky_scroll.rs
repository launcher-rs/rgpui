//! 粘性滚动（M5，`editor` feature 门控）：顶部大纲栈面包屑。
//!
//! 最薄的一项：大纲数据（`document_symbols`，K2）已有，只差显示层。
//! 大纲是扁平表（范围有序，无父子指针），嵌套由范围包含推导：
//! 包含光标的符号按（起点升序，长度降序）排即外层在前链。
//!
//! 落点（M1 冻结的后果）：`Editor` 直接消费 `Entity<EditorState>`（大纲缓存在
//! 外壳手里），渲染顶栏读 `sticky_stack`，点击走 `goto_symbol`——不在内部
//! `InputState` 上 duplicat 数据、不碰 `input/`。
//! 无大纲（无高亮器）= 不显示，不报错；高度固定一行，`flex_none` 不挤占编辑区。

use crate::{App, Context};

use super::state::EditorState;

impl EditorState {
    /// 当前大纲栈（包含光标的最内层链，外层在前；无大纲返回空）。
    pub fn sticky_stack(&self, cx: &App) -> Vec<crate::highlight::DocumentSymbol> {
        let cursor = self.cursor(cx);
        let mut stack: Vec<crate::highlight::DocumentSymbol> = self
            .outline
            .iter()
            .filter(|symbol| symbol.range.start <= cursor && cursor <= symbol.range.end)
            .cloned()
            .collect();
        // 外层在前：起点升序，起点相同长度降序（外层范围大）。
        stack.sort_by(|a, b| {
            a.range
                .start
                .cmp(&b.range.start)
                .then((b.range.end - b.range.start).cmp(&(a.range.end - a.range.start)))
        });
        stack
    }

    /// 设置粘性滚动开关（默认开，仅 `Editor` 渲染顶栏用）。
    pub fn set_sticky_scroll(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.sticky_scroll = enabled;
        cx.notify();
    }

    /// 粘性滚动是否开启。
    pub fn sticky_scroll_enabled(&self) -> bool {
        self.sticky_scroll
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlight::{DocumentSymbol, SymbolKind};
    use crate::{AppContext as _, Entity, Render, SharedString, Window};

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

    /// 假高亮器（固定返回嵌套符号：outer 0..20 包 inner 5..10）。
    struct NestedStub;

    impl crate::highlight::Highlighter for NestedStub {
        fn language(&self) -> SharedString {
            "stub".into()
        }

        fn update(
            &mut self,
            _edit: Option<crate::highlight::TextEdit>,
            _text: &ropey::Rope,
            _folding: bool,
            _window: &mut Window,
            _cx: &mut App,
        ) {
        }

        fn styles(
            &self,
            range: &std::ops::Range<usize>,
            _resolver: &dyn crate::highlight::HighlightStyleResolver,
        ) -> Vec<(std::ops::Range<usize>, crate::HighlightStyle)> {
            vec![(range.clone(), crate::HighlightStyle::default())]
        }

        fn fold_ranges(&self, _text: &ropey::Rope) -> Vec<crate::highlight::FoldRange> {
            Vec::new()
        }

        fn document_symbols(&self, _text: &ropey::Rope) -> Vec<DocumentSymbol> {
            vec![
                DocumentSymbol {
                    kind: SymbolKind::Struct,
                    name: "Outer".into(),
                    range: 0..20,
                    start_row: 0,
                },
                DocumentSymbol {
                    kind: SymbolKind::Function,
                    name: "inner".into(),
                    range: 5..10,
                    start_row: 0,
                },
            ]
        }
    }

    fn with_nested(cx: &mut crate::TestAppContext) -> Entity<EditorState> {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor =
                cx.new(|cx| EditorState::new(window, cx, "0123456789abcdefghij0123456789"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_highlighter(Some(Box::new(NestedStub)), window, cx);
            });
        });
        editor
    }

    /// 嵌套栈：光标在 inner 内得 [outer, inner]，只在 outer 得 [outer]。
    #[rgpui::test]
    fn sticky_stack_nested(cx: &mut crate::TestAppContext) {
        let editor = with_nested(cx);
        // 默认开。
        assert!(editor.read_with(cx, |state, _| state.sticky_scroll_enabled()));
        editor.update(cx, |state, cx| {
            state.set_selected_range(7..7, cx);
        });
        let names: Vec<String> = editor.read_with(cx, |state, cx| {
            state
                .sticky_stack(cx)
                .iter()
                .map(|s| s.name.to_string())
                .collect()
        });
        assert_eq!(names, vec!["Outer".to_string(), "inner".to_string()]);
        editor.update(cx, |state, cx| {
            state.set_selected_range(15..15, cx);
        });
        let names: Vec<String> = editor.read_with(cx, |state, cx| {
            state
                .sticky_stack(cx)
                .iter()
                .map(|s| s.name.to_string())
                .collect()
        });
        assert_eq!(names, vec!["Outer".to_string()]);
    }

    /// 光标在大纲外 = 空栈（顶栏不显示，不报错）。
    #[rgpui::test]
    fn sticky_stack_empty_outside(cx: &mut crate::TestAppContext) {
        let editor = with_nested(cx);
        editor.update(cx, |state, cx| {
            state.set_selected_range(25..25, cx);
        });
        assert!(
            editor
                .read_with(cx, |state, cx| state.sticky_stack(cx))
                .is_empty()
        );
    }

    /// 无高亮器 = 空栈；开关可关。
    #[rgpui::test]
    fn sticky_empty_without_highlighter_and_toggle(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn main() {}\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        assert!(
            editor
                .read_with(cx, |state, cx| state.sticky_stack(cx))
                .is_empty()
        );
        editor.update(cx, |state, cx| {
            state.set_sticky_scroll(false, cx);
        });
        assert!(!editor.read_with(cx, |state, _| state.sticky_scroll_enabled()));
    }
}
