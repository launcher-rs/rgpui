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

use crate::input_ui::{Input, InputState};
use crate::{
    ActiveTheme as _, AnyElement, App, AppContext as _, Axis, Bounds, Context, Div, DragMoveEvent,
    ElementExt as _, Empty, Entity, EntityId, EventEmitter, FocusHandle, Focusable, Hsla,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseUpEvent,
    ParentElement as _, Pixels, Point, Render, RenderOnce, ScrollStrategy, ScrollableMask,
    Scrollbar, SharedString, Size, Stateful, StatefulInteractiveElement as _, Styled as _,
    VirtualListScrollHandle, Window, div, h_flex, h_virtual_list, prelude::FluentBuilder, px, size,
    v_flex, v_virtual_list,
};

/// 行头列宽（像素）。
pub const ROW_HEADER_WIDTH: Pixels = px(44.0);
/// 列头行高（像素）。
pub const COL_HEADER_HEIGHT: Pixels = px(24.0);
/// 默认列宽（像素）。
pub const DEFAULT_COLUMN_WIDTH: Pixels = px(96.0);
/// 默认行高（像素）。
pub const DEFAULT_ROW_HEIGHT: Pixels = px(24.0);
/// 列宽最小值（拖拽夹取下限，像素）。
pub const MIN_COLUMN_WIDTH: Pixels = px(24.0);
/// 行高最小值（拖拽夹取下限，像素）。
pub const MIN_ROW_HEIGHT: Pixels = px(16.0);

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
    /// 拖拽列宽结束：以框架内预览的最终宽度发给应用持久化（拖拽过程不经应用层，保证跟手）。
    ColumnResized {
        /// 被调整的列号。
        col: usize,
        /// 调整后的列宽（像素，已含最小值夹取）。
        width: Pixels,
    },
    /// 拖拽行高结束：以框架内预览的最终高度发给应用持久化。
    RowResized {
        /// 被调整的行号。
        row: usize,
        /// 调整后的行高（像素，已含最小值夹取）。
        height: Pixels,
    },
    /// 双击列头分隔线，请求按内容自动调整列宽。
    AutoFitColumn {
        /// 目标列号。
        col: usize,
    },
    /// 双击行头分隔线，请求重置该行为默认行高。
    ResetRowHeight {
        /// 目标行号。
        row: usize,
    },
}

/// 行/列尺寸拖拽状态。
///
/// `start_pos` 与 `start_size` 在按下时固定不变，预览值每次从起点重算——
/// 切勿把新尺寸写回 `start_size`：Windows 每次重绘会补发同位置的合成
/// `WM_MOUSEMOVE`，叠加计算会让尺寸正反馈式暴涨。
struct ResizeDrag {
    /// 被调整的行/列号。
    index: usize,
    /// 按下时的光标位置（窗口坐标）。
    start_pos: Point<Pixels>,
    /// 按下时的原始尺寸（列宽或行高，像素）。
    start_size: Pixels,
    /// 当前预览尺寸。
    current: Pixels,
}

/// 列宽拖拽载荷（拖拽预览渲染为空）。
#[derive(Clone)]
pub(crate) struct ResizeColDrag {
    /// 发起拖拽的网格实体（防止多网格串扰）。
    pub(crate) grid: EntityId,
    /// 被调整的列。
    pub(crate) col: usize,
}

/// 行高拖拽载荷（拖拽预览渲染为空）。
#[derive(Clone)]
pub(crate) struct ResizeRowDrag {
    /// 发起拖拽的网格实体（防止多网格串扰）。
    pub(crate) grid: EntityId,
    /// 被调整的行。
    pub(crate) row: usize,
}

impl Render for ResizeColDrag {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

impl Render for ResizeRowDrag {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// 合并单元格区域（左上角 + 跨度），由委托按需提供。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MergeRegion {
    /// 左上角行号。
    pub row: usize,
    /// 左上角列号。
    pub col: usize,
    /// 行跨度。
    pub row_span: usize,
    /// 列跨度。
    pub col_span: usize,
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

    /// 行高（逐行），默认 [`DEFAULT_ROW_HEIGHT`]。
    ///
    /// 实现应按行返回，网格用每行尺寸驱动垂直虚拟化（`v_virtual_list`）。
    fn row_height(&self, _row: usize, _cx: &App) -> Pixels {
        DEFAULT_ROW_HEIGHT
    }

