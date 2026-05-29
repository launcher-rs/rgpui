use rgpui::{Context, Point, Window};

use crate::input::{
    InputState, MoveDown, MoveEnd, MoveHome, MoveLeft, MovePageDown, MovePageUp, MoveRight,
    MoveToEnd, MoveToNextWord, MoveToPreviousWord, MoveToStart, MoveUp, RopeExt as _,
};

/// 移动方向
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoveDirection {
    /// 向上
    Up,
    /// 向下
    Down,
}

impl InputState {
    /// 移动光标后调用。如果知道光标当前位置，则更新首选列。
    pub(super) fn update_preferred_column(&mut self) {
        let Some(last_layout) = &self.last_layout else {
            self.preferred_column = None;
            return;
        };

        let point = self.text.offset_to_point(self.cursor());
        let Some(line) = last_layout.line(point.row) else {
            self.preferred_column = None;
            return;
        };

        let Some(pos) = line.position_for_index(point.column, last_layout, false) else {
            self.preferred_column = None;
            return;
        };

        self.preferred_column = Some((pos.x, point.column));
    }

    /// 将光标移动到给定的偏移量。
    ///
    /// 偏移量为 UTF-8 偏移量。
    ///
    /// 确保使用 self.next_boundary 或 self.previous_boundary 获取正确的偏移量。
    pub(crate) fn move_to(
        &mut self,
        offset: usize,
        direction: Option<MoveDirection>,
        cx: &mut Context<Self>,
    ) {
        let offset = offset.clamp(0, self.text.len());
        self.cursor_line_end_affinity = false;
        self.selected_range = (offset..offset).into();
        self.scroll_to(offset, direction, cx);
        self.pause_blink_cursor(cx);
        self.update_preferred_column();
        self.hide_context_menu(cx);
        cx.notify()
    }

    /// 垂直移动光标一行（向上或向下），同时尽可能保持列位置。
    ///
    /// move_lines: 垂直移动的行数（正数向下，负数向上）。
    pub(super) fn move_vertical(
        &mut self,
        move_lines: isize,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mode.is_single_line() {
            return;
        }
        let Some(last_layout) = &self.last_layout else {
            return;
        };

        let offset = self.cursor();
        let was_preferred_column = self.preferred_column;

        let mut display_point = self.display_map.offset_to_wrap_display_point(offset);

        // 将换行行转换为显示行（跳过折叠行），移动，然后再转换回来
        let current_display_row = self
            .display_map
            .wrap_row_to_display_row(display_point.row)
            .unwrap_or_else(|| {
                self.display_map
                    .nearest_visible_display_row(display_point.row)
            });
        let max_display_row = self.display_map.display_row_count().saturating_sub(1);
        let target_display_row = current_display_row
            .saturating_add_signed(move_lines)
            .min(max_display_row);
        let target_wrap_row = self
            .display_map
            .display_row_to_wrap_row(target_display_row)
            .unwrap_or(display_point.row);

        display_point.row = target_wrap_row;
        display_point.column = 0;
        let mut new_offset = self.display_map.wrap_display_point_to_offset(display_point);

        if let Some((preferred_x, column)) = was_preferred_column {
            // 再次获取显示点以更新 local_row。
            let mut next_display_point = self.display_map.offset_to_wrap_display_point(new_offset);
            next_display_point.column = 0;
            let next_point = self
                .display_map
                .wrap_display_point_to_point(next_display_point);
            let line_start_offset = self.text.line_start_offset(next_point.row);

            // 如果在可见范围内，优先使用位置获取列。
            if let Some(line) = last_layout.line(next_point.row) {
                if let Some(x) = line.closest_index_for_position(
                    Point {
                        x: preferred_x,
                        y: next_display_point.local_row * last_layout.line_height,
                    },
                    last_layout,
                ) {
                    new_offset = line_start_offset + x;
                }
            } else {
                // 不在可见范围内，直接使用列。
                let max_line_len = self.text.slice_line(next_point.row).len();
                new_offset = line_start_offset + column.min(max_line_len);
            }
        }

        self.pause_blink_cursor(cx);
        let direction = if move_lines < 0 {
            MoveDirection::Up
        } else {
            MoveDirection::Down
        };
        self.move_to(new_offset, Some(direction), cx);
        // 恢复首选列
        self.preferred_column = was_preferred_column;
        cx.notify();
    }

