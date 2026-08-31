//! 树形组件 - 分层树视图（文件树、目录树等）。
//!
//! 从上游 `gpui-component::tree` 移植而来，用于展示具有层级关系的数据。
//! 核心类型包括 [`TreeItem`]（树节点）、[`TreeState`]（管理状态）、
//! [`TreeEntry`]（扁平化后的节点+深度）以及 [`Tree`]（渲染元素）。

use std::{borrow::BorrowMut, cell::RefCell, ops::Range, rc::Rc};

use crate::ScrollStrategy;
use crate::{
    App, Context, ElementId, Entity, EventEmitter, FocusHandle, InteractiveElement as _,
    IntoElement, KeyBinding, ListSizingBehavior, MouseButton, ParentElement, Render, RenderOnce,
    ScrollableElement, Selectable as _, SharedString, StyleRefinement, Styled, StyledExt,
    UniformListScrollHandle, Window, div,
    list::ListItem,
    menu::ContextMenuExt as _,
    menu::PopupMenu,
    menu::{Confirm, SelectDown, SelectLeft, SelectRight, SelectUp},
    prelude::FluentBuilder as _,
    uniform_list,
};

const CONTEXT: &str = "Tree";

/// 初始化树形组件所需的关键键绑定（上/下/左/右导航）。
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", SelectUp, Some(CONTEXT)),
        KeyBinding::new("down", SelectDown, Some(CONTEXT)),
        KeyBinding::new("left", SelectLeft, Some(CONTEXT)),
        KeyBinding::new("right", SelectRight, Some(CONTEXT)),
    ]);
}

/// 创建一个 [`Tree`] 视图。
///
/// # 参数
///
/// * `state` - 管理树节点共享状态的对象。
/// * `render_item` - 渲染每个树节点的闭包。
///
/// ```ignore
/// let state = cx.new(|_| {
///     TreeState::new().items(vec![
///         TreeItem::new("src")
///             .child(TreeItem::new("lib.rs")),
///         TreeItem::new("Cargo.toml"),
///         TreeItem::new("README.md"),
///     ])
/// });
///
/// tree(&state, |ix, entry, selected, window, cx| {
///     let item = entry.item();
///     ListItem::new(ix).pl(px(16.) * entry.depth()).child(item.label.clone())
/// })
/// ```
pub fn tree<R>(state: &Entity<TreeState>, render_item: R) -> Tree
where
    R: Fn(usize, &TreeEntry, bool, &mut Window, &mut App) -> ListItem + 'static,
{
    Tree::new(state, render_item)
}

struct TreeItemState {
    expanded: bool,
    disabled: bool,
}

/// 一个带有标签、子节点和展开状态的树节点。
#[derive(Clone)]
pub struct TreeItem {
    /// 唯一标识该节点的 id（如完整文件路径）。
    pub id: SharedString,
    /// 显示用的标签文本。
    pub label: SharedString,
    /// 子节点列表。
    pub children: Vec<TreeItem>,
    state: Rc<RefCell<TreeItemState>>,
}

/// 树节点的扁平化表示，附带其在树中的深度。
#[derive(Clone)]
pub struct TreeEntry {
    item: TreeItem,
    depth: usize,
}

impl TreeEntry {
    /// 获取源树节点。
    #[inline]
    pub fn item(&self) -> &TreeItem {
        &self.item
    }

    /// 该节点在树中的深度（根节点为 0）。
    #[inline]
    pub fn depth(&self) -> usize {
        self.depth
    }

    #[inline]
    fn is_root(&self) -> bool {
        self.depth == 0
    }

    /// 该节点是否为文件夹（拥有子节点）。
    #[inline]
    pub fn is_folder(&self) -> bool {
        self.item.is_folder()
    }

    /// 返回该节点是否已展开。
    #[inline]
    pub fn is_expanded(&self) -> bool {
        self.item.is_expanded()
    }

    #[inline]
    /// 返回该节点是否被禁用。
    pub fn is_disabled(&self) -> bool {
        self.item.is_disabled()
    }
}

/// [`TreeState`] 在用户可见状态变化（展开/折叠）时触发的事件。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeEvent {
    /// 树节点被展开。
    Expanded(SharedString),
    /// 树节点被折叠。
    Collapsed(SharedString),
}