    /// 覆盖给定单元格的合并区域；无合并返回 `None`（默认）。
    ///
    /// 合并区的左上格横向跨列绘制内容，被覆盖的格渲染为空白。
    fn merge_region_at(&self, _cell: CellRef, _cx: &App) -> Option<MergeRegion> {
        None
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
    /// 垂直滚动（行虚拟化，逐行尺寸）。
    vertical_scroll: VirtualListScrollHandle,
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
    /// 编辑中的单元格内输入框（应用侧提供，由网格渲染到编辑格上）。
    editing_input: Option<Entity<InputState>>,
    /// 单击选区后直接键入的首字符（用于"键入即编辑"）。
    typed_char: Option<char>,
    /// 列宽拖拽状态（起点与按下时的原始列宽不可变，预览值每次从起点重算）。
    resize_col: Option<ResizeDrag>,
    /// 行高拖拽状态。
    resize_row: Option<ResizeDrag>,
    /// 冻结的行数（顶部固定）。
    frozen_rows: usize,
    /// 冻结的列数（左侧固定）。
    frozen_cols: usize,
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
            vertical_scroll: VirtualListScrollHandle::new(),
            horizontal_scroll: VirtualListScrollHandle::new(),
            focus_handle: cx.focus_handle(),
            bounds: Bounds::default(),
            dragging: false,
            filling: false,
            fill_target: None,
            editing: None,
            editing_input: None,
            typed_char: None,
            resize_col: None,
            resize_row: None,
            frozen_rows: 0,
            frozen_cols: 0,
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
    ///
    /// `input` 为应用侧提供的单元格内输入框；进入编辑时由网格渲染到该格上，
    /// 回车/点击其他格提交、Esc 由应用层决定。置 `None` 退出编辑。
    pub fn set_editing(
        &mut self,
        cell: Option<CellRef>,
        input: Option<Entity<InputState>>,
        cx: &mut Context<Self>,
    ) {
        if self.editing != cell || self.editing_input != input {
            // 只有切换到别的单元格（或退出编辑）才丢弃待输入的首字符。
            // 响应 EditRequested 时，on_key_down 已把首字符存进 typed_char 并把
            // editing 置为该格；此处仅为同一格绑定输入框，必须保留首字符，
            // 否则应用先 set_editing(Some(cell), Some(input)) 再 take_typed_char()
            // 的预期用法会静默丢失它。
            if self.editing != cell {
                self.typed_char = None;
            }
            self.editing = cell;
            self.editing_input = input;
            cx.notify();
        }
    }

    /// 取出"键入即编辑"的首字符（单击选区后直接打字）。
    pub fn take_typed_char(&mut self) -> Option<char> {
        self.typed_char.take()
    }

    /// 冻结的行数。
    pub fn frozen_rows(&self) -> usize {
        self.frozen_rows
    }

    /// 冻结的列数。
    pub fn frozen_cols(&self) -> usize {
        self.frozen_cols
    }

    /// 设置冻结行列（顶部 `rows` 行、左侧 `cols` 列不随滚动）。
    ///
    /// 会至少保留一行/一列参与滚动，避免整个视图被冻结。
    pub fn set_frozen(&mut self, rows: usize, cols: usize, cx: &mut Context<Self>) {
        if self.rows == 0 || self.cols == 0 {
            self.rows = self.delegate.rows_count(cx);
            self.cols = self.delegate.cols_count(cx);
        }
        let rows = rows.min(self.rows.saturating_sub(1));
        let cols = cols.min(self.cols.saturating_sub(1));
        if self.frozen_rows != rows || self.frozen_cols != cols {
            self.frozen_rows = rows;
            self.frozen_cols = cols;
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

    /// 当前列宽（拖拽预览优先）：正在拖拽的列返回预览值，其余读委托。
    ///
    /// 拖拽过程中框架内直接改预览并重绘，不经过应用层往返，保证跟手；
    /// 松手时才把最终尺寸以事件发给应用持久化。
    fn effective_column_width(&self, col: usize, cx: &App) -> Pixels {
        if let Some(resize) = &self.resize_col
            && resize.index == col
        {
            return resize.current;
        }
        self.delegate.column_width(col, cx)
    }

    /// 当前行高（拖拽预览优先），语义同 [`Self::effective_column_width`]。
    fn effective_row_height(&self, row: usize, cx: &App) -> Pixels {
        if let Some(resize) = &self.resize_row
            && resize.index == row
        {
            return resize.current;
        }
        self.delegate.row_height(row, cx)
    }

    /// 滚动到指定单元格使其可见（冻结区域内的行列无需滚动）。
    pub fn scroll_to_cell(&mut self, cell: CellRef, cx: &mut Context<Self>) {
        self.vertical_scroll.scroll_to_item(
            cell.row.saturating_sub(self.frozen_rows),
            ScrollStrategy::Nearest,
        );

        // 水平方向：冻结列固定，滚动区域从第一非冻结列开始累计宽度。
        if cell.col < self.frozen_cols {
            cx.notify();
            return;
        }
        let viewport_width = self.horizontal_scroll.bounds().size.width;
        if viewport_width > px(0.0) {
            let mut col_x = px(0.0);
            for col in self.frozen_cols..cell.col {
                col_x += self.effective_column_width(col, cx);
            }
            let col_w = self.effective_column_width(cell.col, cx);
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

    /// 鼠标释放：结束拖拽 / 填充 / 行列尺寸调整。
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
        self.emit_resize_finish(cx);
        cx.notify();
    }

    /// 列宽手柄按下：双击请求自动列宽，单击记录拖拽起点。
    fn on_col_resize_down(
        &mut self,
        event: &MouseDownEvent,
        col: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 手柄压在列头上，必须截断事件，避免触发表头整列选择。
        cx.stop_propagation();
        self.focus(window, cx);
        if event.click_count >= 2 {
            cx.emit(SheetGridEvent::AutoFitColumn { col });
            return;
        }
        let width = self.delegate.column_width(col, cx);
        self.resize_col = Some(ResizeDrag {
            index: col,
            start_pos: event.position,
            start_size: width,
            current: width,
        });
    }

    /// 列宽拖拽移动：按下列宽加总位移，更新框架内预览值并重绘（应用层不参与，保证跟手）。
    fn on_col_resize_move(&mut self, event: &DragMoveEvent<ResizeColDrag>, cx: &mut Context<Self>) {
        let drag = event.drag(cx);
        if drag.grid != cx.entity_id() {
            return;
        }
        let Some(resize) = self.resize_col.as_mut() else {
            return;
        };
        if resize.index != drag.col {
            return;
        }
        // 每次都从按下的原始尺寸重算（不能在旧预览值上累加：Windows 每次重绘
        // 会补发同位置的合成 move 事件，叠加会导致尺寸正反馈暴涨）。
        let width = (resize.start_size + (event.event.position.x - resize.start_pos.x))
            .max(MIN_COLUMN_WIDTH);
        if resize.current == width {
            // 位置没变（合成 move 事件），无需重绘，切断重绘→合成事件→重绘的循环。
            return;
        }
        resize.current = width;
        cx.notify();
    }

    /// 行高手柄按下：记录拖拽起点。
    fn on_row_resize_down(
        &mut self,
        event: &MouseDownEvent,
        row: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 手柄压在行头上，必须截断事件，避免触发表头整行选择。
        cx.stop_propagation();
        self.focus(window, cx);
        // 双击：重置该行为默认行高。
        if event.click_count >= 2 {
            cx.emit(SheetGridEvent::ResetRowHeight { row });
            return;
        }
        let height = self.delegate.row_height(row, cx);
        self.resize_row = Some(ResizeDrag {
            index: row,
            start_pos: event.position,
            start_size: height,
            current: height,
        });
    }

    /// 行高拖拽移动：按下行高加总位移，更新框架内预览值并重绘。
    fn on_row_resize_move(&mut self, event: &DragMoveEvent<ResizeRowDrag>, cx: &mut Context<Self>) {
        let drag = event.drag(cx);
        if drag.grid != cx.entity_id() {
            return;
        }
        let Some(resize) = self.resize_row.as_mut() else {
            return;
        };
        if resize.index != drag.row {
            return;
        }
        let height =
            (resize.start_size + (event.event.position.y - resize.start_pos.y)).max(MIN_ROW_HEIGHT);
        if resize.current == height {
            return;
        }
        resize.current = height;
        cx.notify();
    }

    /// 拖拽结束（松手）：把预览的最终尺寸以事件发给应用；尺寸未变则不发（避免误标脏）。
    fn emit_resize_finish(&mut self, cx: &mut Context<Self>) {
        let col_resize = self.resize_col.take();
        let row_resize = self.resize_row.take();
        if let Some(resize) = col_resize
            && resize.current != self.delegate.column_width(resize.index, cx)
        {
            cx.emit(SheetGridEvent::ColumnResized {
                col: resize.index,
                width: resize.current,
            });
        } else if let Some(resize) = row_resize
            && resize.current != self.delegate.row_height(resize.index, cx)
        {
            cx.emit(SheetGridEvent::RowResized {
                row: resize.index,
                height: resize.current,
            });
        }
    }

    /// 列头右缘的列宽拖拽手柄。
    fn render_col_resize_handle(&self, col: usize, cx: &mut Context<Self>) -> AnyElement {
        const HANDLE_SIZE: Pixels = px(5.0);
        let seed = ResizeColDrag {
            grid: cx.entity_id(),
            col,
        };
        div()
            .id(("sheet-col-resize", col))
            .occlude()
            .cursor_col_resize()
            .absolute()
            .top_0()
            .right(px(-3.0))
            .w(HANDLE_SIZE)
            .h_full()
            .hover(|this| this.bg(cx.theme().border))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |state, event, window, cx| {
                    state.on_col_resize_down(event, col, window, cx);
                }),
            )
            .on_drag(seed, |drag, _point, _window, cx| {
                cx.stop_propagation();
                cx.new(|_| drag.clone())
            })
            .on_drag_move(cx.listener(move |state, event, _window, cx| {
                state.on_col_resize_move(event, cx);
            }))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|state, _event, _window, cx| {
                    // 松手落在网格根之外时，根的 release 监听收不到，这里兜底提交。
                    state.emit_resize_finish(cx);
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    /// 行头下缘的行高拖拽手柄。
    fn render_row_resize_handle(&self, row: usize, cx: &mut Context<Self>) -> AnyElement {
        const HANDLE_SIZE: Pixels = px(5.0);
        let seed = ResizeRowDrag {
            grid: cx.entity_id(),
            row,
        };
        div()
            .id(("sheet-row-resize", row))
            .occlude()
            .cursor_row_resize()
            .absolute()
            .left_0()
            .bottom(px(-3.0))
            .w_full()
            .h(HANDLE_SIZE)
            .hover(|this| this.bg(cx.theme().border))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |state, event, window, cx| {
                    state.on_row_resize_down(event, row, window, cx);
                }),
            )
            .on_drag(seed, |drag, _point, _window, cx| {
                cx.stop_propagation();
                cx.new(|_| drag.clone())
            })
            .on_drag_move(cx.listener(move |state, event, _window, cx| {
                state.on_row_resize_move(event, cx);
            }))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|state, _event, _window, cx| {
                    // 松手落在网格根之外时，根的 release 监听收不到，这里兜底提交。
                    state.emit_resize_finish(cx);
                    cx.notify();
                }),
            )
            .into_any_element()
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

        // 键入即编辑：非编辑态下直接打字，首字符作为新内容的开头。
        if !self.editing.is_some()
            && !modifiers.control
            && !modifiers.alt
            && !modifiers.platform
            && !modifiers.shift
            && key.len() == 1
            && let Some(ch) = key.chars().next()
            && !ch.is_control()
        {
            let cursor = self.selection.cursor;
            self.typed_char = Some(ch);
            self.editing = Some(cursor);
            cx.emit(SheetGridEvent::EditRequested(cursor));
            cx.stop_propagation();
            cx.notify();
            return;
        }
        // 翻页步长按活动单元格所在行的行高估算，冻结行高度逐行累加。
        let frozen_height: Pixels = (0..self.frozen_rows.min(self.rows))
            .map(|row| self.effective_row_height(row, cx))
            .fold(px(0.0), |sum, h| sum + h);
        let page_rows = ((self.bounds.size.height - COL_HEADER_HEIGHT - frozen_height)
            / self.effective_row_height(self.selection.cursor.row, cx))
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

    /// 冻结区域总宽度。
    fn frozen_width(&self, cx: &App) -> Pixels {
        (0..self.frozen_cols).fold(px(0.0), |sum, col| {
            sum + self.effective_column_width(col, cx)
        })
    }

    /// 列头单元格。
    fn render_col_header(&self, col: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let border = theme.border;
        let label = self.delegate.column_label(col, cx);
        div()
            .id(format!("sheet-col-head:{col}"))
            // 无障碍语义：列头。
            .role(accesskit::Role::ColumnHeader)
            .aria_column_index(col)
            .relative()
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
            .child(self.render_col_resize_handle(col, cx))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |state, _event, window, cx| {
                    state.focus(window, cx);
                    state.select_col(col, cx);
                }),
            )
            .into_any_element()
    }

