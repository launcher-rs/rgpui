use rgpui::{
    AnyElement, App, Bounds, ClickEvent, Context, DismissEvent, Edges, ElementId, Entity,
    EventEmitter, FocusHandle, Focusable, Hsla, InteractiveElement, IntoElement, KeyBinding,
    Length, MouseDownEvent, ParentElement, Pixels, Render, RenderOnce, SharedString,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, anchored, deferred, div,
    prelude::FluentBuilder, px, rems,
};

use rust_i18n::t;

use crate::{
    ActiveTheme, Disableable, ElementExt as _, Icon, IconName, IndexPath, Sizable, Size,
    StyleSized, StyledExt,
    actions::{Cancel, Confirm, SelectDown, SelectUp},
    global_state::GlobalState,
    h_flex,
    input::{clear_button, input_style},
    list::{List, ListState},
    searchable_list::{
        SearchableListAdapter, SearchableListChange, SearchableListDelegate, SearchableListItem,
        SearchableListState,
    },
    v_flex,
};

const CONTEXT: &str = "Combobox";

/// 初始化组合框的键盘绑定
pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", SelectUp, Some(CONTEXT)),
        KeyBinding::new("down", SelectDown, Some(CONTEXT)),
        KeyBinding::new("enter", Confirm { secondary: false }, Some(CONTEXT)),
        KeyBinding::new(
            "secondary-enter",
            Confirm { secondary: true },
            Some(CONTEXT),
        ),
        KeyBinding::new("escape", Cancel, Some(CONTEXT)),
    ])
}

// MARK: ComboboxTriggerCtx

/// 传递给 [`Combobox`] 的 `render_trigger` 闭包的上下文
pub struct ComboboxTriggerCtx<'a, D: SearchableListDelegate + 'static> {
    /// 当前选中项
    pub selection: &'a [(IndexPath, D::Item)],
    /// 占位文本
    pub placeholder: Option<&'a SharedString>,
    /// 是否打开
    pub open: bool,
    /// 是否禁用
    pub disabled: bool,
    /// 组件尺寸
    pub size: Size,
}

// MARK: ComboboxChange

/// 向后兼容别名 — 新代码应直接使用 [`SearchableListChange`]
pub type ComboboxChange = SearchableListChange;

// MARK: ComboboxOptions

/// 组合框配置选项
struct ComboboxOptions {
    /// 样式引用
    style: StyleRefinement,
    /// 组件尺寸
    size: Size,
    /// 是否显示清除按钮
    cleanable: bool,
    /// 占位文本
    placeholder: Option<SharedString>,
    /// 搜索框占位文本
    search_placeholder: Option<SharedString>,
    /// 菜单宽度
    menu_width: Length,
    /// 菜单最大高度
    menu_max_h: Length,
    /// 是否禁用
    disabled: bool,
    /// 是否显示外观样式
    appearance: bool,
    /// 触发器图标
    trigger_icon: Option<Icon>,
    /// 选中标记图标
    check_icon: Option<Icon>,
}

impl Default for ComboboxOptions {
    fn default() -> Self {
        Self {
            style: StyleRefinement::default(),
            size: Size::default(),
            cleanable: false,
            placeholder: None,
            search_placeholder: None,
            menu_width: Length::Auto,
            menu_max_h: rems(20.).into(),
            disabled: false,
            appearance: true,
            trigger_icon: None,
            check_icon: None,
        }
    }
}

// MARK: ComboboxState

/// [`Combobox`] 组件的状态
pub struct ComboboxState<D: SearchableListDelegate + 'static>
where
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    /// 底层可搜索列表状态
    pub(crate) state: SearchableListState<D>,

    // Combobox 特有字段
    /// 是否多选
    multiple: bool,
    /// 是否可搜索
    searchable: bool,
    /// 触发器图标
    trigger_icon: Option<Icon>,
    /// 选中标记图标
    check_icon: Option<Icon>,
    /// 自定义触发器渲染闭包
    render_trigger:
        Option<Box<dyn Fn(&ComboboxTriggerCtx<D>, &mut Window, &mut App) -> AnyElement + 'static>>,
    /// 页脚渲染闭包
    footer: Option<Box<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>>,
}

/// [`ComboboxState`] 发出的事件
pub enum ComboboxEvent<D: SearchableListDelegate + 'static>
where
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    /// 每次切换（添加或移除项）时发出
    Change(Vec<<D::Item as SearchableListItem>::Value>),
    /// 弹出框关闭时发出
    Confirm(Vec<<D::Item as SearchableListItem>::Value>),
}