impl TreeItem {
    /// 创建带标签的新树节点。
    ///
    /// - `id` 用于唯一标识该节点，后续可用于选中或定位等操作。
    /// - `label` 为该节点显示的文本。
    ///
    /// 例如 `id` 是完整文件路径，`label` 是文件名。
    ///
    /// ```ignore
    /// TreeItem::new("src/ui/button.rs", "button.rs")
    /// ```
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children: Vec::new(),
            state: Rc::new(RefCell::new(TreeItemState {
                expanded: false,
                disabled: false,
            })),
        }
    }

    /// 添加单个子节点。
    pub fn child(mut self, child: TreeItem) -> Self {
        self.children.push(child);
        self
    }

    /// 添加多个子节点。
    pub fn children(mut self, children: impl IntoIterator<Item = TreeItem>) -> Self {
        self.children.extend(children);
        self
    }

    /// 设置该节点的展开状态。
    pub fn expanded(self, expanded: bool) -> Self {
        RefCell::borrow_mut(&self.state).expanded = expanded;
        self
    }

    /// 设置该节点的禁用状态。
    pub fn disabled(self, disabled: bool) -> Self {
        RefCell::borrow_mut(&self.state).disabled = disabled;
        self
    }

    /// 该节点是否为文件夹（拥有子节点）。
    #[inline]
    pub fn is_folder(&self) -> bool {
        self.children.len() > 0
    }

    /// 返回该节点是否被禁用。
    pub fn is_disabled(&self) -> bool {
        RefCell::borrow(&self.state).disabled
    }

    /// 返回该节点是否已展开。
    #[inline]
    pub fn is_expanded(&self) -> bool {
        RefCell::borrow(&self.state).expanded
    }

    fn find_ancestors(&self, target_id: &SharedString) -> Option<Vec<TreeItem>> {
        if self.id == *target_id {
            return Some(vec![]);
        }

        for child in &self.children {
            if let Some(mut path) = child.find_ancestors(target_id) {
                path.push(self.clone());
                return Some(path);
            }
        }

        None
    }
}

/// 管理树节点的状态。
pub struct TreeState {
    focus_handle: FocusHandle,
    entries: Vec<TreeEntry>,
    scroll_handle: UniformListScrollHandle,
    selected_ix: Option<usize>,
    right_clicked_ix: Option<usize>,
    render_item: Rc<dyn Fn(usize, &TreeEntry, bool, &mut Window, &mut App) -> ListItem>,
    context_menu_builder: Option<
        Rc<dyn Fn(usize, &TreeEntry, PopupMenu, &mut Window, &mut Context<TreeState>) -> PopupMenu>,
    >,
}

impl EventEmitter<TreeEvent> for TreeState {}

impl TreeState {
    /// 创建空树状态。
    pub fn new(cx: &mut App) -> Self {
        Self {
            selected_ix: None,
            right_clicked_ix: None,
            focus_handle: cx.focus_handle(),
            scroll_handle: UniformListScrollHandle::default(),
            entries: Vec::new(),
            render_item: Rc::new(|_, _, _, _, _| ListItem::new(0)),
            context_menu_builder: None,
        }
    }

    /// 设置树节点。
    pub fn items(mut self, items: impl Into<Vec<TreeItem>>) -> Self {
        let items = items.into();
        self.entries.clear();
        for item in items.into_iter() {
            self.add_entry(item, 0);
        }
        self
    }

    /// 设置树节点。
    pub fn set_items(&mut self, items: impl Into<Vec<TreeItem>>, cx: &mut Context<Self>) {
        let items = items.into();
        self.entries.clear();
        for item in items.into_iter() {
            self.add_entry(item, 0);
        }
        self.selected_ix = None;
        self.right_clicked_ix = None;
        cx.notify();
    }