    /// 行头单元格。
    fn render_row_header(
        &self,
        row: usize,
        row_height: Pixels,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let border = theme.border;
        let head_bg = theme.table_head;
        let head_fg = theme.table_head_foreground;
        let row_label = self.delegate.row_label(row, cx);
        div()
            .id(("sheet-row-head", row))
            // 无障碍语义：行头。
            .role(accesskit::Role::RowHeader)
            .aria_row_index(row)
            .relative()
            .w(ROW_HEADER_WIDTH)
            .h(row_height)
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .border_r_1()
            .border_b_1()
            .border_color(border)
            .bg(head_bg)
            .text_color(head_fg)
            .text_sm()
            .child(row_label)
            .child(self.render_row_resize_handle(row, cx))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |state, _event, window, cx| {
                    state.focus(window, cx);
                    state.select_row(row, cx);
                }),
            )
            .into_any_element()
    }

    /// 渲染列头行（冻结列固定在左侧，其余随水平滚动）。
    fn render_header(
        &mut self,
        body_col_sizes: Rc<Vec<Size<Pixels>>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let border = theme.border;
        let head_bg = theme.table_head;
        let head_fg = theme.table_head_foreground;
        let horizontal = self.horizontal_scroll.clone();
        let frozen_cols = self.frozen_cols;
        let frozen_width = self.frozen_width(cx);

        let columns = h_virtual_list(cx.entity(), "sheet-col-header", body_col_sizes, {
            move |state: &mut Self, range: Range<usize>, _window, cx| {
                range
                    .map(|index| state.render_col_header(index + frozen_cols, cx))
                    .collect::<Vec<AnyElement>>()
            }
        })
        .track_scroll(&horizontal)
        .flex_1();

        let mut frozen_headers = Vec::new();
        for col in 0..frozen_cols {
            let width = self.effective_column_width(col, cx);
            frozen_headers.push(
                div()
                    .w(width)
                    .h_full()
                    .flex_shrink_0()
                    .child(self.render_col_header(col, cx))
                    .into_any_element(),
            );
        }

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
            .when(frozen_cols > 0, |this| {
                this.child(
                    h_flex()
                        .w(frozen_width)
                        .h_full()
                        .flex_shrink_0()
                        .children(frozen_headers),
                )
            })
            .child(columns)
            .into_any_element()
    }

    /// 渲染滚动区域的单行单元格（行头由独立列表绘制）。
    fn render_row_cells(
        &mut self,
        row: usize,
        row_height: Pixels,
        body_col_sizes: Rc<Vec<Size<Pixels>>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = cx.theme();
        let border = theme.border;
        let horizontal = self.horizontal_scroll.clone();
        let frozen_cols = self.frozen_cols;

        let cells = h_virtual_list(cx.entity(), ("sheet-row", row), body_col_sizes, {
            move |state: &mut Self, range: Range<usize>, window, cx| {
                range
                    .map(|index| {
                        state.render_cell(
                            CellRef::new(row, index + frozen_cols),
                            border,
                            window,
                            cx,
                        )
                    })
                    .collect::<Vec<AnyElement>>()
            }
        })
        .with_scroll_handle(&horizontal)
        .flex_1();

        div()
            .id(("sheet-row", row))
            .flex()
            .flex_row()
            .w_full()
            .h(row_height)
            .flex_shrink_0()
            .border_b_1()
            .border_color(border)
            .child(cells)
    }

    /// 渲染固定宽度的单元格行（冻结列 / 冻结交角）。
    #[allow(clippy::too_many_arguments)]
    fn render_fixed_row(
        &mut self,
        id: &'static str,
        row: usize,
        row_height: Pixels,
        cols: &[usize],
        widths: &[Pixels],
        border: Hsla,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut row_div = h_flex()
            .id((id, row))
            .h(row_height)
            .flex_shrink_0()
            .border_b_1()
            .border_color(border);
        for (col, width) in cols.iter().zip(widths.iter()) {
            row_div = row_div.child(
                div()
                    .w(*width)
                    .h_full()
                    .flex_shrink_0()
                    .child(self.render_cell(CellRef::new(row, *col), border, window, cx)),
            );
        }
        row_div.into_any_element()
    }

    /// 渲染单个单元格（含选区高亮、活动边框与填充柄）。
    fn render_cell(
        &mut self,
        cell: CellRef,
        border: Hsla,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // 编辑中的格：渲染应用提供的单元格内输入框（直接在格上改字）。
        if self.editing == Some(cell)
            && let Some(input) = self.editing_input.clone()
        {
            return div()
                .id(format!("sheet-cell:{}:{}", cell.row, cell.col))
                .relative()
                .w_full()
                .h_full()
                .overflow_hidden()
                .bg(cx.theme().background)
                .border_2()
                .border_color(cx.theme().table_active_border)
                .child(
                    Input::new(&input)
                        .bordered(false)
                        .focus_bordered(false)
                        .h_full()
                        .w_full()
                        // 静态单元格文字垂直居中且没有内边距；输入框默认
                        // 带 input_py（Medium 为 8px），进入编辑会把文字
                        // 整体压低。单元格内编辑清掉纵向内边距。
                        .py(px(0.0))
                        .px(px(2.0)),
                )
                .into_any_element();
        }
        let theme = cx.theme();
        let selection = self.selection;
        let selected = selection.contains(cell);
        let is_active = selection.cursor == cell;
        let is_fill_corner = selection.max() == cell && !is_active;
        let active_bg = theme.table_active;
        let active_border = theme.table_active_border;
        let hover_bg = theme.table_hover;
        let surface = theme.background;

        // 合并感知：被覆盖的格渲染为空白（末列保留右边框）；左上格内容
        // 横向溢出到覆盖列（委托侧决定内容与背景）。
        let region = self.delegate.merge_region_at(cell, cx);
        if let Some(region) = region
            && (region.row_span > 1 || region.col_span > 1)
        {
            let is_top_left = cell.row == region.row && cell.col == region.col;
            if !is_top_left {
                let last_col = cell.col == region.col + region.col_span - 1;
                let mut covered = div()
                    .id(format!("sheet-cell:{}:{}", cell.row, cell.col))
                    .w_full()
                    .h_full();
                if last_col {
                    covered = covered.border_r_1().border_color(border);
                }
                return covered.into_any_element();
            }
            let span_width: Pixels = (region.col..region.col + region.col_span)
                .map(|col| self.delegate.column_width(col, cx))
                .fold(px(0.0), |sum, width| sum + width);
            let content = self.delegate.render_cell(cell, selected, window, cx);
            return div()
                .id(format!("sheet-cell:{}:{}", cell.row, cell.col))
                .role(accesskit::Role::Cell)
                .aria_row_index(cell.row)
                .aria_column_index(cell.col)
                .relative()
                .w_full()
                .h_full()
                .border_r_1()
                .border_color(border)
                .when(selected, |this| this.bg(active_bg))
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .w(span_width)
                        .h_full()
                        .px(px(4.0))
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .child(content),
                )
                .when(is_active, |this| {
                    this.child(
                        div()
                            .absolute()
                            .inset_0()
                            .border_2()
                            .border_color(active_border),
                    )
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |state, event, window, cx| {
                        state.on_cell_mouse_down(event, cell, window, cx);
                    }),
                )
                .into_any_element();
        }

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
        let frozen_rows = self.frozen_rows.min(rows.saturating_sub(1));
        let frozen_cols = self.frozen_cols.min(cols.saturating_sub(1));
        let frozen_width = self.frozen_width(cx);
        // 冻结行高度逐行累加（行高已逐行化，不再均匀）。
        let frozen_height: Pixels = (0..frozen_rows)
            .map(|row| self.effective_row_height(row, cx))
            .fold(px(0.0), |sum, h| sum + h);
        let border = cx.theme().border;
        let head_bg = cx.theme().table_head;
        let head_fg = cx.theme().table_head_foreground;
        let vertical = self.vertical_scroll.clone();
        let horizontal = self.horizontal_scroll.clone();

        let body_col_sizes: Rc<Vec<Size<Pixels>>> = Rc::new(
            (frozen_cols..cols)
                .map(|col| size(self.effective_column_width(col, cx), px(0.0)))
                .collect(),
        );
        // 滚动区逐行高度（虚拟列表只取沿轴分量，即 height）。
        let body_row_sizes: Rc<Vec<Size<Pixels>>> = Rc::new(
            (frozen_rows..rows)
                .map(|row| size(px(0.0), self.effective_row_height(row, cx)))
                .collect(),
        );
        let frozen_col_indices: Rc<Vec<usize>> = Rc::new((0..frozen_cols).collect());
        let frozen_col_widths: Rc<Vec<Pixels>> = Rc::new(
            (0..frozen_cols)
                .map(|col| self.effective_column_width(col, cx))
                .collect(),
        );

        let header = self.render_header(body_col_sizes.clone(), window, cx);

        // 冻结行条：冻结列 / 冻结行 / 交角 / 可见固定行头。
        let frozen_strip = if frozen_rows > 0 {
            let mut row_headers = Vec::new();
            let mut corner_rows = Vec::new();
            let mut cell_rows = Vec::new();
            for row in 0..frozen_rows {
                let row_height = self.effective_row_height(row, cx);
                row_headers.push(self.render_row_header(row, row_height, cx));
                if frozen_cols > 0 {
                    corner_rows.push(self.render_fixed_row(
                        "sheet-freeze-corner",
                        row,
                        row_height,
                        &frozen_col_indices,
                        &frozen_col_widths,
                        border,
                        window,
                        cx,
                    ));
                }
                let list = h_virtual_list(
                    cx.entity(),
                    ("sheet-freeze-row", row),
                    body_col_sizes.clone(),
                    {
                        move |state: &mut Self, range: Range<usize>, window, cx| {
                            range
                                .map(|index| {
                                    state.render_cell(
                                        CellRef::new(row, index + frozen_cols),
                                        border,
                                        window,
                                        cx,
                                    )
                                })
                                .collect::<Vec<AnyElement>>()
                        }
                    },
                )
                .with_scroll_handle(&horizontal)
                .flex_1();
                cell_rows.push(
                    div()
                        .id(("sheet-freeze-row-cells", row))
                        .flex()
                        .flex_row()
                        .h(row_height)
                        .flex_shrink_0()
                        .border_b_1()
                        .border_color(border)
                        .child(list)
                        .into_any_element(),
                );
            }
            Some(
                h_flex()
                    .h(frozen_height)
                    .flex_shrink_0()
                    .child(
                        v_flex()
                            .id("sheet-freeze-row-heads")
                            .w(ROW_HEADER_WIDTH)
                            .h_full()
                            .flex_shrink_0()
                            .bg(head_bg)
                            .text_color(head_fg)
                            .children(row_headers),
                    )
                    .when(frozen_cols > 0, |this| {
                        this.child(
                            v_flex()
                                .id("sheet-freeze-corner")
                                .w(frozen_width)
                                .h_full()
                                .flex_shrink_0()
                                .children(corner_rows),
                        )
                    })
                    .child(
                        v_flex()
                            .id("sheet-freeze-rows")
                            .flex_1()
                            .h_full()
                            .children(cell_rows),
                    )
                    .into_any_element(),
            )
        } else {
            None
        };

        let row_headers = v_virtual_list(
            cx.entity(),
            "sheet-row-headers",
            body_row_sizes.clone(),
            move |state: &mut Self, range: Range<usize>, _window, cx| {
                range
                    .map(|index| {
                        let row = index + frozen_rows;
                        let height = state.effective_row_height(row, cx);
                        state.render_row_header(row, height, cx)
                    })
                    .collect::<Vec<AnyElement>>()
            },
        )
        .track_scroll(&vertical)
        .w(ROW_HEADER_WIDTH)
        .flex_shrink_0();

        let frozen_col_cells = (frozen_cols > 0).then(|| {
            let indices = frozen_col_indices.clone();
            let widths = frozen_col_widths.clone();
            v_virtual_list(
                cx.entity(),
                "sheet-freeze-cols",
                body_row_sizes.clone(),
                move |state: &mut Self, range: Range<usize>, window, cx| {
                    range
                        .map(|index| {
                            let row = index + frozen_rows;
                            let height = state.effective_row_height(row, cx);
                            state.render_fixed_row(
                                "sheet-freeze-col",
                                row,
                                height,
                                &indices,
                                &widths,
                                border,
                                window,
                                cx,
                            )
                        })
                        .collect::<Vec<AnyElement>>()
                },
            )
            .track_scroll(&vertical)
            .w(frozen_width)
            .flex_shrink_0()
            .into_any_element()
        });

        let body = v_virtual_list(
            cx.entity(),
            "sheet-rows",
            body_row_sizes,
            move |state: &mut Self, visible_range: Range<usize>, window, cx| {
                visible_range
                    .map(|index| {
                        let row = index + frozen_rows;
                        let height = state.effective_row_height(row, cx);
                        state.render_row_cells(row, height, body_col_sizes.clone(), window, cx)
                    })
                    .collect::<Vec<_>>()
            },
        )
        .flex_grow_1()
        .track_scroll(&vertical);

        let mut content = v_flex().size_full().child(header);
        if let Some(strip) = frozen_strip {
            content = content.child(strip);
        }
        content = content.child(
            // 用 items_stretch（而非 h_flex 默认的 items_center）：虚拟列表高度为
            // auto，必须由父容器拉伸到整行，否则会塌成 0 高。
            div()
                .flex()
                .flex_row()
                .flex_grow_1()
                .min_h(px(0.0))
                .child(row_headers)
                .when_some(frozen_col_cells, |this, cells| this.child(cells))
                .child(body),
        );

        div()
            .size_full()
            .relative()
            .overflow_hidden()
            .on_prepaint({
                let state = cx.entity();
                move |bounds, _window, cx| state.update(cx, |state, _| state.bounds = bounds)
            })
            .child(content)
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
                    .left(ROW_HEADER_WIDTH + frozen_width)
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
                &self.vertical_scroll.base_handle(),
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
                state.set_editing(Some(CellRef::new(1, 1)), None, cx)
            });
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.editing()),
            Some(CellRef::new(1, 1))
        );
        cx.update(|_, cx| state.update(cx, |state, cx| state.set_editing(None, None, cx)));
        assert_eq!(state.read_with(cx, |state, _| state.editing()), None);
    }

    /// 键入即编辑：为同一格绑定输入框不能吞掉 on_key_down 存下的首字符；
    /// 切换到别的格才丢弃。
    #[rgpui::test]
    fn set_editing_keeps_typed_char_for_same_cell(cx: &mut crate::TestAppContext) {
        use crate::input_ui::InputState;
        let (state, _events, mut cx) = setup(cx);
        let cell = CellRef::new(0, 1);

        // 模拟 on_key_down：首字符入 typed_char，editing 直接置为该格（不走 set_editing）。
        cx.update(|_, cx| {
            state.update(cx, |state, _| {
                state.typed_char = Some('x');
                state.editing = Some(cell);
            });
        });

        // 应用响应 EditRequested：同一格绑定真实输入框。
        cx.update(|window, cx| {
            let input = cx.new(|cx| InputState::new(window, cx));
            state.update(cx, |state, cx| {
                state.set_editing(Some(cell), Some(input), cx)
            });
        });
        let taken = state.update(cx, |state, _| state.take_typed_char());
        assert_eq!(taken, Some('x'), "同一格绑定输入框不应吞掉首字符");

        // 切换到另一格：首字符应被清空。
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.typed_char = Some('y');
                state.set_editing(Some(CellRef::new(2, 1)), None, cx);
            });
        });
        let taken = state.update(cx, |state, _| state.take_typed_char());
        assert_eq!(taken, None, "切换单元格应丢弃上一格的首字符");
    }

    /// 冻结行列至少保留一行一列滚动。
    #[rgpui::test]
    fn freeze_clamps_to_leave_scrollable_area(cx: &mut crate::TestAppContext) {
        let (state, _events, mut cx) = setup(cx);
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_frozen(5, 5, cx));
        });
        assert_eq!(state.read_with(cx, |state, _| state.frozen_rows()), 2);
        assert_eq!(state.read_with(cx, |state, _| state.frozen_cols()), 1);

        cx.update(|_, cx| state.update(cx, |state, cx| state.set_frozen(0, 0, cx)));
        assert_eq!(state.read_with(cx, |state, _| state.frozen_rows()), 0);
        assert_eq!(state.read_with(cx, |state, _| state.frozen_cols()), 0);
    }
}
