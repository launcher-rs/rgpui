# 列表 / 表格 / 标签页（List/Table/Tabs）

**目录：** [概述](#概述) · [命名陷阱](#命名陷阱) · [List（委托式）](#list委托式) · [Table（委托式）](#table委托式) · [Table（静态结构）](#table静态结构) · [TabBar/Tab](#tabbartab) · [Tabs/Accordion/Collapsible](#tabsaccordioncollapsible) · [tab_drag](#tab_drag) · [最佳实践](#最佳实践)

## 概述

三子系统位于 `crates/rgpui/src/list|table|tabs/`，分别为 `pub mod list;`（`rgpui.rs:231`）、
`pub mod table;`（`:234`）、`pub mod tabs;`（`:237`），路径 `rgpui::list::*` /
`rgpui::table::*` / `rgpui::tabs::*`。**三者都不在 prelude**（prelude 仅导出
`tabs::tab_drag` 四件套，`prelude.rs:67`），**根也无 `pub use list/table/tabs::*`**。

初始化：`list::init(cx)`（`list/list.rs:19`）、`table::init(cx)`（`table/mod.rs:35`），
由统一 `init.rs` 调用。

## 命名陷阱

| 名称 | 含义 |
|------|------|
| `rgpui::List` / `rgpui::ListState` | **旧式**列表（`elements/list.rs:37`/`:54`，来自 `elements::*`） |
| `rgpui::list::List<D>` / `rgpui::list::ListState<D>` | **委托式**列表（`list/list.rs:702`/`:64`） |
| `rgpui::table::Table` | **静态结构**表格（`table/table.rs:37`，无状态） |
| `rgpui::table::DataTable<D>` / `TableState<D>` | **委托式**数据表格（`table/data_table.rs:91`/`table/state.rs:188`） |

示例：`cargo run -p showcase --bin list` / `--bin table` / `--bin tabs`。

## List（委托式）

### ListDelegate

`trait ListDelegate`（`list/delegate.rs:10`）：`type Item: Selectable + IntoElement`。

**必填：**
- `items_count(&self, section: usize, cx) -> usize`（`:35`）
- `render_item(&mut self, ix: IndexPath, window, cx) -> Option<Self::Item>`（`:42`）
- `set_selected_index(&mut self, Option<IndexPath>, window, cx)`（`:117`）

**可选（节/搜索）：** `perform_search`（`:15`）、`sections_count`（`:27`，默认 1）、
`render_section_header`（`:52`）、`render_section_footer`（`:64`）。
**可选（渲染）：** `render_empty`（`:74`）、`render_initial`（`:94`）、
`loading`（`:103`）、`render_loading`（`:108`）。
**可选（交互/加载）：** `set_right_clicked_index`（`:125`）、`confirm(secondary,...)`（`:136`）、
`cancel`（`:140`）、`has_more`（`:145`）、`load_more_threshold`（`:155`，默认 20）、
`load_more`（`:165`）。

### ListState / List

`ListState<D>`（`list/list.rs:64`）：`new(delegate, window, cx)`（`:89`）、
`searchable(bool)`（`:119`）/`set_searchable`（`:125`）、`selectable(bool)`（`:131`）/`set_selectable`（`:137`）、
`delegate()`（`:143`）/`delegate_mut()`（`:148`）、`focus(window, cx)`（`:153`）、
`is_focused(window, cx)`（`:158`）、`set_selected_index`（`:179`）、`selected_index()`（`:190`）、
`set_right_clicked_index`（`:195`）、`set_query(&str, window, cx)`（`:211`）、
`set_item_to_measure_index`（`:222`）、`scroll_to_item`（`:233`）、`scroll_handle()`（`:253`）、
`scroll_to_selected_item`（`:258`）、`reset_on_cancel(bool)`（`:346`）。

`List<D>`（`:702`）：`new(&Entity<ListState<D>>)`（`:713`）、`scrollbar_visible(bool)`（`:722`）、
`search_placeholder(impl Into<SharedString>)`（`:728`），`impl Styled`/`Sizable`。

`ListEvent`（`:32`）：`Select(IndexPath)` / `Confirm(IndexPath)` / `Cancel`。
`ListItem`（`list_item.rs:27`）：`new(id)`（`:46`）、`separator()`（`:66`）、
`check_icon`（`:72`）、`selected(bool)`（`:78`）、`confirmed(bool)`（`:84`）、
`disabled(bool)`（`:90`）、`suffix(builder)`（`:96`）、`on_click`（`:108`）、
`on_mouse_down`（`:117`）、`on_mouse_enter`（`:127`）。`SeparatorItem`（`separator_item.rs`）。

```rust
struct MyDelegate { items: Vec<String> }
impl ListDelegate for MyDelegate {
    type Item = ListItem;
    fn items_count(&self, _s: usize, _cx: &App) -> usize { self.items.len() }
    fn render_item(&mut self, ix: IndexPath, _w: &mut Window, cx: &mut Context<ListState<Self>>) -> Option<ListItem> {
        let name = self.items.get(ix.row)?.clone();
        Some(ListItem::new(ix.row).child(name).on_click(cx.listener(|_, _, _, _| {})))
    }
    fn set_selected_index(&mut self, _ix: Option<IndexPath>, _w: &mut Window, _cx: &mut Context<ListState<Self>>) {}
}
let state = cx.new(|cx| ListState::new(MyDelegate { items }, window, cx));
// 渲染：List::new(&state)
```

## Table（委托式）

### TableDelegate

`trait TableDelegate`（`table/delegate.rs:15`）：

**必填：**
- `columns_count(&self, cx) -> usize`（`:17`）
- `rows_count(&self, cx) -> usize`（`:20`）
- `column(&self, col_ix, cx) -> Column`（`:25`）
- `render_td(&mut self, row_ix, col_ix, window, cx) -> impl IntoElement`（`:109`）

**可选（渲染）：** `render_header`（`:38`）、`render_th`（`:76`）、`render_tr`（`:88`）、
`render_empty`（`:128`）、`render_loading(size,...)`（`:149`）、`render_last_empty_col`（`:183`）。
**可选（分组/排序）：** `group_headers`（`:49`）、`render_group_th`（`:55`）、
`perform_sort`（`:28`）、`context_menu`（`:98`）、`move_column`（`:118`）。
**可选（加载/通知）：** `loading`（`:142`）、`has_more`（`:161`）、`load_more_threshold`（`:170`，默认 20）、
`load_more`（`:180`）、`visible_rows_changed`（`:197`）、`visible_columns_changed`（`:211`）、
`cell_text`（`:223`，导出用）。

### TableState / DataTable

`TableState<D>`（`table/state.rs:188`）：`new(delegate, window, cx)`（`:265`）、
`delegate()`/`delegate_mut()`（`:303`/`:308`）、`loop_selection(bool)`（`:313`）、
`col_movable`（`:319`）、`col_resizable`（`:325`）、`sortable`（`:331`）、
`row_selectable`（`:337`）、`col_selectable`（`:343`）、`cell_selectable`（`:365`）、
`row_header`（`:378`）、`refresh(cx)`（`:384`）、`scroll_to_row`/`scroll_to_col`（`:389`/`:396`）、
`selected_row()`/`set_selected_row`（`:405`/`:410`）、`right_clicked_row`/`set_right_clicked_row`（`:436`/`:444`）、
`selected_col`/`set_selected_col`（`:450`/`:455`）、`selected_cell`/`set_selected_cell`（`:476`/`:495`）、
`clear_selection`（`:509`）、`visible_range()`（`:521`）、`dump(cx)`（`:528`）、
`refresh_header_layout(cx)`（`:554`）。

`DataTable<D>`（`table/data_table.rs:91`）：`new(&Entity<TableState<D>>)`（`:101`）、
`stripe(bool)`（`:109`）、`bordered(bool)`（`:115`）、`scrollbar_visible(v, h)`（`:121`）。

`Column`（`table/column.rs:10`）：`new(key, name)`（`:93`）、`sort(ColumnSort)`（`:104`）、
`sortable()`（`:112`）、`ascending`/`descending`（`:118`/`:124`）、`text_center`/`text_right`（`:130`/`:138`）、
`paddings`/`p_0`（`:144`/`:150`）、`width`（`:156`）、`fixed(ColumnFixed)`/`fixed_left`（`:162`/`:168`）、
`resizable(bool)`（`:174`）、`movable(bool)`（`:180`）、`selectable(bool)`（`:200`）、
`min_width`/`max_width`（`:206`/`:218`）。`ColumnGroup`（`:53`）：`new(label, span)`（`:64`）。
`ColumnSort`（`:267`）：`Default`/`Ascending`/`Descending`。
`ColumnFixed`（`:234`）：`Left`。
`TableEvent`（`table/state.rs:58`）：`SelectRow(usize)`、`DoubleClickedRow(usize)`、
`SelectColumn(usize)`、`SelectCell(usize, usize)`、`DoubleClickedCell(usize, usize)`、
`ColumnWidthsChanged(Vec<Pixels>)`、`MoveColumn(usize, usize)`、`RightClickedRow(Option<usize>)`、
`RightClickedCell(usize, usize)` / `ClearSelection` / `LoadMore` 等（见源码）。

`TableVisibleRange`（`:109`）：`rows()`（`:118`）、`cols()`（`:123`）。

## Table（静态结构）

无需委托，像 HTML 表格一样拼装（均在 `table/table.rs`）：
`Table::new()`（`:37`）、`TableHeader::new()`（`:112`）、`TableBody::new()`（`:188`）、
`TableFooter::new()`（`:260`）、`TableRow::new()`（`:336`）、`TableHead::new()`（`:412`）、
`TableCell::new()`（`:505`）、`TableCaption::new()`（`:598`）。
`TableHead`/`TableCell`：`col_span(usize)`（`:435`/`:528`）、`text_center()`（`:441`/`:534`）、
`text_right()`（`:447`/`:540`）。

## TabBar/Tab

`TabBar`（`tabs/tab_bar.rs:40`）：`new(id)`（`:58`）、变体 `with_variant(TabVariant)`（`:78`）、
`pill()`（`:84`）、`outline()`（`:90`）、`segmented()`（`:96`）、`underline()`（`:102`）、
`menu(bool)`（`:108`）、`track_scroll(&ScrollHandle)`（`:114`）、`prefix`/`suffix`（`:120`/`:126`）、
`child(impl Into<Tab>)`（`:138`）/`children`（`:132`）、`selected_index(usize)`（`:144`）、
`last_empty_space`（`:150`）、`on_change`（`:158`）。

`Tab`（`tabs/tab.rs:411`）：`new()`（`:487`）、`label`（`:492`）、`icon`（`:498`）、
`with_variant`（`:504`）/`pill`/`outline`/`segmented`/`underline`（`:510`-`:528`）、
`prefix`/`suffix`（`:534`/`:540`）、`disabled`（`:546`）、`on_click`（`:552`）。
`impl From<&str/String/SharedString/Icon/IconName>`（`:432`-`:456`）。
`TabVariant`（`:16`）：`Tab`(默认)/`Outline`/`Pill`/`Segmented`/`Underline`。

## Tabs/Accordion/Collapsible

**Tabs**（`tabs/tabs.rs:62`）：`new(Vec<TabsItem>)`（`:71`）、`active(Option<SharedString>)`（`:81`）、
`on_change`（`:87`）。`TabsItem`（`:25`）：`new(id, label)`（`:38`）、`icon(IconName)`（`:48`）、
`disabled(bool)`（`:54`）。

**Accordion**（`tabs/accordion.rs:24`）：`new(id)`（`:37`）、`multiple(bool)`（`:51`）、
`bordered(bool)`（`:57`）、`disabled(bool)`（`:63`）、`item(child)`（`:69`）、
`on_toggle_click`（`:81`）。`AccordionItem`（`:353`）：`new()`（`:371`）、`icon`（`:390`）、
`title`（`:396`）、`open(bool)`（`:402`）、`disabled`（`:408`）、`title_style`（`:414`）、
`hover`（`:423`）、`content_style`（`:429`）。

**Collapsible**（`tabs/collapsible.rs:23`）：`new()`（`:31`）、`open(bool)`（`:40`）、
`content(impl IntoElement)`（`:48`）。

## tab_drag

`pub mod tabs::tab_drag`（`tabs/mod.rs:12`），prelude 已导出其四件套
（`prelude.rs:67`）：`TabDragDrop` / `TabDragEvent` / `TabDragState` / `TabItem`。

## 最佳实践

- **别混淆旧式与新式**：需要委托/虚拟化用 `rgpui::list::List`/`DataTable`；
  `rgpui::List`/`rgpui::ListState` 是旧式。
- **三者都要显式 import**（不在 prelude，根也不 glob 它们）。
- **大列表/大表格必须用委托式**（虚拟化只渲染可见项）。
- **静态表格**（`table::Table`）适合小规模固定内容，直接拼 HTML 风格结构。
- **`TableDelegate::cell_text` 用于导出**（如 CSV），默认空字符串。
- **`visible_rows_changed`/`visible_columns_changed` 会被频繁调用**，保持轻量。