    /// 获取当前选中索引（如有）。
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_ix
    }

    /// 设置选中索引，或传 `None` 清除选中。
    pub fn set_selected_index(&mut self, ix: Option<usize>, cx: &mut Context<Self>) {
        self.selected_ix = ix;
        cx.notify();
    }

    /// 通过树节点设置选中索引，或传 `None` 清除选中。
    pub fn set_selected_item(&mut self, item: Option<&TreeItem>, cx: &mut Context<Self>) {
        if let Some(item) = item {
            let ix = self
                .entries
                .iter()
                .position(|entry| entry.item.id == item.id);
            if ix.is_some() {
                self.selected_ix = ix;
            } else {
                self.expand_ancestors(item.id.clone(), cx);
                self.selected_ix = self
                    .entries
                    .iter()
                    .position(|entry| entry.item.id == item.id);
            }
        } else {
            self.selected_ix = None;
        }
        cx.notify();
    }

    /// 获取当前选中的树节点（如有）。
    pub fn selected_item(&self) -> Option<&TreeItem> {
        self.selected_ix
            .and_then(|ix| self.entries.get(ix).map(|entry| &entry.item))
    }

    /// 将树滚动至指定索引的节点，使用给定滚动策略对齐。
    pub fn scroll_to_item(&mut self, ix: usize, strategy: ScrollStrategy) {
        self.scroll_handle.scroll_to_item(ix, strategy);
    }

    /// 获取当前选中的节点条目（如有）。
    pub fn selected_entry(&self) -> Option<&TreeEntry> {
        self.selected_ix.and_then(|ix| self.entries.get(ix))
    }

    fn expand_ancestors(&mut self, target_id: SharedString, cx: &mut Context<Self>) {
        let mut ancestors = Vec::new();

        for entry in &self.entries {
            if let Some(found_ancestors) = entry.item.find_ancestors(&target_id) {
                ancestors = found_ancestors;
                break;
            }
        }

        if ancestors.is_empty() {
            return;
        }

        for ancestor in ancestors.into_iter().rev() {
            if !ancestor.is_expanded() {
                RefCell::borrow_mut(&ancestor.state).expanded = true;
                cx.emit(TreeEvent::Expanded(ancestor.id.clone()));
            }
        }

        self.rebuild_entries();
    }

    fn add_entry(&mut self, item: TreeItem, depth: usize) {
        self.entries.push(TreeEntry {
            item: item.clone(),
            depth,
        });
        if item.is_expanded() {
            for child in &item.children {
                self.add_entry(child.clone(), depth + 1);
            }
        }
    }

    fn toggle_expand(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(entry) = self.entries.get_mut(ix) else {
            return;
        };
        if !entry.is_folder() {
            return;
        }

        let expanded = !entry.is_expanded();
        let id = entry.item.id.clone();
        RefCell::borrow_mut(&entry.item.state).expanded = expanded;

        if expanded {
            cx.emit(TreeEvent::Expanded(id));
        } else {
            cx.emit(TreeEvent::Collapsed(id));
        }

        self.right_clicked_ix = None;
        self.rebuild_entries();
    }

    fn rebuild_entries(&mut self) {
        let root_items: Vec<TreeItem> = self
            .entries
            .iter()
            .filter(|e| e.is_root())
            .map(|e| e.item.clone())
            .collect();
        self.entries.clear();
        for item in root_items.into_iter() {
            self.add_entry(item, 0);
        }
    }

    /// 将焦点移到树视图上。
    pub fn focus(&mut self, window: &mut Window, cx: &mut App) {
        self.focus_handle.focus(window, cx);
    }

    fn on_action_confirm(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(selected_ix) = self.selected_ix {
            if let Some(entry) = self.entries.get(selected_ix) {
                if entry.is_folder() {
                    self.toggle_expand(selected_ix, cx);
                    cx.notify();
                }
            }
        }
    }

    fn on_action_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(selected_ix) = self.selected_ix {
            if let Some(entry) = self.entries.get(selected_ix) {
                if entry.is_folder() && entry.is_expanded() {
                    self.toggle_expand(selected_ix, cx);
                    cx.notify();
                }
            }
        }
    }

    fn on_action_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(selected_ix) = self.selected_ix {
            if let Some(entry) = self.entries.get(selected_ix) {
                if entry.is_folder() && !entry.is_expanded() {
                    self.toggle_expand(selected_ix, cx);
                    cx.notify();
                }
            }
        }
    }

    fn on_action_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        let mut selected_ix = self.selected_ix.unwrap_or(0);

        if selected_ix > 0 {
            selected_ix = selected_ix - 1;
        } else {
            selected_ix = self.entries.len().saturating_sub(1);
        }

        self.selected_ix = Some(selected_ix);
        self.scroll_handle
            .scroll_to_item(selected_ix, ScrollStrategy::Top);
        cx.notify();
    }

    fn on_action_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        let mut selected_ix = self.selected_ix.unwrap_or(0);
        if selected_ix + 1 < self.entries.len() {
            selected_ix = selected_ix + 1;
        } else {
            selected_ix = 0;
        }

        self.selected_ix = Some(selected_ix);
        self.scroll_handle
            .scroll_to_item(selected_ix, ScrollStrategy::Bottom);
        cx.notify();
    }

    fn on_entry_click(&mut self, ix: usize, _: &mut Window, cx: &mut Context<Self>) {
        self.selected_ix = Some(ix);
        self.toggle_expand(ix, cx);
        cx.notify();
    }
}

