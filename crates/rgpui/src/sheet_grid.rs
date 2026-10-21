//! 电子表格网格 —— 行列头 + 虚拟化单元格 + 矩形选区 + 填充柄。
//!
//! 面向电子表格/日历/甘特等「二维单元格」场景的专用网格控件：以行列坐标定位
//! 单元格，支持**行列双轴虚拟化**（海量行列只渲染可见范围）、矩形选区（鼠标拖拽 +
//! 键盘扩展）、行列头点击整行/整列选择、填充柄拖拽，并把选择/激活/编辑请求以事件
//! 形式抛给应用。框架**只做控件与交互**，单元格数据与公式计算留在应用层（见
//! [`SheetDelegate`]）。
//!
//! 与 [`DataTable`](crate::DataTable) 的区别：DataTable 面向「列定义 + 数据行」，
//! 本控件面向「左上角原点 + A/B/C 列 + 1/2/3 行」的电子表格语义，并提供矩形选区与填充柄。
//!
//! ## 用法
//!
//! ```ignore
//! use rgpui::sheet_grid::{SheetGrid, SheetGridEvent, SheetGridState, CellRef, SheetDelegate};
//!
//! struct MySheet { cells: Vec<Vec<String>> }
//!
//! impl SheetDelegate for MySheet {
//!     fn rows_count(&self, _: &App) -> usize { self.cells.len() }
//!     fn cols_count(&self, _: &App) -> usize { self.cells.first().map_or(0, Vec::len) }
//!     fn text(&self, cell: CellRef, _: &App) -> SharedString {
//!         self.cells[cell.row][cell.col].clone().into()
//!     }
//! }
//!
//! let grid = cx.new(|cx| SheetGridState::new(MySheet { .. }, cx));
//! cx.subscribe_in(&grid, window, |this, _, event: &SheetGridEvent, window, cx| { .. }).detach();
//!
//! // 视图树中：
//! SheetGrid::new(&grid)
//! ```

use std::{ops::Range, rc::Rc};

use crate::{
    ActiveTheme as _, AnyElement, App, Axis, Bounds, Context, Div, ElementExt as _, Entity,
    EventEmitter, FocusHandle, Focusable, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseUpEvent, ParentElement as _, Pixels, Render, RenderOnce,
    ScrollStrategy, ScrollableMask, Scrollbar, SharedString, Size, Stateful,
    StatefulInteractiveElement as _, Styled as _, UniformListScrollHandle, VirtualListScrollHandle,
    Window, div, h_flex, h_virtual_list, prelude::FluentBuilder, px, size, uniform_list, v_flex,
};

/// 行头列宽（像素）。
pub const ROW_HEADER_WIDTH: Pixels = px(44.0);
/// 列头行高（像素）。
pub const COL_HEADER_HEIGHT: Pixels = px(24.0);
/// 默认列宽（像素）。
pub const DEFAULT_COLUMN_WIDTH: Pixels = px(96.0);
/// 默认行高（像素）。
pub const DEFAULT_ROW_HEIGHT: Pixels = px(24.0);

/// 单元格坐标（0 起的行、列）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellRef {
    /// 行号（0 起）。
    pub row: usize,
    /// 列号（0 起）。
    pub col: usize,
}

impl CellRef {
    /// 创建单元格坐标。
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

impl From<(usize, usize)> for CellRef {
    fn from((row, col): (usize, usize)) -> Self {
        Self { row, col }
    }
}

/// 矩形选区：`anchor` 为起点、`cursor` 为活动端（可正可逆）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellRange {
    /// 选区起点（拖拽/键盘扩展时的固定端）。
    pub anchor: CellRef,
    /// 选区活动端（光标所在单元格）。
    pub cursor: CellRef,
}

impl CellRange {
    /// 单格选区。
    pub fn single(cell: CellRef) -> Self {
        Self {
            anchor: cell,
            cursor: cell,
        }
    }

    /// 以 `anchor` 为起点、`cursor` 为活动端构造选区。
    pub fn new(anchor: CellRef, cursor: CellRef) -> Self {
        Self { anchor, cursor }
    }

    /// 左上角坐标（归一化后的较小值）。
    pub fn min(&self) -> CellRef {
        CellRef::new(
            self.anchor.row.min(self.cursor.row),
            self.anchor.col.min(self.cursor.col),
        )
    }

    /// 右下角坐标（归一化后的较大值）。
    pub fn max(&self) -> CellRef {
        CellRef::new(
            self.anchor.row.max(self.cursor.row),
            self.anchor.col.max(self.cursor.col),
        )
    }

