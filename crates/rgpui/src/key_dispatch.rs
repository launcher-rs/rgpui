//! KeyDispatch 是 GPUI 处理将操作绑定到按键事件的地方。
//!
//! 使键绑定工作的关键部分是定义操作，
//! 实现一个以操作类型参数为参数的方法，
//! 然后在渲染期间在具有键映射上下文的聚焦节点上注册操作：
//!
//! ```ignore
//! actions!(editor,[Undo, Redo]);
//!
//! impl Editor {
//!   fn undo(&mut self, _: &Undo, _window: &mut Window, _cx: &mut Context<Self>) { ... }
//!   fn redo(&mut self, _: &Redo, _window: &mut Window, _cx: &mut Context<Self>) { ... }
//! }
//!
//! impl Render for Editor {
//!   fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
//!     div()
//!       .track_focus(&self.focus_handle(cx))
//!       .key_context("Editor")
//!       .on_action(cx.listener(Editor::undo))
//!       .on_action(cx.listener(Editor::redo))
//!     ...
//!    }
//! }
//!```
//!
//! 键绑定本身通过调用 cx.bind_keys() 独立管理。
//! （尽管在开发 Zed 本身时，你通常只需要在
//!  assets/keymaps/default-{platform}.json 中添加一行）。
//!
//! ```ignore
//! cx.bind_keys([
//!   KeyBinding::new("cmd-z", Editor::undo, Some("Editor")),
//!   KeyBinding::new("cmd-shift-z", Editor::redo, Some("Editor")),
//! ])
//! ```
//!
//! 有了所有这些，GPUI 将确保如果你有一个包含
//! 焦点的 Editor，按下 cmd-z 将撤销。
//!
//! 在实际应用中，这比这稍微复杂一些，因为通常你有
//! 多个嵌套视图，每个视图都注册键盘处理程序。在这种情况下，操作匹配
//! 从底部向上冒泡。例如在 Zed 中，Workspace 是顶级视图，包含 Pane，而 Pane 包含 Editor。如果存在冲突的键绑定定义，
//! 则 Editor 的绑定优先于 Pane 的绑定，而 Pane 的绑定优先于 Workspace。
//!
//! 在 GPUI 中，键绑定不仅限于单个按键，你可以定义
//! 序列，通过用空格分隔按键：
//!
//!  KeyBinding::new("cmd-k left", pane::SplitLeft, Some("Pane"))

use crate::collections::FxHashMap;
use crate::{
    Action, ActionRegistry, App, DispatchPhase, EntityId, FocusId, KeyBinding, KeyContext, Keymap,
    Keystroke, ModifiersChangedEvent, Window,
};
use smallvec::SmallVec;
use std::{
    any::{Any, TypeId},
    cell::RefCell,
    mem,
    ops::Range,
    rc::Rc,
};

/// 分发树节点 ID。注意这些 ID 在帧之间**不**稳定，
/// 因此 `DispatchNodeId` 仅应与提供它的 `DispatchTree` 一起使用。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) struct DispatchNodeId(usize);

/// 分发树 - GPUI 键盘事件分发的核心数据结构。
///
/// 在每一帧渲染期间，GPUI 会根据 UI 树构建一棵分发树。
/// 分发树中的每个节点对应一个可聚焦的 UI 元素，记录了：
/// - 键盘事件监听器（key_listeners）
/// - Action 监听器（action_listeners）
/// - 修饰键变化监听器（modifiers_changed_listeners）
/// - 键盘上下文（KeyContext）
///
/// 按键事件会沿着分发路径（从叶子到根）逐级冒泡，
/// 直到找到匹配的绑定并执行对应的 Action。
pub(crate) struct DispatchTree {
    /// 节点栈 - 构建树时使用的临时栈
    node_stack: Vec<DispatchNodeId>,
    /// 上下文栈 - 当前激活路径上的 KeyContext 序列
    pub(crate) context_stack: Vec<KeyContext>,
    /// 视图栈 - 当前激活路径上的视图 ID 序列
    view_stack: Vec<EntityId>,
    /// 所有节点的扁平存储
    nodes: Vec<DispatchNode>,
    /// FocusId -> 节点 ID 的映射，用于快速查找焦点节点
    focusable_node_ids: FxHashMap<FocusId, DispatchNodeId>,
    /// 视图 ID -> 节点 ID 的映射
    view_node_ids: FxHashMap<EntityId, DispatchNodeId>,
    /// 全局快捷键映射表
    keymap: Rc<RefCell<Keymap>>,
    /// Action 注册表，用于根据 TypeId 构建 Action 实例
    action_registry: Rc<ActionRegistry>,
}

