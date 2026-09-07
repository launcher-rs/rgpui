//! 当前行高亮：光标所在整行铺底色。
//!
//! 与括号匹配高亮同机制（独立装饰集合 + 三处刷新点：`move_to` / `select_to` /
//! 替换收尾），原地写存储（借用中调 `set` 会重入 panic，见
//! [`TextDecorationCollection::set`] 文档）。
//!
//! 限制：装饰只覆盖文本区宽度，空行无可见高亮（范围为空被规范化丢弃）。

use crate::theme::ActiveTheme as _;
use crate::{Context, HighlightStyle, Hsla};

use super::RopeExt as _;
use super::decorations::{TextDecoration, normalize};
use super::state::InputState;

/// 当前行高亮的背景色（主题 accent 极低透明，弱于括号匹配）。
fn current_line_color(cx: &crate::App) -> Hsla {
    cx.theme().tokens.accent.color.opacity(0.08)
}

impl InputState {
    /// 按当前光标刷新当前行高亮（关闭/单行时清空）。
    pub(super) fn refresh_current_line(&mut self, cx: &mut Context<Self>) {
        if !self.current_line_highlight || !self.mode.is_multi_line() {
            self.clear_current_line(cx);
            return;
        }
        let cursor = self.cursor();
        let row = self.text.offset_to_point(cursor).row;
        let raw = self.text.line_start_offset(row)..self.text.line_end_offset(row);
        if raw.is_empty() {
            // 空行无可见高亮，直接清空。
            self.clear_current_line(cx);
            return;
        }
        let style = HighlightStyle {
            background_color: Some(current_line_color(cx)),
            ..Default::default()
        };
        let decorations = vec![TextDecoration::new(raw, style)];
        // 原地写（借用中调 `set` 会重入 panic）。
        if let Some(collection) = self.current_line_collection.clone() {
            let decorations = normalize(&self.text, decorations);
            if collection.set_in_place(&mut self.decorations, decorations) {
                cx.notify();
            }
        } else {
            self.current_line_collection =
                Some(self.create_decorations_collection(decorations, cx));
        }
    }

    /// 清除当前行高亮（保留集合句柄供复用）。
    pub(super) fn clear_current_line(&mut self, cx: &mut Context<Self>) {
        if let Some(collection) = self.current_line_collection.clone() {
            if collection.clear_in_place(&mut self.decorations) {
                cx.notify();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppContext as _;
    use crate::{Entity, Window};

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

    #[rgpui::test]
    fn current_line_follows_cursor(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            state.update(cx, |state, cx| state.replace("ab\ncde\nf", window, cx));
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        // 光标移到第二行：高亮覆盖该行。
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_selected_range(4..4, cx));
        });
        let ranges = state.read_with(cx, |state, cx| {
            state
                .current_line_collection
                .as_ref()
                .map(|collection| collection.get_ranges(cx))
                .unwrap_or_default()
        });
        assert_eq!(ranges, vec![3..6]);
        // 关闭开关：高亮清空。
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_current_line_highlight(false, cx));
        });
        let ranges = state.read_with(cx, |state, cx| {
            state
                .current_line_collection
                .as_ref()
                .map(|collection| collection.get_ranges(cx))
                .unwrap_or_default()
        });
        assert!(ranges.is_empty());
    }
}