    /// 归一化后的行范围。
    pub fn rows(&self) -> Range<usize> {
        let (min, max) = (self.min(), self.max());
        min.row..max.row + 1
    }

    /// 归一化后的列范围。
    pub fn cols(&self) -> Range<usize> {
        let (min, max) = (self.min(), self.max());
        min.col..max.col + 1
    }

    /// 选区包含的行数。
    pub fn height(&self) -> usize {
        let (min, max) = (self.min(), self.max());
        max.row - min.row + 1
    }

    /// 选区包含的列数。
    pub fn width(&self) -> usize {
        let (min, max) = (self.min(), self.max());
        max.col - min.col + 1
    }

    /// 选区包含的单元格数。
    pub fn len(&self) -> usize {
        self.height() * self.width()
    }

    /// 选区是否为空（单格或退化）。恒为 `false`，仅为满足 lint；保留以表达语义。
    pub fn is_empty(&self) -> bool {
        false
    }

    /// 是否包含给定单元格。
    pub fn contains(&self, cell: CellRef) -> bool {
        let (min, max) = (self.min(), self.max());
        (min.row..=max.row).contains(&cell.row) && (min.col..=max.col).contains(&cell.col)
    }

    /// 将选区夹取到 `[0, rows) × [0, cols)` 范围内（行列至少为 1）。
    pub fn clamp(&self, rows: usize, cols: usize) -> Self {
        let last_row = rows.saturating_sub(1);
        let last_col = cols.saturating_sub(1);
        let clamp_cell = |c: CellRef| CellRef::new(c.row.min(last_row), c.col.min(last_col));
        Self {
            anchor: clamp_cell(self.anchor),
            cursor: clamp_cell(self.cursor),
        }
    }
}

impl Default for CellRange {
    fn default() -> Self {
        Self::single(CellRef::new(0, 0))
    }
}

/// 将列号（0 起）转为列标签：`0 -> A`、`25 -> Z`、`26 -> AA`。
pub fn column_label(col: usize) -> String {
    let mut col = col;
    let mut label = String::new();
    loop {
        label.insert(0, (b'A' + (col % 26) as u8) as char);
        if col < 26 {
            break;
        }
        col = col / 26 - 1;
    }
    label
}

/// 网格抛出的事件（应用经 `subscribe` 接收）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SheetGridEvent {
    /// 选区变化（含活动单元格）。值已归一化前的原始选区，可用 [`CellRange::min`] 归一化。
    SelectionChanged(CellRange),
    /// 请求编辑某单元格（双击 / `Enter` / `F2` / 直接键入）。
    EditRequested(CellRef),
    /// 请求清空选区（`Delete` / `Backspace`）。
    Clear(CellRange),
    /// 填充柄拖拽结束：以 `source` 为源、覆盖到 `target`。
    Fill {
        /// 源选区。
        source: CellRange,
        /// 目标选区（含源）。
        target: CellRange,
    },
    /// 点击列头（`col >= 0`）或行头（`row >= 0`，`col` 无意义）。
    HeaderClicked {
        /// 整列选择时的列号；行头点击为 `None`。
        col: Option<usize>,
        /// 整行选择时的行号；列头点击为 `None`。
        row: Option<usize>,
    },
}

/// 为电子表格网格提供数据与单元格渲染的代理 trait。
pub trait SheetDelegate: Sized + 'static {
    /// 行数。
    fn rows_count(&self, cx: &App) -> usize;

    /// 列数。
    fn cols_count(&self, cx: &App) -> usize;

    /// 列头标签，默认 `A`、`B`、…。
    fn column_label(&self, col: usize, _cx: &App) -> SharedString {
        column_label(col).into()
    }

    /// 行头标签，默认 `1`、`2`、…。
    fn row_label(&self, row: usize, _cx: &App) -> SharedString {
        (row + 1).to_string().into()
    }

    /// 列宽，默认 [`DEFAULT_COLUMN_WIDTH`]。
    fn column_width(&self, _col: usize, _cx: &App) -> Pixels {
        DEFAULT_COLUMN_WIDTH
    }

    /// 统一行高，默认 [`DEFAULT_ROW_HEIGHT`]。
    fn row_height(&self, _cx: &App) -> Pixels {
        DEFAULT_ROW_HEIGHT
    }

    /// 单元格文本（供默认渲染与公式栏同步使用）。
    fn text(&self, _cell: CellRef, _cx: &App) -> SharedString {
        SharedString::default()
    }

    /// 渲染单元格内容（默认渲染 [`Self::text`]）。
    fn render_cell(
        &mut self,
        cell: CellRef,
        _selected: bool,
        _window: &mut Window,
        cx: &mut Context<SheetGridState<Self>>,
    ) -> impl IntoElement {
        div().w_full().child(self.text(cell, cx))
    }
}

