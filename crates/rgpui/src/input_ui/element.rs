use smallvec::SmallVec;
use std::ops::Range;
use std::rc::Rc;

use crate::{
    AnyElement, App, Bounds, Corners, Edges, Element, ElementId, ElementInputHandler, Entity,
    GlobalElementId, Half, HighlightStyle, Hitbox, HitboxBehavior, Hsla, InteractiveElement as _,
    IntoElement, LayoutId, MouseButton, MouseMoveEvent, MouseUpEvent, Path, PathBuilder, Pixels,
    Point, Position, ShapedLine, SharedString, Size, Style, Styled as _, TextAlign, TextRun,
    TextStyle, UnderlineStyle, Window, fill, point, px, relative, size,
};
use ropey::Rope;

use crate::theme::ActiveTheme as _;
use crate::{Button, ButtonVariants as _, IconName, Scrollbar, Selectable as _, Sizable as _};

use super::{
    InputState, LastLayout, MASK_CHAR, RopeExt as _, TextDecoration, WhitespaceIndicators,
    blink_cursor::CURSOR_WIDTH, display_map::LineLayout, mode::InputMode,
};

const BOTTOM_MARGIN_ROWS: usize = 3;
pub(super) const RIGHT_MARGIN: Pixels = px(10.);
pub(super) const LINE_NUMBER_RIGHT_MARGIN: Pixels = px(10.);
const FOLD_ICON_WIDTH: Pixels = px(14.);
const FOLD_ICON_HITBOX_WIDTH: Pixels = px(18.);

/// 将文本装饰范围限制在可见范围内并合并样式。
fn compose_decorations(
    mut styles: Vec<(Range<usize>, HighlightStyle)>,
    decorations: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
    visible_byte_range: Range<usize>,
) -> Option<Vec<(Range<usize>, HighlightStyle)>> {
    let mut visible_decorations = decorations
        .into_iter()
        .filter_map(|(range, style)| {
            let range =
                range.start.max(visible_byte_range.start)..range.end.min(visible_byte_range.end);
            (!range.is_empty()).then_some((range, style))
        })
        .peekable();

    if visible_decorations.peek().is_none() {
        return (!styles.is_empty()).then_some(styles);
    }
    if styles.is_empty() {
        styles.push((visible_byte_range.clone(), HighlightStyle::default()));
    }

    Some(crate::combine_highlights(visible_decorations, styles).collect())
}

/// 依次组合多个装饰集合，靠前的集合优先级更高。
fn compose_decoration_collections<'a>(
    mut styles: Vec<(Range<usize>, HighlightStyle)>,
    collections: impl IntoIterator<Item = &'a [TextDecoration]>,
    visible_byte_range: Range<usize>,
) -> Option<Vec<(Range<usize>, HighlightStyle)>> {
    for decorations in collections {
        styles = compose_decorations(
            styles,
            decorations
                .iter()
                .map(|decoration| (decoration.range.clone(), decoration.style)),
            visible_byte_range.clone(),
        )
        .unwrap_or_default();
    }

    (!styles.is_empty()).then_some(styles)
}

/// 编辑区滚动条布局信息。
#[derive(Clone, Copy, Debug, PartialEq)]
struct EditorScrollbarLayout {
    bounds: Bounds<Pixels>,
    scroll_size: Size<Pixels>,
}

/// 编辑区滚动条快照，用于在 paint 阶段重建滚动条。
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct EditorScrollbarSnapshot {
    layout: EditorScrollbarLayout,
    cursor_scroll_offset: Point<Pixels>,
    soft_wrap: bool,
}

impl EditorScrollbarSnapshot {
    /// 从输入边界、布局与滚动尺寸创建快照。
    fn new(
        input_bounds: Bounds<Pixels>,
        last_layout: &LastLayout,
        scroll_size: Size<Pixels>,
        cursor_scroll_offset: Point<Pixels>,
        state: &InputState,
    ) -> Self {
        Self {
            layout: EditorScrollbarLayout::new(
                input_bounds,
                last_layout.line_number_width,
                scroll_size,
                state.editor_scrollbar_paddings.get(),
            ),
            cursor_scroll_offset,
            soft_wrap: state.soft_wrap,
        }
    }
}

impl EditorScrollbarLayout {
    /// 计算滚动条边界与内容尺寸。
    fn new(
        input_bounds: Bounds<Pixels>,
        line_number_width: Pixels,
        scroll_size: Size<Pixels>,
        paddings: Edges<Pixels>,
    ) -> Self {
        let left = if line_number_width == px(0.) {
            px(0.)
        } else {
            paddings.left + line_number_width - LINE_NUMBER_RIGHT_MARGIN
        };

        Self {
            bounds: Bounds::new(
                point(
                    input_bounds.origin.x + left,
                    input_bounds.origin.y - paddings.top,
                ),
                size(
                    input_bounds.size.width - left + paddings.right,
                    input_bounds.size.height + paddings.top + paddings.bottom,
                ),
            ),
            scroll_size: size(
                scroll_size.width - left + paddings.right + RIGHT_MARGIN,
                scroll_size.height,
            ),
        }
    }
}

/// 编辑区滚动条，渲染在输入框之上。
pub(super) struct EditorScrollbar {
    state: Entity<InputState>,
}

impl EditorScrollbar {
    /// 创建绑定到给定状态的滚动条。
    pub(super) fn new(state: Entity<InputState>) -> Self {
        Self { state }
    }
}

impl IntoElement for EditorScrollbar {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for EditorScrollbar {
    type RequestLayoutState = ();
    type PrepaintState = Option<AnyElement>;

    fn id(&self) -> Option<ElementId> {
        Some("editor-scrollbar".into())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&crate::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.position = Position::Absolute;
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();

        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&crate::InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let state = self.state.read(cx);
        let Some(snapshot) = state.editor_scrollbar_snapshot.get() else {
            return None;
        };
        let scroll_handle = state.scroll_handle.clone();

        if scroll_handle.offset() != snapshot.cursor_scroll_offset {
            scroll_handle.set_offset(snapshot.cursor_scroll_offset);
        }

        let mut scrollbar = if !snapshot.soft_wrap {
            Scrollbar::new(&scroll_handle)
        } else {
            Scrollbar::vertical(&scroll_handle)
        }
        .scroll_size(snapshot.layout.scroll_size)
        .into_any_element();

        scrollbar.prepaint_as_root(
            snapshot.layout.bounds.origin,
            snapshot.layout.bounds.size.into(),
            window,
            cx,
        );
        Some(scrollbar)
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&crate::InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(scrollbar) = prepaint.as_mut() {
            scrollbar.paint(window, cx);
        }
    }
}

/// 将自动增长模式的垂直滚动偏移钳制到视口范围内。
fn clamp_auto_grow_vertical_scroll_offset(
    mode: &InputMode,
    scroll_top: Pixels,
    scroll_height: Pixels,
    input_height: Pixels,
) -> Pixels {
    if mode.is_auto_grow() {
        scroll_top.clamp((input_height - scroll_height).min(px(0.)), px(0.))
    } else {
        scroll_top
    }
}

/// 将原文中的字节偏移转换为掩码显示串中的字节偏移。
///
/// 掩码串由每个字符重复一个 `MASK_CHAR` 组成。由于 `MASK_CHAR` 在 UTF-8 中
/// 可能占多个字节，掩码串中的字节偏移为 `char_index * MASK_CHAR.len_utf8()`。
fn masked_display_offset(text: &Rope, original_offset: usize) -> usize {
    text.offset_to_char_index(original_offset) * MASK_CHAR.len_utf8()
}

/// 将 IME 标记范围（基于原文）映射到显示文本坐标空间，
/// 使 run 边界不会落在多字节 `MASK_CHAR` 内部而引发塑形 panic。
fn ime_marked_display_range(
    text: &Rope,
    marked_range: Option<Range<usize>>,
    masked: bool,
) -> Option<Range<usize>> {
    let marked = marked_range?;
    if masked {
        Some(masked_display_offset(text, marked.start)..masked_display_offset(text, marked.end))
    } else {
        Some(marked)
    }
}

/// 光标应保持距视口顶/底边缘的最小像素填充，在自动滚动生效前使用。
/// 支撑 [`InputState::cursor_surrounding_lines`]。
///
/// 自动增长使用一行。否则 `None` 回退到历史启发式（`BOTTOM_MARGIN_ROWS` 行，
/// 或小视口时一行）；`Some(n)` 使用 `n` 行。结果会饱和到半视口，
/// 以免过大的覆盖值反转上下阈值导致滚动反馈循环。
pub(super) fn cursor_surrounding_padding(
    is_auto_grow: bool,
    override_lines: Option<usize>,
    visible_lines: usize,
    line_height: Pixels,
) -> Pixels {
    if is_auto_grow {
        return line_height;
    }
    let raw = match override_lines {
        Some(lines) => lines as f32 * line_height,
        None => {
            if visible_lines < BOTTOM_MARGIN_ROWS * 8 {
                line_height
            } else {
                BOTTOM_MARGIN_ROWS * line_height
            }
        }
    };
    // 饱和到半视口，使上下边距能够共存。
    let viewport_half = (visible_lines as f32 * line_height).half();
    raw.min(viewport_half)
}

/// 编辑区可滚动区域中最后一行下方的空余像素高度。支撑 [`InputState::scroll_beyond_last_line`]。
///
/// 代码编辑器模式外为 `0`。内部 `None` 为半视口（下限 `BOTTOM_MARGIN_ROWS` 行高）；
/// `Some(n)` 精确为 `n` 个行高。
fn empty_bottom_height(
    is_code_editor: bool,
    override_rows: Option<usize>,
    viewport_height: Pixels,
    line_height: Pixels,
) -> Pixels {
    if !is_code_editor {
        return px(0.);
    }
    match override_rows {
        Some(rows) => rows as f32 * line_height,
        None => viewport_height.half().max(BOTTOM_MARGIN_ROWS * line_height),
    }
}

/// 折叠图标布局信息。
struct FoldIconLayout {
    /// 行号区域命中框（用于悬停检测）
    line_number_hitbox: Hitbox,
    /// 每个折叠候选的 (display_row, is_folded, icon_element) 列表
    icons: Vec<(usize, bool, crate::AnyElement)>,
}

/// 文本元素，负责渲染输入框中的文本、光标、选区、行号与折叠图标。
pub(super) struct TextElement {
    pub(crate) state: Entity<InputState>,
    placeholder: SharedString,
}

impl TextElement {
    /// 创建绑定到给定状态的文本元素。
    pub(super) fn new(state: Entity<InputState>) -> Self {
        Self {
            state,
            placeholder: SharedString::default(),
        }
    }

