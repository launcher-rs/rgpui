//! 代码编辑器状态（`editor` feature 门控，P3 分拆）。
//!
//! 编排外壳：唯一真相源是内部 `Entity<InputState>`（配置为代码编辑）；
//! `EditorState` 只做编辑器级编排（大纲缓存/符号跳转/高亮接入），不复制文本逻辑。
//! 文本核复用仍在（`InputState` 经 P1a 持有 `TextCore`），渲染走 `Editor` 组件。

use super::super::InputState;
use crate::highlight::{DocumentSymbol, Highlighter};
use crate::{App, AppContext as _, Context, Entity, Window};

/// 代码编辑器状态（`cx.new` 持有，`Editor` 组件消费）。
pub struct EditorState {
    /// 内部输入实体（唯一真相源，代码编辑配置）。
    input: Entity<InputState>,
    /// 大纲缓存（文本变更时刷新，`outline()` 读取）。
    outline: Vec<DocumentSymbol>,
}

impl EditorState {
    /// 创建编辑器状态（内部输入配好多行 + 行号 + 折叠 + 引导线 + 键入体验）。
    ///
    /// 初始内容走 `set_value`：不进撤销栈（打开即 undo 不清空），光标落末尾。
    pub fn new(window: &mut Window, cx: &mut Context<Self>, initial: &str) -> Self {
        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .multi_line(true)
                .rows(2)
                .line_number(true)
                .folding(true)
                .indent_guides(true)
                .auto_close_pairs(true)
                .bracket_match(true)
                .current_line_highlight(true);
            let end = initial.len();
            state.set_value(initial, window, cx);
            state.set_selected_range(end..end, cx);
            state
        });
        let mut this = Self {
            input,
            outline: Vec::new(),
        };
        this.refresh_outline(cx);
        // 文本一改就刷新大纲。
        cx.subscribe(&this.input, move |this, _, event, cx| {
            if !matches!(event, crate::input_ui::InputEvent::Change) {
                return;
            }
            this.refresh_outline(cx);
        })
        .detach();
        this
    }

    /// 内部输入实体（接搜索面板等需要 `Entity<InputState>` 的 API）。
    pub fn input(&self) -> &Entity<InputState> {
        &self.input
    }

    /// 全文只读视图。
    pub fn text(&self, cx: &App) -> String {
        self.input
            .read_with(cx, |state, _| state.text().to_string())
    }

    /// 当前光标偏移（UTF-8 字节）。
    pub fn cursor(&self, cx: &App) -> usize {
        self.input.read_with(cx, |state, _| state.cursor())
    }

    /// 设置语法高亮器（如 `highlight::rust_highlighter()`），透传内部输入。
    pub fn set_highlighter(
        &self,
        highlighter: Option<Box<dyn Highlighter>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let _ = self.input.update(cx, |state, cx| {
            state.set_highlighter(highlighter, window, cx);
        });
    }

    /// 大纲缓存（`refresh_outline` 维护）。
    pub fn outline(&self) -> &[DocumentSymbol] {
        &self.outline
    }

    /// 跳转到符号（光标落符号头 + 滚动可见）。
    pub fn goto_symbol(&mut self, symbol: &DocumentSymbol, cx: &mut Context<Self>) {
        let symbol = symbol.clone();
        self.input.update(cx, |state, cx| {
            state.goto_symbol(&symbol, cx);
        });
    }

    /// 按内部高亮器重算大纲（文本变更后调用）。
    fn refresh_outline(&mut self, cx: &mut Context<Self>) {
        let symbols = self
            .input
            .read_with(cx, |state, _| state.document_symbols());
        self.outline = symbols;
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Entity;

    /// 持有编辑器状态的测试宿主视图。
    struct Probe {
        state: Entity<EditorState>,
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

    /// 编辑器文本往返（实体创建→读回→光标落点）。
    #[rgpui::test]
    fn editor_state_text_roundtrip(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn main() {}\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        let text = editor.read_with(cx, |state, cx| state.text(cx));
        assert_eq!(text, "fn main() {}\n");
        let cursor = editor.read_with(cx, |state, cx| state.cursor(cx));
        assert_eq!(cursor, "fn main() {}\n".len());
    }

    /// 无高亮器时大纲为空（不崩溃）。
    #[rgpui::test]
    fn document_symbols_empty_without_highlighter(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn main() {}\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        let symbols = editor.read_with(cx, |state, _| state.outline().to_vec());
        assert!(symbols.is_empty());
    }

    /// goto_symbol 跳转到符号头（经内部输入）。
    #[rgpui::test]
    fn goto_symbol_moves_cursor(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn main() {}\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        let symbol = crate::highlight::DocumentSymbol {
            kind: crate::highlight::SymbolKind::Function,
            name: "main".into(),
            range: 0..2,
            start_row: 0,
        };
        editor.update(cx, |state, cx| {
            state.goto_symbol(&symbol, cx);
        });
        let cursor = editor.read_with(cx, |state, cx| state.cursor(cx));
        assert_eq!(cursor, 0);
    }

    /// 初始内容不进撤销栈：打开即 undo 不得清空内容；
    /// 同一输入 burst（1 秒分组窗内）合并为一个撤销单元。
    #[rgpui::test]
    fn initial_content_not_undoable(cx: &mut crate::TestAppContext) {
        const INITIAL: &str = "fn main() {}\n";
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, INITIAL));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        let input = editor.read_with(cx, |state, _| state.input().clone());
        // 快速连输两个字符（同一分组窗）。
        cx.update(|window, cx| {
            input.update(cx, |state, cx| {
                crate::EntityInputHandler::replace_text_in_range(state, None, "a", window, cx);
                crate::EntityInputHandler::replace_text_in_range(state, None, "b", window, cx);
            });
        });
        assert_eq!(
            input.read_with(cx, |state, _| state.value().to_string()),
            "fn main() {}\nab".to_string()
        );
        // 一次 undo 回退整个 burst，初始内容保留。
        cx.update(|window, cx| {
            input.update(cx, |state, cx| {
                state.undo(&crate::input_ui::Undo, window, cx)
            });
        });
        assert_eq!(
            input.read_with(cx, |state, _| state.value().to_string()),
            INITIAL.to_string()
        );
        // 栈已空：再 undo 无变化。
        cx.update(|window, cx| {
            input.update(cx, |state, cx| {
                state.undo(&crate::input_ui::Undo, window, cx)
            });
        });
        assert_eq!(
            input.read_with(cx, |state, _| state.value().to_string()),
            INITIAL.to_string()
        );
    }
}