/// 网格状态：持有委托、选区、滚动与焦点。
pub struct SheetGridState<D: SheetDelegate> {
    /// 数据/渲染委托。
    delegate: D,
    /// 当前选区。
    selection: CellRange,
    /// 垂直滚动（行虚拟化）。
    vertical_scroll: UniformListScrollHandle,
    /// 水平滚动（列虚拟化）。
    horizontal_scroll: VirtualListScrollHandle,
    /// 焦点句柄。
    focus_handle: FocusHandle,
    /// 根元素边界（预绘制捕获，用于翻页步长）。
    bounds: Bounds<Pixels>,
    /// 是否正在拖拽扩展选区。
    dragging: bool,
    /// 是否正在拖拽填充柄。
    filling: bool,
    /// 填充柄当前拖到的单元格。
    fill_target: Option<CellRef>,
    /// 正在编辑的单元格（应用侧编辑时置位，用于抑制重复请求）。
    editing: Option<CellRef>,
    /// 最近一次渲染时缓存的行数。
    rows: usize,
    /// 最近一次渲染时缓存的列数。
    cols: usize,
}

impl<D: SheetDelegate> SheetGridState<D> {
    /// 创建网格状态。
    pub fn new(delegate: D, cx: &mut Context<Self>) -> Self {
        Self {
            delegate,
            selection: CellRange::default(),
            vertical_scroll: UniformListScrollHandle::new(),
            horizontal_scroll: VirtualListScrollHandle::new(),
            focus_handle: cx.focus_handle(),
            bounds: Bounds::default(),
            dragging: false,
            filling: false,
            fill_target: None,
            editing: None,
            rows: 0,
            cols: 0,
        }
    }

    /// 数据委托。
    pub fn delegate(&self) -> &D {
        &self.delegate
    }

    /// 数据委托（可变）。
    pub fn delegate_mut(&mut self) -> &mut D {
        &mut self.delegate
    }

    /// 当前选区。
    pub fn selection(&self) -> CellRange {
        self.selection
    }

    /// 活动单元格（选区活动端）。
    pub fn active_cell(&self) -> CellRef {
        self.selection.cursor
    }

    /// 正在编辑的单元格。
    pub fn editing(&self) -> Option<CellRef> {
        self.editing
    }

    /// 设置正在编辑的单元格。
    pub fn set_editing(&mut self, cell: Option<CellRef>, cx: &mut Context<Self>) {
        if self.editing != cell {
            self.editing = cell;
            cx.notify();
        }
    }

    /// 行数（最近一次渲染缓存）。
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// 列数（最近一次渲染缓存）。
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// 设置选区（自动夹取到数据范围并发出事件）。
    pub fn set_selection(&mut self, selection: CellRange, cx: &mut Context<Self>) {
        let selection = selection.clamp(self.rows.max(1), self.cols.max(1));
        if self.selection != selection {
            self.selection = selection;
            cx.emit(SheetGridEvent::SelectionChanged(selection));
            cx.notify();
        }
    }

    /// 选中单个单元格并使其可见。
    pub fn set_active_cell(&mut self, cell: CellRef, cx: &mut Context<Self>) {
        self.set_selection(CellRange::single(cell), cx);
        self.scroll_to_cell(cell, cx);
    }

    /// 选中整行。
    pub fn select_row(&mut self, row: usize, cx: &mut Context<Self>) {
        let row = row.min(self.rows.saturating_sub(1));
        self.set_selection(
            CellRange::new(
                CellRef::new(row, 0),
                CellRef::new(row, self.cols.saturating_sub(1)),
            ),
            cx,
        );
        cx.emit(SheetGridEvent::HeaderClicked {
            col: None,
            row: Some(row),
        });
    }

    /// 选中整列。
    pub fn select_col(&mut self, col: usize, cx: &mut Context<Self>) {
        let col = col.min(self.cols.saturating_sub(1));
        self.set_selection(
            CellRange::new(
                CellRef::new(0, col),
                CellRef::new(self.rows.saturating_sub(1), col),
            ),
            cx,
        );
        cx.emit(SheetGridEvent::HeaderClicked {
            row: None,
            col: Some(col),
        });
    }