impl<D> ComboboxState<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    /// 创建新的 `Combobox` 状态
    pub fn new(
        delegate: D,
        selected_indices: Vec<IndexPath>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let weak = cx.entity().downgrade();
        let weak_confirm = weak.clone();
        let weak_cancel = weak.clone();
        let weak_empty = weak;

        let state = SearchableListState::new(
            delegate,
            selected_indices,
            move |selected_index, _secondary, window, cx| {
                cx.defer_in(window, {
                    let weak_confirm = weak_confirm.clone();
                    move |list_state, window, cx| {
                        let Some(index) = selected_index else {
                            return;
                        };
                        // 防护：验证项是否存在再继续
                        let Some(item) = list_state.delegate().delegate.item(index).cloned() else {
                            return;
                        };

                        let ix = index;

                        let Some(weak) = weak_confirm.upgrade() else {
                            return;
                        };

                        let (multiple, mut selection) = {
                            let s = weak.read(cx);
                            (s.multiple, s.state.selection.clone())
                        };

                        let changes = Self::selection_changes(multiple, &selection, ix, &item);

                        let before_indices: Vec<IndexPath> =
                            selection.iter().map(|(ix, _)| *ix).collect();

                        // on_will_change 直接调用 — 实体句柄访问会
                        // 重新进入 defer_in 为此回调持有的 ListState 锁。
                        list_state
                            .delegate_mut()
                            .delegate
                            .on_will_change(&mut selection, &changes);

                        let after_indices: Vec<IndexPath> =
                            selection.iter().map(|(ix, _)| *ix).collect();
                        let changed = before_indices != after_indices;
                        let should_close = changed && !multiple;

                        let new_selection = weak_confirm.update(cx, |this, cx| {
                            this.state.selection = selection;

                            if changed {
                                cx.emit(ComboboxEvent::Change(this.selected_values()));
                                cx.notify();
                            }

                            if should_close {
                                cx.emit(ComboboxEvent::Confirm(this.selected_values()));
                                this.set_open(false, cx);
                                this.focus(window, cx);
                            }

                            this.state.selection.clone()
                        });

                        // 同步快照并直接触发 on_confirm — 同样的重入防护
                        if let Ok(new_selection) = new_selection {
                            list_state
                                .delegate_mut()
                                .update_selection_snapshot(new_selection.clone());

                            if should_close {
                                list_state
                                    .delegate_mut()
                                    .delegate
                                    .on_confirm(&new_selection);
                            }
                        }
                    }
                });
            },
            // on_cancel — 关闭并发出当前值的 Confirm 事件
            move |_final_selected_index, window, cx| {
                cx.defer_in(window, {
                    let weak_cancel = weak_cancel.clone();
                    move |_list_state, window, cx| {
                        _ = weak_cancel.update(cx, |this, cx| {
                            cx.emit(ComboboxEvent::Confirm(this.selected_values()));
                            this.set_open(false, cx);
                            this.focus(window, cx);
                        });
                    }
                });
            },
            // on_render_empty
            move |window, cx| {
                if let Some(empty) = weak_empty
                    .upgrade()
                    .and_then(|e| e.read(cx).state.empty.as_ref().map(|f| f(window, cx)))
                {
                    empty
                } else {
                    h_flex()
                        .justify_center()
                        .py_6()
                        .text_color(cx.theme().muted_foreground.opacity(0.6))
                        .child(Icon::new(IconName::Inbox).size(px(28.)))
                        .into_any_element()
                }
            },
            Self::on_blur,
            window,
            cx,
        );

        Self {
            state,
            multiple: false,
            searchable: false,
            trigger_icon: None,
            check_icon: None,
            render_trigger: None,
            footer: None,
        }
    }

    /// 启用多选模式
    ///
    /// 当为 `true` 时，点击项会切换其选中状态，弹出框保持打开
    /// 当为 `false`（默认）时，点击项会替换选中项并关闭弹出框
    pub fn multiple(mut self, multiple: bool) -> Self {
        self.multiple = multiple;
        self
    }

    /// 启用或禁用下拉框顶部的搜索输入
    pub fn searchable(mut self, searchable: bool) -> Self {
        self.searchable = searchable;
        self
    }

    /// 返回当前选中的值
    pub fn selected_values(&self) -> Vec<<D::Item as SearchableListItem>::Value> {
        self.state.selected_values()
    }

    /// 返回第一个选中的值，未选中时返回 `None`
    ///
    /// 适用于单选模式（`.multiple(false)`）的便捷方法
    pub fn selected_value(&self) -> Option<<D::Item as SearchableListItem>::Value> {
        self.state.selected_values().into_iter().next()
    }

    /// 返回当前选中的 `(IndexPath, Item)` 对
    pub fn selection(&self) -> &[(IndexPath, D::Item)] {
        self.state.selection()
    }

    /// 替换整个选中集合
    pub fn set_selected_indices(
        &mut self,
        indices: impl IntoIterator<Item = IndexPath>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.set_selected_indices(indices, cx);
        self.state.sync_snapshot(cx);
        cx.notify();
    }

    /// 向选中集合添加单个索引（如果不存在），返回是否已添加
    pub fn add_selected_index(&mut self, index: IndexPath, cx: &mut Context<Self>) -> bool {
        let added = self.state.add_selected_index(index, cx);

        if added {
            self.state.sync_snapshot(cx);
            cx.notify();
        }

        added
    }

    /// 从选中集合移除单个索引，返回是否已移除
    pub fn remove_selected_index(&mut self, index: IndexPath, cx: &mut Context<Self>) -> bool {
        let removed = self.state.remove_selected_index(index);

        if removed {
            self.state.sync_snapshot(cx);
        }

        removed
    }

    /// 清除所有选中值
    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.state.selection.clear();
        self.state.sync_snapshot(cx);
        cx.emit(ComboboxEvent::Change(self.selected_values()));
        cx.notify();
    }

    /// 替换底层代理（项数据源）
    pub fn set_items(&mut self, items: D, _: &mut Window, cx: &mut Context<Self>) {
        self.state.list.update(cx, |list, _| {
            list.delegate_mut().delegate = items;
        });
    }

    /// 聚焦触发器
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.state.focus_handle.focus(window, cx);
    }

    fn selection_changes(
        multiple: bool,
        selection: &[(IndexPath, D::Item)],
        ix: IndexPath,
        item: &D::Item,
    ) -> Vec<SearchableListChange> {
        let is_selected = selection
            .iter()
            .any(|(_, selected_item)| selected_item.value() == item.value());

        if multiple {
            if is_selected {
                vec![SearchableListChange::Deselect { index: ix }]
            } else {
                vec![SearchableListChange::Select { index: ix }]
            }
        } else {
            let mut changes: Vec<SearchableListChange> = selection
                .iter()
                .map(|(cur_ix, _)| SearchableListChange::Deselect { index: *cur_ix })
                .collect();
            changes.push(SearchableListChange::Select { index: ix });
            changes
        }
    }

    /// Process an item click: single-select replaces the selection and closes; multi-select toggles.
    ///
    /// Calls `delegate.on_will_change` before committing and `delegate.on_confirm` when closing.
    #[allow(dead_code)]
    pub(crate) fn handle_item_select(
        &mut self,
        ix: IndexPath,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(item) = self
            .state
            .list
            .read(cx)
            .delegate()
            .delegate
            .item(ix)
            .cloned()
        else {
            return;
        };

        let changes = Self::selection_changes(self.multiple, &self.state.selection, ix, &item);

        let mut selection = self.state.selection.clone();
        let before_indices: Vec<IndexPath> = selection.iter().map(|(ix, _)| *ix).collect();

        self.state.list.update(cx, |list, _cx| {
            list.delegate_mut()
                .delegate
                .on_will_change(&mut selection, &changes);
        });

        let after_indices: Vec<IndexPath> = selection.iter().map(|(ix, _)| *ix).collect();
        let changed = before_indices != after_indices;
        let should_close = changed && !self.multiple;

        self.state.selection = selection;
        self.state.sync_snapshot(cx);

        if changed {
            cx.emit(ComboboxEvent::Change(self.selected_values()));
            cx.notify();
        }

        if should_close {
            let final_selection = self.state.selection.clone();
            self.state.list.update(cx, |list, _cx| {
                list.delegate_mut().delegate.on_confirm(&final_selection);
            });

            cx.emit(ComboboxEvent::Confirm(self.selected_values()));
            self.set_open(false, cx);
            self.focus(window, cx);
        }
    }

    /// 失去焦点时关闭菜单
    fn on_blur(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.list.read(cx).is_focused(window, cx)
            || self.state.focus_handle.is_focused(window)
        {
            return;
        }

        self.set_open(false, cx);
        cx.notify();
    }

    /// 处理向上选择动作
    fn up(&mut self, _: &SelectUp, window: &mut Window, cx: &mut Context<Self>) {
        if !self.state.open {
            self.set_open(true, cx);
        }

        self.state.list.focus_handle(cx).focus(window, cx);
        cx.propagate();
    }

    /// 处理向下选择动作
    fn down(&mut self, _: &SelectDown, window: &mut Window, cx: &mut Context<Self>) {
        if !self.state.open {
            self.set_open(true, cx);
        }

        self.state.list.focus_handle(cx).focus(window, cx);
        cx.propagate();
    }

    /// 处理确认动作
    fn enter(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        cx.propagate();

        if !self.state.open {
            self.set_open(true, cx);
            cx.notify();
        }

        self.state.list.focus_handle(cx).focus(window, cx);
    }

    /// 切换菜单打开状态
    fn toggle_menu(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();

        self.set_open(!self.state.open, cx);

        if self.state.open {
            self.state.list.focus_handle(cx).focus(window, cx);
        }

        cx.notify();
    }

    /// 处理取消动作（Escape）
    fn escape(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        if !self.state.open {
            cx.propagate();
            return;
        }

        cx.stop_propagation();
        cx.emit(ComboboxEvent::Confirm(self.selected_values()));

        self.set_open(false, cx);
        self.focus(window, cx);
        cx.notify();
    }

    /// 设置打开状态
    fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.state.open = open;

        if self.state.open {
            GlobalState::global_mut(cx).register_deferred_popover(&self.state.focus_handle)
        } else {
            GlobalState::global_mut(cx).unregister_deferred_popover(&self.state.focus_handle)
        }

        cx.notify();
    }

    /// 清除选中
    fn clean(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.clear_selection(cx);
    }

    /// 默认触发器内容渲染
    fn default_trigger_body(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let placeholder_text = self
            .state
            .placeholder
            .clone()
            .unwrap_or_else(|| t!("Combobox.placeholder").into());

        if self.state.selection.is_empty() {
            return div()
                .text_color(cx.theme().muted_foreground)
                .child(placeholder_text)
                .into_any_element();
        }

        if self.multiple {
            let items: Vec<SharedString> = self
                .state
                .selection
                .iter()
                .map(|(_, i)| i.title())
                .collect();

            div()
                .w_full()
                .overflow_hidden()
                .whitespace_nowrap()
                .truncate()
                .child(items.join(", "))
                .into_any_element()
        } else {
            let title = self
                .state
                .selection
                .first()
                .map(|(_, i)| i.title())
                .unwrap_or_default();

            div()
                .w_full()
                .overflow_hidden()
                .whitespace_nowrap()
                .truncate()
                .child(title)
                .into_any_element()
        }
    }
}

