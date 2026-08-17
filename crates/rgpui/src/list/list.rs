use std::ops::Range;

use crate::input_ui::{Input, InputEvent, InputState};
use crate::list::cache::{MeasuredEntrySize, RowEntry, RowsCache};
use crate::list::delegate::ListDelegate;
use crate::menu::{Cancel, Confirm, SelectDown, SelectUp};
use crate::prelude::FluentBuilder as _;
use crate::{
    ActiveTheme, App, AppContext, AvailableSpace, ClickEvent, Context, DefiniteLength,
    EdgesRefinement, ElementSize, Entity, EventEmitter, FocusHandle, Focusable, Icon, IconName,
    IndexPath, InteractiveElement as _, IntoElement, KeyBinding, Length, ListSizingBehavior,
    MouseButton, ParentElement as _, Render, RenderOnce, Role, ScrollStrategy, Scrollbar,
    Selectable, SharedString, Sizable, StatefulInteractiveElement as _, StyleRefinement, Styled,
    StyledExt, Subscription, Task, VirtualListScrollHandle, Window, div, px, size, v_flex,
    v_virtual_list,
};

/// 初始化 List 的全局快捷键绑定
pub fn init(cx: &mut App) {
    let context: Option<&str> = Some("List");
    cx.bind_keys([
        KeyBinding::new("escape", Cancel, context),
        KeyBinding::new("enter", Confirm { secondary: false }, context),
        KeyBinding::new("secondary-enter", Confirm { secondary: true }, context),
        KeyBinding::new("up", SelectUp, context),
        KeyBinding::new("down", SelectDown, context),
    ]);
}

/// 列表事件。
#[derive(Clone)]
pub enum ListEvent {
    /// 移动以选择条目。
    Select(IndexPath),
    /// 点击条目或按下回车。
    Confirm(IndexPath),
    /// 按下 ESC 取消选择条目。
    Cancel,
}

struct ListOptions {
    size: ElementSize,
    scrollbar_visible: bool,
    search_placeholder: Option<SharedString>,
    max_height: Option<Length>,
    paddings: EdgesRefinement<DefiniteLength>,
}

impl Default for ListOptions {
    fn default() -> Self {
        Self {
            size: ElementSize::default(),
            scrollbar_visible: true,
            max_height: None,
            search_placeholder: None,
            paddings: EdgesRefinement::default(),
        }
    }
}

/// List 的状态。
///
/// List 要求所有条目具有相同的高度。
pub struct ListState<D: ListDelegate> {
    pub(crate) focus_handle: FocusHandle,
    pub(crate) query_input: Entity<InputState>,
    options: ListOptions,
    delegate: D,
    last_query: Option<String>,
    scroll_handle: VirtualListScrollHandle,
    rows_cache: RowsCache,
    selected_index: Option<IndexPath>,
    item_to_measure_index: IndexPath,
    deferred_scroll_to_index: Option<(IndexPath, ScrollStrategy)>,
    mouse_right_clicked_index: Option<IndexPath>,
    reset_on_cancel: bool,
    searchable: bool,
    selectable: bool,
    _search_task: Task<()>,
    _load_more_task: Task<()>,
    _query_input_subscription: Subscription,
}