    /// 全选。
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        if self.rows == 0 || self.cols == 0 {
            return;
        }
        self.set_selection(
            CellRange::new(
                CellRef::new(0, 0),
                CellRef::new(self.rows - 1, self.cols - 1),
            ),
            cx,
        );
    }

    /// 以 `(drow, dcol)` 移动活动单元格；`extend` 为真时从起点扩展选区。
    pub fn move_active(&mut self, drow: i64, dcol: i64, extend: bool, cx: &mut Context<Self>) {
        if self.rows == 0 || self.cols == 0 {
            return;
        }
        let cursor = self.selection.cursor;
        let row = (cursor.row as i64 + drow).clamp(0, self.rows as i64 - 1) as usize;
        let col = (cursor.col as i64 + dcol).clamp(0, self.cols as i64 - 1) as usize;
        let next = CellRef::new(row, col);
        let selection = if extend {
            CellRange::new(self.selection.anchor, next)
        } else {
            CellRange::single(next)
        };
        self.set_selection(selection, cx);
        self.scroll_to_cell(next, cx);
    }

    /// 聚焦网格。
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
    }

    /// 滚动到指定单元格使其可见。
    pub fn scroll_to_cell(&mut self, cell: CellRef, cx: &mut Context<Self>) {
        self.vertical_scroll
            .scroll_to_item(cell.row, ScrollStrategy::Nearest);

        // 水平方向：按列宽累计计算目标位置，手动夹取偏移。
        let viewport_width = self.horizontal_scroll.bounds().size.width;
        if viewport_width > px(0.0) {
            let mut col_x = px(0.0);
            for col in 0..cell.col {
                col_x += self.delegate.column_width(col, cx);
            }
            let col_w = self.delegate.column_width(cell.col, cx);
            let mut offset = self.horizontal_scroll.offset();
            let left = col_x + offset.x;
            let right = col_x + col_w + offset.x;
            if left < px(0.0) {
                offset.x = -col_x;
            } else if right > viewport_width {
                offset.x = viewport_width - col_x - col_w;
            }
            self.horizontal_scroll.set_offset(offset);
        }
        cx.notify();
    }

    /// 重新读取委托的行列数并夹取选区（应用修改委托数据后调用，渲染每帧亦会同步）。
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.rows = self.delegate.rows_count(cx);
        self.cols = self.delegate.cols_count(cx);
        if self.rows > 0 && self.cols > 0 {
            let clamped = self.selection.clamp(self.rows, self.cols);
            if clamped != self.selection {
                self.selection = clamped;
            }
        }
    }

    /// 单元格鼠标按下：选中 / 双击请求编辑。
    fn on_cell_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        cell: CellRef,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus(window, cx);
        if event.click_count >= 2 {
            self.set_selection(CellRange::single(cell), cx);
            self.editing = Some(cell);
            cx.emit(SheetGridEvent::EditRequested(cell));
            return;
        }
        self.dragging = true;
        if event.modifiers.shift {
            self.set_selection(CellRange::new(self.selection.anchor, cell), cx);
        } else {
            self.set_selection(CellRange::single(cell), cx);
        }
    }

    /// 单元格鼠标移动：拖拽扩展选区 / 填充柄。
    fn on_cell_mouse_move(&mut self, cell: CellRef, cx: &mut Context<Self>) {
        if self.filling {
            if self.fill_target != Some(cell) {
                self.fill_target = Some(cell);
                cx.notify();
            }
        } else if self.dragging {
            self.set_selection(CellRange::new(self.selection.anchor, cell), cx);
        }
    }

    /// 填充柄按下：进入填充拖拽。
    fn on_fill_handle_down(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus(window, cx);
        self.filling = true;
        self.fill_target = Some(self.selection.cursor);
        cx.notify();
    }

    /// 鼠标释放：结束拖拽 / 填充。
    fn on_mouse_release(&mut self, cx: &mut Context<Self>) {
        if self.filling {
            if let Some(target) = self.fill_target.take() {
                let source = self.selection;
                let target =
                    CellRange::new(source.cursor, target).clamp(self.rows.max(1), self.cols.max(1));
                if target != source {
                    cx.emit(SheetGridEvent::Fill {
                        source,
                        target: CellRange::new(source.min(), target.max()),
                    });
                }
            }
        }
        self.filling = false;
        self.dragging = false;
        cx.notify();
    }

    /// 键盘交互：方向键 / Shift 扩展 / Tab / Home / End / PageUp / PageDown / Ctrl+A /
    /// Enter·F2（请求编辑）/ Delete·Backspace（请求清空）。
    fn on_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if self.rows == 0 || self.cols == 0 {
            return;
        }
        let key = event.keystroke.key.to_ascii_lowercase();
        let modifiers = event.keystroke.modifiers;
        let extend = modifiers.shift;
        let command = modifiers.control || modifiers.platform;
        let page_rows = (self.visible_body_height() / self.delegate.row_height(cx))
            .max(1.0)
            .floor() as i64;

        match key.as_str() {
            "up" => self.move_active(-1, 0, extend, cx),
            "down" => self.move_active(1, 0, extend, cx),
            "left" => self.move_active(0, -1, extend, cx),
            "right" => self.move_active(0, 1, extend, cx),
            "pageup" => self.move_active(-page_rows, 0, extend, cx),
            "pagedown" => self.move_active(page_rows, 0, extend, cx),
            "tab" => self.move_active(0, if modifiers.shift { -1 } else { 1 }, false, cx),
            "home" => {
                if command {
                    self.set_active_cell(CellRef::new(0, 0), cx);
                } else {
                    let cursor = self.selection.cursor;
                    self.set_active_cell(CellRef::new(cursor.row, 0), cx);
                }
            }
            "end" => {
                if command {
                    self.set_active_cell(CellRef::new(self.rows - 1, self.cols - 1), cx);
                } else {
                    let cursor = self.selection.cursor;
                    self.set_active_cell(CellRef::new(cursor.row, self.cols - 1), cx);
                }
            }
            "a" if command => self.select_all(cx),
            "enter" | "f2" => {
                let cursor = self.selection.cursor;
                self.editing = Some(cursor);
                cx.emit(SheetGridEvent::EditRequested(cursor));
                cx.notify();
            }
            "delete" | "backspace" => {
                if !self.editing.is_some() {
                    cx.emit(SheetGridEvent::Clear(self.selection));
                }
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    /// 内容区可视高度（扣掉列头）。
    fn visible_body_height(&self) -> Pixels {
        (self.bounds.size.height - COL_HEADER_HEIGHT).max(px(0.0))
    }

    /// 渲染列头行。
    fn render_header(
        &mut self,
        col_sizes: Rc<Vec<Size<Pixels>>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let border = theme.border;
        let head_bg = theme.table_head;
        let head_fg = theme.table_head_foreground;
        let horizontal = self.horizontal_scroll.clone();

        let columns = h_virtual_list(cx.entity(), "sheet-col-header", col_sizes, {
            move |state: &mut Self, range: Range<usize>, _window, cx| {
                range
                    .map(|col| {
                        let label = state.delegate.column_label(col, cx);
                        div()
                            .id(format!("sheet-col-head:{col}"))
                            // 无障碍语义：列头。
                            .role(accesskit::Role::ColumnHeader)
                            .aria_column_index(col)
                            .w_full()
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_r_1()
                            .border_b_1()
                            .border_color(border)
                            .text_sm()
                            .child(label)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |state, _event, window, cx| {
                                    state.focus(window, cx);
                                    state.select_col(col, cx);
                                }),
                            )
                            .into_any_element()
                    })
                    .collect::<Vec<AnyElement>>()
            }
        })
        .track_scroll(&horizontal)
        .flex_1();

        h_flex()
            .h(COL_HEADER_HEIGHT)
            .flex_shrink_0()
            .bg(head_bg)
            .text_color(head_fg)
            .child(
                div()
                    .w(ROW_HEADER_WIDTH)
                    .h_full()
                    .flex_shrink_0()
                    .border_r_1()
                    .border_b_1()
                    .border_color(border),
            )
            .child(columns)
            .into_any_element()
    }

    /// 渲染单行：行头 + 虚拟化单元格。
    fn render_row(
        &mut self,
        row: usize,
        row_height: Pixels,
        col_sizes: Rc<Vec<Size<Pixels>>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = cx.theme();
        let border = theme.border;
        let head_bg = theme.table_head;
        let head_fg = theme.table_head_foreground;
        let horizontal = self.horizontal_scroll.clone();
        let row_label = self.delegate.row_label(row, cx);

        let cells = h_virtual_list(cx.entity(), ("sheet-row", row), col_sizes, {
            move |state: &mut Self, range: Range<usize>, window, cx| {
                range
                    .map(|col| state.render_cell(CellRef::new(row, col), border, window, cx))
                    .collect::<Vec<AnyElement>>()
            }
        })
        .with_scroll_handle(&horizontal)
        .flex_1();

        div()
            .id(("sheet-row", row))
            .flex()
            .flex_row()
            .h(row_height)
            .flex_shrink_0()
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .id(("sheet-row-head", row))
                    // 无障碍语义：行头。
                    .role(accesskit::Role::RowHeader)
                    .aria_row_index(row)
                    .w(ROW_HEADER_WIDTH)
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_r_1()
                    .border_color(border)
                    .bg(head_bg)
                    .text_color(head_fg)
                    .text_sm()
                    .child(row_label)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |state, _event, window, cx| {
                            state.focus(window, cx);
                            state.select_row(row, cx);
                        }),
                    ),
            )
            .child(cells)
    }

    /// 渲染单个单元格（含选区高亮、活动边框与填充柄）。
    fn render_cell(
        &mut self,
        cell: CellRef,
        border: Hsla,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let selection = self.selection;
        let selected = selection.contains(cell);
        let is_active = selection.cursor == cell;
        let is_fill_corner = selection.max() == cell && !is_active;
        let active_bg = theme.table_active;
        let active_border = theme.table_active_border;
        let hover_bg = theme.table_hover;
        let surface = theme.background;
        let content = self.delegate.render_cell(cell, selected, window, cx);

        div()
            .id(format!("sheet-cell:{}:{}", cell.row, cell.col))
            // 无障碍语义：单元格角色 + 行列索引。
            .role(accesskit::Role::Cell)
            .aria_row_index(cell.row)
            .aria_column_index(cell.col)
            .relative()
            .w_full()
            .h_full()
            .flex()
            .items_center()
            .px(px(4.0))
            .overflow_hidden()
            .border_r_1()
            .border_color(border)
            .hover(move |this| this.bg(hover_bg))
            .when(selected, |this| this.bg(active_bg))
            .child(content)
            .when(is_active, |this| {
                this.child(
                    div()
                        .absolute()
                        .inset_0()
                        .border_2()
                        .border_color(active_border),
                )
            })
            .when(is_fill_corner, |this| {
                this.child(
                    div()
                        .id(format!("sheet-fill:{}:{}", cell.row, cell.col))
                        .absolute()
                        .right(px(-3.0))
                        .bottom(px(-3.0))
                        .size(px(6.0))
                        .bg(active_border)
                        .border_1()
                        .border_color(surface)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |state, _event, window, cx| {
                                state.on_fill_handle_down(window, cx);
                            }),
                        ),
                )
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |state, event, window, cx| {
                    state.on_cell_mouse_down(event, cell, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |state, _event, _window, cx| {
                state.on_cell_mouse_move(cell, cx);
            }))
            .into_any_element()
    }
}

