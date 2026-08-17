use std::{ops::Range, rc::Rc, time::Duration};

use crate::{
    ActiveTheme, ElementExt, Icon, IconName, StyleSized as _, StyledExt, VirtualListScrollHandle,
    h_flex, v_flex,
};
use crate::{
    AppContext, Axis, Bounds, ClickEvent, Context, Div, DragMoveEvent, EventEmitter, FocusHandle,
    Focusable, InteractiveElement, IntoElement, ListSizingBehavior, MouseButton, MouseDownEvent,
    ParentElement, Pixels, Point, Render, ScrollStrategy, ScrollableMask, Scrollbar, SharedString,
    Size, Stateful, StatefulInteractiveElement as _, Styled, Task, UniformListScrollHandle, Window,
    div, prelude::FluentBuilder, px, transparent_white, uniform_list,
};
use crate::{
    menu::PopupMenu,
    menu::{
        Cancel, ContextMenuExt, SelectDown, SelectFirst, SelectLast, SelectNextColumn,
        SelectPageDown, SelectPageUp, SelectPrevColumn, SelectUp,
    },
};

use super::*;

/// 测量是否开启的辅助函数。
///
/// 读取环境变量 `ZED_MEASUREMENTS` 或 `rgpui_MEASUREMENTS` 判断是否输出渲染耗时。
#[inline]
fn measure_enable() -> bool {
    std::env::var("ZED_MEASUREMENTS").is_ok() || std::env::var("rgpui_MEASUREMENTS").is_ok()
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SelectionMode {
    Column,
    Row,
    Cell,
}

impl SelectionMode {
    #[inline(always)]
    fn is_row(&self) -> bool {
        matches!(self, SelectionMode::Row)
    }

    #[inline(always)]
    fn is_column(&self) -> bool {
        matches!(self, SelectionMode::Column)
    }

    #[inline(always)]
    fn is_cell(&self) -> bool {
        matches!(self, SelectionMode::Cell)
    }
}

/// 表格事件。
#[derive(Clone)]
pub enum TableEvent {
    /// 单击或移动到选中行。
    SelectRow(usize),
    /// 双击行。
    DoubleClickedRow(usize),
    /// 选中列。
    SelectColumn(usize),
    /// 已选中一个单元格（单击或通过键盘导航到）。
    ///
    /// 在单元格选择模式下，单元格被选中时发出。
    /// 第一个 `usize` 是行索引，第二个 `usize` 是列索引。
    ///
    /// 使用键盘快捷键在单元格之间导航时也会发出此事件。
    SelectCell(usize, usize),
    /// 已双击一个单元格。
    ///
    /// 在单元格选择模式下，单元格被双击时发出。
    /// 第一个 `usize` 是行索引，第二个 `usize` 是列索引。
    ///
    /// 使用此事件触发打开详情视图或编辑单元格内容等操作。
    DoubleClickedCell(usize, usize),
    /// 列宽已改变。
    ///
    /// `Vec<Pixels>` 包含所有列的新宽度。
    ColumnWidthsChanged(Vec<Pixels>),
    /// 列已被移动。
    ///
    /// 第一个 `usize` 是列的原始索引，
    /// 第二个 `usize` 是列的新索引。
    MoveColumn(usize, usize),
    /// 行已被右键点击。
    ///
    /// 包含行索引，或在空白区域右键点击时为 `None`。
    /// 使用此事件显示行的上下文菜单。
    RightClickedRow(Option<usize>),
    /// 单元格已被右键点击。
    ///
    /// 在单元格选择模式下，单元格被右键点击时发出。
    /// 第一个 `usize` 是行索引，第二个 `usize` 是列索引。
    ///
    /// 使用此事件显示特定于单元格内容的上下文菜单。
    /// 被右键点击的单元格会显示细边框，直到点击另一个单元格。
    RightClickedCell(usize, usize),
    /// 选择已被清除。
    ///
    /// 选择被清除时发出此事件。
    ClearSelection,
}

/// 行和列的可见范围。
#[derive(Debug, Default)]
pub struct TableVisibleRange {
    /// 行的可见范围。
    rows: Range<usize>,
    /// 列的可见范围。
    cols: Range<usize>,
}

impl TableVisibleRange {
    /// 返回行的可见范围。
    pub fn rows(&self) -> &Range<usize> {
        &self.rows
    }

    /// 返回列的可见范围。
    pub fn cols(&self) -> &Range<usize> {
        &self.cols
    }
}

/// [`DataTable`] 的状态。
///
/// # 选择模式
///
/// 表格支持三种选择模式：
/// - **行选择**：选择整行（默认模式）
/// - **列选择**：选择整列
/// - **单元格选择**：选择单个单元格
///
/// ## 单元格选择
///
/// 启用 `cell_selectable` 后，用户可以：
/// - 点击单元格选中
/// - 右键点击单元格以标记用于上下文菜单
/// - 双击单元格触发操作
/// - 使用键盘在单元格之间导航（方向键、Home、End、PageUp、PageDown、Tab）
///
/// 在单元格选择模式下，左侧会出现行头列，
/// 允许用户点击它选择整行。
///
/// # 事件
///
/// 表格发出以下与单元格选择相关的事件：
/// - [`TableEvent::SelectCell`]：单元格被选中时
/// - [`TableEvent::DoubleClickedCell`]：单元格被双击时
/// - [`TableEvent::RightClickedCell`]：单元格被右键点击时
///
/// # 示例
///
/// ```rust,ignore
/// let table_state = cx.new(|cx| {
///     TableState::new(delegate, cx)
///         .cell_selectable(true)
///         .row_selectable(true)
/// });
///
/// // 订阅单元格事件
/// cx.subscribe(&table_state, |this, table, event, cx| {
///     match event {
///         TableEvent::SelectCell(row_ix, col_ix) => {
///             println!("Selected cell: ({}, {})", row_ix, col_ix);
///         }
///         TableEvent::DoubleClickedCell(row_ix, col_ix) => {
///             println!("Double-clicked cell: ({}, {})", row_ix, col_ix);
///         }
///         _ => {}
///     }
/// });
/// ```
#[derive(Clone)]
pub(crate) struct HeaderCell {
    pub label: SharedString,
    pub width: Pixels,
    col_span: usize,
    is_leaf: bool,
    leaf_col_ix: Option<usize>,
    start_leaf_col_ix: usize,
}

/// 表格状态，管理表格的列、行、选择、排序与滚动等全部运行时状态。
pub struct TableState<D: TableDelegate> {
    focus_handle: FocusHandle,
    delegate: D,
    pub(super) options: TableOptions,
    /// 表格容器的边界。
    bounds: Bounds<Pixels>,
    /// 固定列头的边界。
    fixed_head_cols_bounds: Bounds<Pixels>,

    col_groups: Vec<ColGroup>,
    header_layout: Vec<Vec<HeaderCell>>,

    /// 表格是否可循环选择，默认为 true。
    ///
    /// 当前后选择超出表格边界时，选择将循环到另一侧。
    pub loop_selection: bool,
    /// 表格是否可选中列。
    pub col_selectable: bool,
    /// 表格是否可选中行。
    pub row_selectable: bool,
    /// 表格是否可选中单元格，默认为 false。
    ///
    /// 启用后：
    /// - 用户可以点击单个单元格选中
    /// - 左侧出现用于选择整行的行头列
    ///   （可通过 [`Self::row_header`] 隐藏）
    /// - 键盘导航在单元格级别工作（方向键在单元格之间移动）
    /// - 支持单元格的右键和双击事件
    pub cell_selectable: bool,
    /// 启用 `cell_selectable` 时行头列是否可见，
    /// 默认为 `true`。
    ///
    /// 设为 `false` 可在保留单元格选择的同时隐藏最左侧的窄头列，
    /// 当你想在左侧放置自己的内容（例如行索引列）时很有用。
    /// 隐藏时，再次点击已选中的单元格会将选择升级为整行，
    /// 这样用户仍可选择行；升级需要启用 `row_selectable`。
    pub row_header: bool,
    /// 表格是否可排序。
    pub sortable: bool,
    /// 表格是否可调整列宽。
    pub col_resizable: bool,
    /// 表格是否可移动列。
    pub col_movable: bool,
    /// 启用/禁用固定列功能。
    pub col_fixed: bool,

    /// 垂直方向的滚动句柄，用于行方向的虚拟滚动。
    pub vertical_scroll_handle: UniformListScrollHandle,
    /// 水平方向的滚动句柄，用于列方向的虚拟滚动。
    pub horizontal_scroll_handle: VirtualListScrollHandle,

    selected_row: Option<usize>,
    selection_mode: SelectionMode,
    right_clicked_row: Option<usize>,
    right_clicked_cell: Option<(usize, usize)>,
    selected_col: Option<usize>,
    selected_cell: Option<(usize, usize)>,

    /// 正在调整宽度的列索引。
    resizing_col: Option<usize>,

    /// 拖拽列头时的插入间隔索引（`0..=cols_count`）：
    /// 拖拽的列将在放下时插入到列 `gap - 1` 和 `gap` 之间。
    col_drag_gap: Option<usize>,

    /// 行和列的可见范围。
    visible_range: TableVisibleRange,

    _measure: Vec<Duration>,
    _load_more_task: Task<()>,
}

impl<D> TableState<D>
where
    D: TableDelegate,
{
    /// 使用给定的代理创建新的 TableState。
    pub fn new(delegate: D, _: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            focus_handle: cx.focus_handle().tab_stop(true),
            options: TableOptions::default(),
            delegate,
            col_groups: Vec::new(),
            header_layout: Vec::new(),
            horizontal_scroll_handle: VirtualListScrollHandle::new(),
            vertical_scroll_handle: UniformListScrollHandle::new(),
            selection_mode: SelectionMode::Row,
            selected_row: None,
            right_clicked_row: None,
            right_clicked_cell: None,
            selected_col: None,
            selected_cell: None,
            resizing_col: None,
            col_drag_gap: None,
            bounds: Bounds::default(),
            fixed_head_cols_bounds: Bounds::default(),
            visible_range: TableVisibleRange::default(),
            loop_selection: true,
            col_selectable: true,
            row_selectable: true,
            cell_selectable: false,
            row_header: true,
            sortable: true,
            col_movable: true,
            col_resizable: true,
            col_fixed: true,
            _load_more_task: Task::ready(()),
            _measure: Vec::new(),
        };

        this.prepare_col_groups(cx);
        this
    }

    /// 返回代理的不可变引用。
    pub fn delegate(&self) -> &D {
        &self.delegate
    }

    /// 返回代理的可变引用。
    pub fn delegate_mut(&mut self) -> &mut D {
        &mut self.delegate
    }

    /// 设置是否循环选择，默认为 true。
    pub fn loop_selection(mut self, loop_selection: bool) -> Self {
        self.loop_selection = loop_selection;
        self
    }

    /// 设置是否启用/禁用列移动，默认为 true。
    pub fn col_movable(mut self, col_movable: bool) -> Self {
        self.col_movable = col_movable;
        self
    }

    /// 设置是否启用/禁用列调整宽度，默认为 true。
    pub fn col_resizable(mut self, col_resizable: bool) -> Self {
        self.col_resizable = col_resizable;
        self
    }

    /// 设置是否启用/禁用列排序，默认为 true
    pub fn sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// 设置是否启用/禁用行选择，默认为 true
    pub fn row_selectable(mut self, row_selectable: bool) -> Self {
        self.row_selectable = row_selectable;
        self
    }

    /// 设置是否启用/禁用列选择，默认为 true
    pub fn col_selectable(mut self, col_selectable: bool) -> Self {
        self.col_selectable = col_selectable;
        self
    }

    /// 设置是否启用/禁用单元格选择，默认为 false。
    ///
    /// 启用后：
    /// - 单个单元格可通过点击选中
    /// - 左侧出现行头列（可通过 [`Self::row_header`] 隐藏）
    /// - 键盘导航在单元格级别操作
    /// - 发出单元格特定事件（SelectCell、DoubleClickedCell、RightClickedCell）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let table_state = cx.new(|cx| {
    ///     TableState::new(delegate, cx)
    ///         .cell_selectable(true)  // 启用单元格选择
    ///         .row_selectable(true)   // 也允许通过行头选择行
    /// });
    /// ```
    pub fn cell_selectable(mut self, cell_selectable: bool) -> Self {
        self.cell_selectable = cell_selectable;
        self
    }

    /// 设置行头列是否显示，默认为 `true`。
    ///
    /// 仅在 `cell_selectable` 为 `true` 时生效，否则行头列永远不会渲染。
    /// 当你想将最左侧的列用于自己的内容（例如行索引列）时，隐藏它。
    ///
    /// 隐藏时，第一次点击单元格选择单元格；再次点击已选中的单元格
    /// 会升级为选择整行，这样用户无需专用头列即可选择行。行升级
    /// 需要启用 `row_selectable`。
    pub fn row_header(mut self, row_header: bool) -> Self {
        self.row_header = row_header;
        self
    }

    /// 当列或行更新时，需要刷新表格。
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.prepare_col_groups(cx);
    }

    /// 滚动到给定索引处的行。
    pub fn scroll_to_row(&mut self, row_ix: usize, cx: &mut Context<Self>) {
        self.vertical_scroll_handle
            .scroll_to_item(row_ix, ScrollStrategy::Top);
        cx.notify();
    }

    /// 滚动到给定索引处的列。
    pub fn scroll_to_col(&mut self, col_ix: usize, cx: &mut Context<Self>) {
        let col_ix = col_ix.saturating_sub(self.fixed_left_cols_count());

        self.horizontal_scroll_handle
            .scroll_to_item(col_ix, ScrollStrategy::Top);
        cx.notify();
    }

    /// 返回当前选中的行索引。
    pub fn selected_row(&self) -> Option<usize> {
        self.selected_row
    }

    /// 将选中的行设置为给定索引。
    pub fn set_selected_row(&mut self, row_ix: usize, cx: &mut Context<Self>) {
        let is_down = match self.selected_row {
            Some(selected_row) => row_ix > selected_row,
            None => true,
        };

        cx.stop_propagation();
        self.selection_mode = SelectionMode::Row;
        self.right_clicked_row = None;
        self.selected_row = Some(row_ix);
        if let Some(row_ix) = self.selected_row {
            self.vertical_scroll_handle.scroll_to_item(
                row_ix,
                if is_down {
                    ScrollStrategy::Bottom
                } else {
                    ScrollStrategy::Top
                },
            );
        }
        cx.emit(TableEvent::SelectRow(row_ix));
        cx.emit(TableEvent::RightClickedRow(None));
        cx.notify();
    }

    /// 返回已被右键点击的行。
    pub fn right_clicked_row(&self) -> Option<usize> {
        self.right_clicked_row
    }

    /// 设置或清除右键点击的行状态。
    ///
    /// 传入 `None` 以清除，在打开头部上下文菜单时
    /// 用于防止行上下文菜单同时出现。
    pub fn set_right_clicked_row(&mut self, row: Option<usize>, cx: &mut Context<Self>) {
        self.right_clicked_row = row;
        cx.notify();
    }

    /// 返回当前选中的列索引。
    pub fn selected_col(&self) -> Option<usize> {
        self.selected_col
    }

    /// 将选中的列设置为给定索引。
    pub fn set_selected_col(&mut self, col_ix: usize, cx: &mut Context<Self>) {
        self.selection_mode = SelectionMode::Column;
        self.selected_col = Some(col_ix);
        if let Some(col_ix) = self.selected_col {
            self.scroll_to_col(col_ix, cx);
        }
        cx.emit(TableEvent::SelectColumn(col_ix));
        cx.notify();
    }

    /// 返回选中的单元格，格式为 `(row_ix, col_ix)`。
    ///
    /// 如果当前没有选中单元格或表格处于行/列选择模式，则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// if let Some((row_ix, col_ix)) = table_state.read(cx).selected_cell() {
    ///     println!("Selected cell: ({}, {})", row_ix, col_ix);
    /// }
    /// ```
    pub fn selected_cell(&self) -> Option<(usize, usize)> {
        self.selected_cell
    }

    /// 将选中的单元格设置为给定行和列索引。
    ///
    /// 此方法：
    /// - 将表格切换到单元格选择模式
    /// - 滚动使单元格可见（垂直居中）
    /// - 发出 [`TableEvent::SelectCell`] 事件
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 选中第 5 行第 3 列的单元格
    /// table_state.update(cx, |state, cx| {
    ///     state.set_selected_cell(5, 3, cx);
    /// });
    /// ```
    pub fn set_selected_cell(&mut self, row_ix: usize, col_ix: usize, cx: &mut Context<Self>) {
        self.selection_mode = SelectionMode::Cell;
        self.selected_cell = Some((row_ix, col_ix));

        // 滚动到单元格
        self.vertical_scroll_handle
            .scroll_to_item(row_ix, ScrollStrategy::Center);
        self.scroll_to_col(col_ix, cx);

        cx.emit(TableEvent::SelectCell(row_ix, col_ix));
        cx.notify();
    }

    /// 清除表格的选择。
    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.selection_mode = SelectionMode::Row;
        self.selected_row = None;
        self.selected_col = None;
        self.selected_cell = None;
        cx.emit(TableEvent::ClearSelection);
        cx.notify();
    }

    /// 返回行和列的可见范围。
    ///
    /// 参见 [`TableVisibleRange`]。
    pub fn visible_range(&self) -> &TableVisibleRange {
        &self.visible_range
    }

    /// 导出表格数据。
    ///
    /// 返回一个元组（表头、行），其中每行是一个单元格值的向量。
    pub fn dump(&self, cx: &App) -> (Vec<String>, Vec<Vec<String>>) {
        // 获取表头行
        let columns_count = self.delegate.columns_count(cx);
        let mut headers = Vec::with_capacity(columns_count);
        for col_ix in 0..columns_count {
            let column = self.delegate.column(col_ix, cx);
            headers.push(column.name.to_string());
        }

        // 获取数据行
        let rows_count = self.delegate.rows_count(cx);
        let mut rows = Vec::with_capacity(rows_count);
        for row_ix in 0..rows_count {
            let mut row = Vec::with_capacity(columns_count);
            for col_ix in 0..columns_count {
                row.push(self.delegate.cell_text(row_ix, col_ix, cx));
            }
            rows.push(row);
        }

        (headers, rows)
    }

    /// 根据当前代理重新计算表头布局。
    ///
    /// 在改变影响 `group_headers` 的代理状态后调用。
    pub fn refresh_header_layout(&mut self, cx: &mut Context<Self>) {
        self.update_header_layout(cx);
        cx.notify();
    }

    fn prepare_col_groups(&mut self, cx: &mut Context<Self>) {
        self.col_groups = (0..self.delegate.columns_count(cx))
            .map(|col_ix| {
                let column = self.delegate().column(col_ix, cx);
                ColGroup {
                    width: column.width,
                    bounds: Bounds::default(),
                    column,
                }
            })
            .collect();

        self.update_header_layout(cx);
    }

    fn update_header_layout(&mut self, cx: &mut Context<Self>) {
        let group_rows = self.delegate.group_headers(cx);

        let mut layout = match group_rows.as_ref() {
            Some(rows) => Vec::with_capacity(rows.len() + 1),
            None => Vec::with_capacity(1),
        };

        if let Some(group_rows) = group_rows {
            for row in group_rows {
                let mut cell_row = Vec::with_capacity(row.len());
                let mut current_leaf_ix = 0;
                for group in row {
                    let mut width = px(0.);
                    let start_leaf_col_ix = current_leaf_ix;
                    for i in 0..group.span {
                        if current_leaf_ix + i < self.col_groups.len() {
                            width += self.col_groups[current_leaf_ix + i].width;
                        }
                    }
                    current_leaf_ix += group.span;
                    cell_row.push(HeaderCell {
                        label: group.label.clone(),
                        width,
                        col_span: group.span,
                        is_leaf: false,
                        leaf_col_ix: None,
                        start_leaf_col_ix,
                    });
                }
                layout.push(cell_row);
            }
        }

        let mut leaf_row = Vec::with_capacity(self.col_groups.len());
        for (ix, group) in self.col_groups.iter().enumerate() {
            leaf_row.push(HeaderCell {
                label: group.column.name.clone(),
                width: group.width,
                col_span: 1,
                is_leaf: true,
                leaf_col_ix: Some(ix),
                start_leaf_col_ix: ix,
            });
        }
        layout.push(leaf_row);

        self.header_layout = layout;
    }

    fn fixed_left_cols_count(&self) -> usize {
        if !self.col_fixed {
            return 0;
        }

        self.col_groups
            .iter()
            .filter(|col| col.column.fixed == Some(ColumnFixed::Left))
            .count()
    }

    fn page_item_count(&self) -> usize {
        let row_height = self.options.size.table_row_height();
        let height = self.bounds.size.height;
        let count = (height / row_height).floor() as usize;
        count.saturating_sub(1).max(1)
    }

    fn on_row_right_click(
        &mut self,
        _: &MouseDownEvent,
        row_ix: Option<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.right_clicked_row = row_ix;
        self.right_clicked_cell = None;
        cx.emit(TableEvent::RightClickedRow(row_ix));
    }

    fn on_cell_right_click(
        &mut self,
        _: &MouseDownEvent,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.cell_selectable {
            return;
        }

        cx.stop_propagation();
        self.right_clicked_cell = Some((row_ix, col_ix));
        self.right_clicked_row = None;
        cx.emit(TableEvent::RightClickedCell(row_ix, col_ix));
    }

    fn on_row_left_click(
        &mut self,
        e: &ClickEvent,
        row_ix: usize,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.row_selectable {
            return;
        }

        self.set_selected_row(row_ix, cx);

        if e.click_count() == 2 {
            cx.emit(TableEvent::DoubleClickedRow(row_ix));
        }
    }

    fn on_col_head_click(&mut self, col_ix: usize, _: &mut Window, cx: &mut Context<Self>) {
        if !self.col_selectable {
            return;
        }

        let Some(col_group) = self.col_groups.get(col_ix) else {
            return;
        };

        if !col_group.column.selectable {
            return;
        }

        self.set_selected_col(col_ix, cx)
    }

    fn on_cell_click(
        &mut self,
        e: &ClickEvent,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.cell_selectable {
            return;
        }

        cx.stop_propagation();

        let is_double_click = e.click_count() == 2;

        // 当行头列被隐藏时，单击已选中的单元格会将选择升级为整行，
        // 从而给用户一种无需专用头列即可选择行的方法。
        // 双击会传递到 `DoubleClickedCell`，绝不触发升级。
        let is_reselect =
            self.selection_mode.is_cell() && self.selected_cell == Some((row_ix, col_ix));
        let should_escalate_to_row =
            !self.row_header && self.row_selectable && is_reselect && !is_double_click;
        if should_escalate_to_row {
            self.set_selected_row(row_ix, cx);
            return;
        }

        self.set_selected_cell(row_ix, col_ix, cx);

        if is_double_click {
            cx.emit(TableEvent::DoubleClickedCell(row_ix, col_ix));
        }
    }

    fn has_selection(&self) -> bool {
        self.selected_row.is_some() || self.selected_col.is_some() || self.selected_cell.is_some()
    }

    pub(super) fn action_cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            self.clear_selection(cx);
            return;
        }
        cx.propagate();
    }

    pub(super) fn action_select_prev(
        &mut self,
        _: &SelectUp,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rows_count = self.delegate.rows_count(cx);
        if rows_count < 1 {
            return;
        }

        // 单元格选择模式：在同一列内上移
        if self.selection_mode.is_cell() {
            if let Some((row_ix, col_ix)) = self.selected_cell {
                let new_row = if row_ix > 0 {
                    row_ix.saturating_sub(1)
                } else if self.loop_selection {
                    rows_count.saturating_sub(1)
                } else {
                    row_ix
                };
                self.set_selected_cell(new_row, col_ix, cx);
            } else {
                // 无选中单元格，选中第一个单元格
                self.set_selected_cell(0, 0, cx);
            }
            return;
        }

        // 行选择模式
        let mut selected_row = self.selected_row.unwrap_or(0);
        if selected_row > 0 {
            selected_row = selected_row.saturating_sub(1);
        } else {
            if self.loop_selection {
                selected_row = rows_count.saturating_sub(1);
            }
        }

        self.set_selected_row(selected_row, cx);
    }

    pub(super) fn action_select_next(
        &mut self,
        _: &SelectDown,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rows_count = self.delegate.rows_count(cx);
        if rows_count < 1 {
            return;
        }

        // 单元格选择模式：在同一列内下移
        if self.selection_mode.is_cell() {
            if let Some((row_ix, col_ix)) = self.selected_cell {
                let new_row = if row_ix < rows_count.saturating_sub(1) {
                    row_ix + 1
                } else if self.loop_selection {
                    0
                } else {
                    row_ix
                };
                self.set_selected_cell(new_row, col_ix, cx);
            } else {
                // 无选中单元格，选中第一个单元格
                self.set_selected_cell(0, 0, cx);
            }
            return;
        }

        // 行选择模式
        let selected_row = match self.selected_row {
            Some(selected_row) if selected_row < rows_count.saturating_sub(1) => selected_row + 1,
            Some(selected_row) => {
                if self.loop_selection {
                    0
                } else {
                    selected_row
                }
            }
            _ => 0,
        };

        self.set_selected_row(selected_row, cx);
    }

    pub(super) fn action_select_first_column(
        &mut self,
        _: &SelectFirst,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 单元格选择模式：移动到当前行的第一个单元格
        if self.selection_mode.is_cell() {
            if let Some((row_ix, _)) = self.selected_cell {
                self.set_selected_cell(row_ix, 0, cx);
            } else {
                // 无选中单元格，选中第一行第一个单元格
                self.set_selected_cell(0, 0, cx);
            }
            return;
        }

        // 列选择模式
        self.set_selected_col(0, cx);
    }

    pub(super) fn action_select_last_column(
        &mut self,
        _: &SelectLast,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let columns_count = self.delegate.columns_count(cx);

        // 单元格选择模式：移动到当前行的最后一个单元格
        if self.selection_mode.is_cell() {
            if let Some((row_ix, _)) = self.selected_cell {
                self.set_selected_cell(row_ix, columns_count.saturating_sub(1), cx);
            } else {
                // 无选中单元格，选中第一行最后一个单元格
                self.set_selected_cell(0, columns_count.saturating_sub(1), cx);
            }
            return;
        }

        // 列选择模式
        self.set_selected_col(columns_count.saturating_sub(1), cx);
    }

    pub(super) fn action_select_page_up(
        &mut self,
        _: &SelectPageUp,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let step = self.page_item_count();

        // 单元格选择模式：在同一列内向上翻页
        if self.selection_mode.is_cell() {
            if let Some((row_ix, col_ix)) = self.selected_cell {
                let target = row_ix.saturating_sub(step);
                self.set_selected_cell(target, col_ix, cx);
            } else {
                // 无选中单元格，选中第一个单元格
                self.set_selected_cell(0, 0, cx);
            }
            return;
        }

        // 行选择模式
        let current = self.selected_row.unwrap_or(0);
        let target = current.saturating_sub(step);
        self.set_selected_row(target, cx);
    }

    pub(super) fn action_select_page_down(
        &mut self,
        _: &SelectPageDown,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rows_count = self.delegate.rows_count(cx);
        if rows_count == 0 {
            return;
        }

        let step = self.page_item_count();

        // 单元格选择模式：在同一列内向下翻页
        if self.selection_mode.is_cell() {
            if let Some((row_ix, col_ix)) = self.selected_cell {
                let max_row = rows_count.saturating_sub(1);
                let target = (row_ix + step).min(max_row);
                self.set_selected_cell(target, col_ix, cx);
            } else {
                // 无选中单元格，选中第一个单元格
                self.set_selected_cell(0, 0, cx);
            }
            return;
        }

        // 行选择模式
        let current = self.selected_row.unwrap_or(0);
        let max_row = rows_count.saturating_sub(1);
        let target = (current + step).min(max_row);
        self.set_selected_row(target, cx);
    }

    pub(super) fn action_select_prev_col(
        &mut self,
        _: &SelectPrevColumn,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let columns_count = self.delegate.columns_count(cx);

        // 单元格选择模式：在同一行内左移
        if self.selection_mode.is_cell() {
            if let Some((row_ix, col_ix)) = self.selected_cell {
                let new_col = if col_ix > 0 {
                    col_ix.saturating_sub(1)
                } else if self.loop_selection {
                    columns_count.saturating_sub(1)
                } else {
                    col_ix
                };
                self.set_selected_cell(row_ix, new_col, cx);
            } else {
                // 无选中单元格，选中第一个单元格
                self.set_selected_cell(0, 0, cx);
            }
            return;
        }

        // 列选择模式
        let mut selected_col = self.selected_col.unwrap_or(0);
        if selected_col > 0 {
            selected_col = selected_col.saturating_sub(1);
        } else {
            if self.loop_selection {
                selected_col = columns_count.saturating_sub(1);
            }
        }
        self.set_selected_col(selected_col, cx);
    }

    pub(super) fn action_select_next_col(
        &mut self,
        _: &SelectNextColumn,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let columns_count = self.delegate.columns_count(cx);

        // 单元格选择模式：在同一行内右移
        if self.selection_mode.is_cell() {
            if let Some((row_ix, col_ix)) = self.selected_cell {
                let new_col = if col_ix < columns_count.saturating_sub(1) {
                    col_ix + 1
                } else if self.loop_selection {
                    0
                } else {
                    col_ix
                };
                self.set_selected_cell(row_ix, new_col, cx);
            } else {
                // 无选中单元格，选中第一个单元格
                self.set_selected_cell(0, 0, cx);
            }
            return;
        }

        // 列选择模式
        let mut selected_col = self.selected_col.unwrap_or(0);
        if selected_col < columns_count.saturating_sub(1) {
            selected_col += 1;
        } else {
            if self.loop_selection {
                selected_col = 0;
            }
        }

        self.set_selected_col(selected_col, cx);
    }

    /// 鼠标位置接近表格边界时滚动表格。
    fn scroll_table_by_col_resizing(
        &mut self,
        mouse_position: Point<Pixels>,
        col_group: &ColGroup,
    ) {
        // 如果鼠标位置在表格边界右侧之外则不做任何操作，避免向右滚动。
        if mouse_position.x > self.bounds.right() {
            return;
        }

        let mut offset = self.horizontal_scroll_handle.offset();
        let col_bounds = col_group.bounds;

        if mouse_position.x < self.bounds.left()
            && col_bounds.right() < self.bounds.left() + px(20.)
        {
            offset.x += px(1.);
        } else if mouse_position.x > self.bounds.right()
            && col_bounds.right() > self.bounds.right() - px(20.)
        {
            offset.x -= px(1.);
        }

        self.horizontal_scroll_handle.set_offset(offset);
    }

    /// `ix` 是要调整宽度的列索引，
    /// `size` 是该列的新尺寸。
    fn resize_cols(&mut self, ix: usize, size: Pixels, _: &mut Window, cx: &mut Context<Self>) {
        if !self.col_resizable {
            return;
        }

        let mut changed = false;
        if let Some(col_group) = self.col_groups.get_mut(ix) {
            if col_group.is_resizable() {
                let new_width = size.clamp(col_group.column.min_width, col_group.column.max_width);
                if col_group.width != new_width {
                    col_group.width = new_width;
                    changed = true;
                }
            }
        }

        if changed {
            self.update_header_layout(cx);
            cx.notify();
        }
    }

    fn perform_sort(&mut self, col_ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.sortable {
            return;
        }

        let sort = self.col_groups.get(col_ix).and_then(|g| g.column.sort);
        if sort.is_none() {
            return;
        }

        let sort = sort.unwrap();
        let sort = match sort {
            ColumnSort::Ascending => ColumnSort::Default,
            ColumnSort::Descending => ColumnSort::Ascending,
            ColumnSort::Default => ColumnSort::Descending,
        };

        for (ix, col_group) in self.col_groups.iter_mut().enumerate() {
            if ix == col_ix {
                col_group.column.sort = Some(sort);
            } else {
                if col_group.column.sort.is_some() {
                    col_group.column.sort = Some(ColumnSort::Default);
                }
            }
        }

        self.delegate_mut().perform_sort(col_ix, sort, window, cx);

        cx.notify();
    }

    fn move_column(
        &mut self,
        col_ix: usize,
        to_ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if col_ix == to_ix {
            return;
        }

        self.delegate.move_column(col_ix, to_ix, window, cx);
        let col_group = self.col_groups.remove(col_ix);
        self.col_groups.insert(to_ix, col_group);

        cx.emit(TableEvent::MoveColumn(col_ix, to_ix));
        cx.notify();
    }

    /// 解析在窗口坐标 `x` 处列头拖拽的插入间隔，或当在此处放下
    /// 不会移动 `drag_col_ix` 处的拖拽列时返回 `None`。
    fn drag_gap_at(&self, x: Pixels, drag_col_ix: usize) -> Option<usize> {
        let fixed_count = self.fixed_left_cols_count();

        // 滚动到固定区域下方的列保留过期的边界，因此当 `x` 落在
        // 该区域时仅针对固定列解析，否则针对可见的可滚动列解析。
        let candidates = if fixed_count > 0 && x < self.fixed_head_cols_bounds.right() {
            0..fixed_count
        } else {
            self.calculate_visible_leaf_col_range(fixed_count).0
        };

        // 间隔位于中心在 `x` 左侧的最后一个候选列之后。
        let mut gap = candidates.start;
        for ix in candidates {
            if x < self.col_groups[ix].bounds.center().x {
                break;
            }
            gap = ix + 1;
        }

        // 如果在那里放下会使拖拽的列回到原处，则没有间隔。
        if gap == drag_col_ix || gap == drag_col_ix + 1 {
            None
        } else {
            Some(gap)
        }
    }

    /// 可见范围接近末尾时，调度代理的 `load_more` 方法。
    fn load_more_if_need(
        &mut self,
        rows_count: usize,
        visible_end: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let threshold = self.delegate.load_more_threshold();
        // 安全处理减法逻辑，防止减法溢出
        if visible_end >= rows_count.saturating_sub(threshold) {
            if !self.delegate.has_more(cx) {
                return;
            }

            self._load_more_task = cx.spawn_in(window, async move |view, window| {
                _ = view.update_in(window, |view, window, cx| {
                    view.delegate.load_more(window, cx);
                });
            });
        }
    }

    fn update_visible_range_if_need(
        &mut self,
        visible_range: Range<usize>,
        axis: Axis,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 当可见范围只有 1 项时跳过。
        // visual_list 将使用第一个条目进行测量。
        if visible_range.len() <= 1 {
            return;
        }

        if axis == Axis::Vertical {
            if self.visible_range.rows == visible_range {
                return;
            }
            self.delegate_mut()
                .visible_rows_changed(visible_range.clone(), window, cx);
            self.visible_range.rows = visible_range;
        } else {
            if self.visible_range.cols == visible_range {
                return;
            }
            self.delegate_mut()
                .visible_columns_changed(visible_range.clone(), window, cx);
            self.visible_range.cols = visible_range;
        }
    }

    fn render_cell(
        &self,
        _row_ix: Option<usize>,
        col_ix: usize,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Div {
        let Some(col_group) = self.col_groups.get(col_ix) else {
            return div();
        };

        let col_width = col_group.width;
        let col_padding = col_group.column.paddings;

        div()
            .w(col_width)
            .h_full()
            .flex_shrink_0()
            .overflow_hidden()
            .whitespace_nowrap()
            .table_cell_size(self.options.size)
            .map(|this| match col_padding {
                Some(padding) => this
                    .pl(padding.left)
                    .pr(padding.right)
                    .pt(padding.top)
                    .pb(padding.bottom),
                None => this,
            })
    }

    /// 显示列选择样式，当列被选中且选择状态为 Column 时。
    /// 注意：当单元格被选中时，不显示列选择样式。
    fn render_col_wrap(
        &self,
        _row_ix: Option<usize>,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let el = h_flex().h_full();
        let selectable = self.col_selectable
            && self
                .col_groups
                .get(col_ix)
                .map(|col_group| col_group.column.selectable)
                .unwrap_or(false);

        // 如果选中了单元格，则不显示列选择样式
        if self.selection_mode.is_cell() {
            return el;
        }

        if selectable && self.selected_col == Some(col_ix) && self.selection_mode.is_column() {
            el.bg(cx.theme().tokens.table_active)
        } else {
            el
        }
    }

    fn render_resize_handle(
        &self,
        ix: usize,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        const HANDLE_SIZE: Pixels = px(2.);

        let resizable = self.col_resizable
            && self
                .col_groups
                .get(ix)
                .map(|col| col.is_resizable())
                .unwrap_or(false);
        if !resizable {
            return div().into_any_element();
        }

        let group_id = SharedString::from(format!("resizable-handle:{}", ix));

        h_flex()
            .id(("resizable-handle", ix))
            .group(group_id.clone())
            .occlude()
            .cursor_col_resize()
            .h_full()
            .w(HANDLE_SIZE)
            .ml(-(HANDLE_SIZE))
            .justify_end()
            .items_center()
            .child(
                div()
                    .h_full()
                    .justify_center()
                    .bg(cx.theme().table_row_border)
                    .group_hover(&group_id, |this| this.bg(cx.theme().border).h_full())
                    .w(px(1.)),
            )
            .on_drag_move(
                cx.listener(move |view, e: &DragMoveEvent<ResizeColumn>, window, cx| {
                    match e.drag(cx) {
                        ResizeColumn((entity_id, ix)) => {
                            if cx.entity_id() != *entity_id {
                                return;
                            }

                            let ix = *ix;
                            view.resizing_col = Some(ix);

                            let col_group = view
                                .col_groups
                                .get(ix)
                                .expect("BUG: invalid col index")
                                .clone();

                            view.resize_cols(
                                ix,
                                e.event.position.x - HANDLE_SIZE - col_group.bounds.left(),
                                window,
                                cx,
                            );

                            // 拖拽接近边缘时滚动表格
                            view.scroll_table_by_col_resizing(e.event.position, &col_group);
                        }
                    };
                }),
            )
            .on_drag(ResizeColumn((cx.entity_id(), ix)), |drag, _, _, cx| {
                cx.stop_propagation();
                cx.new(|_| drag.clone())
            })
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    if view.resizing_col.is_none() {
                        return;
                    }

                    view.resizing_col = None;

                    let new_widths = view.col_groups.iter().map(|g| g.width).collect();
                    cx.emit(TableEvent::ColumnWidthsChanged(new_widths));
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    /// 渲染行头单元格（启用 cell_selectable 时）
    fn render_row_header_cell(
        &self,
        row_ix: usize,
        is_head: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(("row-header", row_ix))
            .w_3()
            .h_full()
            .border_r_1()
            .border_color(cx.theme().table_row_border)
            .bg(cx.theme().tokens.table_head)
            .flex_shrink_0()
            .table_cell_size(self.options.size)
            .when(!is_head, |this| {
                this.when(self.row_selectable, |this| {
                    this.on_click(cx.listener(move |table, _, _window, cx| {
                        table.set_selected_row(row_ix, cx);
                    }))
                })
            })
    }

    fn render_sort_icon(
        &self,
        col_ix: usize,
        col_group: &ColGroup,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        if !self.sortable {
            return None;
        }

        let Some(sort) = col_group.column.sort else {
            return None;
        };

        let (icon, is_on) = match sort {
            ColumnSort::Ascending => (IconName::SortAscending, true),
            ColumnSort::Descending => (IconName::SortDescending, true),
            ColumnSort::Default => (IconName::ChevronsUpDown, false),
        };

        Some(
            div()
                .id(("icon-sort", col_ix))
                .p(px(2.))
                .rounded(cx.theme().radius / 2.)
                .map(|this| match is_on {
                    true => this,
                    false => this.opacity(0.5),
                })
                .hover(|this| this.bg(cx.theme().tokens.secondary).opacity(7.))
                .active(|this| this.bg(cx.theme().tokens.secondary_active).opacity(1.))
                .on_click(
                    cx.listener(move |table, _, window, cx| table.perform_sort(col_ix, window, cx)),
                )
                .child(
                    Icon::new(icon)
                        .size_3()
                        .text_color(cx.theme().secondary_foreground),
                ),
        )
    }

    /// 渲染列头。
    /// 子元素必须是一个一个的条目。
    /// 因为水平滚动句柄将使用 child_item_bounds 来计算
    /// 其 `scroll_to_item` 方法的条目位置。
    fn render_th(&mut self, col_ix: usize, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let entity_id = cx.entity_id();
        let col_group = self.col_groups.get(col_ix).expect("BUG: invalid col index");

        let movable = self.col_movable && col_group.column.movable;
        let paddings = col_group.column.paddings;
        let name = col_group.column.name.clone();

        h_flex()
            .h_full()
            .child(
                self.render_cell(None, col_ix, window, cx)
                    .id(("col-header", col_ix))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.on_col_head_click(col_ix, window, cx);
                    }))
                    .child(
                        h_flex()
                            .size_full()
                            .justify_between()
                            .items_center()
                            .child(self.delegate.render_th(col_ix, window, cx))
                            .when_some(paddings, |this, paddings| {
                                // 如果该列有自定义内边距，则为排序图标留出右侧空间
                                let offset_pr =
                                    self.options.size.table_cell_padding().right - paddings.right;
                                this.pr(offset_pr.max(px(0.)))
                            })
                            .children(self.render_sort_icon(col_ix, &col_group, window, cx)),
                    )
                    .when(movable, |this| {
                        this.on_drag(
                            DragColumn {
                                entity_id,
                                col_ix,
                                name,
                                width: col_group.width,
                            },
                            |drag, _, _, cx| {
                                cx.stop_propagation();
                                cx.new(|_| drag.clone())
                            },
                        )
                    })
                    .map(|this| {
                        // 在间隔列的左边缘或最后一列的右边缘绘制插入指示器。
                        // 使用绝对定位的覆盖层而不是边框，以避免移动单元格内容。
                        let last_gap = col_ix + 1 == self.col_groups.len();
                        match self.col_drag_gap {
                            Some(gap)
                                if cx.has_active_drag()
                                    && (gap == col_ix || (last_gap && gap == col_ix + 1)) =>
                            {
                                let right_side = gap == col_ix + 1;
                                this.relative().child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .bottom_0()
                                        .w(px(2.))
                                        .map(|d| if right_side { d.right_0() } else { d.left_0() })
                                        .bg(cx.theme().drag_border),
                                )
                            }
                            _ => this,
                        }
                    }),
            )
            // 调整宽度的句柄
            .child(self.render_resize_handle(col_ix, window, cx))
            // 保存该列的边界
            .on_prepaint({
                let view = cx.entity();
                move |bounds, _, cx| view.update(cx, |r, _| r.col_groups[col_ix].bounds = bounds)
            })
    }

    /// 计算用于表头渲染的可见非固定叶子列范围。
    ///
    /// 返回 `(visible_range, left_spacer_width)`，其中：
    /// - `visible_range` 是需要渲染的列索引范围。
    /// - `left_spacer_width` 是屏幕外左侧列的总宽度，
    ///   用作间隔 div 以保持可见列在正确位置。
    ///
    /// 在第一帧 `self.bounds` 为零，因此返回覆盖所有列的
    /// 回退值以避免初始绘制时出现空白表头。
    fn calculate_visible_leaf_col_range(
        &self,
        left_columns_count: usize,
    ) -> (Range<usize>, Pixels) {
        let total_cols = self.col_groups.len();

        if self.bounds.size.width == px(0.) {
            return (left_columns_count..total_cols, px(0.));
        }

        let fixed_width = self.fixed_head_cols_bounds.size.width;
        let available_width = (self.bounds.size.width - fixed_width).max(px(0.));
        // 向右滚动时滚动句柄偏移为负；取反以获得距滚动区域左边缘的正距离。
        let scroll_x = (-self.horizontal_scroll_handle.offset().x).max(px(0.));

        // 从左到右遍历非固定列，找到右边缘进入视口的第一个列。
        // 被跳过列的累计宽度成为左侧间隔宽度。
        let mut range_start = left_columns_count;
        let mut left_spacer = px(0.);
        let mut cumulative = px(0.);
        for i in left_columns_count..total_cols {
            let right_edge = cumulative + self.col_groups[i].width;
            if right_edge > scroll_x {
                range_start = i;
                left_spacer = cumulative;
                break;
            }
            cumulative = right_edge;
        }

        // 从 `range_start` 继续（跳过已扫描的列）以找到仍在
        // 视口内的最后一列。200px 的过度绘制缓冲区可防止
        // 用户快速滚动时出现可见闪烁。
        let right_bound = scroll_x + available_width + px(200.);
        let mut range_end = total_cols;
        let mut cumulative = left_spacer; // 已累加 `range_start` 之前的宽度
        for i in range_start..total_cols {
            cumulative += self.col_groups[i].width;
            if cumulative > right_bound {
                range_end = (i + 1).min(total_cols);
                break;
            }
        }

        (range_start..range_end, left_spacer)
    }

    fn render_table_header(
        &mut self,
        left_columns_count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity();
        let horizontal_scroll_handle = self.horizontal_scroll_handle.clone();

        // 表头叶子列虚拟化。
        //
        // `render_th` 创建带有调整宽度句柄监听器的交互式元素。
        // 每帧为每一列调用它是 O(n) 的列数；对于 1000 多列，
        // 即使在发布模式下也会将 FPS 降至 60 以下。
        //
        // 我们将渲染限制在溢出滚动视口内当前可见的列上，
        // 用惰性间隔 div 环绕它们：
        //
        //   [left_spacer] [visible columns...] [right_spacer] [last_empty_col]
        //
        // 间隔保持 flex 容器的总内容宽度，使滚动条范围保持正确。
        let total_cols = self.col_groups.len();
        let (visible_col_range, left_spacer) =
            self.calculate_visible_leaf_col_range(left_columns_count);

        let layout_len = self.header_layout.len();

        // 如果没有固定列，则重置固定表头列边界
        if left_columns_count == 0 {
            self.fixed_head_cols_bounds = Bounds::default();
        }

        let mut header = self.delegate_mut().render_header(window, cx);
        let style = header.style().clone();
        let layout = self.header_layout.clone();

        header
            .h_flex()
            .w_full()
            .flex_shrink_0()
            .bg(cx.theme().tokens.table_head)
            .text_color(cx.theme().table_head_foreground)
            .refine_style(&style)
            .on_drag_move(cx.listener(|table, e: &DragMoveEvent<DragColumn>, _, cx| {
                let drag = e.drag(cx);
                let (drag_entity_id, drag_col_ix) = (drag.entity_id, drag.col_ix);

                let gap =
                    if drag_entity_id == cx.entity_id() && e.bounds.contains(&e.event.position) {
                        table.drag_gap_at(e.event.position.x, drag_col_ix)
                    } else {
                        None
                    };

                if table.col_drag_gap != gap {
                    table.col_drag_gap = gap;
                    cx.notify();
                }
            }))
            .on_drop(cx.listener(|table, drag: &DragColumn, window, cx| {
                if drag.entity_id != cx.entity_id() {
                    return;
                }

                // 将拖拽的列插入到指示的间隔中。
                let Some(gap) = table.col_drag_gap.take() else {
                    return;
                };
                let to_ix = if drag.col_ix < gap { gap - 1 } else { gap };
                table.move_column(drag.col_ix, to_ix, window, cx);
            }))
            .when(self.cell_selectable && self.row_header, |this| {
                this.child(self.render_row_header_cell(0, true, cx))
            })
            .when(left_columns_count > 0, |this| {
                let view = view.clone();
                // 渲染左侧固定列
                this.child(
                    h_flex()
                        .relative()
                        .h_full()
                        .bg(cx.theme().tokens.table_head)
                        .child(v_flex().min_w_full().flex_shrink_0().children(
                            layout.iter().enumerate().map(|(_row_ix, row_cells)| {
                                h_flex()
                                    .min_w_full()
                                    .h(self.options.size.table_row_height())
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .children(row_cells.iter().filter_map(|cell| {
                                        if cell.start_leaf_col_ix < left_columns_count {
                                            if cell.is_leaf {
                                                if let Some(ix) = cell.leaf_col_ix {
                                                    return Some(
                                                        self.render_th(ix, window, cx)
                                                            .into_any_element(),
                                                    );
                                                }
                                            } else {
                                                return Some(
                                                    self.delegate_mut()
                                                        .render_group_th(
                                                            &cell.label,
                                                            cell.col_span,
                                                            cell.width,
                                                            window,
                                                            cx,
                                                        )
                                                        .into_any_element(),
                                                );
                                            }
                                        }
                                        None
                                    }))
                            }),
                        ))
                        .child(
                            // 固定列边框
                            div()
                                .absolute()
                                .top_0()
                                .right_0()
                                .bottom_0()
                                .w_0()
                                .flex_shrink_0()
                                .border_r_1()
                                .border_color(cx.theme().border),
                        )
                        .on_prepaint(move |bounds, _, cx| {
                            view.update(cx, |r, _| r.fixed_head_cols_bounds = bounds)
                        }),
                )
            })
            .child(
                // 列
                h_flex()
                    .id("table-head")
                    .size_full()
                    .overflow_scroll()
                    .relative()
                    .track_scroll(&horizontal_scroll_handle)
                    .bg(cx.theme().tokens.table_head)
                    .child(v_flex().min_w_full().flex_shrink_0().children(
                        layout.iter().enumerate().map(|(row_ix, row_cells)| {
                            let is_leaf_row = row_ix + 1 == layout_len;
                            h_flex()
                                .min_w_full()
                                .h(self.options.size.table_row_height())
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .map(|this| {
                                    if is_leaf_row {
                                        // 叶子行：应用间隔虚拟化模式。
                                        // 只渲染 `visible_range` 内的列；两个
                                        // 间隔 div 保持容器的总内容宽度，
                                        // 使滚动条范围保持正确。
                                        this.when(left_spacer > px(0.), |r| {
                                            r.child(div().w(left_spacer).h_full().flex_shrink_0())
                                        })
                                        .children(row_cells.iter().filter_map(|cell| {
                                            if cell.is_leaf {
                                                let ix = cell.leaf_col_ix?;
                                                if !visible_col_range.contains(&ix) {
                                                    return None;
                                                }
                                                Some(
                                                    self.render_th(ix, window, cx)
                                                        .into_any_element(),
                                                )
                                            } else {
                                                None
                                            }
                                        }))
                                        .when(visible_col_range.end < total_cols, |r| {
                                            let right_spacer: Pixels = self.col_groups
                                                [visible_col_range.end..total_cols]
                                                .iter()
                                                .map(|g| g.width)
                                                .sum();
                                            r.child(div().w(right_spacer).h_full().flex_shrink_0())
                                        })
                                        .child(self.delegate.render_last_empty_col(window, cx))
                                    } else {
                                        // 分组表头行的单元格数量要少得多（每组一个），
                                        // 因此渲染所有单元格的成本可以忽略不计。
                                        this.children(row_cells.iter().filter_map(|cell| {
                                            if cell.start_leaf_col_ix >= left_columns_count {
                                                if cell.is_leaf {
                                                    if let Some(ix) = cell.leaf_col_ix {
                                                        return Some(
                                                            self.render_th(ix, window, cx)
                                                                .into_any_element(),
                                                        );
                                                    }
                                                } else {
                                                    return Some(
                                                        self.delegate_mut()
                                                            .render_group_th(
                                                                &cell.label,
                                                                cell.col_span,
                                                                cell.width,
                                                                window,
                                                                cx,
                                                            )
                                                            .into_any_element(),
                                                    );
                                                }
                                            }
                                            None
                                        }))
                                        .child(self.delegate.render_last_empty_col(window, cx))
                                    }
                                })
                        }),
                    )),
            )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_table_row(
        &mut self,
        row_ix: usize,
        rows_count: usize,
        left_columns_count: usize,
        col_sizes: Rc<Vec<Size<Pixels>>>,
        columns_count: usize,
        is_filled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let horizontal_scroll_handle = self.horizontal_scroll_handle.clone();
        let is_stripe_row = self.options.stripe && row_ix % 2 != 0;
        let is_selected = self.selected_row == Some(row_ix);
        let view = cx.entity();
        let row_height = self.options.size.table_row_height();

        if row_ix < rows_count {
            let is_last_row = row_ix + 1 == rows_count;
            let need_render_border = is_selected || !is_last_row || !is_filled;

            let mut tr = self.delegate.render_tr(row_ix, window, cx);
            let style = tr.style().clone();

            tr.h_flex()
                .w_full()
                .h(row_height)
                .when(need_render_border, |this| {
                    this.border_b_1().border_color(cx.theme().table_row_border)
                })
                .when(is_stripe_row, |this| this.bg(cx.theme().tokens.table_even))
                .refine_style(&style)
                .hover(|this| {
                    if is_selected || self.right_clicked_row == Some(row_ix) {
                        this
                    } else {
                        this.bg(cx.theme().tokens.table_hover)
                    }
                })
                .when(self.cell_selectable && self.row_header, |this| {
                    this.child(self.render_row_header_cell(row_ix, false, cx))
                })
                .when(left_columns_count > 0, |this| {
                    // 左侧固定列
                    this.child(
                        h_flex()
                            .relative()
                            .h_full()
                            .children({
                                let mut items = Vec::with_capacity(left_columns_count);

                                (0..left_columns_count).for_each(|col_ix| {
                                    let is_cell_selected = self.selected_cell
                                        == Some((row_ix, col_ix))
                                        && self.selection_mode.is_cell();
                                    let is_cell_right_clicked =
                                        self.right_clicked_cell == Some((row_ix, col_ix));

                                    items.push(
                                        self.render_col_wrap(Some(row_ix), col_ix, window, cx)
                                            .child(
                                                self.render_cell(Some(row_ix), col_ix, window, cx)
                                                    .id(format!("table-cell:{}:{}", row_ix, col_ix))
                                                    .relative()
                                                    .child(self.measure_render_td(
                                                        row_ix, col_ix, window, cx,
                                                    ))
                                                    .when(is_cell_selected, |this| {
                                                        this.child(
                                                            div()
                                                                .absolute()
                                                                .inset_0()
                                                                .bg(cx.theme().tokens.table_active)
                                                                .border_1()
                                                                .border_color(
                                                                    cx.theme().table_active_border,
                                                                ),
                                                        )
                                                    })
                                                    .when(
                                                        is_cell_right_clicked && !is_cell_selected,
                                                        |this| {
                                                            this.child(
                                                                div()
                                                                    .absolute()
                                                                    .inset_0()
                                                                    .border_1()
                                                                    .border_color(
                                                                        cx.theme()
                                                                            .table_active_border
                                                                            .opacity(0.5),
                                                                    ),
                                                            )
                                                        },
                                                    )
                                                    .when(self.cell_selectable, |this| {
                                                        this.on_click(cx.listener(
                                                            move |table, e, window, cx| {
                                                                table.on_cell_click(
                                                                    e, row_ix, col_ix, window, cx,
                                                                );
                                                            },
                                                        ))
                                                        .on_mouse_down(
                                                            MouseButton::Right,
                                                            cx.listener(
                                                                move |table, e, window, cx| {
                                                                    table.on_cell_right_click(
                                                                        e, row_ix, col_ix, window,
                                                                        cx,
                                                                    );
                                                                },
                                                            ),
                                                        )
                                                    }),
                                            ),
                                    );
                                });

                                items
                            })
                            .child(
                                // 固定列边框
                                div()
                                    .absolute()
                                    .top_0()
                                    .right_0()
                                    .bottom_0()
                                    .w_0()
                                    .flex_shrink_0()
                                    .border_r_1()
                                    .border_color(cx.theme().border),
                            ),
                    )
                })
                .child(
                    h_flex()
                        .flex_1()
                        .h_full()
                        .overflow_hidden()
                        .relative()
                        .child(
                            crate::h_virtual_list(view, row_ix, col_sizes, {
                                move |table, visible_range: Range<usize>, window, cx| {
                                    table.update_visible_range_if_need(
                                        visible_range.clone(),
                                        Axis::Horizontal,
                                        window,
                                        cx,
                                    );

                                    let mut items =
                                        Vec::with_capacity(visible_range.end - visible_range.start);

                                    visible_range.for_each(|col_ix| {
                                        let col_ix = col_ix + left_columns_count;
                                        let is_cell_selected = table.selected_cell
                                            == Some((row_ix, col_ix))
                                            && table.selection_mode.is_cell();
                                        let is_cell_right_clicked =
                                            table.right_clicked_cell == Some((row_ix, col_ix));

                                        let el = table
                                            .render_col_wrap(Some(row_ix), col_ix, window, cx)
                                            .child(
                                                table
                                                    .render_cell(Some(row_ix), col_ix, window, cx)
                                                    .id(format!("table-cell-{}:{}", row_ix, col_ix))
                                                    .relative()
                                                    .child(table.measure_render_td(
                                                        row_ix, col_ix, window, cx,
                                                    ))
                                                    .when(is_cell_selected, |this| {
                                                        this.child(
                                                            div()
                                                                .absolute()
                                                                .inset_0()
                                                                .bg(cx.theme().tokens.table_active)
                                                                .border_1()
                                                                .border_color(
                                                                    cx.theme().table_active_border,
                                                                ),
                                                        )
                                                    })
                                                    .when(
                                                        is_cell_right_clicked && !is_cell_selected,
                                                        |this| {
                                                            this.child(
                                                                div()
                                                                    .absolute()
                                                                    .inset_0()
                                                                    .border_1()
                                                                    .border_color(
                                                                        cx.theme()
                                                                            .table_active_border
                                                                            .opacity(0.5),
                                                                    ),
                                                            )
                                                        },
                                                    )
                                                    .when(table.cell_selectable, |this| {
                                                        this.on_click(cx.listener(
                                                            move |table, e, window, cx| {
                                                                cx.stop_propagation();
                                                                table.on_cell_click(
                                                                    e, row_ix, col_ix, window, cx,
                                                                );
                                                            },
                                                        ))
                                                        .on_mouse_down(
                                                            MouseButton::Right,
                                                            cx.listener(
                                                                move |table, e, window, cx| {
                                                                    table.on_cell_right_click(
                                                                        e, row_ix, col_ix, window,
                                                                        cx,
                                                                    );
                                                                },
                                                            ),
                                                        )
                                                    }),
                                            );

                                        items.push(el);
                                    });

                                    items
                                }
                            })
                            .with_scroll_handle(&self.horizontal_scroll_handle),
                        )
                        .child(self.delegate.render_last_empty_col(window, cx)),
                )
                // 行选中样式
                // 注意：如果选中了单元格，则不显示行选中样式
                .when_some(self.selected_row, |this, _| {
                    this.when(is_selected && self.selection_mode.is_row(), |this| {
                        this.map(|this| {
                            if cx.theme().list.active_highlight {
                                this.border_color(transparent_white()).child(
                                    div()
                                        .top(if row_ix == 0 { px(0.) } else { px(-1.) })
                                        .left(px(0.))
                                        .right(px(0.))
                                        .bottom(px(-1.))
                                        .absolute()
                                        .bg(cx.theme().tokens.table_active)
                                        .border_1()
                                        .border_color(cx.theme().table_active_border),
                                )
                            } else {
                                this.bg(cx.theme().tokens.accent)
                            }
                        })
                    })
                })
                // 行右键点击样式
                .when(self.right_clicked_row == Some(row_ix), |this| {
                    this.border_color(transparent_white()).child(
                        div()
                            .top(if row_ix == 0 { px(0.) } else { px(-1.) })
                            .left(px(0.))
                            .right(px(0.))
                            .bottom(px(-1.))
                            .absolute()
                            .border_1()
                            .border_color(cx.theme().selection),
                    )
                })
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, e, window, cx| {
                        this.on_row_right_click(e, Some(row_ix), window, cx);
                    }),
                )
                .on_click(cx.listener(move |this, e, window, cx| {
                    this.on_row_left_click(e, row_ix, window, cx);
                }))
        } else {
            // 渲染填充剩余表格空间的假行
            self.delegate
                .render_tr(row_ix, window, cx)
                .h_flex()
                .w_full()
                .h(row_height)
                .border_b_1()
                .border_color(cx.theme().table_row_border)
                .when(is_stripe_row, |this| this.bg(cx.theme().tokens.table_even))
                .when(self.cell_selectable && self.row_header, |this| {
                    // 为假行渲染空的行头单元格
                    this.child(
                        div()
                            .w(px(40.))
                            .h_full()
                            .flex_shrink_0()
                            .table_cell_size(self.options.size),
                    )
                })
                .children((0..columns_count).map(|col_ix| {
                    h_flex()
                        .left(horizontal_scroll_handle.offset().x)
                        .child(self.render_cell(None, col_ix, window, cx))
                }))
                .child(self.delegate.render_last_empty_col(window, cx))
        }
    }

    /// 当 `stripe` 为 true 时，计算填充表格空白区域所需的额外行数。
    fn calculate_extra_rows_needed(
        &self,
        total_height: Pixels,
        actual_height: Pixels,
        row_height: Pixels,
    ) -> usize {
        let mut extra_rows_needed = 0;

        let remaining_height = total_height - actual_height;
        if remaining_height > px(0.) {
            extra_rows_needed = (remaining_height / row_height).floor() as usize;
        }

        extra_rows_needed
    }

    #[inline]
    fn measure_render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if !measure_enable() {
            return self
                .delegate
                .render_td(row_ix, col_ix, window, cx)
                .into_any_element();
        }

        let start = std::time::Instant::now();
        let el = self.delegate.render_td(row_ix, col_ix, window, cx);
        self._measure.push(start.elapsed());
        el.into_any_element()
    }

    fn measure(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        if !measure_enable() {
            return;
        }

        // 打印每个 td 的平均测量时间
        if self._measure.len() > 0 {
            let total = self
                ._measure
                .iter()
                .fold(Duration::default(), |acc, d| acc + *d);
            let avg = total / self._measure.len() as u32;
            eprintln!(
                "last render {} cells total: {:?}, avg: {:?}",
                self._measure.len(),
                total,
                avg,
            );
        }
        self._measure.clear();
    }

    fn render_vertical_scrollbar(
        &mut self,

        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let header_rows = self.header_layout.len().max(1);
        Some(
            div()
                .absolute()
                .top(self.options.size.table_row_height() * header_rows as f32)
                .right_0()
                .bottom_0()
                .w(Scrollbar::width())
                .child(Scrollbar::vertical(&self.vertical_scroll_handle).max_fps(60)),
        )
    }

    fn render_horizontal_scrollbar(
        &mut self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .absolute()
            .left(self.fixed_head_cols_bounds.size.width)
            .right_0()
            .bottom_0()
            .h(Scrollbar::width())
            .child(Scrollbar::horizontal(&self.horizontal_scroll_handle))
    }
}

