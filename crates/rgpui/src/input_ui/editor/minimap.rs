//! Minimap 缩略图（O2，`editor` feature 门控）：文档缩略条 + 可视区高亮 + 点击/拖动跳转。
//!
//! 纯 overlay（不占布局：绝对定位盖在编辑区右侧；`occlude` 吞掉鼠标事件，不穿透
//! 到底层编辑器；条块不可选中，与 inlay 三不同理）；大文档抽样渲染
//! （`MAX_MINIMAP_BARS` 上限，全文档覆盖）；默认关，零开销（关闭时不建任何元素）。
//!
//! 条块宽度按区间内最长行字符数等比缩放，还原文档"天际线"形状；空行不绘制但保留
//! 垂直槽位以对齐。交互由容器统一处理：条带任意位置（条块/空白/内边距）按下即按
//! 纵坐标→行号映射跳转（与条块视觉严格对应），按住拖动跟随；条带内外松开结束。
//! 跳转走现有 `reveal_offset`（最小滚动，不改变光标与选区）；拖动仅当按下起始于
//! 缩略图时跟随（`dragging` 标记），编辑器内开始的选区拖拽划过此处不响应，避免误滚动。

use std::ops::Range;
use std::rc::Rc;

use ropey::{LineType, Rope};

use crate::{
    ActiveTheme as _, AnyElement, App, Bounds, Context, ElementExt as _, Entity,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Styled, div, px,
};

use super::state::EditorState;

/// 单屏最多条块数（超出按步长抽样，保证全文档覆盖）。
const MAX_MINIMAP_BARS: usize = 200;

/// 条块高度（固定 2px，条块紧密排列无间隙，超出容器裁掉，顶部对齐）。
/// 纵坐标→行号映射按此节距计算，与条块视觉位置严格对应。
const BAR_HEIGHT: Pixels = crate::px(2.);

/// 条带上下内边距（容器 `py` 用同值；映射时扣除顶部内边距）。
const MINIMAP_PAD_Y: Pixels = crate::px(4.);

/// 条块区可用宽度（容器 76px - 左内边距 6px - 右内边距 4px）：最长行占满。
const BAR_FULL_WIDTH: f32 = 66.0;

/// 非空条块最小宽度（再短的行也显示一个小 tick）。
const MIN_BAR_WIDTH: Pixels = crate::px(4.);

/// 缩略图接入状态（`EditorState` 内嵌小字段）。
pub(super) struct MinimapState {
    /// 是否开启（默认关）。
    enabled: bool,
    /// 拖拽手势是否起始于缩略图（左键按在条带上置位，松开清除）。
    /// 仅此时划动跟随跳转；编辑器内开始的拖拽划过此处不响应。
    pub(super) dragging: bool,
    /// 条块宽度缓存：(全文字节数, 总行数, 每条块最长行字符数)。
    /// 宽度只与文本内容有关、与滚动无关；键不变直接复用，避免每帧全文档扫描。
    bar_cache: Option<(usize, usize, Vec<usize>)>,
    /// 上次跳转的目标行（同行重复触发直接跳过，避免无效滚动与重渲染）。
    last_jump_row: Option<usize>,
    /// 条带边界（窗口坐标；`on_prepaint` 静默记录，不 notify，避免渲染循环）。
    /// 纵坐标→行号映射用。
    strip_bounds: Bounds<Pixels>,
}