impl<D: SheetDelegate> EventEmitter<SheetGridEvent> for SheetGridState<D> {}

impl<D: SheetDelegate> Focusable for SheetGridState<D> {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl<D: SheetDelegate> Render for SheetGridState<D> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh(cx);

        let rows = self.rows;
        let cols = self.cols;
        let row_height = self.delegate.row_height(cx);
        let col_sizes: Rc<Vec<Size<Pixels>>> = Rc::new(
            (0..cols)
                .map(|col| size(self.delegate.column_width(col, cx), row_height))
                .collect(),
        );

        let header = self.render_header(col_sizes.clone(), window, cx);
        let vertical = self.vertical_scroll.clone();

        let body = uniform_list(
            "sheet-rows",
            rows,
            cx.processor(
                move |state: &mut Self, visible_range: Range<usize>, window, cx| {
                    visible_range
                        .map(|row| state.render_row(row, row_height, col_sizes.clone(), window, cx))
                        .collect::<Vec<_>>()
                },
            ),
        )
        .flex_grow_1()
        .track_scroll(&vertical);

        div()
            .size_full()
            .relative()
            .overflow_hidden()
            .on_prepaint({
                let state = cx.entity();
                move |bounds, _window, cx| state.update(cx, |state, _| state.bounds = bounds)
            })
            .child(v_flex().size_full().child(header).child(body))
            .child(
                div()
                    .absolute()
                    .top(COL_HEADER_HEIGHT)
                    .right_0()
                    .bottom_0()
                    .w(Scrollbar::width())
                    .child(Scrollbar::vertical(&self.vertical_scroll).max_fps(60)),
            )
            .child(
                div()
                    .absolute()
                    .left(ROW_HEADER_WIDTH)
                    .right_0()
                    .bottom_0()
                    .h(Scrollbar::width())
                    .child(Scrollbar::horizontal(&self.horizontal_scroll)),
            )
            .child(ScrollableMask::new(
                Axis::Horizontal,
                &self.horizontal_scroll,
            ))
            .child(ScrollableMask::new(
                Axis::Vertical,
                &self.vertical_scroll.0.borrow().base_handle,
            ))
    }
}

