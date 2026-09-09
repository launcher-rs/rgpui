//! Minimap 缩略图（O2，`editor` feature 门控）：文档缩略条 + 可视区高亮 + 点击/拖动跳转。
//!
//! 纯 overlay（不占布局：绝对定位盖在编辑区右侧；不进 `Rope`；条块不可选中，
//! 与 inlay 三不同理）；大文档抽样渲染（`MAX_MINIMAP_BARS` 上限，全文档覆盖）；
//! 默认关，零开销（关闭时不建任何元素）。
//!
//! 跳转走现有 `reveal_offset`（最小滚动，不另起滚动语义）；拖动靠
//! `MouseMoveEvent.pressed_button` 在条块上连续触发，无需几何换算。

use std::ops::Range;
use std::rc::Rc;

use ropey::LineType;

use crate::{
    ActiveTheme as _, AnyElement, App, Context, Entity, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Pixels, StatefulInteractiveElement, Styled, div, px,
};

use super::state::EditorState;

/// 单屏最多条块数（超出按步长抽样，保证全文档覆盖）。
const MAX_MINIMAP_BARS: usize = 200;

/// 条块高度（固定 2px + 1px 间隙，超出容器裁掉，顶部对齐）。
const BAR_HEIGHT: Pixels = crate::px(2.);

/// 缩略图接入状态（`EditorState` 内嵌小字段）。
pub(super) struct MinimapState {
    /// 是否开启（默认关）。
    enabled: bool,
}

impl MinimapState {
    pub(super) fn new() -> Self {
        Self { enabled: false }
    }
}

/// 文档行按上限抽样为条块区间（`[start, end)` 缓冲行；空文档给单条 `0..0`）。
pub(crate) fn minimap_rows(total_rows: usize, max_bars: usize) -> Vec<Range<usize>> {
    if total_rows == 0 {
        return vec![0..0];
    }
    let max_bars = max_bars.max(1);
    let stride = total_rows.div_ceil(max_bars).max(1);
    (0..total_rows)
        .step_by(stride)
        .map(|start| start..(start + stride).min(total_rows))
        .collect()
}

/// 可视区是否覆盖条块区间（`None` 视为未布局，全暗）。
fn bar_visible(bar: &Range<usize>, visible: &Option<Range<usize>>) -> bool {
    visible
        .as_ref()
        .is_some_and(|range| bar.start < range.end && range.start < bar.end)
}

impl EditorState {
    /// 设置缩略图开关（默认关；关闭不建任何元素，零开销）。
    pub fn set_minimap_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.minimap.enabled = enabled;
        cx.notify();
    }

    /// 缩略图是否开启。
    pub fn minimap_enabled(&self) -> bool {
        self.minimap.enabled
    }

    /// 行号 → 行首字节偏移（越界钳制到末行；跳转用）。
    fn minimap_offset(&self, row: usize, cx: &App) -> usize {
        self.input.read_with(cx, |state, _| {
            let text = state.text();
            let last_row = text.len_lines(LineType::LF).saturating_sub(1);
            let row = row.min(last_row);
            text.line_to_byte_idx(row, LineType::LF).min(text.len())
        })
    }
}

/// 缩略图浮层（`Editor` 渲染内调用；关闭时调用方直接跳过）。
pub(super) fn render_minimap(editor: &Entity<EditorState>, cx: &mut App) -> AnyElement {
    let (input, total_rows, visible) = editor.read_with(cx, |state, cx| {
        let text_len = state
            .input
            .read_with(cx, |state, _| state.text().len_lines(LineType::LF));
        let visible = state
            .input
            .read_with(cx, |state, _| state.visible_row_range());
        (state.input().clone(), text_len, visible)
    });
    let bars = minimap_rows(total_rows, MAX_MINIMAP_BARS);
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .right_0()
        .w(px(76.))
        .py(px(4.))
        .pl(px(6.))
        .pr(px(4.))
        .border_l_1()
        .border_color(cx.theme().border)
        .overflow_hidden()
        .children(bars.into_iter().enumerate().map(|(ix, bar)| {
            let editor = editor.clone();
            let input = input.clone();
            let bright = bar_visible(&bar, &visible);
            // 跳转目标：区间中行（点击/拖动共用）。
            let jump = Rc::new(move |cx: &mut App| {
                let row = (bar.start + bar.end) / 2;
                let offset = editor.read_with(cx, |state, cx| state.minimap_offset(row, cx));
                input.update(cx, |state, cx| {
                    state.reveal_offset(offset, cx);
                });
            });
            let jump_for_move = jump.clone();
            div()
                .id(("minimap-bar", ix))
                .w_full()
                .h(BAR_HEIGHT)
                .mb(px(1.))
                .rounded_full()
                .bg(if bright {
                    cx.theme().foreground.opacity(0.55)
                } else {
                    cx.theme().muted_foreground.opacity(0.35)
                })
                .cursor_pointer()
                .on_click(move |_, _, cx| {
                    jump(cx);
                })
                .on_mouse_move(move |event, _, cx| {
                    // 按住左键划过即连续跳转（拖动语义，无需几何换算）。
                    if event.pressed_button == Some(MouseButton::Left) {
                        jump_for_move(cx);
                    }
                })
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppContext as _;
    use crate::Render;

    /// 持有编辑器状态的测试宿主视图（渲染冒烟用：带缩略图的 `Editor`）。
    struct Probe {
        state: Entity<EditorState>,
    }

    impl Render for Probe {
        fn render(
            &mut self,
            window: &mut crate::Window,
            cx: &mut Context<Self>,
        ) -> impl crate::IntoElement {
            use crate::{IntoElement as _, RenderOnce as _};
            super::super::Editor::new(&self.state)
                .render(window, cx)
                .into_element()
        }
    }

    /// 抽样覆盖全文档（步长自适应；空文档单条）。
    #[test]
    fn minimap_rows_cover_all() {
        assert_eq!(minimap_rows(0, 200), vec![0..0]);
        assert_eq!(minimap_rows(3, 200), vec![0..1, 1..2, 2..3]);
        let rows = minimap_rows(500, 200);
        assert!(rows.len() <= 200);
        assert_eq!(rows.first().unwrap().start, 0);
        assert_eq!(rows.last().unwrap().end, 500);
        // 连续无缝。
        for pair in rows.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
    }

    /// 可视相交即高亮（未布局全暗）。
    #[test]
    fn bar_visibility_intersects() {
        assert!(bar_visible(&(10..20), &Some(15..25)));
        assert!(!bar_visible(&(10..20), &Some(20..30)));
        assert!(!bar_visible(&(10..20), &None));
    }

    /// 开关默认关；打开后带缩略图的 `Editor` 渲染不 panic（冒烟）。
    #[rgpui::test]
    fn minimap_toggle_and_render_smoke(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let text = (0..300)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, &text));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        assert!(!editor.read_with(cx, |state, _| state.minimap_enabled()));
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.set_minimap_enabled(true, cx);
            });
        });
        assert!(editor.read_with(cx, |state, _| state.minimap_enabled()));
        // 渲染一帧（含缩略图浮层）：布局/绘制路径无 panic 即过。
        cx.run_until_parked();
    }
}