impl<D> Render for ComboboxState<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let searchable = self.searchable;
        let is_focused = self.state.focus_handle.is_focused(window);
        let show_clean = self.state.cleanable && !self.state.selection.is_empty();
        let bounds = self.state.bounds;
        let allow_open = !(self.state.open || self.state.disabled);
        let outline_visible = self.state.open || (is_focused && !self.state.disabled);
        let disabled = self.state.disabled;

        let (bg, fg) = input_style(disabled, cx);

        self.state.list.update(cx, |list, cx| {
            list.set_searchable(searchable, cx);
            list.delegate_mut().size = self.state.size;
            list.delegate_mut().check_icon = self.check_icon.clone();
        });

        let selection = &self.state.selection;
        let placeholder = self.state.placeholder.as_ref();
        let open = self.state.open;
        let size = self.state.size;
        let has_custom_trigger = self.render_trigger.is_some();

        let trigger_icon = self
            .trigger_icon
            .clone()
            .unwrap_or_else(|| Icon::new(IconName::ChevronDown));

        let trigger_body = if let Some(render_trigger) = &self.render_trigger {
            let ctx = ComboboxTriggerCtx {
                selection,
                placeholder,
                open,
                disabled,
                size,
            };

            render_trigger(&ctx, window, cx)
        } else {
            self.default_trigger_body(window, cx)
        };

        let trailing: AnyElement = if has_custom_trigger {
            div().into_any_element()
        } else if show_clean {
            clear_button(cx)
                .map(|this| {
                    if disabled {
                        this.disabled(true)
                    } else {
                        this.on_click(cx.listener(Self::clean))
                    }
                })
                .into_any_element()
        } else {
            trigger_icon
                .xsmall()
                .text_color(cx.theme().muted_foreground)
                .into_any_element()
        };

        let toggle_handler: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>> =
            if allow_open {
                Some(Box::new(cx.listener(Self::toggle_menu)))
            } else {
                None
            };

        let prepaint_handler: Box<dyn Fn(Bounds<Pixels>, &mut Window, &mut App) + 'static> = {
            let state = cx.entity();
            Box::new(move |bounds, _, cx| state.update(cx, |r, _| r.state.bounds = bounds))
        };

        let footer_el = self.footer.as_ref().map(|f| f(window, cx));

        let dismiss_handler: Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static> =
            Box::new(cx.listener(|this, _, window, cx| this.escape(&Cancel, window, cx)));

        div()
            .size_full()
            .relative()
            .child(render_trigger_container(
                disabled,
                self.state.appearance,
                self.state.size,
                &self.state.style,
                bg,
                fg,
                outline_visible,
                allow_open,
                trigger_body,
                trailing,
                toggle_handler,
                prepaint_handler,
                cx,
            ))
            .when(self.state.open, |this| {
                this.child(
                    deferred(render_popup_shell(
                        &self.state.list,
                        self.state.menu_width,
                        self.state.search_placeholder.clone(),
                        self.state.size,
                        self.state.menu_max_h,
                        bounds,
                        footer_el,
                        dismiss_handler,
                        cx,
                    ))
                    .with_priority(1),
                )
            })
    }
}