    /// 设置输入框的占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 注册拖拽移动与鼠标释放的监听器，用于选区拖拽与自动滚动。
    fn paint_mouse_listeners(&mut self, window: &mut Window, _: &mut App) {
        window.on_mouse_event({
            let state = self.state.clone();

            move |event: &MouseMoveEvent, _, window, cx| {
                if event.pressed_button == Some(MouseButton::Left) {
                    state.update(cx, |state, cx| {
                        state.on_drag_move(event, window, cx);
                    });
                }
            }
        });

        window.on_mouse_event({
            let state = self.state.clone();
            move |_: &MouseUpEvent, phase, _, cx| {
                if !phase.bubble() {
                    return;
                }

                // 鼠标释放时停止自动滚动，同时停止选择。
                state.update(cx, |state, _| {
                    state.auto_scroll.stop();
                    state.selecting = false;
                });
            }
        });
    }

    /// 返回：
    ///
    /// - 光标边界
    /// - 滚动偏移
    /// - 当前行索引（所有行，不仅是可见行）
    ///
    /// 此方法也会更新滚动以跟踪光标。
    fn layout_cursor(
        &self,
        last_layout: &LastLayout,
        bounds: &mut Bounds<Pixels>,
        scroll_size: Size<Pixels>,
        _: &mut Window,
        cx: &mut App,
    ) -> (Option<Bounds<Pixels>>, Point<Pixels>, Option<usize>) {
        let state = self.state.read(cx);

        let line_height = last_layout.line_height;
        let visible_range = &last_layout.visible_range;
        let lines = &last_layout.lines;
        let line_number_width = last_layout.line_number_width;

        let mut selected_range = state.selected_range;

        if let Some(ime_marked_range) = &state.ime_marked_range {
            selected_range = (ime_marked_range.end..ime_marked_range.end).into();
        }
        let is_selected_all = selected_range.len() == state.text.len();

        let mut cursor = state.cursor();
        // 原始（掩码前）偏移对应的 buffer 行，用于定位光标行。
        let cursor_row = state.text.offset_to_point(cursor).row;
        let sel_start_row = state.text.offset_to_point(selected_range.start).row;
        let sel_end_row = state.text.offset_to_point(selected_range.end).row;
        if state.masked {
            selected_range.start = masked_display_offset(&state.text, selected_range.start);
            selected_range.end = masked_display_offset(&state.text, selected_range.end);
            cursor = masked_display_offset(&state.text, cursor);
        }

        let mut scroll_offset = state.scroll_handle.offset();

        // 光标与视口顶/底边缘之间保留的填充，用于下面的自动滚入视图计算。
        let top_bottom_margin = cursor_surrounding_padding(
            state.mode.is_auto_grow(),
            state.cursor_surrounding_lines,
            visible_range.len(),
            line_height,
        );

        // 将光标或选区端点解析为内容空间中的位置。
        let visible_buffer_lines = &last_layout.visible_buffer_lines;
        let caret_for = |row: usize, offset: usize, affinity: bool| -> Point<Pixels> {
            // buffer 行 `row` 顶部在内容空间中的 y。
            let top = line_height * state.display_map.buffer_line_to_display_row(row);
            let line_origin = point(px(0.), top);

            if let Some(vi) = visible_buffer_lines.iter().position(|&bl| bl == row) {
                let line = &lines[vi];
                let line_start = last_layout.visible_line_byte_offsets[vi];
                let local = offset.saturating_sub(line_start);
                if let Some(pos) = line.position_for_index(local, last_layout, affinity) {
                    return line_origin + pos;
                }
            }
            line_origin
        };

        let current_row = Some(cursor_row);
        let cursor_pos = caret_for(cursor_row, cursor, state.cursor_line_end_affinity);
        let cursor_start = caret_for(sel_start_row, selected_range.start, false);
        let cursor_end = caret_for(sel_end_row, selected_range.end, false);

        let cursor_bounds = {
            let selection_changed = state.last_selected_range != Some(selected_range);
            let auto_scrolling = state.auto_scroll.is_active();
            if selection_changed && !is_selected_all {
                // 右对齐使用 0 边距：光标被单独钳制在边界内，
                // 因此我们从不针对边界光标滚动文本，避免首次点击跳变。
                let safety_margin = match last_layout.text_align {
                    TextAlign::Left => RIGHT_MARGIN,
                    TextAlign::Right => px(0.),
                    TextAlign::Center => CURSOR_WIDTH,
                };

                scroll_offset.x = if scroll_offset.x + cursor_pos.x
                    > (bounds.size.width - line_number_width - safety_margin)
                {
                    // 光标在右侧之外
                    bounds.size.width - line_number_width - safety_margin - cursor_pos.x
                } else if scroll_offset.x + cursor_pos.x < px(0.) {
                    // 光标在左侧之外
                    scroll_offset.x - cursor_pos.x
                } else {
                    scroll_offset.x
                };

                // 自动滚动接管 y 轴时抑制光标跟随，
                // 避免与后台滚动任务争抢。
                if !auto_scrolling {
                    // 若改变 scroll_offset.y，GPUI 将渲染并触发下一轮循环。
                    // 因此此处只按 `line_height` 调整偏移，使移动更平滑。
                    scroll_offset.y = if scroll_offset.y + cursor_pos.y
                        > bounds.size.height - top_bottom_margin
                    {
                        // 光标在底部之外
                        scroll_offset.y - line_height
                    } else if scroll_offset.y + cursor_pos.y < top_bottom_margin {
                        // 光标在顶部之外
                        (scroll_offset.y + line_height).min(px(0.))
                    } else {
                        scroll_offset.y
                    };
                }

                // 选区反向时跟随选区起点滚动
                if state.selection_reversed {
                    if scroll_offset.x + cursor_start.x < px(0.) {
                        // 选区起点在左侧之外
                        scroll_offset.x = -cursor_start.x;
                    }
                    if !auto_scrolling && scroll_offset.y + cursor_start.y < px(0.) {
                        // 选区起点在顶部之外
                        scroll_offset.y = -cursor_start.y;
                    }
                } else {
                    if scroll_offset.x + cursor_end.x <= px(0.) {
                        // 选区终点在左侧之外
                        scroll_offset.x = -cursor_end.x;
                    }
                    if !auto_scrolling && scroll_offset.y + cursor_end.y <= px(0.) {
                        // 选区终点在顶部之外
                        scroll_offset.y = -cursor_end.y;
                    }
                }
            }

            // 光标边界
            let cursor_height = match state.size {
                crate::ElementSize::Large => 1.,
                crate::ElementSize::Small => 0.75,
                _ => 0.85,
            } * line_height;

            // 使光标匹配延迟滚动目标（下面应用），否则文本绘制在延迟偏移
            // 处而光标跟随光标滚动，在字段中间闪烁。
            let cursor_scroll_x = state
                .deferred_scroll_offset
                .map(|offset| offset.x)
                .unwrap_or(scroll_offset.x);

            // 右对齐时，将光标钳制在 bounds 右边缘内，
            // 使其无需移动文本即可保持可见。
            let cursor_x = bounds.left() + cursor_pos.x + line_number_width + cursor_scroll_x;
            let cursor_x = if last_layout.text_align == TextAlign::Right {
                cursor_x.min(bounds.right() - CURSOR_WIDTH)
            } else {
                cursor_x
            };
            Some(Bounds::new(
                point(
                    cursor_x,
                    bounds.top() + cursor_pos.y + ((line_height - cursor_height) / 2.),
                ),
                size(CURSOR_WIDTH, cursor_height),
            ))
        };

        if let Some(deferred_scroll_offset) = state.deferred_scroll_offset {
            scroll_offset = deferred_scroll_offset;
        }
        scroll_offset.y = clamp_auto_grow_vertical_scroll_offset(
            &state.mode,
            scroll_offset.y,
            scroll_size.height,
            bounds.size.height,
        );

        bounds.origin = bounds.origin + scroll_offset;

        (cursor_bounds, scroll_offset, current_row)
    }