impl<D> ListState<D>
where
    D: ListDelegate,
{
    /// 创建新的 ListState。
    pub fn new(delegate: D, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search..."));

        let _query_input_subscription =
            cx.subscribe_in(&query_input, window, Self::on_query_input_event);

        Self {
            focus_handle: cx.focus_handle(),
            options: ListOptions::default(),
            delegate,
            rows_cache: RowsCache::default(),
            query_input,
            last_query: None,
            selected_index: None,
            selectable: true,
            searchable: false,
            item_to_measure_index: IndexPath::default(),
            deferred_scroll_to_index: None,
            mouse_right_clicked_index: None,
            scroll_handle: VirtualListScrollHandle::new(),
            reset_on_cancel: true,
            _search_task: Task::ready(()),
            _load_more_task: Task::ready(()),
            _query_input_subscription,
        }
    }

    /// 设置列表是否可搜索，默认为 `false`。
    ///
    /// 当为 `true` 时，列表顶部会显示一个搜索输入框。
    pub fn searchable(mut self, searchable: bool) -> Self {
        self.searchable = searchable;
        self
    }

    /// 设置列表是否可搜索。
    pub fn set_searchable(&mut self, searchable: bool, cx: &mut Context<Self>) {
        self.searchable = searchable;
        cx.notify();
    }

    /// 设置列表是否可选中，默认为 true。
    pub fn selectable(mut self, selectable: bool) -> Self {
        self.selectable = selectable;
        self
    }

    /// 设置列表是否可选中。
    pub fn set_selectable(&mut self, selectable: bool, cx: &mut Context<Self>) {
        self.selectable = selectable;
        cx.notify();
    }

    /// 返回 ListDelegate 的不可变引用。
    pub fn delegate(&self) -> &D {
        &self.delegate
    }

    /// 返回 ListDelegate 的可变引用。
    pub fn delegate_mut(&mut self) -> &mut D {
        &mut self.delegate
    }

    /// 聚焦列表；如果列表可搜索，则聚焦搜索输入框。
    pub fn focus(&mut self, window: &mut Window, cx: &mut App) {
        self.focus_handle(cx).focus(window, cx);
    }

    /// 如果列表或搜索输入框已聚焦则返回 true。
    pub fn is_focused(&self, window: &Window, cx: &App) -> bool {
        self.focus_handle.is_focused(window) || self.query_input.focus_handle(cx).is_focused(window)
    }

    /// 设置列表的选中索引，此方法也会滚动到选中条目。
    pub(crate) fn _set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.selectable {
            return;
        }

        self.selected_index = ix;
        self.delegate.set_selected_index(ix, window, cx);
        self.scroll_to_selected_item(window, cx);
    }

    /// 设置列表的选中索引，此方法不会滚动到选中条目。
    pub fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_index = ix;
        self.delegate.set_selected_index(ix, window, cx);
    }

    /// 返回当前选中的索引。
    pub fn selected_index(&self) -> Option<IndexPath> {
        self.selected_index
    }

    /// 设置被右键点击的条目索引。
    pub fn set_right_clicked_index(
        &mut self,
        ix: Option<IndexPath>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mouse_right_clicked_index = ix;
        self.delegate.set_right_clicked_index(ix, window, cx);
    }

    /// 返回被右键点击的条目索引。
    pub fn right_clicked_index(&self) -> Option<IndexPath> {
        self.mouse_right_clicked_index
    }

    /// 设置搜索输入框的查询文本，这会触发一次搜索。
    pub fn set_query(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        let query = query.to_string();
        self.query_input.update(cx, |input, cx| {
            input.set_value(query.clone(), window, cx);
        });

        // `set_value` 不会发出 `InputEvent::Change`，因此在这里开始搜索。
        self.start_search(query, window, cx);
    }

    /// 设置用于测量的特定列表条目。
    pub fn set_item_to_measure_index(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.item_to_measure_index = ix;
        cx.notify();
    }

    /// 滚动到指定索引的条目。
    pub fn scroll_to_item(
        &mut self,
        ix: IndexPath,
        strategy: ScrollStrategy,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if ix.section == 0 && ix.row == 0 {
            // 如果条目是第一个条目，则滚动到顶部。
            let mut offset = self.scroll_handle.base_handle().offset();
            offset.y = px(0.);
            self.scroll_handle.base_handle().set_offset(offset);
            cx.notify();
            return;
        }
        self.deferred_scroll_to_index = Some((ix, strategy));
        cx.notify();
    }

    /// 获取滚动句柄。
    pub fn scroll_handle(&self) -> &VirtualListScrollHandle {
        &self.scroll_handle
    }

    /// 滚动到选中条目。
    pub fn scroll_to_selected_item(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.selected_index {
            self.deferred_scroll_to_index = Some((ix, ScrollStrategy::Top));
            cx.notify();
        }
    }

    fn on_query_input_event(
        &mut self,
        state: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => {
                let text = state.read(cx).value();
                let text = text.trim().to_string();
                if Some(&text) == self.last_query.as_ref() {
                    return;
                }

                self.start_search(text, window, cx);
            }
            _ => {}
        }
    }

    fn start_search(&mut self, query: String, window: &mut Window, cx: &mut Context<Self>) {
        self.set_searching(true, window, cx);
        let search = self.delegate.perform_search(&query, window, cx);

        if self.rows_cache.len() > 0 {
            self._set_selected_index(Some(IndexPath::default()), window, cx);
        } else {
            self._set_selected_index(None, window, cx);
        }

        self._search_task = cx.spawn_in(window, async move |this, window| {
            search.await;

            _ = this.update_in(window, |this, _, _| {
                this.scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
                this.last_query = Some(query);
            });

            // 始终等待 100ms 以避免闪烁
            window
                .background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            _ = this.update_in(window, |this, window, cx| {
                this.set_searching(false, window, cx);
            });
        });
    }

    fn set_searching(&mut self, searching: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.query_input
            .update(cx, |input, cx| input.set_loading(searching, window, cx));
    }

    /// 当可见范围接近末尾时，调度代理的 `load_more` 方法。
    fn load_more_if_need(
        &mut self,
        entities_count: usize,
        visible_end: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // FIXME: 这里需要空节条目的数量。

        let threshold = self.delegate.load_more_threshold();
        // 安全处理减法逻辑，防止溢出
        if visible_end >= entities_count.saturating_sub(threshold) {
            if !self.delegate.has_more(cx) {
                return;
            }

            self._load_more_task = cx.spawn_in(window, async move |view, cx| {
                _ = view.update_in(cx, |view, window, cx| {
                    view.delegate.load_more(window, cx);
                });
            });
        }
    }

    /// 设置取消时是否重置选中项。
    pub fn reset_on_cancel(mut self, reset: bool) -> Self {
        self.reset_on_cancel = reset;
        self
    }

    fn on_action_cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        cx.propagate();
        if self.reset_on_cancel {
            self._set_selected_index(None, window, cx);
        }

        self.delegate.cancel(window, cx);
        cx.emit(ListEvent::Cancel);
        cx.notify();
    }

    fn on_action_confirm(
        &mut self,
        confirm: &Confirm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.rows_cache.len() == 0 {
            return;
        }

        let Some(ix) = self.selected_index else {
            return;
        };

        self.delegate
            .set_selected_index(self.selected_index, window, cx);
        self.delegate.confirm(confirm.secondary, window, cx);
        cx.emit(ListEvent::Confirm(ix));
        cx.notify();
    }

    fn select_item(&mut self, ix: IndexPath, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selectable {
            return;
        }

        self.selected_index = Some(ix);
        self.delegate.set_selected_index(Some(ix), window, cx);
        self.scroll_to_selected_item(window, cx);
        cx.emit(ListEvent::Select(ix));
        cx.notify();
    }

    pub(crate) fn on_action_select_prev(
        &mut self,
        _: &SelectUp,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.rows_cache.len() == 0 {
            return;
        }

        let prev_ix = self.rows_cache.prev(self.selected_index);
        self.select_item(prev_ix, window, cx);
    }

    pub(crate) fn on_action_select_next(
        &mut self,
        _: &SelectDown,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.rows_cache.len() == 0 {
            return;
        }

        let next_ix = self.rows_cache.next(self.selected_index);
        self.select_item(next_ix, window, cx);
    }

    fn prepare_items_if_needed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sections_count = self.delegate.sections_count(cx).max(1);
        let mut measured_size = MeasuredEntrySize::default();

        // 测量条目高度和节头/节尾高度。
        let available_space = size(AvailableSpace::MinContent, AvailableSpace::MinContent);
        measured_size.item_size = self
            .render_list_item(self.item_to_measure_index, window, cx)
            .into_any_element()
            .layout_as_root(available_space, window, cx);

        if let Some(mut el) = self
            .delegate
            .render_section_header(0, window, cx)
            .map(|r| r.into_any_element())
        {
            measured_size.section_header_size = el.layout_as_root(available_space, window, cx);
        }
        if let Some(mut el) = self
            .delegate
            .render_section_footer(0, window, cx)
            .map(|r| r.into_any_element())
        {
            measured_size.section_footer_size = el.layout_as_root(available_space, window, cx);
        }

        self.rows_cache
            .prepare_if_needed(sections_count, measured_size, cx, |section_ix, cx| {
                self.delegate.items_count(section_ix, cx)
            });
    }

    fn render_list_item(
        &mut self,
        ix: IndexPath,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selectable = self.selectable;
        let selected = self.selected_index.map(|s| s.eq_row(ix)).unwrap_or(false);
        let mouse_right_clicked = self
            .mouse_right_clicked_index
            .map(|s| s.eq_row(ix))
            .unwrap_or(false);
        let id = SharedString::from(format!("list-item-{}", ix));

        let total_items = self.rows_cache.items_count();

        div()
            .id(id)
            .role(Role::ListItem)
            .aria_position_in_set(ix.row + 1)
            .aria_size_of_set(total_items)
            .aria_selected(selected)
            .w_full()
            .relative()
            .overflow_hidden()
            .children(self.delegate.render_item(ix, window, cx).map(|item| {
                item.selected(selected)
                    .secondary_selected(mouse_right_clicked)
            }))
            .when(selectable, |this| {
                this.on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                    this.set_right_clicked_index(None, window, cx);
                    this.selected_index = Some(ix);
                    this.on_action_confirm(
                        &Confirm {
                            secondary: e.modifiers().secondary(),
                        },
                        window,
                        cx,
                    );
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, _, window, cx| {
                        this.set_right_clicked_index(Some(ix), window, cx);
                        cx.notify();
                    }),
                )
            })
    }

    fn render_items(
        &mut self,
        items_count: usize,
        entities_count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let rows_cache = self.rows_cache.clone();
        let scrollbar_visible = self.options.scrollbar_visible;
        let scroll_handle = self.scroll_handle.clone();
        let item_to_measure_index = rows_cache
            .position_of(&self.item_to_measure_index)
            .or_else(|| rows_cache.first_entry_position())
            .unwrap_or(0);

        v_flex()
            .flex_grow_1()
            .relative()
            .size_full()
            .when_some(self.options.max_height, |this, h| this.max_h(h))
            .overflow_hidden()
            .when(items_count == 0, |this| {
                this.child(self.delegate.render_empty(window, cx))
            })
            .when(items_count > 0, {
                |this| {
                    this.child(
                        v_virtual_list(
                            cx.entity(),
                            "virtual-list",
                            rows_cache.entries_sizes.clone(),
                            move |list, visible_range: Range<usize>, window, cx| {
                                list.load_more_if_need(
                                    entities_count,
                                    visible_range.end,
                                    window,
                                    cx,
                                );

                                // 注意：这里的 v_virtual_list 无法使用 gap_y，
                                // 因为节头、节尾始终作为空子条目渲染，
                                // 即使代理给出 None 结果。

                                visible_range
                                    .map(|ix| {
                                        let Some(entry) = rows_cache.get(ix) else {
                                            return div();
                                        };

                                        div().children(match entry {
                                            RowEntry::Entry(index) => Some(
                                                list.render_list_item(index, window, cx)
                                                    .into_any_element(),
                                            ),
                                            RowEntry::SectionHeader(section_ix) => list
                                                .delegate_mut()
                                                .render_section_header(section_ix, window, cx)
                                                .map(|r| r.into_any_element()),
                                            RowEntry::SectionFooter(section_ix) => list
                                                .delegate_mut()
                                                .render_section_footer(section_ix, window, cx)
                                                .map(|r| r.into_any_element()),
                                        })
                                    })
                                    .collect::<Vec<_>>()
                            },
                        )
                        .with_item_to_measure_index(item_to_measure_index)
                        .paddings(self.options.paddings.clone())
                        .when(self.options.max_height.is_some(), |this| {
                            this.with_sizing_behavior(ListSizingBehavior::Infer)
                        })
                        .track_scroll(&scroll_handle)
                        .into_any_element(),
                    )
                }
            })
            .when(scrollbar_visible, |this| {
                this.child(Scrollbar::vertical(&scroll_handle))
            })
    }
}