impl<D> Focusable for TableState<D>
where
    D: TableDelegate,
{
    fn focus_handle(&self, _cx: &crate::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
impl<D> EventEmitter<TableEvent> for TableState<D> where D: TableDelegate {}

impl<D> Render for TableState<D>
where
    D: TableDelegate,
{
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.measure(window, cx);

        let columns_count = self.delegate.columns_count(cx);
        let left_columns_count = self
            .col_groups
            .iter()
            .filter(|col| self.col_fixed && col.column.fixed == Some(ColumnFixed::Left))
            .count();
        let rows_count = self.delegate.rows_count(cx);
        let loading = self.delegate.loading(cx);

        let row_height = self.options.size.table_row_height();
        let total_height = self
            .vertical_scroll_handle
            .0
            .borrow()
            .base_handle
            .bounds()
            .size
            .height;
        let actual_height = row_height * rows_count as f32;
        let extra_rows_count =
            self.calculate_extra_rows_needed(total_height, actual_height, row_height);
        let render_rows_count = if self.options.stripe {
            rows_count + extra_rows_count
        } else {
            rows_count
        };
        let right_clicked_row = self.right_clicked_row;
        let is_filled = total_height > Pixels::ZERO && total_height <= actual_height;

        let loading_view = if loading {
            Some(
                self.delegate
                    .render_loading(self.options.size, window, cx)
                    .into_any_element(),
            )
        } else {
            None
        };

        let empty_view = if rows_count == 0 {
            Some(
                div()
                    .size_full()
                    .child(self.delegate.render_empty(window, cx))
                    .into_any_element(),
            )
        } else {
            None
        };

        let inner_table = v_flex()
            .id("table-inner")
            .size_full()
            .overflow_hidden()
            .child(self.render_table_header(left_columns_count, window, cx))
            .context_menu({
                let view = cx.entity();
                move |this, window: &mut Window, cx: &mut Context<PopupMenu>| {
                    if let Some(row_ix) = view.read(cx).right_clicked_row {
                        view.update(cx, |menu, cx| {
                            menu.delegate_mut().context_menu(row_ix, this, window, cx)
                        })
                    } else {
                        this
                    }
                }
            })
            .map(|this| {
                if rows_count == 0 {
                    this.children(empty_view)
                } else {
                    this.child(
                        h_flex().id("table-body").flex_grow_1().size_full().child(
                            uniform_list(
                                "table-uniform-list",
                                render_rows_count,
                                cx.processor(
                                    move |table, visible_range: Range<usize>, window, cx| {
                                        // 使用 `col.width`（始终最新）而不是
                                        // `col.bounds.size.width`，后者仅在
                                        // prepaint 后设置，因此首帧为零。
                                        let col_sizes: Rc<Vec<Size<Pixels>>> = Rc::new(
                                            table
                                                .col_groups
                                                .iter()
                                                .skip(left_columns_count)
                                                .map(|col| Size {
                                                    width: col.width,
                                                    height: px(0.),
                                                })
                                                .collect(),
                                        );

                                        table.load_more_if_need(
                                            rows_count,
                                            visible_range.end,
                                            window,
                                            cx,
                                        );
                                        table.update_visible_range_if_need(
                                            visible_range.clone(),
                                            Axis::Vertical,
                                            window,
                                            cx,
                                        );

                                        if visible_range.end > rows_count {
                                            table.scroll_to_row(
                                                std::cmp::min(
                                                    visible_range.start,
                                                    rows_count.saturating_sub(1),
                                                ),
                                                cx,
                                            );
                                        }

                                        let mut items = Vec::with_capacity(
                                            visible_range.end.saturating_sub(visible_range.start),
                                        );

                                        // 渲染填充表格的假行
                                        visible_range.for_each(|row_ix| {
                                            // 为可用数据渲染真实行
                                            items.push(table.render_table_row(
                                                row_ix,
                                                rows_count,
                                                left_columns_count,
                                                col_sizes.clone(),
                                                columns_count,
                                                is_filled,
                                                window,
                                                cx,
                                            ));
                                        });

                                        items
                                    },
                                ),
                            )
                            .flex_grow_1()
                            .size_full()
                            .with_sizing_behavior(ListSizingBehavior::Auto)
                            .track_scroll(&self.vertical_scroll_handle)
                            .into_any_element(),
                        ),
                    )
                }
            });

        div()
            .size_full()
            .children(loading_view)
            .when(!loading, |this| {
                this.child(inner_table)
                    .child(ScrollableMask::new(
                        Axis::Horizontal,
                        &self.horizontal_scroll_handle,
                    ))
                    // 防止垂直滚轮滚动泄漏到祖先滚动容器中。
                    // 表格为空时跳过：此时不渲染 `uniform_list`，
                    // 因此句柄的偏移量和 `max_offset` 已过期。
                    .when(rows_count > 0, |this| {
                        this.child(ScrollableMask::new(
                            Axis::Vertical,
                            &self.vertical_scroll_handle.0.borrow().base_handle,
                        ))
                    })
                    .when(right_clicked_row.is_some(), |this| {
                        this.on_mouse_down_out(cx.listener(|this, e, window, cx| {
                            this.on_row_right_click(e, None, window, cx);
                            cx.notify();
                        }))
                    })
            })
            .on_prepaint({
                let state = cx.entity();
                move |bounds, _, cx| state.update(cx, |state, _| state.bounds = bounds)
            })
            .when(!window.is_inspector_picking(cx), |this| {
                this.child(
                    div()
                        .absolute()
                        .top_0()
                        .size_full()
                        .when(self.options.scrollbar_visible.bottom, |this| {
                            this.child(self.render_horizontal_scrollbar(window, cx))
                        })
                        .when(
                            self.options.scrollbar_visible.right && rows_count > 0,
                            |this| this.children(self.render_vertical_scrollbar(window, cx)),
                        ),
                )
            })
    }
}