/// 分发树中的单个节点，对应一个 UI 元素的键盘处理配置。
#[derive(Default)]
pub(crate) struct DispatchNode {
    /// 键盘事件监听器列表
    pub key_listeners: Vec<KeyListener>,
    /// Action 监听器列表
    pub action_listeners: Vec<DispatchActionListener>,
    /// 修饰键（Ctrl、Shift、Alt、Cmd）变化监听器列表
    pub modifiers_changed_listeners: Vec<ModifiersChangedListener>,
    /// 键盘上下文（如 "Editor"、"Pane" 等），用于限定快捷键作用域
    pub context: Option<KeyContext>,
    /// 节点关联的焦点 ID（可选）
    pub focus_id: Option<FocusId>,
    /// 节点关联的视图 ID（可选）
    view_id: Option<EntityId>,
    /// 父节点 ID
    parent: Option<DispatchNodeId>,
}

/// 复用子树信息 - 用于帧间优化，跟踪哪些节点可以重用。
///
/// 当 UI 树在帧间变化不大时，GPUI 可以重用上一帧的分发树子树，
/// 避免完全重建。此结构记录了旧子树和新子树的范围映射关系。
pub(crate) struct ReusedSubtree {
    /// 旧子树在源树中的节点范围
    old_range: Range<usize>,
    /// 新子树在目标树中的节点范围
    new_range: Range<usize>,
    /// 该子树是否包含当前焦点节点
    contains_focus: bool,
}

impl ReusedSubtree {
    /// 将旧子树中的节点 ID 映射到新子树中的对应节点 ID
    pub fn refresh_node_id(&self, node_id: DispatchNodeId) -> DispatchNodeId {
        debug_assert!(
            self.old_range.contains(&node_id.0),
            "node {} was not part of the reused subtree {:?}",
            node_id.0,
            self.old_range
        );
        DispatchNodeId((node_id.0 - self.old_range.start) + self.new_range.start)
    }

    /// 该子树是否包含当前焦点节点
    pub fn contains_focus(&self) -> bool {
        self.contains_focus
    }
}

/// 按键回放信息 - 当部分按键序列不再匹配时，需要回放已推入的按键
#[derive(Default, Debug)]
pub(crate) struct Replay {
    /// 需要回放的按键事件
    pub(crate) keystroke: Keystroke,
    /// 该按键已经匹配到的绑定（可能需要重新处理）
    pub(crate) bindings: SmallVec<[KeyBinding; 1]>,
}

/// 按键分发的结果 - 描述一次按键事件处理后的状态
#[derive(Default, Debug)]
pub(crate) struct DispatchResult {
    /// 待处理的按键序列（当匹配到部分前缀时）
    pub(crate) pending: SmallVec<[Keystroke; 1]>,
    /// 待处理序列是否已有匹配的绑定
    pub(crate) pending_has_binding: bool,
    /// 本次按键匹配到的完整绑定列表
    pub(crate) bindings: SmallVec<[KeyBinding; 1]>,
    /// 需要回放的按键事件（当部分匹配失败时）
    pub(crate) to_replay: SmallVec<[Replay; 1]>,
    /// 当前的上下文栈
    pub(crate) context_stack: Vec<KeyContext>,
}

/// 键盘事件监听器类型 - 接收原始键盘事件
type KeyListener = Rc<dyn Fn(&dyn Any, DispatchPhase, &mut Window, &mut App)>;
/// 修饰键变化监听器类型 - 接收 Ctrl/Shift/Alt/Cmd 等修饰键变化事件
type ModifiersChangedListener = Rc<dyn Fn(&ModifiersChangedEvent, &mut Window, &mut App)>;

/// Action 监听器 - 将 Action 类型与回调函数关联
#[derive(Clone)]
pub(crate) struct DispatchActionListener {
    /// 监听的 Action 类型 ID
    pub(crate) action_type: TypeId,
    /// Action 触发时调用的回调函数
    pub(crate) listener: Rc<dyn Fn(&dyn Any, DispatchPhase, &mut Window, &mut App)>,
}

impl DispatchTree {
    pub fn new(keymap: Rc<RefCell<Keymap>>, action_registry: Rc<ActionRegistry>) -> Self {
        Self {
            node_stack: Vec::new(),
            context_stack: Vec::new(),
            view_stack: Vec::new(),
            nodes: Vec::new(),
            focusable_node_ids: FxHashMap::default(),
            view_node_ids: FxHashMap::default(),
            keymap,
            action_registry,
        }
    }