    /// 将匹配范围布局为路径。
    pub(crate) fn layout_match_range(
        range: Range<usize>,
        last_layout: &LastLayout,
        bounds: &Bounds<Pixels>,
    ) -> Option<Path<Pixels>> {
        if range.is_empty() {
            return None;
        }

        if range.start < last_layout.visible_range_offset.start
            || range.end > last_layout.visible_range_offset.end
        {
            return None;
        }

        let line_height = last_layout.line_height;
        let visible_top = last_layout.visible_top;
        let lines = &last_layout.lines;
        let line_number_width = last_layout.line_number_width;

        let start_ix = range.start;
        let end_ix = range.end;

        // 从 visible_top 开始（已包含可见范围前所有行）
        let mut offset_y = visible_top;
        let mut line_corners = vec![];

        // 仅遍历可见（非隐藏）buffer 行
        for (prev_lines_offset, line) in last_layout
            .visible_line_byte_offsets
            .iter()
            .zip(lines.iter())
        {
            let prev_lines_offset = *prev_lines_offset;
            let line_size = line.size(line_height);
            let line_wrap_width = line_size.width;

            let line_origin = point(px(0.), offset_y);

            let line_cursor_start = line.position_for_index(
                start_ix.saturating_sub(prev_lines_offset),
                last_layout,
                false,
            );
            let line_cursor_end = line.position_for_index(
                end_ix.saturating_sub(prev_lines_offset),
                last_layout,
                false,
            );

            if line_cursor_start.is_some() || line_cursor_end.is_some() {
                let start = line_cursor_start
                    .unwrap_or_else(|| line.position_for_index(0, last_layout, false).unwrap());

                let end = line_cursor_end.unwrap_or_else(|| {
                    line.position_for_index(line.len(), last_layout, false)
                        .unwrap()
                });

                // 将选区拆分为多个片段
                let wrapped_lines =
                    (end.y / line_height).ceil() as usize - (start.y / line_height).ceil() as usize;

                let mut end_x = end.x;
                if wrapped_lines > 0 {
                    end_x = line_wrap_width;
                }

                // 空行选区至少保证 6px 宽度。
                end_x = end_x.max(start.x + px(6.));

                line_corners.push(Corners {
                    top_left: line_origin + point(start.x, start.y),
                    top_right: line_origin + point(end_x, start.y),
                    bottom_left: line_origin + point(start.x, start.y + line_height),
                    bottom_right: line_origin + point(end_x, start.y + line_height),
                });

                // 换行片段
                for i in 1..=wrapped_lines {
                    let indent = line.wrap_indent;
                    let start = point(indent, start.y + i as f32 * line_height);
                    let mut end = point(end.x, end.y + i as f32 * line_height);
                    if i < wrapped_lines {
                        end.x = line_size.width;
                    }

                    line_corners.push(Corners {
                        top_left: line_origin + point(start.x, start.y),
                        top_right: line_origin + point(end.x, start.y),
                        bottom_left: line_origin + point(start.x, start.y + line_height),
                        bottom_right: line_origin + point(end.x, start.y + line_height),
                    });
                }
            }

            if line_cursor_start.is_some() && line_cursor_end.is_some() {
                break;
            }

            offset_y += line_size.height;
        }

        let mut points = vec![];
        if line_corners.is_empty() {
            return None;
        }

        // 修正角点，确保从左到右方向
        for corners in &mut line_corners {
            if corners.top_left.x > corners.top_right.x {
                std::mem::swap(&mut corners.top_left, &mut corners.top_right);
                std::mem::swap(&mut corners.bottom_left, &mut corners.bottom_right);
            }
        }

        for corners in &line_corners {
            points.push(corners.top_right);
            points.push(corners.bottom_right);
            points.push(corners.bottom_left);
        }

        let mut rev_line_corners = line_corners.iter().rev().peekable();
        while let Some(corners) = rev_line_corners.next() {
            points.push(corners.top_left);
            if let Some(next) = rev_line_corners.peek() {
                if next.top_left.x != corners.top_left.x {
                    points.push(point(next.top_left.x, corners.top_left.y));
                }
            }
        }

        let path_origin = bounds.origin + point(line_number_width, px(0.));
        let first_p = *points.get(0).unwrap();
        let mut builder = PathBuilder::fill();
        builder.move_to(path_origin + first_p);
        for p in points.iter().skip(1) {
            builder.line_to(path_origin + *p);
        }

        builder.build().ok()
    }