impl MinimapState {
    pub(super) fn new() -> Self {
        Self {
            enabled: false,
            dragging: false,
            bar_cache: None,
            last_jump_row: None,
            strip_bounds: Bounds::default(),
        }
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

/// 每条块内最长行的字符数（与 [`minimap_rows`] 同步长；空区间为 0）。
///
/// 单遍扫描（`lines()` 迭代器 O(n)），避免每行一次 `line()` 树查找。
pub(crate) fn minimap_bar_lengths(
    text: &Rope,
    total_rows: usize,
    max_bars: usize,
) -> Vec<usize> {
    if total_rows == 0 {
        return vec![0];
    }
    let stride = total_rows.div_ceil(max_bars.max(1)).max(1);
    let bar_count = total_rows.div_ceil(stride);
    let mut lengths = vec![0usize; bar_count];
    for (row, line) in text.lines(LineType::LF).enumerate().take(total_rows) {
        let ix = (row / stride).min(bar_count - 1);
        // 不用 `len_chars()`（需 ropey `metric_chars` feature）：单遍扫描下
        // 逐字符计数总复杂度同样是 O(n)。
        // `lines()` 保留行尾 `\n`，形状统计时剔除（空行计 0，保持空白）。
        let mut len = line.chars().count();
        if len > 0 && line.chars().last() == Some('\n') {
            len -= 1;
        }
        if len > lengths[ix] {
            lengths[ix] = len;
        }
    }
    lengths
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

    /// 鼠标纵坐标（窗口坐标）→ 文档行：按条块节距映射，与条块视觉位置严格对应；
    /// 条带上方钳制到首行（顶部内边距内点击也有效），下方空白钳制到末行。
    fn minimap_row_at_y(&self, y: Pixels, total_rows: usize) -> usize {
        if total_rows == 0 {
            return 0;
        }
        let rel = y - self.minimap.strip_bounds.top() - MINIMAP_PAD_Y;
        if rel <= px(0.) {
            return 0;
        }
        let stride = total_rows.div_ceil(MAX_MINIMAP_BARS).max(1);
        let bar_ix = (rel / BAR_HEIGHT) as usize;
        let start = bar_ix.saturating_mul(stride).min(total_rows - 1);
        let end = (bar_ix + 1).saturating_mul(stride).min(total_rows);
        ((start + end) / 2).min(total_rows - 1)
    }
}

/// 缩略图浮层（`Editor` 渲染内调用；关闭时调用方直接跳过）。
pub(super) fn render_minimap(editor: &Entity<EditorState>, cx: &mut App) -> AnyElement {
    // 轻量读取：文本字节数 + 总行数作缓存键，命中则复用宽度（仅克隆 ≤200 个数）。
    let (input, byte_len, total_rows, visible, cached_lengths) = editor.read_with(cx, |state, cx| {
        let (byte_len, total, visible, cached) = state.input.read_with(cx, |input, _| {
            let text = input.text();
            let total = text.len_lines(LineType::LF);
            let visible = input.visible_row_range();
            let cached = state.minimap.bar_cache.as_ref().and_then(|(b, r, v)| {
                (*b == text.len() && *r == total).then(|| v.clone())
            });
            (text.len(), total, visible, cached)
        });
        (state.input().clone(), byte_len, total, visible, cached)
    });
    // 未命中：全量计算一次并静默回写缓存（不 notify，不触发渲染循环）。
    let bar_lengths = match cached_lengths {
        Some(v) => v,
        None => {
            let v = editor.read_with(cx, |state, cx| {
                state.input.read_with(cx, |input, _| {
                    minimap_bar_lengths(input.text(), total_rows, MAX_MINIMAP_BARS)
                })
            });
            editor.update(cx, |state, _| {
                state.minimap.bar_cache = Some((byte_len, total_rows, v.clone()));
            });
            v
        }
    };
    let bars = minimap_rows(total_rows, MAX_MINIMAP_BARS);
    let max_len = bar_lengths.iter().copied().max().unwrap_or(0);
    let editor_for_down = editor.clone();
    let editor_for_move = editor.clone();
    let editor_for_up = editor.clone();
    let editor_for_up_out = editor.clone();
    let editor_for_paint = editor.clone();
    // 按纵坐标跳转（条带任意位置按下/拖动共用；同行去重）。
    // 条块、空白、内边距都有效：按条块节距映射，与条块视觉严格对应。
    let editor_for_jump = editor.clone();
    let jump_at_y = Rc::new(move |y: Pixels, cx: &mut App| {
        let row = editor_for_jump.read_with(cx, |state, _| state.minimap_row_at_y(y, total_rows));
        let dominated =
            editor_for_jump.read_with(cx, |state, _| state.minimap.last_jump_row == Some(row));
        if dominated {
            return;
        }
        editor_for_jump.update(cx, |state, _| {
            state.minimap.last_jump_row = Some(row);
        });
        let offset = editor_for_jump.read_with(cx, |state, cx| state.minimap_offset(row, cx));
        input.update(cx, |state, cx| {
            state.reveal_offset(offset, cx);
        });
    });
    let jump_at_y_for_move = jump_at_y.clone();
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .right_0()
        .w(px(76.))
        .py(MINIMAP_PAD_Y)
        .pl(px(6.))
        .pr(px(4.))
        .border_l_1()
        .border_color(cx.theme().border)
        .overflow_hidden()
        // 吞掉落在本层的鼠标事件，不穿透到底层编辑器，避免误移动光标或开始选区。
        .occlude()
        .cursor_pointer()
        // 记录条带边界（窗口坐标，供纵坐标→行号映射；静默更新不 notify）。
        .on_prepaint(move |bounds, _, cx| {
            editor_for_paint.update(cx, |state, _| {
                state.minimap.strip_bounds = bounds;
            });
        })
        // 条带任意位置按下即开始拖拽并立即跳转，反馈跟手。
        .on_mouse_down(MouseButton::Left, move |e, _, cx| {
            editor_for_down.update(cx, |state, _| {
                state.minimap.dragging = true;
            });
            jump_at_y(e.position.y, cx);
        })
        .on_mouse_move(move |event, _, cx| {
            if event.pressed_button == Some(MouseButton::Left) {
                // 仅当按下起始于缩略图时跟随跳转；编辑器内开始的选区拖拽
                // 划过此处不响应，避免误滚动干扰选区。
                let dragging =
                    editor_for_move.read_with(cx, |state, _| state.minimap.dragging);
                if dragging {
                    jump_at_y_for_move(event.position.y, cx);
                }
            } else {
                // 无按键悬停：只读检查，仅残留标记时才写（避免每次悬停
                // 都 notify 触发全编辑器重渲染）。
                let stale = editor_for_move.read_with(cx, |state, _| {
                    state.minimap.dragging || state.minimap.last_jump_row.is_some()
                });
                if stale {
                    editor_for_move.update(cx, |state, _| {
                        state.minimap.dragging = false;
                        state.minimap.last_jump_row = None;
                    });
                }
            }
        })
        // 条带内外松开左键都结束拖拽（同时清除上次跳转行，
        // 下次点击同一位置仍会正常跳转）。
        .on_mouse_up(MouseButton::Left, move |_, _, cx| {
            editor_for_up.update(cx, |state, _| {
                state.minimap.dragging = false;
                state.minimap.last_jump_row = None;
            });
        })
        .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
            editor_for_up_out.update(cx, |state, _| {
                state.minimap.dragging = false;
                state.minimap.last_jump_row = None;
            });
        })
        // 条块只做纯展示（无自身事件，由容器统一处理，整条任意位置都可点/拖）。
        .children(bars.into_iter().enumerate().map(|(ix, bar)| {
            let bright = bar_visible(&bar, &visible);
            // 条块宽度按区间内最长行等比缩放，还原文形状；空行不绘制但保留槽位。
            let len = bar_lengths.get(ix).copied().unwrap_or(0);
            let width = if max_len == 0 || len == 0 {
                px(0.)
            } else {
                MIN_BAR_WIDTH.max(px(len as f32 / max_len as f32 * BAR_FULL_WIDTH))
            };
            div()
                .id(("minimap-bar", ix))
                .w(width)
                .h(BAR_HEIGHT)
                .rounded_full()
                .bg(if bright {
                    cx.theme().foreground.opacity(0.55)
                } else {
                    cx.theme().muted_foreground.opacity(0.35)
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

    /// 条块宽度取区间内最长行（空行 0；抽样步长与 `minimap_rows` 一致）。
    #[test]
    fn minimap_bar_lengths_shape() {
        let text = ropey::Rope::from_str("a\nabcdefghij\n\nabc");
        // 4 行：步长 1，每行自成一条。
        assert_eq!(minimap_bar_lengths(&text, 4, 200), vec![1, 10, 0, 3]);
        // 上限 2 条：步长 2，区间 [0..2)、[2..4)，取区间内最长行。
        assert_eq!(minimap_bar_lengths(&text, 4, 2), vec![10, 3]);
        // 空文档单条 0。
        assert_eq!(
            minimap_bar_lengths(&ropey::Rope::from_str(""), 0, 200),
            vec![0]
        );
    }

    /// 纵坐标→行号与条块视觉位置严格对应；条带上下空白分别钳制到首/末行。
    #[rgpui::test]
    fn minimap_row_at_y_maps_strip(cx: &mut crate::TestAppContext) {
        use crate::{point, size};

        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let text = (0..100)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, &text));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|_, cx| {
            editor.update(cx, |state, _| {
                // 模拟条带边界：窗口坐标 top=100，高 408（200 条×2px + 上下各 4px）。
                state.minimap.strip_bounds =
                    Bounds::new(point(px(700.), px(100.)), size(px(76.), px(408.)));
            });
        });
        let row_at = |y: f32| editor.read_with(cx, |state, _| state.minimap_row_at_y(px(y), 100));
        // 顶部内边距内 → 首行。
        assert_eq!(row_at(102.), 0);
        // 第 10 条条块中部（y=104+20+1）→ 第 10 行（步长 1）。
        assert_eq!(row_at(125.), 10);
        // 条块区下方空白 → 末行。
        assert_eq!(row_at(400.), 99);
        // 空文档 → 0。
        assert_eq!(
            editor.read_with(cx, |state, _| state.minimap_row_at_y(px(200.), 0)),
            0
        );
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