    pub fn clear(&mut self) {
        self.node_stack.clear();
        self.context_stack.clear();
        self.view_stack.clear();
        self.nodes.clear();
        self.focusable_node_ids.clear();
        self.view_node_ids.clear();
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn push_node(&mut self) -> DispatchNodeId {
        let parent = self.node_stack.last().copied();
        let node_id = DispatchNodeId(self.nodes.len());

        self.nodes.push(DispatchNode {
            parent,
            ..Default::default()
        });
        self.node_stack.push(node_id);
        node_id
    }

    pub fn set_active_node(&mut self, node_id: DispatchNodeId) {
        let next_node_parent = self.nodes[node_id.0].parent;
        while self.node_stack.last().copied() != next_node_parent && !self.node_stack.is_empty() {
            self.pop_node();
        }

        if self.node_stack.last().copied() == next_node_parent {
            self.node_stack.push(node_id);
            let active_node = &self.nodes[node_id.0];
            if let Some(view_id) = active_node.view_id {
                self.view_stack.push(view_id)
            }
            if let Some(context) = active_node.context.clone() {
                self.context_stack.push(context);
            }
        } else {
            debug_assert_eq!(self.node_stack.len(), 0);

            let mut current_node_id = Some(node_id);
            while let Some(node_id) = current_node_id {
                let node = &self.nodes[node_id.0];
                if let Some(context) = node.context.clone() {
                    self.context_stack.push(context);
                }
                if let Some(view_id) = node.view_id {
                    self.view_stack.push(view_id);
                }
                self.node_stack.push(node_id);
                current_node_id = node.parent;
            }

            self.context_stack.reverse();
            self.view_stack.reverse();
            self.node_stack.reverse();
        }
    }

    pub fn set_key_context(&mut self, context: KeyContext) {
        self.active_node().context = Some(context.clone());
        self.context_stack.push(context);
    }

    pub fn set_focus_id(&mut self, focus_id: FocusId) {
        let node_id = *self.node_stack.last().unwrap();
        self.nodes[node_id.0].focus_id = Some(focus_id);
        self.focusable_node_ids.insert(focus_id, node_id);
    }

    pub fn set_view_id(&mut self, view_id: EntityId) {
        if self.view_stack.last().copied() != Some(view_id) {
            let node_id = *self.node_stack.last().unwrap();
            self.nodes[node_id.0].view_id = Some(view_id);
            self.view_node_ids.insert(view_id, node_id);
            self.view_stack.push(view_id);
        }
    }

    pub fn pop_node(&mut self) {
        let node = &self.nodes[self.active_node_id().unwrap().0];
        if node.context.is_some() {
            self.context_stack.pop();
        }
        if node.view_id.is_some() {
            self.view_stack.pop();
        }
        self.node_stack.pop();
    }

    fn move_node(&mut self, source: &mut DispatchNode) {
        self.push_node();
        if let Some(context) = source.context.clone() {
            self.set_key_context(context);
        }
        if let Some(focus_id) = source.focus_id {
            self.set_focus_id(focus_id);
        }
        if let Some(view_id) = source.view_id {
            self.set_view_id(view_id);
        }

        let target = self.active_node();
        target.key_listeners = mem::take(&mut source.key_listeners);
        target.action_listeners = mem::take(&mut source.action_listeners);
        target.modifiers_changed_listeners = mem::take(&mut source.modifiers_changed_listeners);
    }

    pub fn reuse_subtree(
        &mut self,
        old_range: Range<usize>,
        source: &mut Self,
        focus: Option<FocusId>,
    ) -> ReusedSubtree {
        let new_range = self.nodes.len()..self.nodes.len() + old_range.len();

        let mut contains_focus = false;
        let mut source_stack = vec![];
        for (source_node_id, source_node) in source
            .nodes
            .iter_mut()
            .enumerate()
            .skip(old_range.start)
            .take(old_range.len())
        {
            let source_node_id = DispatchNodeId(source_node_id);
            while let Some(source_ancestor) = source_stack.last() {
                if source_node.parent == Some(*source_ancestor) {
                    break;
                } else {
                    source_stack.pop();
                    self.pop_node();
                }
            }

            source_stack.push(source_node_id);
            if source_node.focus_id.is_some() && source_node.focus_id == focus {
                contains_focus = true;
            }
            self.move_node(source_node);
        }

        while !source_stack.is_empty() {
            source_stack.pop();
            self.pop_node();
        }

        ReusedSubtree {
            old_range,
            new_range,
            contains_focus,
        }
    }

    pub fn truncate(&mut self, index: usize) {
        for node in &self.nodes[index..] {
            if let Some(focus_id) = node.focus_id {
                self.focusable_node_ids.remove(&focus_id);
            }

            if let Some(view_id) = node.view_id {
                self.view_node_ids.remove(&view_id);
            }
        }
        self.nodes.truncate(index);
    }

    pub fn on_key_event(&mut self, listener: KeyListener) {
        self.active_node().key_listeners.push(listener);
    }

    pub fn on_modifiers_changed(&mut self, listener: ModifiersChangedListener) {
        self.active_node()
            .modifiers_changed_listeners
            .push(listener);
    }

    pub fn on_action(
        &mut self,
        action_type: TypeId,
        listener: Rc<dyn Fn(&dyn Any, DispatchPhase, &mut Window, &mut App)>,
    ) {
        self.active_node()
            .action_listeners
            .push(DispatchActionListener {
                action_type,
                listener,
            });
    }

    pub fn focus_contains(&self, parent: FocusId, child: FocusId) -> bool {
        if parent == child {
            return true;
        }

        if let Some(parent_node_id) = self.focusable_node_ids.get(&parent) {
            let mut current_node_id = self.focusable_node_ids.get(&child).copied();
            while let Some(node_id) = current_node_id {
                if node_id == *parent_node_id {
                    return true;
                }
                current_node_id = self.nodes[node_id.0].parent;
            }
        }
        false
    }

    pub fn available_actions(&self, target: DispatchNodeId) -> Vec<Box<dyn Action>> {
        let mut actions = Vec::<Box<dyn Action>>::new();
        for node_id in self.dispatch_path(target) {
            let node = &self.nodes[node_id.0];
            for DispatchActionListener { action_type, .. } in &node.action_listeners {
                if let Err(ix) = actions.binary_search_by_key(action_type, |a| a.as_any().type_id())
                {
                    // 有意静默这些错误而不记录日志。
                    // 如果操作无法默认构建，则不可用。
                    let action = self.action_registry.build_action_type(action_type).ok();
                    if let Some(action) = action {
                        actions.insert(ix, action);
                    }
                }
            }
        }
        actions
    }

    pub fn is_action_available(&self, action: &dyn Action, target: DispatchNodeId) -> bool {
        for node_id in self.dispatch_path(target) {
            let node = &self.nodes[node_id.0];
            if node
                .action_listeners
                .iter()
                .any(|listener| listener.action_type == action.as_any().type_id())
            {
                return true;
            }
        }
        false
    }

    /// 返回在当前聚焦元素上调用操作的键绑定。绑定按
    /// 添加顺序返回。对于显示，最后的绑定应优先。
    ///
    /// 仅当绑定是其按键序列的最高优先级匹配时才包含，因此
    /// 被遮蔽的绑定不包含在内。
    pub fn bindings_for_action(
        &self,
        action: &dyn Action,
        context_stack: &[KeyContext],
    ) -> Vec<KeyBinding> {
        // Ideally this would return a `DoubleEndedIterator` to avoid `highest_precedence_*`
        // methods, but this can't be done very cleanly since keymap must be borrowed.
        let keymap = self.keymap.borrow();
        keymap
            .bindings_for_action(action)
            .filter(|binding| {
                Self::binding_matches_predicate_and_not_shadowed(&keymap, binding, context_stack)
            })
            .cloned()
            .collect()
    }

    /// 返回给定操作和上下文堆栈的最高优先级绑定。这
    /// 与 `bindings_for_action` 的最后一个结果相同，但比获取所有绑定更高效。
    pub fn highest_precedence_binding_for_action(
        &self,
        action: &dyn Action,
        context_stack: &[KeyContext],
    ) -> Option<KeyBinding> {
        let keymap = self.keymap.borrow();
        keymap
            .bindings_for_action(action)
            .rev()
            .find(|binding| {
                Self::binding_matches_predicate_and_not_shadowed(&keymap, binding, context_stack)
            })
            .cloned()
    }

    fn binding_matches_predicate_and_not_shadowed(
        keymap: &Keymap,
        binding: &KeyBinding,
        context_stack: &[KeyContext],
    ) -> bool {
        let (bindings, _) = keymap.bindings_for_input(&binding.keystrokes, context_stack);
        if let Some(found) = bindings.iter().next() {
            found.action.partial_eq(binding.action.as_ref())
        } else {
            false
        }
    }

    fn bindings_for_input(
        &self,
        input: &[Keystroke],
        dispatch_path: &SmallVec<[DispatchNodeId; 32]>,
    ) -> (SmallVec<[KeyBinding; 1]>, bool, Vec<KeyContext>) {
        let context_stack: Vec<KeyContext> = dispatch_path
            .iter()
            .filter_map(|node_id| self.node(*node_id).context.clone())
            .collect();

        let (bindings, partial) = self
            .keymap
            .borrow()
            .bindings_for_input(input, &context_stack);
        (bindings, partial, context_stack)
    }

    /// Find the bindings that can follow the current input sequence.
    pub fn possible_next_bindings_for_input(
        &self,
        input: &[Keystroke],
        context_stack: &[KeyContext],
    ) -> Vec<KeyBinding> {
        self.keymap
            .borrow()
            .possible_next_bindings_for_input(input, context_stack)
    }

    /// dispatch_key 处理按键序列
    /// input 应设置为上一次调用 dispatch_key 的 `pending` 值。
    /// 这返回三条指令给输入处理程序：
    /// - bindings：处理此按键序列前要执行的任何绑定
    /// - pending：要存储的新待处理按键序列
    /// - to_replay：任何已被推入 pending 但不再匹配的按键序列，
    ///   这些应首先重放。
    pub fn dispatch_key(
        &mut self,
        mut input: SmallVec<[Keystroke; 1]>,
        keystroke: Keystroke,
        dispatch_path: &SmallVec<[DispatchNodeId; 32]>,
    ) -> DispatchResult {
        input.push(keystroke.clone());
        let (bindings, pending, context_stack) = self.bindings_for_input(&input, dispatch_path);

        if pending {
            return DispatchResult {
                pending: input,
                pending_has_binding: !bindings.is_empty(),
                context_stack,
                ..Default::default()
            };
        } else if !bindings.is_empty() {
            return DispatchResult {
                bindings,
                context_stack,
                ..Default::default()
            };
        } else if input.len() == 1 {
            return DispatchResult {
                context_stack,
                ..Default::default()
            };
        }
        input.pop();

        let (suffix, mut to_replay) = self.replay_prefix(input, dispatch_path);

        let mut result = self.dispatch_key(suffix, keystroke, dispatch_path);
        to_replay.extend(result.to_replay);
        result.to_replay = to_replay;
        result
    }

    /// 如果用户输入匹配的绑定前缀然后等待超时，
    /// flush_dispatch() 将任何之前待处理的输入转换为重放事件。
    pub fn flush_dispatch(
        &mut self,
        input: SmallVec<[Keystroke; 1]>,
        dispatch_path: &SmallVec<[DispatchNodeId; 32]>,
    ) -> SmallVec<[Replay; 1]> {
        let (suffix, mut to_replay) = self.replay_prefix(input, dispatch_path);

        if !suffix.is_empty() {
            to_replay.extend(self.flush_dispatch(suffix, dispatch_path))
        }

        to_replay
    }

    /// 将 input 的最长前缀转换为重放事件并返回剩余部分。
    fn replay_prefix(
        &self,
        mut input: SmallVec<[Keystroke; 1]>,
        dispatch_path: &SmallVec<[DispatchNodeId; 32]>,
    ) -> (SmallVec<[Keystroke; 1]>, SmallVec<[Replay; 1]>) {
        let mut to_replay: SmallVec<[Replay; 1]> = Default::default();
        for last in (0..input.len()).rev() {
            let (bindings, _, _) = self.bindings_for_input(&input[0..=last], dispatch_path);
            if !bindings.is_empty() {
                to_replay.push(Replay {
                    keystroke: input.drain(0..=last).next_back().unwrap(),
                    bindings,
                });
                break;
            }
        }
        if to_replay.is_empty() {
            to_replay.push(Replay {
                keystroke: input.remove(0),
                ..Default::default()
            });
        }
        (input, to_replay)
    }

    pub fn dispatch_path(&self, target: DispatchNodeId) -> SmallVec<[DispatchNodeId; 32]> {
        let mut dispatch_path: SmallVec<[DispatchNodeId; 32]> = SmallVec::new();
        let mut current_node_id = Some(target);
        while let Some(node_id) = current_node_id {
            dispatch_path.push(node_id);
            current_node_id = self.nodes.get(node_id.0).and_then(|node| node.parent);
        }
        dispatch_path.reverse(); // 反转路径使其从根到聚焦节点。
        dispatch_path
    }

    pub fn focus_path(&self, focus_id: FocusId) -> SmallVec<[FocusId; 8]> {
        let mut focus_path: SmallVec<[FocusId; 8]> = SmallVec::new();
        let mut current_node_id = self.focusable_node_ids.get(&focus_id).copied();
        while let Some(node_id) = current_node_id {
            let node = self.node(node_id);
            if let Some(focus_id) = node.focus_id {
                focus_path.push(focus_id);
            }
            current_node_id = node.parent;
        }
        focus_path.reverse(); // 反转路径使其从根到聚焦节点。
        focus_path
    }

    pub fn view_path_reversed(&self, view_id: EntityId) -> impl Iterator<Item = EntityId> {
        let mut current_node_id = self.view_node_ids.get(&view_id).copied();

        std::iter::successors(
            current_node_id.map(|node_id| self.node(node_id)),
            |node_id| Some(self.node(node_id.parent?)),
        )
        .filter_map(|node| node.view_id)
    }

    pub fn node(&self, node_id: DispatchNodeId) -> &DispatchNode {
        &self.nodes[node_id.0]
    }

    fn active_node(&mut self) -> &mut DispatchNode {
        let active_node_id = self.active_node_id().unwrap();
        &mut self.nodes[active_node_id.0]
    }

    pub fn focusable_node_id(&self, target: FocusId) -> Option<DispatchNodeId> {
        self.focusable_node_ids.get(&target).copied()
    }

    pub fn root_node_id(&self) -> DispatchNodeId {
        debug_assert!(!self.nodes.is_empty());
        DispatchNodeId(0)
    }

    pub fn active_node_id(&self) -> Option<DispatchNodeId> {
        self.node_stack.last().copied()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        self as rgpui, AppContext, DispatchResult, Element, ElementId, GlobalElementId,
        InspectorElementId, Keystroke, LayoutId, Style,
    };
    use core::panic;
    use smallvec::SmallVec;
    use std::{cell::RefCell, ops::Range, rc::Rc};

    use crate::{
        ActionRegistry, App, Bounds, Context, DispatchTree, FocusHandle, InputHandler, IntoElement,
        KeyBinding, KeyContext, Keymap, Pixels, Point, Render, Subscription, TestAppContext,
        UTF16Selection, Unbind, Window,
    };

    actions!(dispatch_test, [TestAction, SecondaryTestAction]);

    fn test_dispatch_tree(bindings: Vec<KeyBinding>) -> DispatchTree {
        let registry = ActionRegistry::default();

        DispatchTree::new(
            Rc::new(RefCell::new(Keymap::new(bindings))),
            Rc::new(registry),
        )
    }

    #[test]
    fn test_keybinding_for_action_bounds() {
        let tree = test_dispatch_tree(vec![KeyBinding::new(
            "cmd-n",
            TestAction,
            Some("ProjectPanel"),
        )]);

        let contexts = vec![
            KeyContext::parse("Workspace").unwrap(),
            KeyContext::parse("ProjectPanel").unwrap(),
        ];

        let keybinding = tree.bindings_for_action(&TestAction, &contexts);

        assert!(keybinding[0].action.partial_eq(&TestAction))
    }

    #[test]
    fn test_bindings_for_action_hides_targeted_unbind_in_active_context() {
        let tree = test_dispatch_tree(vec![
            KeyBinding::new("tab", TestAction, Some("Editor")),
            KeyBinding::new(
                "tab",
                Unbind("dispatch_test::TestAction".into()),
                Some("Editor && edit_prediction"),
            ),
            KeyBinding::new(
                "tab",
                SecondaryTestAction,
                Some("Editor && showing_completions"),
            ),
        ]);

        let contexts = vec![
            KeyContext::parse("Workspace").unwrap(),
            KeyContext::parse("Editor showing_completions edit_prediction").unwrap(),
        ];

        let bindings = tree.bindings_for_action(&TestAction, &contexts);
        assert!(bindings.is_empty());

        let highest = tree.highest_precedence_binding_for_action(&TestAction, &contexts);
        assert!(highest.is_none());

        let fallback_bindings = tree.bindings_for_action(&SecondaryTestAction, &contexts);
        assert_eq!(fallback_bindings.len(), 1);
        assert!(fallback_bindings[0].action.partial_eq(&SecondaryTestAction));
    }

    #[test]
    fn test_bindings_for_action_keeps_targeted_binding_outside_unbind_context() {
        let tree = test_dispatch_tree(vec![
            KeyBinding::new("tab", TestAction, Some("Editor")),
            KeyBinding::new(
                "tab",
                Unbind("dispatch_test::TestAction".into()),
                Some("Editor && edit_prediction"),
            ),
            KeyBinding::new(
                "tab",
                SecondaryTestAction,
                Some("Editor && showing_completions"),
            ),
        ]);

        let contexts = vec![
            KeyContext::parse("Workspace").unwrap(),
            KeyContext::parse("Editor").unwrap(),
        ];

        let bindings = tree.bindings_for_action(&TestAction, &contexts);
        assert_eq!(bindings.len(), 1);
        assert!(bindings[0].action.partial_eq(&TestAction));

        let highest = tree.highest_precedence_binding_for_action(&TestAction, &contexts);
        assert!(highest.is_some_and(|binding| binding.action.partial_eq(&TestAction)));
    }

    #[test]
    fn test_pending_has_binding_state() {
        let bindings = vec![
            KeyBinding::new("ctrl-b h", TestAction, None),
            KeyBinding::new("space", TestAction, Some("ContextA")),
            KeyBinding::new("space f g", TestAction, Some("ContextB")),
        ];
        let mut tree = test_dispatch_tree(bindings);

        type DispatchPath = SmallVec<[super::DispatchNodeId; 32]>;
        fn dispatch(
            tree: &mut DispatchTree,
            pending: SmallVec<[Keystroke; 1]>,
            key: &str,
            path: &DispatchPath,
        ) -> DispatchResult {
            tree.dispatch_key(pending, Keystroke::parse(key).unwrap(), path)
        }

        let dispatch_path: DispatchPath = SmallVec::new();
        let result = dispatch(&mut tree, SmallVec::new(), "ctrl-b", &dispatch_path);
        assert_eq!(result.pending.len(), 1);
        assert!(!result.pending_has_binding);

        let result = dispatch(&mut tree, result.pending, "h", &dispatch_path);
        assert_eq!(result.pending.len(), 0);
        assert_eq!(result.bindings.len(), 1);
        assert!(!result.pending_has_binding);

        let node_id = tree.push_node();
        tree.set_key_context(KeyContext::parse("ContextB").unwrap());
        tree.pop_node();

        let dispatch_path = tree.dispatch_path(node_id);
        let result = dispatch(&mut tree, SmallVec::new(), "space", &dispatch_path);

        assert_eq!(result.pending.len(), 1);
        assert!(!result.pending_has_binding);
    }

    #[crate::test]
    fn test_pending_input_observers_notified_on_focus_change(cx: &mut TestAppContext) {
        #[derive(Clone)]
        struct CustomElement {
            focus_handle: FocusHandle,
            text: Rc<RefCell<String>>,
        }

        impl CustomElement {
            fn new(cx: &mut Context<Self>) -> Self {
                Self {
                    focus_handle: cx.focus_handle(),
                    text: Rc::default(),
                }
            }
        }

        impl Element for CustomElement {
            type RequestLayoutState = ();

            type PrepaintState = ();

            fn id(&self) -> Option<ElementId> {
                Some("custom".into())
            }

            fn source_location(&self) -> Option<&'static panic::Location<'static>> {
                None
            }

            fn request_layout(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                window: &mut Window,
                cx: &mut App,
            ) -> (LayoutId, Self::RequestLayoutState) {
                (window.request_layout(Style::default(), [], cx), ())
            }

            fn prepaint(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                _: Bounds<Pixels>,
                _: &mut Self::RequestLayoutState,
                window: &mut Window,
                cx: &mut App,
            ) -> Self::PrepaintState {
                window.set_focus_handle(&self.focus_handle, cx);
            }

            fn paint(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                _: Bounds<Pixels>,
                _: &mut Self::RequestLayoutState,
                _: &mut Self::PrepaintState,
                window: &mut Window,
                cx: &mut App,
            ) {
                let mut key_context = KeyContext::default();
                key_context.add("Terminal");
                window.set_key_context(key_context);
                window.handle_input(&self.focus_handle, self.clone(), cx);
                window.on_action(std::any::TypeId::of::<TestAction>(), |_, _, _, _| {});
            }
        }

        impl IntoElement for CustomElement {
            type Element = Self;

            fn into_element(self) -> Self::Element {
                self
            }
        }

        impl InputHandler for CustomElement {
            fn selected_text_range(
                &mut self,
                _: bool,
                _: &mut Window,
                _: &mut App,
            ) -> Option<UTF16Selection> {
                None
            }

            fn marked_text_range(&mut self, _: &mut Window, _: &mut App) -> Option<Range<usize>> {
                None
            }

            fn text_for_range(
                &mut self,
                _: Range<usize>,
                _: &mut Option<Range<usize>>,
                _: &mut Window,
                _: &mut App,
            ) -> Option<String> {
                None
            }

            fn replace_text_in_range(
                &mut self,
                replacement_range: Option<Range<usize>>,
                text: &str,
                _: &mut Window,
                _: &mut App,
            ) {
                if replacement_range.is_some() {
                    unimplemented!()
                }
                self.text.borrow_mut().push_str(text)
            }

            fn replace_and_mark_text_in_range(
                &mut self,
                replacement_range: Option<Range<usize>>,
                new_text: &str,
                _: Option<Range<usize>>,
                _: &mut Window,
                _: &mut App,
            ) {
                if replacement_range.is_some() {
                    unimplemented!()
                }
                self.text.borrow_mut().push_str(new_text)
            }

            fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}

            fn bounds_for_range(
                &mut self,
                _: Range<usize>,
                _: &mut Window,
                _: &mut App,
            ) -> Option<Bounds<Pixels>> {
                None
            }

            fn character_index_for_point(
                &mut self,
                _: Point<Pixels>,
                _: &mut Window,
                _: &mut App,
            ) -> Option<usize> {
                None
            }
        }

        impl Render for CustomElement {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                self.clone()
            }
        }

