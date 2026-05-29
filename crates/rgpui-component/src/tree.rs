use std::{cell::RefCell, ops::Range, rc::Rc};

use rgpui::{
    App, Context, ElementId, Entity, FocusHandle, InteractiveElement as _, IntoElement, KeyBinding,
    ListSizingBehavior, MouseButton, ParentElement, Render, RenderOnce, SharedString,
    StyleRefinement, Styled, UniformListScrollHandle, Window, div, prelude::FluentBuilder as _,
    uniform_list,
};

use crate::{
    Selectable as _, StyledExt,
    actions::{Confirm, SelectDown, SelectLeft, SelectRight, SelectUp},
    list::ListItem,
    menu::{ContextMenuExt as _, PopupMenu},
    scroll::ScrollableElement,
};

const CONTEXT: &str = "Tree";

/// 初始化树组件的键盘绑定
pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", SelectUp, Some(CONTEXT)),
        KeyBinding::new("down", SelectDown, Some(CONTEXT)),
        KeyBinding::new("left", SelectLeft, Some(CONTEXT)),
        KeyBinding::new("right", SelectRight, Some(CONTEXT)),
    ]);
}

/// 创建 [`Tree`] 组件
///
/// # 参数
///
/// * `state` - 管理树项的共享状态
/// * `render_item` - 用于渲染每个树项的闭包
///
/// ```ignore
/// let state = cx.new(|cx| {
///     TreeState::new(cx).items(vec![
///         TreeItem::new("src", "src")
///             .child(TreeItem::new("src/lib.rs", "lib.rs")),
///         TreeItem::new("Cargo.toml", "Cargo.toml"),
///         TreeItem::new("README.md", "README.md"),
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

/// 树项的内部状态
struct TreeItemState {
    /// 是否展开
    expanded: bool,
    /// 是否禁用
    disabled: bool,
}

/// 带有标签、子项和展开状态的树项
#[derive(Clone)]
pub struct TreeItem {
    /// 唯一标识符
    pub id: SharedString,
    /// 显示文本
    pub label: SharedString,
    /// 子项列表
    pub children: Vec<TreeItem>,
    /// 内部状态
    state: Rc<RefCell<TreeItemState>>,
}

/// 树项的扁平化表示，包含其深度
#[derive(Clone)]
pub struct TreeEntry {
    /// 源树项
    item: TreeItem,
    /// 树深度
    depth: usize,
}

impl TreeEntry {
    /// 获取源树项
    #[inline]
    pub fn item(&self) -> &TreeItem {
        &self.item
    }

    /// 获取此项在树中的深度
    #[inline]
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// 检查是否为根节点
    #[inline]
    fn is_root(&self) -> bool {
        self.depth == 0
    }

    /// 检查是否为文件夹（有子项）
    #[inline]
    pub fn is_folder(&self) -> bool {
        self.item.is_folder()
    }

    /// 检查此项是否已展开
    #[inline]
    pub fn is_expanded(&self) -> bool {
        self.item.is_expanded()
    }

    /// 检查此项是否已禁用
    #[inline]
    pub fn is_disabled(&self) -> bool {
        self.item.is_disabled()
    }
}

impl TreeItem {
    /// 创建新的树项
    ///
    /// - `id` 用于唯一标识此项，后续可用于选择或其他用途
    /// - `label` 是此项显示的文本
    ///
    /// 例如，`id` 可以是完整文件路径，`label` 可以是文件名
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

    /// 添加子项
    pub fn child(mut self, child: TreeItem) -> Self {
        self.children.push(child);
        self
    }

    /// 添加多个子项
    pub fn children(mut self, children: impl IntoIterator<Item = TreeItem>) -> Self {
        self.children.extend(children);
        self
    }

    /// 设置展开状态
    pub fn expanded(self, expanded: bool) -> Self {
        self.state.borrow_mut().expanded = expanded;
        self
    }

    /// 设置禁用状态
    pub fn disabled(self, disabled: bool) -> Self {
        self.state.borrow_mut().disabled = disabled;
        self
    }