impl<D> EventEmitter<ComboboxEvent<D>> for ComboboxState<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
}
impl<D> EventEmitter<DismissEvent> for ComboboxState<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
}

impl<D> Focusable for ComboboxState<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        if self.state.open {
            self.state.list.focus_handle(cx)
        } else {
            self.state.focus_handle.clone()
        }
    }
}

// MARK: Combobox 元素

/// 支持单选和多选的组合框
///
/// 点击项会切换其选中状态；下拉框保持打开直到用户
/// 按下 Escape 或点击外部
#[derive(IntoElement)]
pub struct Combobox<D: SearchableListDelegate + 'static>
where
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    /// 元素 ID
    id: ElementId,
    /// 组合框状态
    state: Entity<ComboboxState<D>>,
    /// 配置选项
    options: ComboboxOptions,
    /// 自定义触发器渲染闭包
    render_trigger:
        Option<Box<dyn Fn(&ComboboxTriggerCtx<D>, &mut Window, &mut App) -> AnyElement + 'static>>,
    /// 页脚渲染闭包
    footer: Option<Box<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>>,
    /// 空状态渲染闭包
    empty: Option<Box<dyn Fn(&mut Window, &App) -> AnyElement + 'static>>,
}

impl<D> Combobox<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    /// 创建新的组合框
    pub fn new(state: &Entity<ComboboxState<D>>) -> Self {
        Self {
            id: ("multi-combo-box", state.entity_id()).into(),
            state: state.clone(),
            options: ComboboxOptions::default(),
            render_trigger: None,
            footer: None,
            empty: None,
        }
    }

    /// 设置下拉菜单的宽度
    pub fn menu_width(mut self, width: impl Into<Length>) -> Self {
        self.options.menu_width = width.into();
        self
    }

    /// 设置下拉菜单的最大高度
    pub fn menu_max_h(mut self, max_h: impl Into<Length>) -> Self {
        self.options.menu_max_h = max_h.into();
        self
    }

    /// 设置未选中任何项时显示的占位文本
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.options.placeholder = Some(placeholder.into());
        self
    }

    /// 覆盖触发器的下拉箭头图标
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.options.trigger_icon = Some(icon.into());
        self
    }

    /// 覆盖选中项旁边显示的尾部勾选图标
    pub fn check_icon(mut self, icon: impl Into<Icon>) -> Self {
        self.options.check_icon = Some(icon.into());
        self
    }

    /// 设置搜索输入框的占位文本
    pub fn search_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.options.search_placeholder = Some(placeholder.into());
        self
    }

    /// 当至少选中一个项时显示清除按钮
    pub fn cleanable(mut self, cleanable: bool) -> Self {
        self.options.cleanable = cleanable;
        self
    }

    /// 设置禁用状态
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.options.disabled = disabled;
        self
    }

    /// 设置渲染空状态元素的自定义闭包
    pub fn empty<E: IntoElement + 'static>(
        mut self,
        builder: impl Fn(&mut Window, &App) -> E + 'static,
    ) -> Self {
        self.empty = Some(Box::new(move |window, cx| {
            builder(window, cx).into_any_element()
        }));
        self
    }

    /// 控制触发器是否显示边框和背景
    pub fn appearance(mut self, appearance: bool) -> Self {
        self.options.appearance = appearance;
        self
    }

    /// 覆盖整个触发器元素
    pub fn render_trigger<E: IntoElement + 'static>(
        mut self,
        f: impl Fn(&ComboboxTriggerCtx<D>, &mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.render_trigger = Some(Box::new(move |ctx, window, cx| {
            f(ctx, window, cx).into_any_element()
        }));
        self
    }

    /// 在下拉框底部的分隔线下方渲染元素
    pub fn footer<E: IntoElement + 'static>(
        mut self,
        f: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.footer = Some(Box::new(move |window, cx| f(window, cx).into_any_element()));
        self
    }
}

