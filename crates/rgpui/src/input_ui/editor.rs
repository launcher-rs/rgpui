//! 代码编辑器状态（`editor` feature 门控，P3 分拆）。
//!
//! `EditorState` 与表单 `InputState` 共享 [`TextCore`]，互不复制文本逻辑：
//! 编辑器独有的行号/折叠/大纲/多光标状态与行为落在这里，渲染在 `editor_ui.rs`
//!（P2 已落地覆盖层），`Editor` 组件在 P3b 接线。

use ropey::Rope;

use super::FoldRange;
use super::core::TextCore;
use super::display_map::DisplayMap;
use crate::highlight::{DocumentSymbol, Highlighter};
use crate::{Context, Window};

/// 代码编辑器状态实体的数据（`cx.new` 持有）。
///
/// P3a 落子：文本核 + 行映射 + 高亮器（大纲/折叠数据源）。
/// 焦点/滚动/光标等视图句柄在 P3b 渲染接线时加回。
pub struct EditorState {
    /// 共享文本核（与 `InputState` 同一份实现）。
    pub(super) core: TextCore,
    /// 行映射与折叠投影（视图无关的纯数据层，可复用）。
    pub(super) display_map: DisplayMap,
    /// 语法高亮器（大纲/折叠数据源，无则对应能力为空）。
    pub(super) highlighter: Option<Box<dyn Highlighter>>,
}

impl EditorState {
    /// 创建空编辑器状态。
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 焦点/滚动/光标句柄在 P3b 渲染接线时加回，此处不需要 cx 建任何东西。
        let _ = cx;
        let text_style = window.text_style();
        Self {
            core: TextCore::new(),
            display_map: DisplayMap::new(
                text_style.font(),
                text_style.font_size.to_pixels(window.rem_size()),
                None,
            ),
            highlighter: None,
        }
    }

    /// 全文只读视图。
    pub fn text(&self) -> &Rope {
        &self.core.text
    }

    /// 全量设置文本（编辑器打开文件/切换 buffer 用）。
    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let range = 0..self.core.text.len();
        let old = self.core.text.clone();
        self.core.apply_replace(range.clone(), text);
        self.display_map.adjust_folds_for_edit(&old, &range, text);
        self.display_map
            .on_text_changed(&self.core.text, &range, &Rope::from(text), cx);
        cx.notify();
    }

    /// 当前光标偏移（UTF-8 字节）。
    pub fn cursor(&self) -> usize {
        self.core.cursor()
    }

    /// 设置语法高亮器（如 `highlight::rust_highlighter()`），并刷新折叠候选。
    pub fn set_highlighter(
        &mut self,
        mut highlighter: Option<Box<dyn Highlighter>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(highlighter) = highlighter.as_mut() {
            highlighter.update(None, &self.core.text, true, window, cx);
        }
        self.highlighter = highlighter;
        self.refresh_folds();
        cx.notify();
    }

    /// 文档符号大纲（无高亮器时为空）。
    pub fn document_symbols(&self) -> Vec<DocumentSymbol> {
        match self.highlighter.as_ref() {
            Some(highlighter) => highlighter.document_symbols(&self.core.text),
            None => Vec::new(),
        }
    }

    /// 按当前缓存树刷新折叠候选（文本变更后调用）。
    pub fn refresh_folds(&mut self) {
        let folds = match self.highlighter.as_ref() {
            Some(highlighter) => highlighter
                .fold_ranges(&self.core.text)
                .into_iter()
                .map(|range| FoldRange::new(range.start, range.end))
                .collect(),
            None => Vec::new(),
        };
        self.display_map.set_fold_candidates(folds);
    }

    /// 跳转到字节偏移（选区坍缩 + 滚动可见占位；完整滚动驱动在 P3b 渲染接线）。
    pub fn reveal_offset(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = offset.min(self.core.text.len());
        self.core.selected_range = (offset..offset).into();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppContext as _;
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

    /// 空编辑器文本往返（实体创建→写入→读回→光标落点）。
    #[rgpui::test]
    fn editor_state_text_roundtrip(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| {
                let mut state = EditorState::new(window, cx);
                state.set_text("fn main() {}\n", cx);
                state
            });
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        let text = editor.read_with(cx, |state, _| state.text().to_string());
        assert_eq!(text, "fn main() {}\n");
        let cursor = editor.read_with(cx, |state, _| state.cursor());
        assert_eq!(cursor, "fn main() {}\n".len());
    }

    /// 无高亮器时大纲为空（不崩溃）。
    #[rgpui::test]
    fn document_symbols_empty_without_highlighter(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        let symbols = editor.read_with(cx, |state, _| state.document_symbols());
        assert!(symbols.is_empty());
    }

    /// reveal_offset 钳制越界并坍缩选区。
    #[rgpui::test]
    fn reveal_offset_clamps(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| {
                let mut state = EditorState::new(window, cx);
                state.set_text("abc", cx);
                state.reveal_offset(999, cx);
                state
            });
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        let cursor = editor.read_with(cx, |state, _| state.cursor());
        assert_eq!(cursor, 3);
    }
}
