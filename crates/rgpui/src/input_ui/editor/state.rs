//! 代码编辑器状态（`editor` feature 门控，P3 分拆）。
//!
//! 编排外壳：唯一真相源是内部 `Entity<InputState>`（配置为代码编辑）；
//! `EditorState` 只做编辑器级编排（大纲缓存/符号跳转/高亮接入），不复制文本逻辑。
//! 文本核复用仍在（`InputState` 经 P1a 持有 `TextCore`），渲染走 `Editor` 组件。

use std::ops::Range;

use super::super::InputState;
use super::codelens::CodelensState;
use super::extensions::ExtensionsState;
use super::inlay_hints::InlayState;
use super::lsp_attach::LspAttach;
use super::minimap::MinimapState;
use super::snippets::SnippetSession;
use crate::highlight::{DocumentSymbol, Highlighter};
use crate::{App, AppContext as _, Context, Entity, SharedString, Window};

/// 代码编辑器状态（`cx.new` 持有，`Editor` 组件消费）。
pub struct EditorState {
    /// 内部输入实体（唯一真相源，代码编辑配置）。
    pub(super) input: Entity<InputState>,
    /// 大纲缓存（文本变更时刷新，`outline()` 读取）。
    pub(super) outline: Vec<DocumentSymbol>,
    /// LSP 接入状态（provider + 补全/诊断/悬停，见 `lsp_attach.rs`）。
    pub(super) lsp: LspAttach,
    /// 片段会话（进行中为 `Some`，见 `snippets.rs`）。
    pub(super) snippet: Option<SnippetSession>,
    /// inlay 接入状态（provider，见 `inlay_hints.rs`）。
    pub(super) inlay: InlayState,
    /// 缩略图开关状态（见 `minimap.rs`，O2，默认关）。
    pub(super) minimap: MinimapState,
    /// 透镜接入状态（provider + 已落位透镜，见 `codelens.rs`，O4）。
    pub(super) codelens: CodelensState,
    /// 扩展表 + 变更订阅表运行时状态（见 `extensions.rs`，O7）。
    pub(super) extensions: ExtensionsState,
    /// 粘性滚动开关（默认开，见 `sticky_scroll.rs`）。
    pub(super) sticky_scroll: bool,
    /// 当前语言（`set_language` 维护；未设置/已降级为 `None`）。
    language: Option<SharedString>,
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
            lsp: LspAttach::new(cx.new(|_| crate::lsp::CompletionPopupState::default())),
            snippet: None,
            inlay: InlayState::new(),
            minimap: MinimapState::new(),
            codelens: CodelensState::new(),
            extensions: ExtensionsState::new(),
            sticky_scroll: true,
            language: None,
        };
        this.refresh_outline(cx);
        // 文本一改：刷新大纲 → 自动补全（开关开时）→ 透镜（有 provider 即刷新）→
        // 分发变更事件（扩展表/订阅表，O7）。
        //（`subscribe_in` 带 window，`request_*` 与钩子要 window 才能调。）
        cx.subscribe_in(&this.input, window, move |this, _, event, window, cx| {
            if !matches!(event, crate::input_ui::InputEvent::Change) {
                return;
            }
            this.refresh_outline(cx);
            this.maybe_auto_complete(window, cx);
            this.request_codelenses(window, cx);
            let edit = this.edit_event(cx);
            this.fire_edit_event(&edit, window, cx);
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

    /// 程序化写入全文（透传内部输入）。
    ///
    /// 不进撤销栈（打开即 undo 不清空）；多行时光标重置为起始，调用方按需再调
    /// [`set_selected_range`](Self::set_selected_range)；大纲同步刷新（内部
    /// `set_value` 为性能压住了 `Change` 事件，外壳在此补刷）。
    pub fn set_value(
        &mut self,
        value: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.input.update(cx, |state, cx| {
            state.set_value(value, window, cx);
        });
        self.refresh_outline(cx);
    }

    /// 设置选中区（外部驱动选中/跳转；透传内部输入）。
    pub fn set_selected_range(&self, range: Range<usize>, cx: &mut App) {
        let _ = self.input.update(cx, |state, cx| {
            state.set_selected_range(range, cx);
        });
    }

    /// 滚动使给定字节偏移可见，不改变光标与选区（透传内部输入）。
    ///
    /// 首次布局前调用无效果（内部尚无 layout 时直接返回）。
    pub fn reveal_offset(&self, offset: usize, cx: &mut App) {
        let _ = self.input.update(cx, |state, cx| {
            state.reveal_offset(offset, cx);
        });
    }

    /// 滚动使给定区间可见（以区间起点为准），不改变光标与选区（透传内部输入）。
    pub fn reveal_range(&self, range: Range<usize>, cx: &mut App) {
        let _ = self.input.update(cx, |state, cx| {
            state.reveal_range(range, cx);
        });
    }

    /// 设置只读模式（创建后修改；透传内部输入）。
    ///
    /// 只读保持正常样式，允许移动光标/选择/复制；程序化写入不受影响。
    pub fn set_read_only(&self, read_only: bool, cx: &mut App) {
        let _ = self.input.update(cx, |state, cx| {
            state.set_read_only(read_only, cx);
        });
    }

    /// 设置标尺列（字符数；空即关，O5；透传内部输入）。
    pub fn set_rulers(&self, columns: Vec<usize>, cx: &mut App) {
        let _ = self.input.update(cx, |state, cx| {
            state.set_rulers(columns, cx);
        });
    }

    /// 设置标尺颜色（`None` 跟主题边框色；透传内部输入）。
    pub fn set_ruler_color(&self, color: Option<crate::Hsla>, cx: &mut App) {
        let _ = self.input.update(cx, |state, cx| {
            state.set_ruler_color(color, cx);
        });
    }

    /// 设置括号彩虹开关（O5；透传内部输入）。
    pub fn set_bracket_rainbow_enabled(&self, enabled: bool, cx: &mut App) {
        let _ = self.input.update(cx, |state, cx| {
            state.set_bracket_rainbow_enabled(enabled, cx);
        });
    }

    /// 设置 Vim 模式开关（O3；默认关，透传内部输入）。
    ///
    /// 打开即进 normal、坍缩选区；关闭清前缀/锚点。vim 上下文进内部输入的
    /// key_context（与 `Input` 同节点，后注册优先覆盖同键默认行为）。
    pub fn set_vim_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.input.update(cx, |state, cx| {
            state.vim.enabled = enabled;
            state.vim.mode = super::vim::VimMode::Normal;
            state.vim.pending = None;
            state.vim.anchor = None;
            if enabled {
                let cursor = state.cursor();
                state.move_to(cursor, None, cx);
            }
            cx.notify();
        });
        cx.notify();
    }

    /// Vim 是否启用。
    pub fn vim_enabled(&self, cx: &App) -> bool {
        self.input.read_with(cx, |state, _| state.vim.enabled)
    }

    /// Vim 当前模式（未启用返回 `None`，状态行指示用）。
    pub fn vim_mode(&self, cx: &App) -> Option<super::vim::VimMode> {
        self.input
            .read_with(cx, |state, _| state.vim.enabled.then_some(state.vim.mode))
    }

    /// Vim 按键分发（`Editor` 的 `VimKey` 捕获调用；绑定命中即激活态，调用方吞传播）。
    pub(crate) fn vim_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |state, cx| {
            super::vim::handle_key(state, key, window, cx);
        });
    }

    /// Vim Esc 模式切换（`capture_key_down` 调用；`Input` 自身 Esc 照跑，不吞）。
    pub(crate) fn vim_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |state, cx| {
            super::vim::escape_pressed(state, window, cx);
        });
    }

    /// 设置行注释符（创建后修改；透传内部输入，默认 `//`）。
    ///
    /// 按语言设置，如 Python 传 `"#"`、Lua 传 `"--"`。
    pub fn set_line_comment_prefix(&self, prefix: impl Into<SharedString>, cx: &mut App) {
        let _ = self.input.update(cx, |state, cx| {
            state.set_line_comment_prefix(prefix, cx);
        });
    }

    /// 当前光标偏移（UTF-8 字节）。
    pub fn cursor(&self, cx: &App) -> usize {
        self.input.read_with(cx, |state, _| state.cursor())
    }

    /// 设置语法高亮器（如 `highlight::rust_highlighter()`），透传内部输入。
    ///
    /// 高亮器变化不经过 `Change` 事件，外壳在此同步刷新大纲（否则大纲停留在
    /// 旧高亮器的结果上，直到下一次文本编辑）。
    pub fn set_highlighter(
        &mut self,
        highlighter: Option<Box<dyn Highlighter>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.input.update(cx, |state, cx| {
            state.set_highlighter(highlighter, window, cx);
        });
        self.refresh_outline(cx);
    }

    /// 设置语言（查注册表 + 高亮/大纲联动；未注册静默降级为纯文本）。
    ///
    /// 注册表见 `highlight::register_highlighter`；Rust 内置需 `--features
    /// tree-sitter`（默认关，wasm 不可用）；1.2 不加 Rust 之外的 grammar。
    pub fn set_language(&mut self, language: &str, window: &mut Window, cx: &mut Context<Self>) {
        let highlighter = crate::highlight::highlighter_for(language);
        self.language = if highlighter.is_some() {
            Some(language.into())
        } else {
            None
        };
        self.set_highlighter(highlighter, window, cx);
    }

    /// 当前语言（未设置/已降级为 `None`，状态行展示用）。
    pub fn language(&self) -> Option<SharedString> {
        self.language.clone()
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

    /// 未注册语言静默降级（语言记 `None`，大纲为空，不 panic）。
    #[rgpui::test]
    fn set_language_unknown_downgrades(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn main() {}\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_language("cobol-xyz-not-registered", window, cx);
            });
        });
        assert!(editor.read_with(cx, |state, _| state.language().is_none()));
        assert!(editor.read_with(cx, |state, _| state.outline().is_empty()));
    }

    /// Rust 语言可用（tree-sitter feature 门控）。
    #[cfg(all(not(target_family = "wasm"), feature = "tree-sitter"))]
    #[rgpui::test]
    fn set_language_rust_available(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn main() {}\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_language("rust", window, cx);
            });
        });
        assert_eq!(
            editor
                .read_with(cx, |state, _| state.language())
                .map(|s| s.to_string()),
            Some("rust".to_string())
        );
        // Rust 大纲非空（tree-sitter 真解析）。
        assert!(!editor.read_with(cx, |state, _| state.outline().is_empty()));
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

    /// 测试桩高亮器（固定返回一个 `main` 符号，无需 tree-sitter feature）。
    struct StubHighlighter;

    impl crate::highlight::Highlighter for StubHighlighter {
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
            range: &Range<usize>,
            _resolver: &dyn crate::highlight::HighlightStyleResolver,
        ) -> Vec<(Range<usize>, crate::HighlightStyle)> {
            vec![(range.clone(), crate::HighlightStyle::default())]
        }

        fn fold_ranges(&self, _text: &ropey::Rope) -> Vec<crate::highlight::FoldRange> {
            Vec::new()
        }

        fn document_symbols(&self, _text: &ropey::Rope) -> Vec<crate::highlight::DocumentSymbol> {
            vec![crate::highlight::DocumentSymbol {
                kind: crate::highlight::SymbolKind::Function,
                name: "main".into(),
                range: 0..2,
                start_row: 0,
            }]
        }
    }

    /// 设置高亮器后大纲同步刷新（高亮器变化不经过 Change，外壳补刷）。
    #[rgpui::test]
    fn set_highlighter_refreshes_outline(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn main() {}\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        assert!(editor.read_with(cx, |state, _| state.outline().is_empty()));
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_highlighter(Some(Box::new(StubHighlighter)), window, cx);
            });
        });
        let outline = editor.read_with(cx, |state, _| state.outline().to_vec());
        assert_eq!(outline.len(), 1);
        assert_eq!(outline[0].name.to_string(), "main");
    }

    /// `set_value` 程序化写入：内容替换 + 光标重置 + 大纲同步 + 不进撤销栈。
    #[rgpui::test]
    fn set_value_replaces_without_history(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "old\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_highlighter(Some(Box::new(StubHighlighter)), window, cx);
                state.set_value("new\n", window, cx);
            });
        });
        assert_eq!(editor.read_with(cx, |state, cx| state.text(cx)), "new\n");
        // 多行 `set_value` 光标重置为起始。
        assert_eq!(editor.read_with(cx, |state, cx| state.cursor(cx)), 0);
        // 大纲随写入同步（桩返回固定符号，断言刷新通路不断言内容）。
        assert_eq!(editor.read_with(cx, |state, _| state.outline().len()), 1);
        // 不进撤销栈：undo 无变化。
        let input = editor.read_with(cx, |state, _| state.input().clone());
        cx.update(|window, cx| {
            input.update(cx, |state, cx| {
                state.undo(&crate::input_ui::Undo, window, cx)
            });
        });
        assert_eq!(
            input.read_with(cx, |state, _| state.value().to_string()),
            "new\n"
        );
    }
}