impl<D> Sizable for Combobox<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.options.size = size.into();
        self
    }
}

impl<D> Styled for Combobox<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.options.style
    }
}

impl<D> RenderOnce for Combobox<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let disabled = self.options.disabled;
        let focus_handle = self.state.focus_handle(cx);
        let render_trigger = self.render_trigger;
        let footer = self.footer;
        let empty = self.empty;
        let opts = self.options;

        self.state.update(cx, |this, _| {
            this.state.style = opts.style;
            this.state.size = opts.size;
            this.state.cleanable = opts.cleanable;
            this.state.placeholder = opts.placeholder;
            this.state.search_placeholder = opts.search_placeholder;
            this.state.menu_width = opts.menu_width;
            this.state.menu_max_h = opts.menu_max_h;
            this.state.disabled = opts.disabled;
            this.state.appearance = opts.appearance;
            this.trigger_icon = opts.trigger_icon;
            this.check_icon = opts.check_icon;
            this.render_trigger = render_trigger;
            this.footer = footer;

            if let Some(empty) = empty {
                this.state.empty = Some(empty);
            }
        });

        div()
            .id(self.id.clone())
            .key_context(CONTEXT)
            .when(!disabled, |this| {
                this.track_focus(&focus_handle.tab_stop(true))
            })
            .on_action(window.listener_for(&self.state, ComboboxState::up))
            .on_action(window.listener_for(&self.state, ComboboxState::down))
            .on_action(window.listener_for(&self.state, ComboboxState::enter))
            .on_action(window.listener_for(&self.state, ComboboxState::escape))
            .size_full()
            .child(self.state)
    }
}