impl Render for TreeState {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let render_item = self.render_item.clone();
        let state = cx.entity();

        div()
            .id("tree-state")
            .size_full()
            .relative()
            .context_menu({
                move |menu, window, cx: &mut Context<PopupMenu>| {
                    if state.read(cx).context_menu_builder.is_none() {
                        return menu;
                    }

                    let (ix, entry) = {
                        let state = state.read(cx);
                        let entry = state
                            .right_clicked_ix
                            .and_then(|ix| state.entries.get(ix).cloned());
                        (state.right_clicked_ix, entry)
                    };

                    if let (Some(ix), Some(entry)) = (ix, entry) {
                        state.update(cx, |state, cx| {
                            if let Some(build) = state.context_menu_builder.clone() {
                                build(ix, &entry, menu, window, cx)
                            } else {
                                menu
                            }
                        })
                    } else {
                        menu
                    }
                }
            })
            .child(
                uniform_list("entries", self.entries.len(), {
                    cx.processor(move |state, visible_range: Range<usize>, window, cx| {
                        let mut items = Vec::with_capacity(visible_range.len());
                        for ix in visible_range {
                            let entry = &state.entries[ix];
                            let selected = Some(ix) == state.selected_ix;
                            let right_clicked = Some(ix) == state.right_clicked_ix;
                            let item = (render_item)(ix, entry, selected, window, cx.borrow_mut());

                            let el = div()
                                .id(ix)
                                .child(
                                    item.disabled(entry.item().is_disabled())
                                        .selected(selected)
                                        .secondary_selected(right_clicked),
                                )
                                .when(!entry.item().is_disabled(), |this| {
                                    this.on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener({
                                            move |this, _, window, cx| {
                                                this.on_entry_click(ix, window, cx);
                                            }
                                        }),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        cx.listener(move |this, _, _, cx| {
                                            this.right_clicked_ix = Some(ix);
                                            cx.notify();
                                        }),
                                    )
                                });

                            items.push(el)
                        }

                        items
                    })
                })
                .flex_grow_1()
                .size_full()
                .track_scroll(&self.scroll_handle)
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .into_any_element(),
            )
    }
}

/// 展示层级数据的树形视图元素。
#[derive(IntoElement)]
pub struct Tree {
    id: ElementId,
    state: Entity<TreeState>,
    style: StyleRefinement,
    render_item: Rc<dyn Fn(usize, &TreeEntry, bool, &mut Window, &mut App) -> ListItem>,
    context_menu_builder: Option<
        Rc<dyn Fn(usize, &TreeEntry, PopupMenu, &mut Window, &mut Context<TreeState>) -> PopupMenu>,
    >,
}

impl Tree {
    /// 从给定的树状态与渲染闭包创建一个新的 [`Tree`] 视图。
    ///
    /// - `state`：管理树节点共享状态的实体。
    /// - `render_item`：渲染每个树节点的闭包。
    pub fn new<R>(state: &Entity<TreeState>, render_item: R) -> Self
    where
        R: Fn(usize, &TreeEntry, bool, &mut Window, &mut App) -> ListItem + 'static,
    {
        Self {
            id: ElementId::Name(format!("tree-{}", state.entity_id()).into()),
            state: state.clone(),
            style: StyleRefinement::default(),
            render_item: Rc::new(move |ix, item, selected, window, app| {
                render_item(ix, item, selected, window, app)
            }),
            context_menu_builder: None,
        }
    }