/// 电子表格网格元素（薄包装：按键上下文、焦点与键盘处理）。
#[derive(IntoElement)]
pub struct SheetGrid<D: SheetDelegate> {
    state: Entity<SheetGridState<D>>,
}

impl<D: SheetDelegate> SheetGrid<D> {
    /// 使用给定 [`SheetGridState`] 创建网格元素。
    pub fn new(state: &Entity<SheetGridState<D>>) -> Self {
        Self {
            state: state.clone(),
        }
    }
}

impl<D: SheetDelegate> RenderOnce for SheetGrid<D> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus_handle = self.state.read(cx).focus_handle(cx);
        let entity_id = self.state.entity_id();

        div()
            .id(("sheet-grid", entity_id))
            // 无障碍语义：网格根节点。
            .role(accesskit::Role::Grid)
            .key_context("SheetGrid")
            .track_focus(&focus_handle)
            .size_full()
            .on_key_down(window.listener_for(
                &self.state,
                |state, event: &KeyDownEvent, _window, cx| {
                    state.on_key_down(event, cx);
                },
            ))
            .capture_any_mouse_up(window.listener_for(
                &self.state,
                |state, _event: &MouseUpEvent, _window, cx| {
                    state.on_mouse_release(cx);
                },
            ))
            .child(self.state)
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{
        CellRange, CellRef, SheetDelegate, SheetGrid, SheetGridEvent, SheetGridState, column_label,
    };
    use crate::AppContext as _;
    use crate::{
        Context, Entity, IntoElement, ParentElement as _, Render, SharedString, Window, div,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    /// 3 行 2 列的测试委托。
    struct TestSheet;

    impl SheetDelegate for TestSheet {
        fn rows_count(&self, _cx: &crate::App) -> usize {
            3
        }

        fn cols_count(&self, _cx: &crate::App) -> usize {
            2
        }

        fn text(&self, cell: CellRef, _cx: &crate::App) -> SharedString {
            format!("{},{}", cell.row, cell.col).into()
        }
    }

    /// 渲染网格的测试宿主。
    struct Probe {
        state: Entity<SheetGridState<TestSheet>>,
    }

    impl Render for Probe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(SheetGrid::new(&self.state))
        }
    }

    /// 构建宿主、同步行列数并收集网格事件。
    fn setup(
        cx: &mut crate::TestAppContext,
    ) -> (
        Entity<SheetGridState<TestSheet>>,
        Rc<RefCell<Vec<SheetGridEvent>>>,
        &mut crate::VisualTestContext,
    ) {
        let events: Rc<RefCell<Vec<SheetGridEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = events.clone();
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(move |window, cx| {
            let state = cx.new(|cx| SheetGridState::new(TestSheet, cx));
            cx.subscribe_in(
                &state,
                window,
                move |_this, _state, event: &SheetGridEvent, _window, _cx| {
                    sink.borrow_mut().push(event.clone());
                },
            )
            .detach();
            state.update(cx, |state, cx| state.refresh(cx));
            Probe { state }
        });
        let state = probe.read_with(&*cx, |probe, _| probe.state.clone());
        (state, events, cx)
    }

    /// 列号转列标签：A/B/Z/AA/AB/ZZ/AAA。
    #[test]
    fn column_labels_follow_spreadsheet_names() {
        assert_eq!(column_label(0), "A");
        assert_eq!(column_label(1), "B");
        assert_eq!(column_label(25), "Z");
        assert_eq!(column_label(26), "AA");
        assert_eq!(column_label(27), "AB");
        assert_eq!(column_label(51), "AZ");
        assert_eq!(column_label(52), "BA");
        assert_eq!(column_label(701), "ZZ");
        assert_eq!(column_label(702), "AAA");
    }

    /// 选区归一化、包含判定与夹取。
    #[test]
    fn cell_range_normalizes_contains_and_clamps() {
        let range = CellRange::new(CellRef::new(2, 2), CellRef::new(0, 0));
        assert_eq!(range.min(), CellRef::new(0, 0));
        assert_eq!(range.max(), CellRef::new(2, 2));
        assert_eq!(range.rows(), 0..3);
        assert_eq!(range.cols(), 0..3);
        assert_eq!(range.height(), 3);
        assert_eq!(range.width(), 3);
        assert_eq!(range.len(), 9);
        assert!(range.contains(CellRef::new(1, 1)));
        assert!(!range.contains(CellRef::new(3, 0)));

        let clamped = range.clamp(2, 2);
        assert_eq!(clamped.anchor, CellRef::new(1, 1));
        assert_eq!(clamped.cursor, CellRef::new(0, 0));
    }

    /// `refresh` 从委托读回行列数。
    #[rgpui::test]
    fn refresh_reads_delegate_counts(cx: &mut crate::TestAppContext) {
        let (state, _events, mut cx) = setup(cx);
        assert_eq!(state.read_with(cx, |state, _| state.rows()), 3);
        assert_eq!(state.read_with(cx, |state, _| state.cols()), 2);
    }

    /// 设置选区会夹取到数据范围并抛出 `SelectionChanged`。
    #[rgpui::test]
    fn set_selection_clamps_and_emits(cx: &mut crate::TestAppContext) {
        let (state, events, mut cx) = setup(cx);
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.set_selection(CellRange::new(CellRef::new(0, 0), CellRef::new(9, 9)), cx);
            });
        });
        let expected = CellRange::new(CellRef::new(0, 0), CellRef::new(2, 1));
        assert_eq!(state.read_with(cx, |state, _| state.selection()), expected);
        assert_eq!(
            events.borrow().as_slice(),
            [SheetGridEvent::SelectionChanged(expected)]
        );
    }

    /// 行头 / 列头点击选中整行 / 整列并抛出 `HeaderClicked`。
    #[rgpui::test]
    fn header_click_selects_whole_row_and_col(cx: &mut crate::TestAppContext) {
        let (state, events, mut cx) = setup(cx);

        cx.update(|_, cx| state.update(cx, |state, cx| state.select_row(1, cx)));
        assert_eq!(
            state.read_with(cx, |state, _| state.selection()),
            CellRange::new(CellRef::new(1, 0), CellRef::new(1, 1))
        );
        assert_eq!(
            events.borrow().as_slice(),
            [
                SheetGridEvent::SelectionChanged(CellRange::new(
                    CellRef::new(1, 0),
                    CellRef::new(1, 1)
                )),
                SheetGridEvent::HeaderClicked {
                    col: None,
                    row: Some(1)
                },
            ]
        );

        events.borrow_mut().clear();
        cx.update(|_, cx| state.update(cx, |state, cx| state.select_col(1, cx)));
        assert_eq!(
            state.read_with(cx, |state, _| state.selection()),
            CellRange::new(CellRef::new(0, 1), CellRef::new(2, 1))
        );
        assert_eq!(
            events.borrow().as_slice(),
            [
                SheetGridEvent::SelectionChanged(CellRange::new(
                    CellRef::new(0, 1),
                    CellRef::new(2, 1)
                )),
                SheetGridEvent::HeaderClicked {
                    row: None,
                    col: Some(1)
                },
            ]
        );
    }

    /// 方向移动 / Shift 扩展选区 / 全选。
    #[rgpui::test]
    fn movement_extends_and_select_all(cx: &mut crate::TestAppContext) {
        let (state, _events, mut cx) = setup(cx);

        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.set_active_cell(CellRef::new(0, 0), cx)
            })
        });
        cx.update(|_, cx| state.update(cx, |state, cx| state.move_active(1, 0, false, cx)));
        assert_eq!(
            state.read_with(cx, |state, _| state.active_cell()),
            CellRef::new(1, 0)
        );

        // Shift 扩展：锚点保持 (1,0)，活动端移到 (1,1)。
        cx.update(|_, cx| state.update(cx, |state, cx| state.move_active(0, 1, true, cx)));
        let selection = state.read_with(cx, |state, _| state.selection());
        assert_eq!(selection.anchor, CellRef::new(1, 0));
        assert_eq!(selection.cursor, CellRef::new(1, 1));

        // 越界夹取：向下 100 行落到最后一行。
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.set_active_cell(CellRef::new(0, 0), cx)
            })
        });
        cx.update(|_, cx| state.update(cx, |state, cx| state.move_active(100, 100, false, cx)));
        assert_eq!(
            state.read_with(cx, |state, _| state.active_cell()),
            CellRef::new(2, 1)
        );

        cx.update(|_, cx| state.update(cx, |state, cx| state.select_all(cx)));
        assert_eq!(
            state.read_with(cx, |state, _| state.selection()),
            CellRange::new(CellRef::new(0, 0), CellRef::new(2, 1))
        );
    }

    /// `set_editing` 幂等更新编辑态。
    #[rgpui::test]
    fn editing_flag_tracks_cell(cx: &mut crate::TestAppContext) {
        let (state, _events, mut cx) = setup(cx);
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.set_editing(Some(CellRef::new(1, 1)), cx)
            });
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.editing()),
            Some(CellRef::new(1, 1))
        );
        cx.update(|_, cx| state.update(cx, |state, cx| state.set_editing(None, cx)));
        assert_eq!(state.read_with(cx, |state, _| state.editing()), None);
    }
}