// MARK: 渲染辅助函数

/// 渲染带样式的触发器容器
#[allow(clippy::too_many_arguments)]
fn render_trigger_container(
    disabled: bool,
    appearance: bool,
    size: Size,
    style: &StyleRefinement,
    bg: Hsla,
    fg: Hsla,
    outline_visible: bool,
    allow_open: bool,
    trigger_body: AnyElement,
    trailing: AnyElement,
    toggle_handler: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    prepaint_handler: Box<dyn Fn(Bounds<Pixels>, &mut Window, &mut App) + 'static>,
    cx: &mut App,
) -> impl IntoElement {
    div()
        .id("input")
        .relative()
        .flex()
        .items_center()
        .justify_between()
        .border_1()
        .border_color(cx.theme().transparent)
        .when(appearance, |this| {
            this.bg(bg)
                .text_color(fg)
                .when(disabled, |this| this.opacity(0.5))
                .border_color(cx.theme().input)
                .rounded(cx.theme().radius)
                .when(cx.theme().shadow, |this| this.shadow_xs())
        })
        .map(|this| if disabled { this.shadow_none() } else { this })
        .overflow_hidden()
        .input_size(size)
        .input_text_size(size)
        .refine_style(style)
        .when(outline_visible, |this| this.focused_border(cx))
        .when(allow_open, |this| {
            this.when_some(toggle_handler, |this, handler| this.on_click(handler))
        })
        .child(
            h_flex()
                .id("inner")
                .w_full()
                .items_center()
                .justify_between()
                .gap_1()
                .child(trigger_body)
                .child(trailing),
        )
        .on_prepaint(prepaint_handler)
}