    /// 检查是否为文件夹（有子项）
    #[inline]
    pub fn is_folder(&self) -> bool {
        !self.children.is_empty()
    }

    /// 检查是否已禁用
    pub fn is_disabled(&self) -> bool {
        self.state.borrow().disabled
    }

    /// 检查是否已展开
    #[inline]
    pub fn is_expanded(&self) -> bool {
        self.state.borrow().expanded
    }

    /// 查找目标项的祖先路径
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

/// 管理树项的状态
pub struct TreeState {
    /// 焦点句柄
    focus_handle: FocusHandle,
    /// 扁平化的树项列表
    entries: Vec<TreeEntry>,
    /// 滚动句柄
    scroll_handle: UniformListScrollHandle,
    /// 当前选中的索引
    selected_ix: Option<usize>,
    /// 右键点击的索引
    right_clicked_ix: Option<usize>,
    /// 渲染项的闭包
    render_item: Rc<dyn Fn(usize, &TreeEntry, bool, &mut Window, &mut App) -> ListItem>,
    /// 上下文菜单构建器
    context_menu_builder: Option<
        Rc<dyn Fn(usize, &TreeEntry, PopupMenu, &mut Window, &mut Context<TreeState>) -> PopupMenu>,
    >,
}

impl TreeState {
    /// 创建新的空树状态
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

    /// 设置树项
    pub fn items(mut self, items: impl Into<Vec<TreeItem>>) -> Self {
        let items = items.into();
        self.entries.clear();
        for item in items.into_iter() {
            self.add_entry(item, 0);
        }
        self
    }

    /// 设置树项（可变引用版本）
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

    /// 获取当前选中的索引
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_ix
    }

    /// 设置选中的索引，或传入 `None` 清除选择
    pub fn set_selected_index(&mut self, ix: Option<usize>, cx: &mut Context<Self>) {
        self.selected_ix = ix;
        cx.notify();
    }

    /// 通过树项设置选中索引，或传入 `None` 清除选择
    pub fn set_selected_item(&mut self, item: Option<&TreeItem>, cx: &mut Context<Self>) {
        if let Some(item) = item {
            let ix = self
                .entries
                .iter()
                .position(|entry| entry.item.id == item.id);
            if ix.is_some() {
                self.selected_ix = ix;
            } else {
                self.expand_ancestors(item.id.clone());
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

    /// 获取当前选中的树项
    pub fn selected_item(&self) -> Option<&TreeItem> {
        self.selected_ix
            .and_then(|ix| self.entries.get(ix).map(|entry| &entry.item))
    }

    /// 滚动到指定项
    pub fn scroll_to_item(&mut self, ix: usize, strategy: rgpui::ScrollStrategy) {
        self.scroll_handle.scroll_to_item(ix, strategy);
    }

    /// 获取当前选中的条目
    pub fn selected_entry(&self) -> Option<&TreeEntry> {
        self.selected_ix.and_then(|ix| self.entries.get(ix))
    }

    /// 展开目标项的所有祖先节点
    fn expand_ancestors(&mut self, target_id: SharedString) {
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

        for ancestor in ancestors {
            ancestor.state.borrow_mut().expanded = true;
        }

        self.rebuild_entries();
    }

    /// 添加条目到扁平列表
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

    /// 切换指定索引项的展开状态
    fn toggle_expand(&mut self, ix: usize) {
        let Some(entry) = self.entries.get_mut(ix) else {
            return;
        };
        if !entry.is_folder() {
            return;
        }

        entry.item.state.borrow_mut().expanded = !entry.is_expanded();
        self.right_clicked_ix = None;
        self.rebuild_entries();
    }

    /// 重建扁平化条目列表
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

    /// 聚焦树组件
    pub fn focus(&mut self, window: &mut Window, cx: &mut App) {
        self.focus_handle.focus(window, cx);
    }

    /// 处理确认操作（展开/折叠文件夹）
    fn on_action_confirm(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(selected_ix) = self.selected_ix {
            if let Some(entry) = self.entries.get(selected_ix) {
                if entry.is_folder() {
                    self.toggle_expand(selected_ix);
                    cx.notify();
                }
            }
        }
    }

    /// 处理向左选择操作（折叠文件夹）
    fn on_action_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(selected_ix) = self.selected_ix {
            if let Some(entry) = self.entries.get(selected_ix) {
                if entry.is_folder() && entry.is_expanded() {
                    self.toggle_expand(selected_ix);
                    cx.notify();
                }
            }
        }
    }

    /// 处理向右选择操作（展开文件夹）
    fn on_action_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(selected_ix) = self.selected_ix {
            if let Some(entry) = self.entries.get(selected_ix) {
                if entry.is_folder() && !entry.is_expanded() {
                    self.toggle_expand(selected_ix);
                    cx.notify();
                }
            }
        }
    }

    /// 处理向上选择操作
    fn on_action_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        let mut selected_ix = self.selected_ix.unwrap_or(0);

        if selected_ix > 0 {
            selected_ix = selected_ix - 1;
        } else {
            selected_ix = self.entries.len().saturating_sub(1);
        }

        self.selected_ix = Some(selected_ix);
        self.scroll_handle
            .scroll_to_item(selected_ix, rgpui::ScrollStrategy::Top);
        cx.notify();
    }