    /// 为树添加右键菜单。
    ///
    /// 闭包接收：
    /// - `ix`：右键点击的节点索引
    /// - `entry`：右键点击的树节点条目
    /// - `menu`：弹出菜单构建器
    pub fn context_menu<F>(mut self, f: F) -> Self
    where
        F: Fn(usize, &TreeEntry, PopupMenu, &mut Window, &mut Context<TreeState>) -> PopupMenu
            + 'static,
    {
        self.context_menu_builder = Some(Rc::new(f));
        self
    }
}

impl Styled for Tree {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Tree {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus_handle = self.state.read(cx).focus_handle.clone();
        let scroll_handle = self.state.read(cx).scroll_handle.clone();

        self.state.update(cx, |state, _| {
            state.render_item = self.render_item;
            state.context_menu_builder = self.context_menu_builder;
        });

        div()
            .id(self.id)
            .key_context(CONTEXT)
            .track_focus(&focus_handle)
            .on_action(window.listener_for(&self.state, TreeState::on_action_confirm))
            .on_action(window.listener_for(&self.state, TreeState::on_action_left))
            .on_action(window.listener_for(&self.state, TreeState::on_action_right))
            .on_action(window.listener_for(&self.state, TreeState::on_action_up))
            .on_action(window.listener_for(&self.state, TreeState::on_action_down))
            .size_full()
            .child(self.state)
            .refine_style(&self.style)
            .vertical_scrollbar(&scroll_handle)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{TreeEvent, TreeState};
    use crate::{AppContext as _, Render, Subscription};

    struct TestCollector {
        _state: crate::Entity<TreeState>,
        events: Rc<RefCell<Vec<TreeEvent>>>,
        _subscription: Subscription,
    }

    impl TestCollector {
        fn new(state: &crate::Entity<TreeState>, cx: &mut crate::Context<Self>) -> Self {
            let events = Rc::new(RefCell::new(Vec::new()));
            let events_clone = events.clone();
            let _subscription = cx.subscribe(state, move |_, _, ev: &TreeEvent, _| {
                events_clone.borrow_mut().push(ev.clone());
            });
            Self {
                _state: state.clone(),
                events,
                _subscription,
            }
        }
    }

    impl Render for TestCollector {
        fn render(
            &mut self,
            _: &mut crate::Window,
            _: &mut crate::Context<Self>,
        ) -> impl crate::IntoElement {
            crate::div()
        }
    }

    fn assert_entries(entries: &Vec<super::TreeEntry>, expected: &str) {
        let actual: Vec<String> = entries
            .iter()
            .map(|e| {
                let mut s = String::new();
                s.push_str(&"    ".repeat(e.depth));
                s.push_str(e.item().label.as_str());
                s
            })
            .collect();
        let actual = actual.join("\n");
        assert_eq!(actual.trim(), expected.trim());
    }

    #[crate::test]
    fn test_tree_entry(cx: &mut crate::TestAppContext) {
        use super::TreeItem;

        let items = vec![
            TreeItem::new("src", "src")
                .expanded(true)
                .child(
                    TreeItem::new("src/ui", "ui")
                        .expanded(true)
                        .child(TreeItem::new("src/ui/button.rs", "button.rs"))
                        .child(TreeItem::new("src/ui/icon.rs", "icon.rs"))
                        .child(TreeItem::new("src/ui/mod.rs", "mod.rs")),
                )
                .child(TreeItem::new("src/lib.rs", "lib.rs")),
            TreeItem::new("Cargo.toml", "Cargo.toml"),
            TreeItem::new("Cargo.lock", "Cargo.lock").disabled(true),
            TreeItem::new("README.md", "README.md"),
        ];

        let state = cx.new(|cx| TreeState::new(cx).items(items));
        state.update(cx, |state, cx| {
            assert_entries(
                &state.entries,
                "src\n    ui\n        button.rs\n        icon.rs\n        mod.rs\n    lib.rs\nCargo.toml\nCargo.lock\nREADME.md",
            );

            let entry = state.entries.get(0).unwrap();
            assert_eq!(entry.depth(), 0);
            assert_eq!(entry.is_root(), true);
            assert_eq!(entry.is_folder(), true);
            assert_eq!(entry.is_expanded(), true);

            let entry = state.entries.get(1).unwrap();
            assert_eq!(entry.depth(), 1);
            assert_eq!(entry.is_root(), false);
            assert_eq!(entry.is_folder(), true);
            assert_eq!(entry.is_expanded(), true);
            assert_eq!(entry.item().label.as_str(), "ui");

            state.toggle_expand(1, cx);
            let entry = state.entries.get(1).unwrap();
            assert_eq!(entry.is_expanded(), false);
            assert_entries(
                &state.entries,
                "src\n    ui\n    lib.rs\nCargo.toml\nCargo.lock\nREADME.md",
            );
        })
    }

