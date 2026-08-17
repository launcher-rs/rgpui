use std::ops::Range;

use crate::{
    App, Context, Div, InteractiveElement as _, IntoElement, ParentElement as _, Pixels,
    SharedString, Stateful, Styled as _, Window, div,
};

use crate::{
    ActiveTheme as _, ElementSize, Icon, IconName, h_flex,
    menu::PopupMenu,
    table::{Column, ColumnGroup, ColumnSort, TableState, loading::Loading},
};

/// 为表格提供数据和渲染的代理 trait。
pub trait TableDelegate: Sized + 'static {
    /// 返回表格中的列数。
    fn columns_count(&self, cx: &App) -> usize;

    /// 返回表格中的行数。
    fn rows_count(&self, cx: &App) -> usize;

    /// 返回给定索引处的表格列。
    ///
    /// 仅在 Table 准备或刷新时调用。
    fn column(&self, col_ix: usize, cx: &App) -> Column;

    /// 对给定索引处的列执行排序。
    fn perform_sort(
        &mut self,
        _col_ix: usize,
        _sort: ColumnSort,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) {
    }

    /// 渲染表格头部行。
    fn render_header(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) -> Stateful<Div> {
        div().id("header")
    }

    /// 返回分组表头定义（可以是多级）。
    ///
    /// 默认返回 None，表示没有分组表头。
    fn group_headers(&self, _cx: &App) -> Option<Vec<Vec<ColumnGroup>>> {
        None
    }

    /// 自定义渲染分组表头单元格。
    /// 接收组标签、逻辑 col_span 与像素宽度。
    fn render_group_th(
        &mut self,
        label: &SharedString,
        _col_span: usize,
        width: Pixels,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        div()
            .w(width)
            .h_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(label.clone())
    }

    /// 渲染给定列索引处的表头单元格，默认为列名。
    fn render_th(
        &mut self,
        col_ix: usize,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        div().size_full().child(self.column(col_ix, cx).name)
    }

    /// 渲染给定行和列处的行。
    ///
    /// 不包括表格头部行。
    fn render_tr(
        &mut self,
        row_ix: usize,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) -> Stateful<Div> {
        div().id(("row", row_ix))
    }

    /// 为给定行索引渲染右键上下文菜单。
    fn context_menu(
        &mut self,
        _row_ix: usize,
        menu: PopupMenu,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) -> PopupMenu {
        menu
    }

    /// 渲染给定行和列处的单元格。
    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement;

    /// 将给定 `col_ix` 处的列移动到给定 `to_ix` 之前。
    fn move_column(
        &mut self,
        _col_ix: usize,
        _to_ix: usize,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) {
    }

    /// 返回一个在表格为空时显示的元素。
    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        h_flex()
            .size_full()
            .justify_center()
            .text_color(cx.theme().muted_foreground.opacity(0.6))
            .child(Icon::new(IconName::Inbox).size_12())
            .into_any_element()
    }

    /// 返回 true 以显示加载视图。
    fn loading(&self, _cx: &App) -> bool {
        false
    }

    /// 返回一个在表格加载时显示的元素，默认使用内置的 Skeleton 加载视图。
    ///
    /// size 是表格的尺寸。
    fn render_loading(
        &mut self,
        size: ElementSize,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        Loading::new().size(size)
    }

    /// 返回 true 以在滚动到底部时启用加载更多数据。
    ///
    /// 默认：false
    fn has_more(&self, _cx: &App) -> bool {
        false
    }

    /// 返回一个阈值（n 行），当然滚动到底部时，
    /// 剩余行数触发 `load_more`。
    /// 该值应小于首次加载的总行数。
    ///
    /// 默认：20 行
    fn load_more_threshold(&self) -> usize {
        20
    }

    /// 当表格滚动到底部时加载更多数据。
    ///
    /// 这将在后台任务中执行。
    ///
    /// 这总是在表格接近底部时被调用，
    /// 所以你必须检查是否还有更多数据或锁定加载状态。
    fn load_more(&mut self, _window: &mut Window, _cx: &mut Context<TableState<Self>>) {}

    /// 渲染最后一个空列，默认为空。
    fn render_last_empty_col(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        h_flex().w_3().h_full().flex_shrink_0()
    }

    /// 当行的可见范围改变时被调用。
    ///
    /// 注意：确保此方法足够快，因为它会被频繁调用。
    ///
    /// 可用于处理数据更新，仅更新可见行。
    /// 请确保数据在后台任务中更新。
    fn visible_rows_changed(
        &mut self,
        _visible_range: Range<usize>,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) {
    }

    /// 当列的可见范围改变时被调用。
    ///
    /// 注意：确保此方法足够快，因为它会被频繁调用。
    ///
    /// 可用于处理数据更新，仅更新可见列。
    /// 请确保数据在后台任务中更新。
    fn visible_columns_changed(
        &mut self,
        _visible_range: Range<usize>,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) {
    }

    /// 获取单元格的文本表示，用于导出（如 CSV 导出）。
    ///
    /// 默认返回空字符串。实现此方法以支持导出。
    /// 文本应按导出数据中应有的格式格式化。
    fn cell_text(&self, _row_ix: usize, _col_ix: usize, _cx: &App) -> String {
        String::new()
    }
}