/// 渲染延迟锚定弹出框外壳，包含可搜索列表和可选页脚
#[allow(clippy::too_many_arguments)]
fn render_popup_shell<D: SearchableListDelegate + 'static>(
    list: &Entity<ListState<SearchableListAdapter<D>>>,
    menu_width: Length,
    search_placeholder: Option<SharedString>,
    size: Size,
    menu_max_h: Length,
    bounds: Bounds<Pixels>,
    footer_el: Option<AnyElement>,
    dismiss_handler: Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>,
    cx: &mut App,
) -> AnyElement {
    let has_footer = footer_el.is_some();
    let popup_radius = cx.theme().radius.min(px(8.));

    anchored()
        .snap_to_window_with_margin(px(8.))
        .child(
            div()
                .occlude()
                .map(|this| match menu_width {
                    Length::Auto => this.w(bounds.size.width + px(2.)),
                    Length::Definite(w) => this.w(w),
                })
                .child(
                    v_flex()
                        .occlude()
                        .mt_1p5()
                        .bg(cx.theme().background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded(popup_radius)
                        .shadow_md()
                        .child(
                            List::new(list)
                                .when_some(search_placeholder, |this, placeholder| {
                                    this.search_placeholder(placeholder)
                                })
                                .with_size(size)
                                .max_h(menu_max_h)
                                .paddings(Edges::all(px(4.))),
                        )
                        .when(has_footer, |this| {
                            this.child(
                                div()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .p_1()
                                    .when_some(footer_el, |this, el| this.child(el)),
                            )
                        }),
                )
                .on_mouse_down_out(dismiss_handler),
        )
        .into_any_element()
}

// MARK: 测试

#[cfg(test)]
mod tests {
    use rgpui::{AppContext as _, TestAppContext};

    use crate::{
        IndexPath,
        combobox::{Combobox, ComboboxState},
        searchable_list::{
            SearchableListChange, SearchableListDelegate, SearchableListItem, SearchableListState,
            SearchableVec,
        },
    };

    #[rgpui::test]
    fn test_combo_box_builder(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let items = SearchableVec::new(vec!["Rust", "Go", "C++"]);
            let state = cx.new(|cx| ComboboxState::new(items, vec![], window, cx).searchable(true));

            let _cb = Combobox::new(&state)
                .placeholder("Select language")
                .search_placeholder("Search...")
                .menu_width(rgpui::px(300.))
                .menu_max_h(rgpui::rems(15.))
                .cleanable(true)
                .disabled(false)
                .appearance(true);
        });
    }

    #[rgpui::test]
    fn test_combo_box_search_filters_items(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let items = SearchableVec::new(vec!["Rust", "Go", "C++"]);
            let state = cx.new(|cx| ComboboxState::new(items, vec![], window, cx).searchable(true));

            let count_before = state
                .read(cx)
                .state
                .list
                .read(cx)
                .delegate()
                .delegate
                .items_count(0);
            assert_eq!(count_before, 3);

            state.update(cx, |s, cx| {
                s.state.list.update(cx, |list, cx| {
                    let _ = list
                        .delegate_mut()
                        .delegate
                        .perform_search("Rust", window, cx);
                });
            });

            let count_after = state
                .read(cx)
                .state
                .list
                .read(cx)
                .delegate()
                .delegate
                .items_count(0);
            assert_eq!(count_after, 1);
        });
    }

    #[rgpui::test]
    fn test_multi_combo_box_builder(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let items = SearchableVec::new(vec!["React", "Vue", "Angular"]);
            let state = cx.new(|cx| {
                ComboboxState::new(items, vec![IndexPath::new(0)], window, cx)
                    .multiple(true)
                    .searchable(true)
            });

            let _cb = Combobox::new(&state)
                .placeholder("Select frameworks")
                .search_placeholder("Search...")
                .menu_width(rgpui::px(300.))
                .cleanable(true)
                .disabled(false);

            assert_eq!(state.read(cx).selected_values(), vec!["React"]);
        });
    }

    #[rgpui::test]
    fn test_combo_box_initial_selection_seeds_cursor(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let items = SearchableVec::new(vec!["React", "Vue", "Angular"]);
            let state = cx.new(|cx| {
                ComboboxState::new(items, vec![IndexPath::new(1)], window, cx).multiple(true)
            });

            let state_ref = state.read(cx);
            assert_eq!(
                state_ref.state.list.read(cx).selected_index(),
                Some(IndexPath::new(1)),
                "initial selected_indices should seed ListState.selected_index, not just the snapshot",
            );
            assert_eq!(state_ref.selected_values(), vec!["Vue"]);
        });
    }

    #[rgpui::test]
    fn test_multi_combo_box_toggle(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let items = SearchableVec::new(vec!["React", "Vue", "Angular"]);
            let state = cx.new(|cx| ComboboxState::new(items, vec![], window, cx).multiple(true));

            state.update(cx, |s, cx| s.add_selected_index(IndexPath::new(0), cx));
            assert_eq!(state.read(cx).selected_values(), &["React"]);

            state.update(cx, |s, cx| s.add_selected_index(IndexPath::new(1), cx));
            assert_eq!(state.read(cx).selected_values(), &["React", "Vue"]);

            state.update(cx, |s, cx| s.remove_selected_index(IndexPath::new(0), cx));
            assert_eq!(state.read(cx).selected_values(), &["Vue"]);
        });
    }

    #[rgpui::test]
    fn test_multi_combo_box_search_selection_uses_value_identity(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let items = SearchableVec::new(vec!["React", "Vue", "Angular"]);
            let state = cx.new(|cx| ComboboxState::new(items, vec![], window, cx).multiple(true));

            state.update(cx, |s, cx| s.add_selected_index(IndexPath::new(0), cx));
            assert_eq!(state.read(cx).selected_values(), &["React"]);

            state.update(cx, |s, cx| {
                s.state.list.update(cx, |list, cx| {
                    let _ = list
                        .delegate_mut()
                        .delegate
                        .perform_search("Vue", window, cx);
                });
            });

            state.read_with(cx, |s, cx| {
                let selection = s.state.selection.clone();
                let list = s.state.list.read(cx);
                let delegate = &list.delegate().delegate;
                let ix = IndexPath::new(0);
                let item = delegate.item(ix).expect("filtered item exists");

                assert_eq!(item.value(), &"Vue");
                assert!(
                    !delegate.is_item_checked(ix, item, &selection, cx),
                    "filtered row 0 should not inherit React's checked state",
                );
            });

            state.update(cx, |s, cx| {
                s.handle_item_select(IndexPath::new(0), window, cx);
            });
            assert_eq!(state.read(cx).selected_values(), &["React", "Vue"]);
        });
    }

    #[rgpui::test]
    fn test_multi_combo_box_search_deselects_by_value(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let items = SearchableVec::new(vec!["React", "Vue", "Angular"]);
            let state = cx.new(|cx| ComboboxState::new(items, vec![], window, cx).multiple(true));

            state.update(cx, |s, cx| s.add_selected_index(IndexPath::new(0), cx));

            state.update(cx, |s, cx| {
                s.state.list.update(cx, |list, cx| {
                    let _ = list
                        .delegate_mut()
                        .delegate
                        .perform_search("React", window, cx);
                });
            });

            state.update(cx, |s, cx| {
                s.handle_item_select(IndexPath::new(0), window, cx);
            });
            assert!(state.read(cx).selected_values().is_empty());
        });
    }

    #[rgpui::test]
    fn test_searchable_list_default_change_uses_value_identity(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let mut delegate = SearchableVec::new(vec!["React", "Vue", "Angular"]);
            let mut selection = vec![(IndexPath::new(1), "Vue")];

            let _ = delegate.perform_search("Vue", window, cx);
            delegate.on_will_change(
                &mut selection,
                &[SearchableListChange::Deselect {
                    index: IndexPath::new(0),
                }],
            );
            assert!(selection.is_empty());

            delegate.on_will_change(
                &mut selection,
                &[SearchableListChange::Select {
                    index: IndexPath::new(0),
                }],
            );
            assert_eq!(selection, vec![(IndexPath::new(0), "Vue")]);
        });
    }

    #[rgpui::test]
    fn test_multi_combo_box_clear(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let items = SearchableVec::new(vec!["React", "Vue", "Angular"]);
            let state = cx.new(|cx| {
                ComboboxState::new(
                    items,
                    vec![IndexPath::new(0), IndexPath::new(1)],
                    window,
                    cx,
                )
                .multiple(true)
            });

            assert_eq!(state.read(cx).selected_values().len(), 2);
            state.update(cx, |s, cx| s.clear_selection(cx));
            assert!(state.read(cx).selected_values().is_empty());
        });
    }

    #[rgpui::test]
    fn test_single_combo_box_mode(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let items = SearchableVec::new(vec!["Rust", "Go", "C++"]);
            let state = cx.new(|cx| ComboboxState::new(items, vec![], window, cx));

            // Default mode is Single.
            state.update(cx, |s, cx| s.add_selected_index(IndexPath::new(0), cx));
            assert_eq!(state.read(cx).selected_values(), &["Rust"]);

            state.update(cx, |s, cx| s.add_selected_index(IndexPath::new(1), cx));
            assert_eq!(state.read(cx).selected_values(), &["Rust", "Go"]);
        });
    }

    // Delegate that vetoes all selections via on_will_change by ignoring the changes.
    struct VetoDelegate(SearchableVec<&'static str>);

    impl SearchableListDelegate for VetoDelegate {
        type Item = &'static str;

        fn items_count(&self, section: usize) -> usize {
            self.0.items_count(section)
        }

        fn item(&self, ix: IndexPath) -> Option<&&'static str> {
            self.0.item(ix)
        }

        fn position<V>(&self, value: &V) -> Option<IndexPath>
        where
            &'static str: SearchableListItem<Value = V>,
            V: PartialEq,
        {
            self.0.position(value)
        }

        fn on_will_change(
            &mut self,
            _selection: &mut Vec<(IndexPath, &'static str)>,
            _changes: &[SearchableListChange],
        ) {
            // Leave selection unchanged — acts as a veto.
        }
    }

    #[rgpui::test]
    fn test_on_will_change_veto(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let delegate = VetoDelegate(SearchableVec::new(vec!["Rust", "Go", "C++"]));
            let state = cx.new(|cx| ComboboxState::new(delegate, vec![], window, cx));

            // Pre-select an item directly so we can verify veto prevents changes.
            state.update(cx, |s, cx| s.add_selected_index(IndexPath::new(0), cx));
            assert_eq!(state.read(cx).selected_values(), &["Rust"]);

            // Simulate a click on index 1 via handle_item_select; on_will_change vetoes it.
            state.update(cx, |s, cx| {
                s.handle_item_select(IndexPath::new(1), window, cx);
            });

            // Selection must remain unchanged because on_will_change left it unmodified.
            assert_eq!(state.read(cx).selected_values(), &["Rust"]);
        });
    }

    // Suppress unused import warning for SearchableListState in test module.
    #[allow(unused)]
    fn _uses_state<D: SearchableListDelegate + 'static>(_: &SearchableListState<D>)
    where
        <D::Item as SearchableListItem>::Value: PartialEq + Clone,
    {
    }
}
