//! 编辑器覆盖层渲染：折叠图标、多光标（`editor` feature 门控）。
//!
//! 从 `element.rs` 拆出，保持核心文本渲染与编辑器装饰分离。
//! 无 feature 时这些代码不编译，调用方（`element.rs`）对应调用点同步门控。

use crate::InteractiveElement;
use crate::styled_ext::Selectable;
use crate::{
    App, Bounds, Button, ButtonVariants as _, Entity, Half, Hitbox, HitboxBehavior, IconName,
    IntoElement, MouseButton, Pixels, Point, Sizable as _, Styled as _, TextAlign, Window, point,
    px, size,
};

use super::LastLayout;
use super::blink_cursor::CURSOR_WIDTH;
use super::element::{FOLD_ICON_HITBOX_WIDTH, LINE_NUMBER_RIGHT_MARGIN};
use super::rope_ext::RopeExt as _;
use super::state::InputState;

/// 折叠图标本体宽度（布局与绘制共用）。
const FOLD_ICON_WIDTH: Pixels = px(14.);

/// 折叠图标布局信息。
pub(super) struct FoldIconLayout {
    /// 行号区域命中框（用于悬停检测）
    line_number_hitbox: Hitbox,
    /// 每个折叠候选的 (display_row, is_folded, icon_element) 列表
    icons: Vec<(usize, bool, crate::AnyElement)>,
}

/// 布局折叠图标（第一遍收集候选，第二遍创建并预绘制图标）。
pub(super) fn layout_fold_icons(
    state: &Entity<InputState>,
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
        let state = state.read(cx);
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
                let state = state.clone();
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

/// 绘制折叠图标（仅悬停或当前行可见）。
pub(super) fn paint_fold_icons(
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

/// 计算额外光标边界（多光标，与主光标同尺寸同滚动，不参与滚动驱动）。
pub(super) fn extra_cursor_bounds(
    state: &InputState,
    caret_for: &dyn Fn(usize, usize, bool) -> Point<Pixels>,
    bounds: &Bounds<Pixels>,
    line_number_width: Pixels,
    cursor_scroll_x: Pixels,
    line_height: Pixels,
    cursor_height: Pixels,
    text_align: TextAlign,
) -> Vec<Bounds<Pixels>> {
    let mut extra_cursor_bounds = Vec::new();
    if !state.masked {
        for extra in &state.extra_selections {
            let end = extra.end.min(state.core.text.len());
            let row = state.core.text.offset_to_point(end).row;
            let pos = caret_for(row, end, false);
            let x = bounds.left() + pos.x + line_number_width + cursor_scroll_x;
            let x = if text_align == TextAlign::Right {
                x.min(bounds.right() - CURSOR_WIDTH)
            } else {
                x
            };
            extra_cursor_bounds.push(Bounds::new(
                point(
                    x,
                    bounds.top() + pos.y + ((line_height - cursor_height) / 2.),
                ),
                size(CURSOR_WIDTH, cursor_height),
            ));
        }
    }
    extra_cursor_bounds
}