    #[crate::test]
    fn test_emits_expanded_event(cx: &mut crate::TestAppContext) {
        let items = vec![
            super::TreeItem::new("src", "src").child(super::TreeItem::new("src/lib.rs", "lib.rs")),
        ];
        let state = cx.new(|cx| TreeState::new(cx).items(items));
        let collector = cx.new(|cx| TestCollector::new(&state, cx));

        state.update(cx, |state, cx| {
            state.toggle_expand(0, cx);
        });

        let events = collector.read_with(cx, |c, _| c.events.borrow().clone());
        assert_eq!(events, vec![TreeEvent::Expanded("src".into())]);
    }

    #[crate::test]
    fn test_emits_collapsed_event(cx: &mut crate::TestAppContext) {
        let items = vec![
            super::TreeItem::new("src", "src")
                .expanded(true)
                .child(super::TreeItem::new("src/lib.rs", "lib.rs")),
        ];
        let state = cx.new(|cx| TreeState::new(cx).items(items));
        let collector = cx.new(|cx| TestCollector::new(&state, cx));

        state.update(cx, |state, cx| {
            state.toggle_expand(0, cx);
        });

        let events = collector.read_with(cx, |c, _| c.events.borrow().clone());
        assert_eq!(events, vec![TreeEvent::Collapsed("src".into())]);
    }

    #[crate::test]
    fn test_set_items_does_not_emit_expansion_events(cx: &mut crate::TestAppContext) {
        let items = vec![
            super::TreeItem::new("src", "src")
                .expanded(true)
                .child(super::TreeItem::new("src/lib.rs", "lib.rs")),
        ];
        let state = cx.new(|cx| TreeState::new(cx).items(items));
        let collector = cx.new(|cx| TestCollector::new(&state, cx));

        let new_items = vec![
            super::TreeItem::new("docs", "docs")
                .expanded(true)
                .child(super::TreeItem::new("docs/readme.md", "readme.md")),
        ];
        state.update(cx, |state, cx| {
            state.set_items(new_items, cx);
        });

        let events = collector.read_with(cx, |c, _| c.events.borrow().clone());
        assert!(
            events.is_empty(),
            "set_items should not emit Expanded/Collapsed events"
        );
    }

    #[crate::test]
    fn test_event_carries_item_id(cx: &mut crate::TestAppContext) {
        let items = vec![
            super::TreeItem::new("src", "src").expanded(true).child(
                super::TreeItem::new("src/ui", "ui")
                    .child(super::TreeItem::new("src/ui/button.rs", "button.rs")),
            ),
        ];
        let state = cx.new(|cx| TreeState::new(cx).items(items));
        let collector = cx.new(|cx| TestCollector::new(&state, cx));

        state.update(cx, |state, cx| {
            state.toggle_expand(1, cx);
        });

        let events = collector.read_with(cx, |c, _| c.events.borrow().clone());
        assert_eq!(events, vec![TreeEvent::Expanded("src/ui".into())]);
    }

    #[crate::test]
    fn test_set_selected_item_emits_expanded_events_for_hidden_ancestors(
        cx: &mut crate::TestAppContext,
    ) {
        let target = super::TreeItem::new("src/ui/button.rs", "button.rs");
        let items = vec![
            super::TreeItem::new("src", "src")
                .child(super::TreeItem::new("src/ui", "ui").child(target.clone())),
        ];
        let state = cx.new(|cx| TreeState::new(cx).items(items));
        let collector = cx.new(|cx| TestCollector::new(&state, cx));

        state.update(cx, |state, cx| {
            state.set_selected_item(Some(&target), cx);
        });

        let events = collector.read_with(cx, |c, _| c.events.borrow().clone());
        assert_eq!(
            events,
            vec![
                TreeEvent::Expanded("src".into()),
                TreeEvent::Expanded("src/ui".into())
            ]
        );
    }
}