    /// 处理向下选择操作
    fn on_action_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        let mut selected_ix = self.selected_ix.unwrap_or(0);
        if selected_ix + 1 < self.entries.len() {
            selected_ix = selected_ix + 1;
        } else {
            selected_ix = 0;
        }

        self.selected_ix = Some(selected_ix);
        self.scroll_handle
            .scroll_to_item(selected_ix, rgpui::ScrollStrategy::Bottom);
        cx.notify();
    }

    /// 处理条目点击
    fn on_entry_click(&mut self, ix: usize, _: &mut Window, cx: &mut Context<Self>) {
        self.selected_ix = Some(ix);
        self.toggle_expand(ix);
        cx.notify();
    }
}

impl Render for TreeState {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let render_item = self.render_item.clone();
        let state = cx.entity().clone();

        div()
            .id("tree-state")
            .size_full()
            .relative()
            .context_menu({
                let state = state.clone();
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
                            let item = (render_item)(ix, entry, selected, window, cx);

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
                .flex_grow()
                .size_full()
                .track_scroll(&self.scroll_handle)
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .into_any_element(),
            )
    }
}

/// 树视图组件，用于显示层级数据
#[derive(IntoElement)]
pub struct Tree {
    /// 元素 ID
    id: ElementId,
    /// 树状态
    state: Entity<TreeState>,
    /// 样式引用
    style: StyleRefinement,
    /// 渲染项闭包
    render_item: Rc<dyn Fn(usize, &TreeEntry, bool, &mut Window, &mut App) -> ListItem>,
    /// 上下文菜单构建器
    context_menu_builder: Option<
        Rc<dyn Fn(usize, &TreeEntry, PopupMenu, &mut Window, &mut Context<TreeState>) -> PopupMenu>,
    >,
}

impl Tree {
    /// 创建新的树组件
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

    /// 添加上下文菜单
    ///
    /// 闭包接收以下参数：
    /// - `ix`: 右键点击条目的索引
    /// - `entry`: 右键点击的树条目
    /// - `menu`: 弹出菜单构建器
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
    use indoc::indoc;

    use super::TreeState;
    use rgpui::AppContext as _;

    /// 断言条目列表是否符合预期
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

    #[rgpui::test]
    fn test_tree_entry(cx: &mut rgpui::TestAppContext) {
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
        state.update(cx, |state, _| {
            assert_entries(
                &state.entries,
                indoc! {
                    r#"
                src
                    ui
                        button.rs
                        icon.rs
                        mod.rs
                    lib.rs
                Cargo.toml
                Cargo.lock
                README.md
                "#
                },
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

            state.toggle_expand(1);
            let entry = state.entries.get(1).unwrap();
            assert_eq!(entry.is_expanded(), false);
            assert_entries(
                &state.entries,
                indoc! {
                    r#"
                src
                    ui
                    lib.rs
                Cargo.toml
                Cargo.lock
                README.md
                "#
                },
            );
        })
    }
}