impl<D> Focusable for ListState<D>
where
    D: ListDelegate,
{
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        if self.searchable {
            self.query_input.focus_handle(cx)
        } else {
            self.focus_handle.clone()
        }
    }
}
impl<D> EventEmitter<ListEvent> for ListState<D> where D: ListDelegate {}
impl<D> Render for ListState<D>
where
    D: ListDelegate,
{
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.prepare_items_if_needed(window, cx);

        // 如果设置了选中条目，则滚动到该条目。
        if let Some((ix, strategy)) = self.deferred_scroll_to_index.take() {
            if let Some(item_ix) = self.rows_cache.position_of(&ix) {
                self.scroll_handle.scroll_to_item(item_ix, strategy);
            }
        }

        let loading = self.delegate().loading(cx);
        let query_input = if self.searchable {
            // 同步占位符
            if let Some(placeholder) = &self.options.search_placeholder {
                self.query_input.update(cx, |input, cx| {
                    input.set_placeholder(placeholder.clone(), window, cx);
                });
            }
            Some(self.query_input.clone())
        } else {
            None
        };

        let loading_view = if loading {
            Some(self.delegate.render_loading(window, cx).into_any_element())
        } else {
            None
        };
        let initial_view = if let Some(input) = &query_input {
            if input.read(cx).value().is_empty() {
                self.delegate.render_initial(window, cx)
            } else {
                None
            }
        } else {
            None
        };
        let items_count = self.rows_cache.items_count();
        let entities_count = self.rows_cache.len();
        let mouse_right_clicked_index = self.mouse_right_clicked_index;

        v_flex()
            .key_context("List")
            .id("list-state")
            .track_focus(&self.focus_handle)
            .size_full()
            .relative()
            .overflow_hidden()
            .when_some(query_input, |this, input| {
                this.child(
                    div()
                        .map(|this| match self.options.size {
                            ElementSize::Small => this.px_1p5(),
                            _ => this.px_2(),
                        })
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            Input::new(&input)
                                .with_size(self.options.size)
                                .prefix(
                                    Icon::new(IconName::Search)
                                        .text_color(cx.theme().muted_foreground),
                                )
                                .cleanable(true)
                                .p_0()
                                .appearance(false),
                        ),
                )
            })
            .when(!loading, |this| {
                this.on_action(cx.listener(Self::on_action_cancel))
                    .on_action(cx.listener(Self::on_action_confirm))
                    .on_action(cx.listener(Self::on_action_select_next))
                    .on_action(cx.listener(Self::on_action_select_prev))
                    .map(|this| {
                        if let Some(view) = initial_view {
                            this.child(view)
                        } else {
                            this.child(self.render_items(items_count, entities_count, window, cx))
                        }
                    })
                    // 点击外部以取消右键点击的行
                    .when(mouse_right_clicked_index.is_some(), |this| {
                        this.on_mouse_down_out(cx.listener(|this, _, window, cx| {
                            this.set_right_clicked_index(None, window, cx);
                            cx.notify();
                        }))
                    })
            })
            .children(loading_view)
    }
}