    /// 处理向左移动
    pub(super) fn left(&mut self, _: &MoveLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.pause_blink_cursor(cx);
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor()), None, cx);
        } else {
            self.move_to(self.selected_range.start, None, cx)
        }
    }

    /// 处理向右移动
    pub(super) fn right(&mut self, _: &MoveRight, _: &mut Window, cx: &mut Context<Self>) {
        self.pause_blink_cursor(cx);
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), None, cx);
        } else {
            self.move_to(self.selected_range.end, None, cx)
        }
    }

    /// 处理向上移动
    pub(super) fn up(&mut self, action: &MoveUp, window: &mut Window, cx: &mut Context<Self>) {
        if self.handle_action_for_context_menu(Box::new(action.clone()), window, cx) {
            return;
        }

        if self.mode.is_single_line() {
            return;
        }

        if !self.selected_range.is_empty() {
            self.move_to(
                self.previous_boundary(self.selected_range.start.saturating_sub(1)),
                Some(MoveDirection::Up),
                cx,
            );
        }
        self.pause_blink_cursor(cx);
        self.move_vertical(-1, window, cx);
    }

    /// 处理向下移动
    pub(super) fn down(&mut self, action: &MoveDown, window: &mut Window, cx: &mut Context<Self>) {
        if self.handle_action_for_context_menu(Box::new(action.clone()), window, cx) {
            return;
        }

        if self.mode.is_single_line() {
            return;
        }

        if !self.selected_range.is_empty() {
            self.move_to(
                self.next_boundary(self.selected_range.end.saturating_sub(1)),
                Some(MoveDirection::Down),
                cx,
            );
        }

        self.pause_blink_cursor(cx);
        self.move_vertical(1, window, cx);
    }

    /// 处理向上翻页
    pub(super) fn page_up(&mut self, _: &MovePageUp, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode.is_single_line() {
            return;
        }

        let Some(last_layout) = &self.last_layout else {
            return;
        };

        let display_lines = (self.input_bounds.size.height / last_layout.line_height) as isize;
        self.move_vertical(-display_lines, window, cx);
    }

    /// 处理向下翻页
    pub(super) fn page_down(
        &mut self,
        _: &MovePageDown,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mode.is_single_line() {
            return;
        }

        let Some(last_layout) = &self.last_layout else {
            return;
        };

        let display_lines = (self.input_bounds.size.height / last_layout.line_height) as isize;
        self.move_vertical(display_lines, window, cx);
    }

    /// 处理移动到行首
    pub(super) fn home(&mut self, _: &MoveHome, _: &mut Window, cx: &mut Context<Self>) {
        self.pause_blink_cursor(cx);
        let offset = self.start_of_line();
        self.move_to(offset, Some(MoveDirection::Up), cx);
    }

    /// 处理移动到行尾
    pub(super) fn end(&mut self, _: &MoveEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.pause_blink_cursor(cx);
        let offset = self.end_of_line();
        self.move_to(offset, Some(MoveDirection::Down), cx);
        self.cursor_line_end_affinity = true;
    }

    /// 处理移动到文本开头
    pub(super) fn move_to_start(
        &mut self,
        _: &MoveToStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to(0, None, cx);
    }

    /// 处理移动到文本末尾
    pub(super) fn move_to_end(&mut self, _: &MoveToEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.text.len(), None, cx);
    }

    /// 处理移动到上一个单词
    pub(super) fn move_to_previous_word(
        &mut self,
        _: &MoveToPreviousWord,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let offset = self.previous_start_of_word();
        self.move_to(offset, None, cx);
    }

    /// 处理移动到下一个单词
    pub(super) fn move_to_next_word(
        &mut self,
        _: &MoveToNextWord,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let offset = self.next_end_of_word();
        self.move_to(offset, None, cx);
    }
}