    /// 布局选区路径。
    fn layout_selections(
        &self,
        last_layout: &LastLayout,
        bounds: &mut Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Path<Pixels>> {
        let state = self.state.read(cx);
        if !state.focus_handle.is_focused(window) {
            return None;
        }

        let mut selected_range = state.selected_range;
        if let Some(ime_marked_range) = &state.ime_marked_range {
            if !ime_marked_range.is_empty() {
                selected_range = (ime_marked_range.end..ime_marked_range.end).into();
            }
        }
        if selected_range.is_empty() {
            return None;
        }

        if state.masked {
            selected_range.start = masked_display_offset(&state.text, selected_range.start);
            selected_range.end = masked_display_offset(&state.text, selected_range.end);
        }

        let (start_ix, end_ix) = if selected_range.start < selected_range.end {
            (selected_range.start, selected_range.end)
        } else {
            (selected_range.end, selected_range.start)
        };

        let range = start_ix.max(last_layout.visible_range_offset.start)
            ..end_ix.min(last_layout.visible_range_offset.end);

        Self::layout_match_range(range, &last_layout, bounds)
    }

    /// 计算视口中的可见行范围。
    ///
    /// 返回
    ///
    /// - visible_range: 可见范围基于未换行行（0 起始）。
    /// - visible_buffer_lines: 可见范围内未隐藏的 buffer 行索引。
    /// - visible_top: 滚动视口中第一个可见行的顶部位置。
    fn calculate_visible_range(
        &self,
        state: &InputState,
        line_height: Pixels,
        input_height: Pixels,
    ) -> (Range<usize>, Vec<usize>, Pixels) {
        // 添加额外行，避免滚到底部时显示空白。
        let extra_rows = 1;
        if state.mode.is_single_line() {
            return (0..1, vec![0], px(0.));
        }

        let total_lines = state.display_map.wrap_row_count();
        let display_count = state.display_map.display_row_count();
        let buffer_line_count = state.display_map.buffer_line_count();
        if display_count == 0 || buffer_line_count == 0 {
            return (0..0, Vec::new(), px(0.));
        }

        let mut scroll_top = if let Some(deferred_scroll_offset) = state.deferred_scroll_offset {
            deferred_scroll_offset.y
        } else {
            state.scroll_handle.offset().y
        };
        scroll_top = clamp_auto_grow_vertical_scroll_offset(
            &state.mode,
            scroll_top,
            line_height * total_lines,
            input_height,
        );

        // 显示行均匀为 `line_height` 高，可见窗口直接映射为显示行范围。
        let viewport_top = (-scroll_top).max(px(0.));
        let viewport_bottom = viewport_top + input_height;
        let line_height_f = f32::from(line_height);
        let first_display =
            ((f32::from(viewport_top) / line_height_f).floor() as usize).min(display_count - 1);
        let last_display =
            ((f32::from(viewport_bottom) / line_height_f).ceil() as usize).min(display_count - 1);

        let start_line = state.display_map.display_row_to_buffer_line(first_display);
        let end_line = state.display_map.display_row_to_buffer_line(last_display);

        // 第一个可见 buffer 行顶部在内容空间中的 y。
        let visible_top = match state
            .display_map
            .buffer_line_to_display_row_range(start_line)
        {
            Some(range) => line_height * range.start,
            None => line_height * first_display,
        };

        let visible_range = start_line..(end_line + 1 + extra_rows).min(buffer_line_count);

        // 收集可见范围内的未隐藏 buffer 行
        let mut visible_buffer_lines = Vec::with_capacity(visible_range.len());
        for ix in visible_range.clone() {
            if state.display_map.visible_wrap_row_count_for_buffer_line(ix) > 0 {
                visible_buffer_lines.push(ix);
            }
        }

        (visible_range, visible_buffer_lines, visible_top)
    }

    /// 返回 (line_number_width, line_number_len)
    fn layout_line_numbers(
        state: &InputState,
        text: &Rope,
        font_size: Pixels,
        style: &TextStyle,
        window: &mut Window,
    ) -> (Pixels, usize) {
        let total_lines = text.lines_len();
        // 在最大行号之外再加一列，使右对齐数字与左边缘保持间距。
        let line_number_len = total_lines.max(1).ilog10() as usize + 2;

        let mut line_number_width = if state.mode.line_number() {
            let empty_line_number = window.text_system().shape_line(
                "+".repeat(line_number_len).into(),
                font_size,
                &[TextRun {
                    len: line_number_len,
                    font: style.font(),
                    color: crate::black(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }],
                None,
            );

            empty_line_number.width + LINE_NUMBER_RIGHT_MARGIN
        } else if state.mode.is_code_editor() && state.mode.is_multi_line() {
            LINE_NUMBER_RIGHT_MARGIN
        } else {
            px(0.)
        };

        if state.mode.is_folding() {
            // 为折叠图标预留额外空间
            line_number_width += FOLD_ICON_HITBOX_WIDTH
        }

        (line_number_width, line_number_len)
    }

    /// 为空白指示符（空格和制表符）布局塑形行。
    ///
    /// 返回 `WhitespaceIndicators`，包含空格和制表符字符的塑形行。
    fn layout_whitespace_indicators(
        state: &InputState,
        text_size: Pixels,
        style: &TextStyle,
        window: &mut Window,
        cx: &App,
    ) -> Option<WhitespaceIndicators> {
        if !state.show_whitespaces {
            return None;
        }

        let invisible_color = cx
            .theme()
            .highlight_theme
            .style
            .editor_invisible
            .unwrap_or(cx.theme().muted_foreground);

        let space_font_size = text_size.half();
        let tab_font_size = text_size;

        let space_text = SharedString::new_static("•");
        let space = window.text_system().shape_line(
            space_text.clone(),
            space_font_size,
            &[TextRun {
                len: space_text.len(),
                font: style.font(),
                color: invisible_color,
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        );

        let tab_text = SharedString::new_static("→");
        let tab = window.text_system().shape_line(
            tab_text.clone(),
            tab_font_size,
            &[TextRun {
                len: tab_text.len(),
                font: style.font(),
                color: invisible_color,
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        );

        Some(WhitespaceIndicators { space, tab })
    }

    /// 在 prepaint 阶段布局折叠图标命中框。
    ///
    /// 为折叠图标区域创建命中框，位于行号右侧。
    fn layout_fold_icons(
        &self,
        origin_x: Pixels,
        bounds: &Bounds<Pixels>,
        last_layout: &LastLayout,
        window: &mut Window,
        cx: &mut App,
    ) -> FoldIconLayout {
        // 第一遍：从状态收集折叠信息
        struct FoldInfo {
            buffer_line: usize,
            is_folded: bool,
            display_row: usize,
            offset_y: Pixels,
        }

        let line_number_hitbox = window.insert_hitbox(
            Bounds::new(
                point(origin_x, bounds.origin.y + last_layout.visible_top),
                size(last_layout.line_number_width, bounds.size.height),
            ),
            HitboxBehavior::Normal,
        );

        let mut icon_layout = FoldIconLayout {
            line_number_hitbox,
            icons: vec![],
        };

        let fold_infos: Vec<FoldInfo> = {
            let state = self.state.read(cx);
            if !state.mode.is_folding() {
                return icon_layout;
            }

            let mut infos = Vec::with_capacity(last_layout.visible_buffer_lines.len());
            let mut offset_y = last_layout.visible_top;

            for (line, &buffer_line) in last_layout
                .lines
                .iter()
                .zip(last_layout.visible_buffer_lines.iter())
            {
                if state.display_map.is_fold_candidate(buffer_line) {
                    let is_folded = state.display_map.is_folded_at(buffer_line);
                    infos.push(FoldInfo {
                        buffer_line,
                        is_folded,
                        display_row: buffer_line,
                        offset_y,
                    });
                }

                offset_y += line.wrapped_lines.len() * last_layout.line_height;
            }

            infos
        }; // state 在此处释放

        // 第二遍：创建并预绘制图标
        let line_height = last_layout.line_height;
        let line_number_width =
            last_layout.line_number_width - LINE_NUMBER_RIGHT_MARGIN - FOLD_ICON_HITBOX_WIDTH;
        let icon_relative_pos = point(
            (FOLD_ICON_HITBOX_WIDTH - FOLD_ICON_WIDTH).half(),
            (line_height - FOLD_ICON_WIDTH).half(),
        );

        for (ix, info) in fold_infos.iter().enumerate() {
            // 将折叠图标放在行号右侧。
            // 使用 origin_x（未滚动）使图标在水平滚动时保持在沟槽中。
            let fold_icon_bounds = Bounds::new(
                point(
                    origin_x + icon_relative_pos.x + line_number_width,
                    bounds.origin.y + icon_relative_pos.y + info.offset_y,
                ),
                size(FOLD_ICON_HITBOX_WIDTH, line_height),
            );

            // 创建并预绘制图标
            let mut icon = Button::new(("fold", ix))
                .ghost()
                .icon(if info.is_folded {
                    IconName::ChevronRight
                } else {
                    IconName::ChevronDown
                })
                .xsmall()
                .rounded_xs()
                .size(FOLD_ICON_WIDTH)
                .selected(info.is_folded)
                .on_mouse_down(MouseButton::Left, {
                    let state = self.state.clone();
                    let buffer_line = info.buffer_line;
                    move |_, _: &mut Window, cx: &mut App| {
                        cx.stop_propagation();

                        state.update(cx, |state, cx| {
                            state.display_map.toggle_fold(buffer_line);
                            cx.notify();
                        });
                    }
                })
                .into_any_element();

            icon.prepaint_as_root(
                fold_icon_bounds.origin,
                fold_icon_bounds.size.into(),
                window,
                cx,
            );

            icon_layout
                .icons
                .push((info.display_row, info.is_folded, icon));
        }

        icon_layout
    }

    /// 使用预绘制命中框绘制折叠图标。
    ///
    /// 处理：
    /// - 渲染折叠图标（折叠为右箭头，展开为下箭头）
    /// - 鼠标点击切换折叠状态
    /// - 悬停时切换光标样式
    /// - 仅悬停或当前行时显示图标
    fn paint_fold_icons(
        &mut self,
        fold_icon_layout: &mut FoldIconLayout,
        current_row: Option<usize>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let is_hovered = fold_icon_layout.line_number_hitbox.is_hovered(window);
        for (display_row, is_folded, icon) in fold_icon_layout.icons.iter_mut() {
            let is_current_line = current_row == Some(*display_row);

            if !is_hovered && !is_current_line && !*is_folded {
                continue;
            }

            icon.paint(window, cx);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_lines(
        state: &InputState,
        display_text: &Rope,
        last_layout: &LastLayout,
        font_size: Pixels,
        runs: &[TextRun],
        bg_segments: &[(Range<usize>, Hsla)],
        whitespace_indicators: Option<WhitespaceIndicators>,
        window: &mut Window,
    ) -> Vec<LineLayout> {
        let is_single_line = state.mode.is_single_line();

        if is_single_line {
            let shaped_line = window.text_system().shape_line(
                display_text.to_string().into(),
                font_size,
                &runs,
                None,
            );

            let line_layout = LineLayout::new()
                .lines(smallvec::smallvec![shaped_line])
                .with_whitespaces(whitespace_indicators);
            return vec![line_layout];
        }

        // 空文本使用占位符，占位符不在 wrapper map 中。
        if state.text.len() == 0 {
            let placeholder_text = display_text.to_string();
            let mut placeholder_lines = SmallVec::new();

            for (line, line_runs) in placeholder_line_runs(&placeholder_text, runs) {
                let shaped_line = window.text_system().shape_line(
                    line.to_string().into(),
                    font_size,
                    &line_runs,
                    None,
                );
                placeholder_lines.push(shaped_line);
            }

            // 将占位符行保留在单个布局中，以与 visible_* 元数据保持平行。
            let line_layout = LineLayout::new()
                .lines(placeholder_lines)
                .with_whitespaces(whitespace_indicators);
            return vec![line_layout];
        }

        let mut lines = Vec::with_capacity(last_layout.visible_buffer_lines.len());
        let mut run_offset = 0;

        for (vi, &buffer_line) in last_layout.visible_buffer_lines.iter().enumerate() {
            let line_text: String = display_text.slice_line(buffer_line).into();
            let line_item = state
                .display_map
                .line(buffer_line)
                .expect("line should exists in wrapper");

            debug_assert_eq!(line_item.len(), line_text.len());

            let mut wrapped_lines: SmallVec<[ShapedLine; 1]> = SmallVec::with_capacity(1);

            for range in &line_item.wrapped_lines {
                let line_runs = runs_for_range(runs, run_offset, &range);
                let line_runs = if bg_segments.is_empty() {
                    line_runs
                } else {
                    split_runs_by_bg_segments(
                        last_layout.visible_line_byte_offsets[vi] + (range.start),
                        &line_runs,
                        bg_segments,
                    )
                };

                let sub_line: SharedString = line_text[range.clone()].to_string().into();
                let shaped_line = window
                    .text_system()
                    .shape_line(sub_line, font_size, &line_runs, None);

                wrapped_lines.push(shaped_line);
            }

            // 使用第一个可视行的缩进宽度作为续行缩进。
            let wrap_indent = if line_item.indent > 0 && wrapped_lines.len() > 1 {
                let indent_byte_len = line_text
                    .char_indices()
                    .nth(line_item.indent as usize)
                    .map(|(ix, _)| ix)
                    .unwrap_or(line_text.len());
                wrapped_lines[0].x_for_index(indent_byte_len)
            } else {
                px(0.)
            };

            let line_layout = LineLayout::new()
                .lines(wrapped_lines)
                .wrap_indent(wrap_indent)
                .with_whitespaces(whitespace_indicators.clone());
            lines.push(line_layout);

            // +1 为 `\n`
            run_offset += line_text.len() + 1;
        }

        lines
    }

    /// 计算文本装饰样式（不含语法高亮）。
    fn highlight_lines(
        &self,
        visible_buffer_lines: &[usize],
        _visible_top: Pixels,
        visible_byte_range: Range<usize>,
        cx: &mut App,
    ) -> Option<Vec<(Range<usize>, HighlightStyle)>> {
        let state = self.state.read(cx);

        if state.masked {
            return None;
        }

        let _ = visible_buffer_lines;
        compose_decoration_collections(Vec::new(), state.decorations.iter(), visible_byte_range)
    }
}

/// 文本元素的预绘制状态。
pub(super) struct PrepaintState {
    /// 整个行的布局。
    last_layout: LastLayout,
    /// 仅包含视口中可见行的行号（基于 `visible_range`）。
    ///
    /// 子元素为软行。
    line_numbers: Option<Vec<SmallVec<[ShapedLine; 1]>>>,
    /// 整个行的可滚动区域大小。
    scroll_size: Size<Pixels>,
    cursor_bounds: Option<Bounds<Pixels>>,
    cursor_scroll_offset: Point<Pixels>,
    /// 当前行索引（0 起始，无换行，与光标同行）。
    current_row: Option<usize>,
    selection_path: Option<Path<Pixels>>,
    indent_guides_path: Option<Path<Pixels>>,
    bounds: Bounds<Pixels>,
    /// 折叠图标布局数据
    fold_icon_layout: FoldIconLayout,
}

impl PrepaintState {
    /// 返回考虑滚动偏移后的光标边界（若有）。
    fn cursor_bounds_with_scroll(&self) -> Option<Bounds<Pixels>> {
        self.cursor_bounds.map(|mut bounds| {
            bounds.origin.y += self.cursor_scroll_offset.y;
            bounds
        })
    }
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _: Option<&crate::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let state = self.state.read(cx);
        let line_height = window.line_height();

        let mut style = Style::default();
        style.size.width = relative(1.).into();
        if state.mode.is_multi_line() {
            style.flex_grow = 1.0;
            style.size.height = relative(1.).into();
            if state.mode.is_auto_grow() {
                // 自动增长，使高度匹配行数但不超出最大行数。
                let rows = state.mode.max_rows().min(state.mode.rows());
                style.min_size.height = (rows * line_height).into();
            } else {
                style.min_size.height = line_height.into();
            }
        } else {
            // 单行输入的最小高度应为行高。
            style.size.height = line_height.into();
        };

        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _: Option<&crate::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let style = window.text_style();
        let font = style.font();
        let text_size = style.font_size.to_pixels(window.rem_size());

        self.state.update(cx, |state, cx| {
            state.display_map.set_font(font, text_size, cx);
            state.display_map.ensure_text_prepared(&state.text, cx);
        });

        let state = self.state.read(cx);
        let multi_line = state.mode.is_multi_line();
        let text = state.text.clone();
        let is_empty = text.len() == 0;
        let placeholder = self.placeholder.clone();

        let text_style = window.text_style();
        let disabled = state.disabled;
        let dim = |color: Hsla| if disabled { color.opacity(0.5) } else { color };
        let fg = dim(text_style.color);
        let (display_text, text_color) = if is_empty {
            (
                &Rope::from(placeholder.as_str()),
                dim(cx.theme().muted_foreground),
            )
        } else if state.masked {
            (
                &Rope::from(MASK_CHAR.to_string().repeat(text.chars().count())),
                fg,
            )
        } else {
            (&text, fg)
        };

        // 计算行号宽度
        let (line_number_width, line_number_len) =
            Self::layout_line_numbers(&state, &text, text_size, &text_style, window);

        let mut bounds = bounds;
        let wrap_width = if multi_line && state.soft_wrap {
            Some(bounds.size.width - line_number_width - RIGHT_MARGIN)
        } else {
            None
        };

        let wrapping_indent = state.wrapping_indent;
        let wrap_width_changed = state
            .last_layout
            .as_ref()
            .map(|l| l.wrap_width != wrap_width)
            .unwrap_or(true);

        let wrapping_indent_changed = state
            .last_layout
            .as_ref()
            .map(|l| l.wrapping_indent != wrapping_indent)
            .unwrap_or(true);

        if wrap_width_changed || wrapping_indent_changed {
            self.state.update(cx, |state, cx| {
                state.display_map.on_layout_changed(wrap_width, cx);
                state.display_map.set_wrapping_indent(wrapping_indent, cx);
            });
        }

        let state = self.state.read(cx);
        let line_height = window.line_height();

        let (visible_range, visible_buffer_lines, visible_top) =
            self.calculate_visible_range(&state, line_height, bounds.size.height);
        let visible_start_offset = state.text.line_start_offset(visible_range.start);
        let visible_end_offset = state
            .text
            .line_end_offset(visible_range.end.saturating_sub(1));

        let highlight_styles = self.highlight_lines(
            &visible_buffer_lines,
            visible_top,
            visible_start_offset..visible_end_offset,
            cx,
        );

        let state = self.state.read(cx);

        let visible_line_byte_offsets: Vec<usize> = visible_buffer_lines
            .iter()
            .map(|&bl| state.text.line_start_offset(bl))
            .collect();

        // 密码输入（masked: true）时，将字节偏移转换为掩码显示字节偏移，
        // 使 layout_match_range 和 position_for_index 在正确的坐标空间工作。
        let (visible_line_byte_offsets, visible_range_offset) = if state.masked {
            let offsets = visible_line_byte_offsets
                .iter()
                .map(|&o| masked_display_offset(&text, o))
                .collect();
            let range_offset = masked_display_offset(&text, visible_start_offset)
                ..masked_display_offset(&text, visible_end_offset);
            (offsets, range_offset)
        } else {
            (
                visible_line_byte_offsets,
                visible_start_offset..visible_end_offset,
            )
        };

        let mut last_layout = LastLayout {
            visible_range,
            visible_buffer_lines,
            visible_line_byte_offsets,
            visible_top,
            visible_range_offset,
            line_height,
            wrap_width,
            wrapping_indent,
            line_number_width,
            lines: Rc::new(vec![]),
            cursor_bounds: None,
            text_align: state.text_align,
            content_width: bounds.size.width,
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let marked_run = TextRun {
            len: 0,
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(text_color),
                wavy: false,
            }),
            strikethrough: None,
        };

        let ime_marked_range = ime_marked_display_range(
            &text,
            state.ime_marked_range.as_ref().map(|m| m.start..m.end),
            state.masked,
        );

        let runs = if let (false, Some(highlight_styles)) = (is_empty, highlight_styles) {
            let mut runs = Vec::with_capacity(highlight_styles.len() + 2);

            for (range, style) in &highlight_styles {
                let mut run = text_style.clone().highlight(*style).to_run(range.len());
                if disabled {
                    run.color = run.color.opacity(0.5);
                }

                runs.extend(split_run_for_ime_underline(
                    run,
                    range.clone(),
                    ime_marked_range.clone(),
                    marked_run.underline,
                ));
            }
            runs
        } else {
            split_run_for_ime_underline(
                run,
                0..display_text.len(),
                ime_marked_range,
                marked_run.underline,
            )
            .into_vec()
        };

        // 在布局前为空白指示符创建塑形行
        let whitespace_indicators =
            Self::layout_whitespace_indicators(&state, text_size, &text_style, window, cx);

        let lines = Self::layout_lines(
            &state,
            &display_text,
            &last_layout,
            text_size,
            &runs,
            &[],
            whitespace_indicators,
            window,
        );

        let mut longest_line_width = wrap_width.unwrap_or(px(0.));
        // 1. 单行
        // 2. 多行且未启用软换行。
        if state.mode.is_single_line() || !state.soft_wrap {
            let longest_row = state.display_map.longest_row();
            let longest_line: SharedString = state.text.slice_line(longest_row).to_string().into();
            longest_line_width = window
                .text_system()
                .shape_line(
                    longest_line.clone(),
                    text_size,
                    &[TextRun {
                        len: longest_line.len(),
                        font: style.font(),
                        color: crate::black(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }],
                    wrap_width,
                )
                .width;
        }
        last_layout.lines = Rc::new(lines);

        let total_wrapped_lines = state.display_map.wrap_row_count();
        let empty_bottom_height = empty_bottom_height(
            state.mode.is_code_editor(),
            state.scroll_beyond_last_line,
            bounds.size.height,
            line_height,
        );

        let mut scroll_size = size(
            if longest_line_width + line_number_width + RIGHT_MARGIN > bounds.size.width {
                longest_line_width + line_number_width + RIGHT_MARGIN
            } else {
                longest_line_width
            },
            (total_wrapped_lines as f32 * line_height + empty_bottom_height)
                .max(bounds.size.height),
        );

        // TODO: 右侧应留出一些间距，便于聚焦到边界位置
        if last_layout.text_align == TextAlign::Right || last_layout.text_align == TextAlign::Center
        {
            scroll_size.width = longest_line_width + line_number_width;
        }

        // 计算保持光标在视图内的滚动偏移

        // 在 layout_cursor 用滚动偏移修改 bounds.origin 之前保存未滚动的 x。
        // 折叠图标及其命中框必须使用此值，使其在水平滚动时保持在沟槽中。
        let input_bounds = bounds;
        let original_x = bounds.origin.x;

        let (cursor_bounds, cursor_scroll_offset, current_row) =
            self.layout_cursor(&last_layout, &mut bounds, scroll_size, window, cx);
        last_layout.cursor_bounds = cursor_bounds;

        let selection_path = self.layout_selections(&last_layout, &mut bounds, window, cx);

        let state = self.state.read(cx);
        let line_numbers = if state.mode.line_number() {
            let mut line_numbers = Vec::with_capacity(last_layout.visible_buffer_lines.len());
            let other_line_runs = vec![TextRun {
                len: line_number_len,
                font: style.font(),
                color: cx.theme().muted_foreground,
                background_color: None,
                underline: None,
                strikethrough: None,
            }];
            let current_line_runs = vec![TextRun {
                len: line_number_len,
                font: style.font(),
                color: cx.theme().foreground,
                background_color: None,
                underline: None,
                strikethrough: None,
            }];

            // 构建行号
            for (line, &buffer_line) in last_layout
                .lines
                .iter()
                .zip(last_layout.visible_buffer_lines.iter())
            {
                let line_no: SharedString =
                    format!("{:>width$}", buffer_line + 1, width = line_number_len).into();

                let runs = if current_row == Some(buffer_line) {
                    &current_line_runs
                } else {
                    &other_line_runs
                };

                let mut sub_lines: SmallVec<[ShapedLine; 1]> = SmallVec::new();
                sub_lines.push(
                    window
                        .text_system()
                        .shape_line(line_no, text_size, &runs, None),
                );
                for _ in 0..line.wrapped_lines.len().saturating_sub(1) {
                    sub_lines.push(ShapedLine::default());
                }
                line_numbers.push(sub_lines);
            }
            Some(line_numbers)
        } else {
            None
        };

        let indent_guides_path =
            self.layout_indent_guides(state, &bounds, &last_layout, &text_style, window);
        state
            .editor_scrollbar_snapshot
            .set(Some(EditorScrollbarSnapshot::new(
                input_bounds,
                &last_layout,
                scroll_size,
                cursor_scroll_offset,
                state,
            )));
        let fold_icon_layout =
            self.layout_fold_icons(original_x, &bounds, &last_layout, window, cx);

        PrepaintState {
            bounds,
            last_layout,
            scroll_size,
            line_numbers,
            cursor_bounds,
            cursor_scroll_offset,
            current_row,
            selection_path,
            indent_guides_path,
            fold_icon_layout,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _: Option<&crate::InspectorElementId>,
        input_bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (focus_handle, show_cursor, disabled, selected_range) = {
            let state = self.state.read(cx);
            (
                state.focus_handle.clone(),
                state.show_cursor(window, cx),
                state.disabled,
                state.selected_range,
            )
        };
        let focused = focus_handle.is_focused(window);
        let bounds = prepaint.bounds;
        let text_align = prepaint.last_layout.text_align;

        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.state.clone()),
            cx,
        );

        // 绘制多行文本
        let line_height = window.line_height();
        let origin = bounds.origin;

        let invisible_top_padding = prepaint.last_layout.visible_top;
        let active_line_color = cx
            .theme()
            .highlight_theme
            .style
            .editor_active_line
            .map(|color| if disabled { color.opacity(0.5) } else { color });

        // 绘制活动行
        let mut offset_y = px(0.);
        if let Some(line_numbers) = prepaint.line_numbers.as_ref() {
            offset_y += invisible_top_padding;

            // 每项为普通行。
            for (lines, &buffer_line) in line_numbers
                .iter()
                .zip(prepaint.last_layout.visible_buffer_lines.iter())
            {
                let is_active = prepaint.current_row == Some(buffer_line);
                let p = point(input_bounds.origin.x, origin.y + offset_y);
                let height = line_height * lines.len() as f32;
                // 绘制当前行背景
                if is_active {
                    if let Some(bg_color) = active_line_color {
                        window.paint_quad(fill(
                            Bounds::new(p, size(bounds.size.width, height)),
                            bg_color,
                        ));
                    }
                }
                offset_y += height;
            }
        }

        // 绘制缩进参考线
        if let Some(path) = prepaint.indent_guides_path.take() {
            window.paint_path(path, cx.theme().border.opacity(0.85));
        }

        // 绘制选区
        if window.is_window_active() {
            if let Some(path) = prepaint.selection_path.take() {
                window.paint_path(path, cx.theme().selection);
            }
        }

        // 绘制文本
        let mut offset_y = invisible_top_padding;

        // 滚动条偏移始终为正，从左位置开始
        let scroll_offset = if text_align == TextAlign::Right {
            (prepaint.scroll_size.width - prepaint.bounds.size.width).max(px(0.))
        } else if text_align == TextAlign::Center {
            (prepaint.scroll_size.width - prepaint.bounds.size.width)
                .half()
                .max(px(0.))
        } else {
            px(0.)
        };

        for (line, &buffer_line) in prepaint
            .last_layout
            .lines
            .iter()
            .zip(prepaint.last_layout.visible_buffer_lines.iter())
        {
            let _row = buffer_line;
            let line_y = origin.y + offset_y;
            let p = point(
                origin.x + prepaint.last_layout.line_number_width + (scroll_offset),
                line_y,
            );

            // 绘制实际行
            _ = line.paint(
                p,
                line_height,
                text_align,
                Some(prepaint.last_layout.content_width),
                window,
                cx,
            );
            offset_y += line.size(line_height).height;
        }

        // 绘制闪烁光标
        if focused && show_cursor {
            if let Some(cursor_bounds) = prepaint.cursor_bounds_with_scroll() {
                window.paint_quad(fill(cursor_bounds, cx.theme().caret));
            }
        }

        // 绘制行号
        let mut offset_y = px(0.);
        if let Some(line_numbers) = prepaint.line_numbers.as_ref() {
            offset_y += invisible_top_padding;

            if let Some(gutter_bg) = cx.theme().highlight_theme.style.editor_gutter_background {
                window.paint_quad(fill(
                    Bounds {
                        origin: input_bounds.origin,
                        size: size(
                            prepaint.last_layout.line_number_width - LINE_NUMBER_RIGHT_MARGIN,
                            input_bounds.size.height,
                        ),
                    },
                    gutter_bg,
                ));
            }

            // 每项为普通行。
            for (lines, &buffer_line) in line_numbers
                .iter()
                .zip(prepaint.last_layout.visible_buffer_lines.iter())
            {
                let p = point(input_bounds.origin.x, origin.y + offset_y);
                let is_active = prepaint.current_row == Some(buffer_line);

                let height = line_height * lines.len() as f32;
                // 绘制活动行号背景
                if is_active {
                    if let Some(bg_color) = active_line_color {
                        window.paint_quad(fill(
                            Bounds::new(
                                p,
                                size(
                                    prepaint.last_layout.line_number_width
                                        - LINE_NUMBER_RIGHT_MARGIN,
                                    height,
                                ),
                            ),
                            bg_color,
                        ));
                    }
                }

                for line in lines {
                    _ = line.paint(p, line_height, TextAlign::Left, None, window, cx);
                    offset_y += line_height;
                }
            }
        }

        // 绘制折叠图标（仅悬停或当前行可见）
        self.paint_fold_icons(
            &mut prepaint.fold_icon_layout,
            prepaint.current_row,
            window,
            cx,
        );

        self.state.update(cx, |state, cx| {
            state.last_layout = Some(prepaint.last_layout.clone());
            state.last_bounds = Some(bounds);
            state.last_cursor = Some(state.cursor());
            state.set_input_bounds(input_bounds, cx);
            state.last_selected_range = Some(selected_range);
            state.scroll_size = prepaint.scroll_size;
            state.update_scroll_offset(Some(prepaint.cursor_scroll_offset), cx);
            state.deferred_scroll_offset = None;

            cx.notify();
        });

        self.paint_mouse_listeners(window, cx);
    }
}

/// 将占位文本拆分为显示行，并将 runs 裁剪到每行。
fn placeholder_line_runs<'a>(
    display_text: &'a str,
    runs: &[TextRun],
) -> Vec<(&'a str, Vec<TextRun>)> {
    let mut result = Vec::new();
    let mut line_offset = 0;

    for line in display_text.split('\n') {
        let line_runs = runs_for_range(runs, line_offset, &(0..line.len()));
        debug_assert_eq!(
            line_runs.iter().map(|run| run.len).sum::<usize>(),
            line.len()
        );
        result.push((line, line_runs));
        // 在整个占位符坐标空间中前进，包含分隔符。
        line_offset += line.len() + 1;
    }

    result
}

/// 获取给定范围的 runs。
///
/// 范围是换行行的字节范围。
pub(super) fn runs_for_range(
    runs: &[TextRun],
    line_offset: usize,
    range: &Range<usize>,
) -> Vec<TextRun> {
    let mut result = vec![];
    let range = (line_offset + range.start)..(line_offset + range.end);
    let mut cursor = 0;

    for run in runs {
        let run_start = cursor;
        let run_end = cursor + run.len;

        if run_end <= range.start {
            cursor = run_end;
            continue;
        }

        if run_start >= range.end {
            break;
        }

        let start = range.start.max(run_start) - run_start;
        let end = range.end.min(run_end) - run_start;
        let len = end - start;

        if len > 0 {
            result.push(TextRun { len, ..run.clone() });
        }

        cursor = run_end;
    }

    result
}

/// 拆分 run，使 IME 标记范围获得下划线样式。
fn split_run_for_ime_underline(
    run: TextRun,
    run_range: Range<usize>,
    marked_range: Option<Range<usize>>,
    marked_underline: Option<UnderlineStyle>,
) -> SmallVec<[TextRun; 3]> {
    if run.len == 0 {
        return SmallVec::new();
    }

    let Some(marked) = marked_range else {
        return [run].into_iter().collect();
    };

    let intersection_start = run_range.start.max(marked.start);
    let intersection_end = run_range.end.min(marked.end);
    if intersection_start >= intersection_end {
        return [run].into_iter().collect();
    }

    [
        TextRun {
            len: intersection_start - run_range.start,
            ..run.clone()
        },
        TextRun {
            len: intersection_end - intersection_start,
            underline: marked_underline,
            ..run.clone()
        },
        TextRun {
            len: run_range.end - intersection_end,
            ..run
        },
    ]
    .into_iter()
    .filter(|run| run.len > 0)
    .collect()
}

/// 按背景段拆分 runs，用于文档颜色高亮。
fn split_runs_by_bg_segments(
    start_offset: usize,
    runs: &[TextRun],
    bg_segments: &[(Range<usize>, Hsla)],
) -> Vec<TextRun> {
    let mut result = vec![];

    let mut cursor = start_offset;
    for run in runs {
        let mut run_start = cursor;
        let run_end = cursor + run.len;

        for (bg_range, bg_color) in bg_segments {
            if run_end <= bg_range.start || run_start >= bg_range.end {
                continue;
            }

            // 存在重叠
            if run_start < bg_range.start {
                // 添加背景范围之前的部分
                result.push(TextRun {
                    len: bg_range.start - run_start,
                    ..run.clone()
                });
            }

            // 添加带背景颜色的重叠部分
            let overlap_start = run_start.max(bg_range.start);
            let overlap_end = run_end.min(bg_range.end);
            let text_color = if bg_color.l >= 0.5 {
                crate::black()
            } else {
                crate::white()
            };

            let run_len = overlap_end.saturating_sub(overlap_start);
            if run_len > 0 {
                result.push(TextRun {
                    len: run_len,
                    color: text_color,
                    ..run.clone()
                });

                cursor = bg_range.end;
                run_start = cursor;
            }
        }

        if run_end > cursor {
            // 添加背景范围之后的部分
            result.push(TextRun {
                len: run_end - cursor,
                ..run.clone()
            });
        }

        cursor = run_end;
    }

    result
}

#[cfg(test)]
mod tests {
    use crate::geometry::{Bounds, Edges};
    use crate::{HighlightStyle, TextRun, black, blue, font, point, px, red, size};
    use ropey::Rope;

    use super::*;

    #[test]
    fn test_plain_text_decorations_include_unstyled_gaps() {
        let decoration = HighlightStyle {
            background_color: Some(red()),
            ..Default::default()
        };
        let styles = compose_decorations(Vec::new(), [(2..5, decoration)], 0..10).unwrap();

        assert_eq!(
            styles
                .iter()
                .map(|(range, _)| range.clone())
                .collect::<Vec<_>>(),
            vec![0..2, 2..5, 5..10]
        );
        assert_eq!(styles[0].1, HighlightStyle::default());
        assert_eq!(styles[1].1.background_color, Some(red()));
        assert_eq!(styles[2].1, HighlightStyle::default());
    }

    #[test]
    fn test_first_decoration_collection_has_precedence() {
        let first = [TextDecoration::new(
            0..4,
            HighlightStyle {
                background_color: Some(red()),
                ..Default::default()
            },
        )];
        let second = [TextDecoration::new(
            0..4,
            HighlightStyle {
                background_color: Some(blue()),
                ..Default::default()
            },
        )];

        let styles =
            compose_decoration_collections(Vec::new(), [&first[..], &second[..]], 0..4).unwrap();

        assert_eq!(styles.len(), 1);
        assert_eq!(styles[0].1.background_color, Some(red()));
    }

    #[test]
    fn test_editor_scrollbar_layout_uses_current_scroll_size() {
        let input_bounds = Bounds::new(point(px(10.), px(20.)), size(px(300.), px(80.)));
        let paddings = Edges {
            top: px(2.),
            right: px(3.),
            bottom: px(5.),
            left: px(7.),
        };

        let layout =
            EditorScrollbarLayout::new(input_bounds, px(40.), size(px(1000.), px(200.)), paddings);

        assert_eq!(
            layout.bounds,
            Bounds::new(point(px(47.), px(18.)), size(px(266.), px(87.)))
        );
        assert_eq!(layout.scroll_size, size(px(976.), px(200.)));

        let layout_without_gutter =
            EditorScrollbarLayout::new(input_bounds, px(0.), size(px(500.), px(120.)), paddings);

        assert_eq!(
            layout_without_gutter.bounds,
            Bounds::new(point(px(10.), px(18.)), size(px(303.), px(87.)))
        );
        assert_eq!(layout_without_gutter.scroll_size, size(px(513.), px(120.)));
    }

    #[test]
    fn test_auto_grow_scroll_offset_is_clamped_to_current_viewport() {
        let mode = InputMode::auto_grow(3, 8);

        assert_eq!(
            clamp_auto_grow_vertical_scroll_offset(&mode, px(-260.), px(340.), px(160.)),
            px(-180.)
        );
        assert_eq!(
            clamp_auto_grow_vertical_scroll_offset(&mode, px(-40.), px(340.), px(160.)),
            px(-40.)
        );
        assert_eq!(
            clamp_auto_grow_vertical_scroll_offset(&mode, px(20.), px(340.), px(160.)),
            px(0.)
        );

        let plain_text = InputMode::plain_text().multi_line(true);
        assert_eq!(
            clamp_auto_grow_vertical_scroll_offset(&plain_text, px(-260.), px(340.), px(160.)),
            px(-260.)
        );
    }

    #[test]
    fn test_runs_for_range() {
        let run = TextRun {
            len: 0,
            font: font(".SystemUIFont"),
            color: black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        // use hello this-is-test
        let runs = vec![
            // use
            TextRun {
                len: 3,
                ..run.clone()
            },
            // \s
            TextRun {
                len: 1,
                ..run.clone()
            },
            // hello
            TextRun {
                len: 5,
                ..run.clone()
            },
            // \s
            TextRun {
                len: 1,
                ..run.clone()
            },
            // this-is-test
            TextRun {
                len: 12,
                ..run.clone()
            },
        ];

        #[track_caller]
        fn assert_runs(actual: Vec<TextRun>, expected: &[usize]) {
            let left = actual.iter().map(|run| run.len).collect::<Vec<_>>();
            assert_eq!(left, expected);
        }

        assert_runs(runs_for_range(&runs, 0, &(0..0)), &[]);
        assert_runs(runs_for_range(&runs, 0, &(0..100)), &[3, 1, 5, 1, 12]);

        assert_runs(runs_for_range(&runs, 0, &(0..6)), &[3, 1, 2]);
        assert_runs(runs_for_range(&runs, 0, &(1..6)), &[2, 1, 2]);
        assert_runs(runs_for_range(&runs, 0, &(3..10)), &[1, 5, 1]);
        assert_runs(runs_for_range(&runs, 0, &(5..8)), &[3]);
        assert_runs(runs_for_range(&runs, 3, &(0..3)), &[1, 2]);
        assert_runs(runs_for_range(&runs, 3, &(2..10)), &[4, 1, 3]);
        assert_runs(runs_for_range(&runs, 9, &(0..8)), &[1, 7]);
    }

    #[test]
    fn test_split_runs_preserve_ime_underline_across_highlight_boundaries() {
        let underline = UnderlineStyle {
            thickness: px(1.),
            color: Some(black()),
            wavy: false,
        };

        let runs = [0..4, 4..10]
            .into_iter()
            .flat_map(|range| {
                split_run_for_ime_underline(
                    TextStyle::default().to_run(range.len()),
                    range,
                    Some(2..7),
                    Some(underline),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            runs.iter()
                .map(|run| (run.len, run.underline.is_some()))
                .collect::<Vec<_>>(),
            vec![(2, false), (2, true), (3, true), (3, false)]
        );
    }

    #[test]
    fn test_split_run_applies_ime_underline_without_highlighting() {
        let underline = UnderlineStyle {
            thickness: px(1.),
            color: Some(black()),
            wavy: false,
        };

        let runs = split_run_for_ime_underline(
            TextStyle::default().to_run(10),
            0..10,
            Some(2..7),
            Some(underline),
        );

        assert_eq!(
            runs.iter()
                .map(|run| (run.len, run.underline.is_some()))
                .collect::<Vec<_>>(),
            vec![(2, false), (5, true), (3, false)]
        );
        assert!(
            split_run_for_ime_underline(TextStyle::default().to_run(0), 0..0, None, None)
                .is_empty()
        );
    }

    #[test]
    fn test_masked_ime_underline_splits_on_mask_char_boundaries() {
        let underline = UnderlineStyle {
            thickness: px(1.),
            color: Some(black()),
            wavy: false,
        };
        let text = Rope::from("abcdef");
        let mask_len = MASK_CHAR.len_utf8();

        assert_eq!(
            ime_marked_display_range(&text, Some(4..6), false),
            Some(4..6)
        );
        assert_eq!(ime_marked_display_range(&text, None, true), None);
        assert_eq!(
            ime_marked_display_range(&text, Some(4..6), true),
            Some(4 * mask_len..6 * mask_len)
        );

        let display_text = MASK_CHAR.to_string().repeat(text.chars().count());
        let runs = split_run_for_ime_underline(
            TextStyle::default().to_run(display_text.len()),
            0..display_text.len(),
            ime_marked_display_range(&text, Some(4..6), true),
            Some(underline),
        );

        let mut offset = 0;
        for run in &runs {
            assert!(display_text.is_char_boundary(offset));
            offset += run.len;
        }
        assert_eq!(offset, display_text.len());
    }

    #[test]
    fn test_placeholder_line_runs() {
        let run = TextRun {
            len: 0,
            font: font(".SystemUIFont"),
            color: black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        let runs = vec![
            TextRun {
                len: 2,
                ..run.clone()
            },
            TextRun {
                len: 2,
                ..run.clone()
            },
            TextRun { len: 1, ..run },
        ];

        let placeholder_runs = placeholder_line_runs("ab\n\nc", &runs);

        let lines = placeholder_runs
            .iter()
            .map(|(line, _)| *line)
            .collect::<Vec<_>>();
        assert_eq!(lines, vec!["ab", "", "c"]);

        let run_lengths = placeholder_runs
            .iter()
            .map(|(_, line_runs)| line_runs.iter().map(|run| run.len).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        assert_eq!(run_lengths, vec![vec![2], vec![], vec![1]]);
    }

    #[test]
    fn test_split_runs_by_bg_segments() {
        let run = TextRun {
            len: 0,
            font: font(".SystemUIFont"),
            color: blue(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        let runs = vec![
            TextRun {
                len: 5,
                ..run.clone()
            },
            TextRun {
                len: 7,
                ..run.clone()
            },
            TextRun {
                len: 24,
                ..run.clone()
            },
        ];

        let bg_segments = vec![(8..12, red()), (12..18, blue())];
        let result = split_runs_by_bg_segments(5, &runs, &bg_segments);
        assert_eq!(
            result.iter().map(|run| run.len).collect::<Vec<_>>(),
            vec![3, 2, 2, 5, 1, 23]
        );
        assert_eq!(result[0].color, blue());
        assert_eq!(result[1].color, black());
        assert_eq!(result[2].color, black());
        assert_eq!(result[3].color, black());
        assert_eq!(result[4].color, black());
        assert_eq!(result[5].color, blue());
    }

    #[test]
    fn test_empty_bottom_height_outside_code_editor() {
        // 单行 / 纯文本 / 自动增长模式从不预留底部空白，无论是否覆盖。
        for override_rows in [None, Some(0), Some(3), Some(99)] {
            assert_eq!(
                empty_bottom_height(false, override_rows, px(800.), px(20.)),
                px(0.),
            );
        }
    }

    #[test]
    fn test_empty_bottom_height_code_editor_default() {
        // `None`：约半视口，下限 `BOTTOM_MARGIN_ROWS * line_height`，
        // 使小视口上的空白区域不会坍缩到"少于几行"。
        let line_height = px(20.);

        // 视口远高于下限 → 半视口生效。
        assert_eq!(
            empty_bottom_height(true, None, px(800.), line_height),
            px(400.),
        );

        // 视口短于 2 × 下限 → 下限生效。
        let floor = BOTTOM_MARGIN_ROWS * line_height;
        assert_eq!(empty_bottom_height(true, None, px(40.), line_height), floor);
    }

    #[test]
    fn test_empty_bottom_height_explicit_row_count() {
        // `Some(n)`：精确为 `n` 个行高。调用方完全控制尾部空白空间。
        let line_height = px(20.);

        for rows in [0_usize, 1, 3, 8, 64] {
            let expected = rows as f32 * line_height;
            assert_eq!(
                empty_bottom_height(true, Some(rows), px(800.), line_height),
                expected,
            );
            // 极小视口：仍精确为 `n × line_height`。
            assert_eq!(
                empty_bottom_height(true, Some(rows), px(20.), line_height),
                expected,
            );
        }
    }

    #[test]
    fn test_cursor_surrounding_padding_auto_grow() {
        // 自动增长输入始终按一行填充，无论是否覆盖或可见行数。
        let line_height = px(20.);
        for override_lines in [None, Some(0), Some(3), Some(99)] {
            for visible_lines in [0_usize, 1, 8, 64] {
                assert_eq!(
                    cursor_surrounding_padding(true, override_lines, visible_lines, line_height,),
                    line_height,
                );
            }
        }
    }

    #[test]
    fn test_cursor_surrounding_padding_default() {
        // `None`：历史启发式 —— 普通视口为 `BOTTOM_MARGIN_ROWS`，
        // 小视口（小于 `BOTTOM_MARGIN_ROWS × 8` 行高）回退到一行。
        let line_height = px(20.);

        // 小视口 → 一行回退。
        let small = BOTTOM_MARGIN_ROWS * 8 - 1;
        assert_eq!(
            cursor_surrounding_padding(false, None, small, line_height),
            line_height,
        );

        // `BOTTOM_MARGIN_ROWS × 8` 边界翻转为完整边距。
        let boundary = BOTTOM_MARGIN_ROWS * 8;
        assert_eq!(
            cursor_surrounding_padding(false, None, boundary, line_height),
            BOTTOM_MARGIN_ROWS * line_height,
        );

        // 足够大的视口。
        assert_eq!(
            cursor_surrounding_padding(false, None, 100, line_height),
            BOTTOM_MARGIN_ROWS * line_height,
        );
    }

    #[test]
    fn test_cursor_surrounding_padding_explicit() {
        // `Some(n)`：视口足够时精确为 `n × line_height`；
        // 不足时饱和到半视口。
        let line_height = px(20.);

        for lines in [0_usize, 1, 2, 5, 50] {
            let raw = lines as f32 * line_height;
            for visible_lines in [0_usize, 1, 8, 100] {
                let viewport_half = (visible_lines as f32 * line_height).half();
                assert_eq!(
                    cursor_surrounding_padding(false, Some(lines), visible_lines, line_height,),
                    raw.min(viewport_half),
                );
            }
        }
    }

    #[test]
    fn test_cursor_surrounding_padding_saturates_against_viewport() {
        // 小视口上的激进覆盖不得产生大于可见区域一半的填充——
        // 否则底部自动滚入阈值低于顶部阈值，逐帧滚动调整失去稳定不动点。
        let line_height = px(20.);

        // 覆盖远大于视口 → 钳制到一半。
        let visible_lines = 10;
        let viewport_half = (visible_lines as f32 * line_height).half();
        assert_eq!(
            cursor_surrounding_padding(false, Some(50), visible_lines, line_height),
            viewport_half,
        );

        // 覆盖在范围内 → 原样返回。
        let visible_lines = 40;
        assert_eq!(
            cursor_surrounding_padding(false, Some(3), visible_lines, line_height),
            3.0 * line_height,
        );
    }
}