/// List 元素。
#[derive(IntoElement)]
pub struct List<D: ListDelegate + 'static> {
    state: Entity<ListState<D>>,
    style: StyleRefinement,
    options: ListOptions,
}

impl<D> List<D>
where
    D: ListDelegate + 'static,
{
    /// 使用给定的 ListState 实体创建新的 List 元素。
    pub fn new(state: &Entity<ListState<D>>) -> Self {
        Self {
            state: state.clone(),
            style: StyleRefinement::default(),
            options: ListOptions::default(),
        }
    }

    /// 设置滚动条是否可见，默认为 `true`。
    pub fn scrollbar_visible(mut self, visible: bool) -> Self {
        self.options.scrollbar_visible = visible;
        self
    }

    /// 设置搜索输入框的占位符文本。
    pub fn search_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.options.search_placeholder = Some(placeholder.into());
        self
    }
}

impl<D> Styled for List<D>
where
    D: ListDelegate + 'static,
{
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<D> Sizable for List<D>
where
    D: ListDelegate + 'static,
{
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.options.size = size.into();
        self
    }
}

impl<D> RenderOnce for List<D>
where
    D: ListDelegate + 'static,
{
    fn render(mut self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        // 取出 paddings 和 max_height 到 options 中，并清空 style，
        // 因为它们会被应用到内部的虚拟列表上。
        self.options.paddings = self.style.padding.clone();
        self.options.max_height = self.style.max_size.height;
        self.style.padding = EdgesRefinement::default();
        self.style.max_size.height = None;

        self.state.update(cx, |state, _| {
            state.options = self.options;
        });

        div()
            .id("list")
            .role(Role::List)
            .size_full()
            .refine_style(&self.style)
            .child(self.state.clone())
    }
}