        cx.update(|cx| {
            cx.bind_keys([KeyBinding::new("ctrl-b", TestAction, Some("Terminal"))]);
            cx.bind_keys([KeyBinding::new("ctrl-b h", TestAction, Some("Terminal"))]);
        });

        let (test, cx) = cx.add_window_view(|_, cx| CustomElement::new(cx));
        let focus_handle = test.update(cx, |test, _| test.focus_handle.clone());

        let pending_input_changed_count = Rc::new(RefCell::new(0usize));
        let pending_input_changed_count_for_observer = pending_input_changed_count.clone();

        struct PendingInputObserver {
            _subscription: Subscription,
        }

        let _observer = cx.update(|window, cx| {
            cx.new(|cx| PendingInputObserver {
                _subscription: cx.observe_pending_input(window, move |_, _, _| {
                    *pending_input_changed_count_for_observer.borrow_mut() += 1;
                }),
            })
        });

        cx.update(|window, cx| {
            window.focus(&focus_handle, cx);
            window.activate_window();
        });

        cx.simulate_keystrokes("ctrl-b");

        let count_after_pending = Rc::new(RefCell::new(0usize));
        let count_after_pending_for_assertion = count_after_pending.clone();

        cx.update(|window, cx| {
            assert!(window.has_pending_keystrokes());
            *count_after_pending.borrow_mut() = *pending_input_changed_count.borrow();
            assert!(*count_after_pending.borrow() > 0);

            window.focus(&cx.focus_handle(), cx);

            assert!(!window.has_pending_keystrokes());
        });

        // Focus-triggered pending-input notifications are deferred to the end of the current
        // effect cycle, so the observer callback should run after the focus update completes.
        cx.update(|_, _| {
            let count_after_focus_change = *pending_input_changed_count.borrow();
            assert!(count_after_focus_change > *count_after_pending_for_assertion.borrow());
        });
    }

    #[crate::test]
    fn test_input_handler_pending(cx: &mut TestAppContext) {
        #[derive(Clone)]
        struct CustomElement {
            focus_handle: FocusHandle,
            text: Rc<RefCell<String>>,
        }
        impl CustomElement {
            fn new(cx: &mut Context<Self>) -> Self {
                Self {
                    focus_handle: cx.focus_handle(),
                    text: Rc::default(),
                }
            }
        }
        impl Element for CustomElement {
            type RequestLayoutState = ();

            type PrepaintState = ();

            fn id(&self) -> Option<ElementId> {
                Some("custom".into())
            }
            fn source_location(&self) -> Option<&'static panic::Location<'static>> {
                None
            }
            fn request_layout(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                window: &mut Window,
                cx: &mut App,
            ) -> (LayoutId, Self::RequestLayoutState) {
                (window.request_layout(Style::default(), [], cx), ())
            }
            fn prepaint(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                _: Bounds<Pixels>,
                _: &mut Self::RequestLayoutState,
                window: &mut Window,
                cx: &mut App,
            ) -> Self::PrepaintState {
                window.set_focus_handle(&self.focus_handle, cx);
            }
            fn paint(
                &mut self,
                _: Option<&GlobalElementId>,
                _: Option<&InspectorElementId>,
                _: Bounds<Pixels>,
                _: &mut Self::RequestLayoutState,
                _: &mut Self::PrepaintState,
                window: &mut Window,
                cx: &mut App,
            ) {
                let mut key_context = KeyContext::default();
                key_context.add("Terminal");
                window.set_key_context(key_context);
                window.handle_input(&self.focus_handle, self.clone(), cx);
                window.on_action(std::any::TypeId::of::<TestAction>(), |_, _, _, _| {});
            }
        }
        impl IntoElement for CustomElement {
            type Element = Self;

            fn into_element(self) -> Self::Element {
                self
            }
        }

        impl InputHandler for CustomElement {
            fn selected_text_range(
                &mut self,
                _: bool,
                _: &mut Window,
                _: &mut App,
            ) -> Option<UTF16Selection> {
                None
            }

            fn marked_text_range(&mut self, _: &mut Window, _: &mut App) -> Option<Range<usize>> {
                None
            }

            fn text_for_range(
                &mut self,
                _: Range<usize>,
                _: &mut Option<Range<usize>>,
                _: &mut Window,
                _: &mut App,
            ) -> Option<String> {
                None
            }

            fn replace_text_in_range(
                &mut self,
                replacement_range: Option<Range<usize>>,
                text: &str,
                _: &mut Window,
                _: &mut App,
            ) {
                if replacement_range.is_some() {
                    unimplemented!()
                }
                self.text.borrow_mut().push_str(text)
            }

            fn replace_and_mark_text_in_range(
                &mut self,
                replacement_range: Option<Range<usize>>,
                new_text: &str,
                _: Option<Range<usize>>,
                _: &mut Window,
                _: &mut App,
            ) {
                if replacement_range.is_some() {
                    unimplemented!()
                }
                self.text.borrow_mut().push_str(new_text)
            }

            fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}

            fn bounds_for_range(
                &mut self,
                _: Range<usize>,
                _: &mut Window,
                _: &mut App,
            ) -> Option<Bounds<Pixels>> {
                None
            }

            fn character_index_for_point(
                &mut self,
                _: Point<Pixels>,
                _: &mut Window,
                _: &mut App,
            ) -> Option<usize> {
                None
            }
        }
        impl Render for CustomElement {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                self.clone()
            }
        }

        cx.update(|cx| {
            cx.bind_keys([KeyBinding::new("ctrl-b", TestAction, Some("Terminal"))]);
            cx.bind_keys([KeyBinding::new("ctrl-b h", TestAction, Some("Terminal"))]);
        });
        let (test, cx) = cx.add_window_view(|_, cx| CustomElement::new(cx));
        let focus_handle = test.update(cx, |test, _| test.focus_handle.clone());
        cx.update(|window, cx| {
            window.focus(&focus_handle, cx);
            window.activate_window();
        });
        cx.simulate_keystrokes("ctrl-b [");
        test.update(cx, |test, _| assert_eq!(test.text.borrow().as_str(), "["))
    }
}
