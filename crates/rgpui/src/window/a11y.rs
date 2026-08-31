//! 无障碍支持，由 [AccessKit][accesskit] 提供。
//!
//! 用户级指南文档请参见[此处](crate::_accessibility)。
//!
//! ## 架构
//!
//! ```text
//!                              ┌────────────────────────────────┐   ┌─────────────────────┐
//!                           ┌─▶│ AccessKit Adapter (MacOS)      │◀─▶│ MacOS System APIs   │
//!                           │  └────────────────────────────────┘   └─────────────────────┘
//!                           │
//! ┌──────┐   ┌───────────┐  │  ┌────────────────────────────────┐   ┌─────────────────────┐
//! │ RGPUI │◀─▶│ AccessKit │◀─┼─▶│ AccessKit Adapter (Windows)    │◀─▶│ Windows System APIs │
//! └──────┘   └───────────┘  │  └────────────────────────────────┘   └─────────────────────┘
//!                           │
//!                           │  ┌────────────────────────────────┐   ┌─────────────────────┐
//!                           └─▶│ AccessKit Adapter (Linux)      │◀─▶│ dbus                │
//!                              └────────────────────────────────┘   └─────────────────────┘
//! ```
//!
//! 为了使 RGPUI 应用能够被辅助技术用户使用，
//! 我们需要做以下几件事：
//! - 当 UI 发生有意义的变化时通知系统。这包括：
//!   - 报告新增/移除/更改的 UI 元素
//!   - *不*报告无关的 UI 变化，例如添加了一个不可见的 `div()`。
//!   - 报告每个 UI 元素的外观和功能。例如：
//!     - 这段文本内容是什么？
//!     - 这个进度条加载到哪里了？
//!     - 此节点能否获得焦点？
//!     - 此节点能否直接赋值？（例如滑块）
//! - 允许系统通过向节点派发操作来与 UI 交互。请注意，AccessKit 有自己的
//!   [`Action`] 类型，它不是 [`crate::Action`] trait。
//! - 在系统请求时激活和停用无障碍功能。
//!
//! 在正确的时间激活和停用是微不足道的，所以我不会在这里详细说明。
//! 另外两项在实现上几乎是正交的。
//!
//! 两者的状态都存在于本模块的 [`A11y`] 结构体中。
//!
//! ### 报告 UI 变化
//!
//! 每帧，我们构建一个 [`TreeUpdate`] 并将其发送到特定平台的适配器。
//! [`TreeUpdate`] 是 UI 树子集的表示。当适配器收到更新时，它会将
//! 其与上一次更新进行差异比较，并调用特定平台的 API 来通知屏幕阅读器
//! 这些变化。节点可能已被创建、销毁或更新。
//!
//! 每个节点都有一个 ID，这个 ID *应该*跨帧保持稳定。如果节点的 ID 发生了
//! 变化，那么从 AccessKit 的角度来看，它是一个不同的节点。
//!
//! 我们从 [`GlobalElementId`] 的 [`GlobalElementId::accesskit_node_id`]
//! 中派生节点 ID。没有 [`GlobalElementId`] 的节点无法产生 AccessKit [`NodeId`]，
//! 因此不会包含在无障碍树中。当我们尝试在未设置 ID 的 [`div()`] 上使用
//! 无障碍 API 时，会尝试发出警告。
//!
//! 这一切都发生在 [`Drawable::prepaint`] 中。[`A11y`] 结构体在预绘制期间
//! 维护一个节点栈，我们可以用它来计算 [`NodeId`] 并记录父子关系。
//! 一旦一帧中的所有 [`Element`] 都已预绘制完成，我们将生成的 [`TreeUpdate`]
//! 对象发送到适配器，屏幕阅读器就可以宣布这些变化。
//!
//! #### 合成子节点
//!
//! 此外，某些节点可以使用 [`Element::a11y_synthetic_children`] 注册
//! "合成子节点"。通常，每个具有角色和 ID 的 [`Element`] 都会推送一个
//! accesskit 节点。但是，有时单个元素可能想要生成多个 accesskit 节点。
//! 这些额外节点被称为提供非默认 [`Element::a11y_synthetic_children`]
//! 实现的元素的"合成子节点"。
//!
//! 用户通过 [`A11ySubtreeBuilder`] 获得构建器风格的 API，允许他们
//! 创建作为当前节点子节点的推送节点，以及修改当前节点本身。
//!
//! RGPUI 在预绘制*之后*调用此回调（并且在弹出相应元素之前），
//! 因为此步骤可能需要预绘制信息可用。将来，我们可能希望更广泛地
//! 将预绘制信息添加到 [`Element::write_a11y_info`]，但目前没有必要。
//!
//! ### 响应操作
//!
//! 在适配器创建时，我们向适配器提供一个回调，可用于派发操作。此回调
//! 转发到 [`A11y::action_listeners`]，这是一个从 [`NodeId`] 到操作处理程序
//! （基本上就是 `Box<dyn Fn()>`）的映射。
//!
//! 它在以下位置填充：
//! - [`Window::on_a11y_action`]，它被以下调用：
//! - [`Interactivity::paint`]，它被以下调用：
//! - [`StatefulInteractiveElement::on_a11y_action`]，这是一个面向公共的 API
//!
//! 这些在帧开始时被清除，并在绘制期间重新填充。
//!
//! [`NodeId`]: accesskit::NodeId

use crate::*;

pub(crate) mod debug;

use crate::collections::{FxHashMap, FxHashSet};
use crate::{App, Bounds, FocusId, Pixels, SharedString, Window};
use accesskit::{Action, NodeId, TreeUpdate};
use smallvec::SmallVec;
use std::hash::{Hash, Hasher};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// 每个窗口无障碍树根节点的固定 AccessKit 节点 ID。
pub(crate) const ROOT_NODE_ID: NodeId = NodeId(0);

/// 特定节点上无障碍操作的监听器。
pub(crate) type A11yActionListener =
    Box<dyn FnMut(Option<&accesskit::ActionData>, &mut Window, &mut App) + 'static>;

/// 每窗口无障碍状态。
///
/// 管理每帧构建的 AccessKit 树，以及将传入的操作请求
/// 分派回正确元素所需的映射。
pub(crate) struct A11y {
    /// 该窗口的无障碍功能是否已被[强制禁用]。
    ///
    /// [强制禁用]: crate::Application::new_inaccessible
    force_disabled: bool,
    /// 系统是否已请求无障碍功能。
    ///
    /// 由 AccessKit 通过提供给适配器的回调更新。
    /// 可能在帧中途发生变化。
    active_flag: Arc<AtomicBool>,
    /// 无障碍功能在*当前帧*是否活跃。
    ///
    /// 每帧开始时，我们加载 [`Self::active_flag`]（使用
    /// [`Self::sync_active_flag`]）并据此判断是否为该帧
    /// 构建 [`TreeUpdate`]。该值在帧内必须保持稳定，
    /// 因为此类型暴露的构建器 API 维护一个节点栈，
    /// 每个节点必须恰好入栈和出栈一次。
    ///
    /// 帧结束时，我们再次调用 [`Self::sync_active_flag`] 以判断
    /// 是否应发送已完成的 [`TreeUpdate`]。
    active_this_frame: bool,
    pub(crate) nodes: A11yNodeBuilder,
    pub(crate) focus_ids: FxHashMap<NodeId, FocusId>,
    pub(crate) node_bounds: FxHashMap<NodeId, Bounds<Pixels>>,
    pub(crate) action_listeners: FxHashMap<NodeId, Vec<(Action, A11yActionListener)>>,
    /// 窗口标题，用于标记根节点，以便辅助技术区分不同窗口。
    window_title: Option<SharedString>,
    /// 最近一次报告的没有无障碍节点的焦点 ID，
    /// 用于每次焦点变化最多记录一次，而非每帧都记录。
    last_focus_without_node: Option<FocusId>,
    /// 保留最后一次树更新（调试构建中还包括每个节点的来源），
    /// 以便通过 [`crate::Window::debug_a11y_tree_json`] 转储。
    debug: debug::A11yDebug,
    /// 将视图的 [`EntityId`] 映射到其 `Render` 类型名称
    #[cfg(debug_assertions)]
    pub(crate) view_type_names: FxHashMap<EntityId, &'static str>,
}

impl A11y {
    pub(crate) fn new(
        active_flag: Arc<AtomicBool>,
        force_disabled: bool,
        window_title: Option<SharedString>,
    ) -> Self {
        Self {
            force_disabled,
            active_flag,
            active_this_frame: false,
            nodes: A11yNodeBuilder::new(),
            focus_ids: FxHashMap::default(),
            node_bounds: FxHashMap::default(),
            action_listeners: FxHashMap::default(),
            window_title,
            last_focus_without_node: None,
            debug: debug::A11yDebug::default(),
            #[cfg(debug_assertions)]
            view_type_names: FxHashMap::default(),
        }
    }

    /// 记录（每次焦点变化一次）焦点元素未暴露给辅助技术，
    /// 因为它没有无障碍节点。当这种情况发生时，屏幕阅读器
    /// 会回退为播报整个窗口而非焦点元素。修复方法是为元素
    /// 同时设置 `.id(...)` 和 `.role(...)`。
    pub(crate) fn note_focus_without_node(&mut self, focus_id: FocusId, reason: &str) {
        if self.last_focus_without_node != Some(focus_id) {
            self.last_focus_without_node = Some(focus_id);
            log::info!(
                "a11y: focused element ({focus_id:?}) has no accessibility node \
                 ({reason}); assistive technology will announce the whole window \
                 instead. Give it both an `.id(...)` and a `.role(...)` to expose it."
            );
        }
    }

    pub(crate) fn set_window_title(&mut self, title: impl Into<SharedString>) {
        self.window_title = Some(title.into());
    }

    /// 确保 [`Self::is_active`] 返回最新信息。
    ///
    /// 详见 [`Self::active_flag`] 和 [`Self::active_this_frame`]
    /// 的文档说明。
    pub(crate) fn sync_active_flag(&mut self) {
        self.active_this_frame = !self.force_disabled && self.active_flag.load(Ordering::SeqCst);
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active_this_frame
    }

    pub(crate) fn set_focusable(&mut self, node_id: NodeId, focus_id: FocusId) {
        self.focus_ids.insert(node_id, focus_id);
    }

    /// 将 `node_id` 报告为当前焦点节点（如果它存在于树中）。
    ///
    /// 每帧只能调用一次。
    pub(crate) fn set_focus(&mut self, node_id: NodeId) {
        // 焦点节点必须在本帧已注册为可聚焦的。
        if !self.focus_ids.contains_key(&node_id) {
            if cfg!(debug_assertions) {
                panic!("set_focus called for a node that was not registered with set_focusable");
            } else {
                log::warn!(
                    "a11y: set_focus called for a node that was not registered with \
                     set_focusable ({node_id:?})"
                );
            }
        }
        if self.nodes.has_node(node_id) {
            // 焦点元素已正确暴露；重置去重标记，
            // 以便后续对无节点元素的焦点再次记录。
            self.last_focus_without_node = None;
            self.nodes.set_focus(node_id);
        } else {
            // 元素注册了焦点句柄和 ID，但因为没有角色而从未获得节点。
            if let Some(focus_id) = self.focus_ids.get(&node_id).copied() {
                self.note_focus_without_node(focus_id, "it has an id but no role");
            }
        }
    }

    pub(crate) fn set_active_descendant(&mut self, node_id: NodeId) {
        // 活动后代必须是焦点容器的后代，而非焦点节点本身。
        if self.nodes.node_is_focused(node_id) {
            if cfg!(debug_assertions) {
                panic!("set_active_descendant called on the focused node");
            } else {
                log::warn!("a11y: set_active_descendant called on the focused node ({node_id:?})");
            }
            return;
        }
        if self.nodes.has_node(node_id) && self.nodes.focus_is_ancestor_of_current() {
            self.nodes.set_active_descendant(node_id);
        }
    }

    /// 清除每帧状态并推送根节点以开始新帧。
    pub(crate) fn begin_frame(&mut self) {
        self.focus_ids.clear();
        self.node_bounds.clear();
        self.action_listeners.clear();
        self.nodes.begin_frame(self.window_title.as_ref());
    }

    /// 完成树的构建并为平台适配器生成 [`TreeUpdate`]。
    pub(crate) fn end_frame(&mut self, frame: debug::FrameDebugInfo) -> TreeUpdate {
        let update = self.nodes.finalize();
        self.debug.capture(
            &update,
            self.nodes.focus,
            self.nodes.active_descendant,
            self.window_title.as_ref(),
            frame,
        );
        #[cfg(debug_assertions)]
        self.debug.capture_node_info(&self.nodes.node_info);
        update
    }

    pub(crate) fn debug_tree_json(&self) -> Option<String> {
        self.debug.to_json()
    }
}

/// 合成子节点的构建器 API。详见
/// [`Element::a11y_synthetic_children`] 的文档。
pub struct A11ySubtreeBuilder<'a> {
    parent_id: NodeId,
    nodes: &'a mut A11yNodeBuilder,
    /// 运行 `a11y_synthetic_children` 的真实元素的来源信息。
    #[cfg(debug_assertions)]
    creator: debug::NodeCreator,
}

impl<'a> A11ySubtreeBuilder<'a> {
    pub(crate) fn new(parent_id: NodeId, nodes: &'a mut A11yNodeBuilder) -> Self {
        Self {
            parent_id,
            nodes,
            #[cfg(debug_assertions)]
            creator: debug::NodeCreator::default(),
        }
    }

    #[cfg(debug_assertions)]
    pub(crate) fn with_creator(mut self, creator: debug::NodeCreator) -> Self {
        self.creator = creator;
        self
    }

    /// 为合成子节点派生 [`NodeId`]。
    ///
    /// 生成的 ID 基于 `key` 的哈希值以及父节点的 ID。
    /// 这意味着 `key` 在同一次 [`Element::a11y_synthetic_children`]
    /// 调用中必须唯一，但可以在不同调用中重复。
    pub fn synthetic_node_id(&self, key: impl Hash) -> NodeId {
        let mut hasher = std::hash::DefaultHasher::default();
        self.parent_id.0.hash(&mut hasher);
        key.hash(&mut hasher);
        NodeId(hasher.finish())
    }

    /// 将合成叶节点作为当前元素节点的子节点追加。
    ///
    /// 如果树中已存在具有此 ID 的节点则返回 `false`，
    /// 此时该节点会被丢弃。
    pub fn push_child(&mut self, id: NodeId, node: accesskit::Node) -> bool {
        let pushed = self.nodes.push_leaf(id, node);
        #[cfg(debug_assertions)]
        if pushed {
            self.nodes.record_node_info(
                id,
                debug::NodeDebugInfo {
                    synthetic: true,
                    view: self.creator.view,
                    element_id: self.creator.element_id.clone(),
                    source_location: self.creator.source_location,
                },
            );
        }
        pushed
    }

    /// 父节点的可变引用。
    pub fn parent_node(&mut self) -> &mut accesskit::Node {
        self.nodes
            .current_node_mut()
            .expect("A11ySubtreeBuilder exists only while its element's node is on the stack")
    }
}

pub(crate) struct A11yNodeBuilder {
    ids_stack: SmallVec<[NodeId; 16]>,
    nodes_stack: SmallVec<[accesskit::Node; 16]>,
    /// 这是 accesskit 所要求的确切类型，因此我们不能简单地
    /// 将其改为 `HashMap<NodeId, Node>` 来移除 `seen_ids` 的需要
    all_nodes: Vec<(NodeId, accesskit::Node)>,
    seen_ids: FxHashSet<NodeId>,
    /// RGPUI 认为已聚焦的节点。注意这可能与报告给 accesskit 的
    /// 不同——详见 [`Self::active_descendant`]
    focus: Option<NodeId>,
    /// 如果某个节点调用了 `.aria_active_descendant()`，且某个
    /// 祖先节点已聚焦，则将其覆盖为聚焦节点。这支持"活动后代"
    /// 模式，允许已聚焦的容器表现为其某个后代节点已聚焦。
    active_descendant: Option<NodeId>,
    #[cfg(debug_assertions)]
    node_info: FxHashMap<NodeId, debug::NodeDebugInfo>,
}

impl A11yNodeBuilder {
    fn new() -> Self {
        Self {
            ids_stack: SmallVec::new(),
            nodes_stack: SmallVec::new(),
            all_nodes: Vec::new(),
            seen_ids: FxHashSet::default(),
            focus: None,
            active_descendant: None,
            #[cfg(debug_assertions)]
            node_info: FxHashMap::default(),
        }
    }

    /// 记录本帧已推送节点的来源信息。仅限调试构建。
    #[cfg(debug_assertions)]
    pub(crate) fn record_node_info(&mut self, id: NodeId, info: debug::NodeDebugInfo) {
        self.node_info.insert(id, info);
    }

    #[must_use]
    fn can_push(&mut self, id: NodeId) -> bool {
        debug_assert!(!self.ids_stack.is_empty(), "node pushed before push_root");

        if !self.seen_ids.insert(id) {
            debug_assert!(
                false,
                "Duplicate a11y node id: {id:?}. In a release build, this node would be silently discarded from the a11y tree."
            );
            return false;
        }

        true
    }

    /// 将新节点推入栈中。它成为当前栈顶节点的子节点。
    ///
    /// 如果节点成功入栈则返回 `true`。
    pub(crate) fn push(&mut self, id: NodeId, node: accesskit::Node) -> bool {
        if !self.can_push(id) {
            return false;
        }

        if let Some(parent) = self.nodes_stack.last_mut() {
            parent.push_child(id);
        }
        self.ids_stack.push(id);
        self.nodes_stack.push(node);
        true
    }

    /// 将叶节点作为当前栈顶节点的子节点添加，而不将其推入栈。
    /// 语义上等同于 [`Self::push`] 后接 [`Self::pop`]。
    ///
    /// 如果节点成功入栈则返回 `true`。
    pub(crate) fn push_leaf(&mut self, id: NodeId, node: accesskit::Node) -> bool {
        if !self.can_push(id) {
            return false;
        }

        if let Some(parent) = self.nodes_stack.last_mut() {
            parent.push_child(id);
        }
        self.all_nodes.push((id, node));
        true
    }

    pub(crate) fn current_node_mut(&mut self) -> Option<&mut accesskit::Node> {
        self.nodes_stack.last_mut()
    }

    /// 将当前节点从栈中弹出并最终归入 all_nodes 列表。
    pub(crate) fn pop(&mut self) {
        debug_assert!(self.ids_stack.len() > 1, "pop would remove the root node");

        if let (Some(id), Some(node)) = (self.ids_stack.pop(), self.nodes_stack.pop()) {
            self.all_nodes.push((id, node));
        }
    }

    /// 推送根节点以开始新帧。
    fn begin_frame(&mut self, window_title: Option<&SharedString>) {
        self.all_nodes.clear();
        self.ids_stack.clear();
        self.nodes_stack.clear();
        self.seen_ids.clear();
        #[cfg(debug_assertions)]
        self.node_info.clear();
        let mut root_node = accesskit::Node::new(accesskit::Role::Window);
        if let Some(title) = window_title {
            root_node.set_label(title.to_string());
        }

        self.ids_stack.push(ROOT_NODE_ID);
        self.nodes_stack.push(root_node);
        self.focus = None;
        self.active_descendant = None;
    }

    /// 返回本帧是否已推送具有给定 ID 的节点。
    pub(crate) fn has_node(&self, id: NodeId) -> bool {
        id == ROOT_NODE_ID || self.seen_ids.contains(&id)
    }

    /// 返回 `id` 是否是当前报告为已聚焦的节点。
    pub(crate) fn node_is_focused(&self, id: NodeId) -> bool {
        self.focus == Some(id)
    }

    pub(crate) fn focus_is_ancestor_of_current(&self) -> bool {
        let Some(focus) = self.focus else {
            return false;
        };

        // 当前节点在栈顶；其下方所有节点均为祖先节点。
        let ancestor_count = self.ids_stack.len().saturating_sub(1);
        self.ids_stack[..ancestor_count].contains(&focus)
    }

    pub(crate) fn set_active_descendant(&mut self, id: NodeId) {
        if self
            .active_descendant
            .is_some_and(|existing| existing != id)
        {
            if cfg!(debug_assertions) {
                panic!("active descendant claimed by multiple nodes in one frame");
            } else {
                log::warn!(
                    "a11y: multiple nodes claimed the active descendant this frame; \
                     using last-wins ({id:?})"
                );
            }
        }
        self.active_descendant = Some(id);
    }

    pub(crate) fn set_focus(&mut self, id: NodeId) {
        if self.focus.is_some() {
            if cfg!(debug_assertions) {
                panic!("set_focus called more than once in a single frame");
            } else {
                log::warn!(
                    "a11y: set_focus called more than once in a single frame; \
                     using last-wins ({id:?})"
                );
            }
        }
        self.focus = Some(id);
    }

    fn finalize(&mut self) -> TreeUpdate {
        // Stack should contain only the root node
        debug_assert_eq!(self.ids_stack.len(), 1);
        debug_assert_eq!(self.ids_stack[0], ROOT_NODE_ID);

        if self.ids_stack.len() != 1 {
            log::error!(
                "a11y: Stack imbalance at end of frame: expected 1 (root), got {}. \
                 Some elements may have pushed without popping.",
                self.ids_stack.len()
            );
        }

        // Pop remaining nodes (should just be the root).
        while !self.ids_stack.is_empty() {
            if let (Some(id), Some(node)) = (self.ids_stack.pop(), self.nodes_stack.pop()) {
                self.all_nodes.push((id, node));
            }
        }

        let focus = match self.active_descendant {
            Some(id) if self.has_node(id) => id,
            Some(id) => {
                if cfg!(debug_assertions) {
                    panic!("active_descendant set to {id:?}, which is not in the tree");
                } else {
                    log::warn!("active_descendant set to {id:?}, which is not in the tree");
                    self.focus.unwrap_or(ROOT_NODE_ID)
                }
            }

            _ => self.focus.unwrap_or(ROOT_NODE_ID),
        };

        let nodes = std::mem::take(&mut self.all_nodes);
        let update = TreeUpdate {
            nodes,
            tree: Some(accesskit::Tree::new(ROOT_NODE_ID)),
            tree_id: accesskit::TreeId::ROOT,
            focus,
        };

        Self::repair_tree_update(update)
    }

    /// Accesskit 在无效 [`TreeUpdate`] 上会 panic。此函数
    /// 防御性地检查 accesskit 会 panic 的不变量，并尝试修复它们。
    fn repair_tree_update(mut update: TreeUpdate) -> TreeUpdate {
        let node_ids: FxHashSet<NodeId> = update.nodes.iter().map(|(id, _)| *id).collect();

        // 焦点必须指向树中的某个节点。
        if !node_ids.contains(&update.focus) {
            log::error!(
                "a11y: Focused node {:?} is not in the tree ({} nodes). \
                 Falling back to root. This is a bug in the a11y tree builder.",
                update.focus,
                update.nodes.len()
            );
            update.focus = ROOT_NODE_ID;
        }

        // 每个子引用必须指向更新中的某个节点。
        for (id, node) in &mut update.nodes {
            let has_invalid_child = node
                .children()
                .iter()
                .any(|child_id| !node_ids.contains(child_id));
            if has_invalid_child {
                let children = node.children();
                let invalid_count = children
                    .iter()
                    .filter(|child_id| !node_ids.contains(child_id))
                    .count();
                log::error!(
                    "a11y: Node {:?} references {} children not present in the tree. \
                     Stripping invalid child references.",
                    id,
                    invalid_count
                );
                let valid: Vec<NodeId> = children
                    .iter()
                    .copied()
                    .filter(|child_id| node_ids.contains(child_id))
                    .collect();
                node.set_children(valid);
            }
        }

        update
    }
}

#[cfg(test)]
mod tests {
    // 导入特定项而非通配符导入 `super`，因为后者会
    // 拉入 rgpui 自身的 `test` 属性宏并遮蔽标准库的宏。
    use super::{A11y, A11yNodeBuilder, ROOT_NODE_ID};
    use crate::FocusId;
    use accesskit::{NodeId, Role};
    use std::sync::{Arc, atomic::AtomicBool};

    fn test_node() -> accesskit::Node {
        accesskit::Node::new(Role::GenericContainer)
    }

    fn new_builder() -> A11yNodeBuilder {
        let mut builder = A11yNodeBuilder::new();
        builder.begin_frame(None);
        builder
    }

    fn new_a11y() -> A11y {
        let mut a11y = A11y::new(Arc::new(AtomicBool::new(true)), false, None);
        a11y.begin_frame();
        a11y
    }

    #[test]
    fn active_descendant_honored_when_container_focused() {
        let mut builder = new_builder();
        let container = NodeId(1);
        let item = NodeId(2);

        assert!(builder.push(container, test_node()));
        builder.set_focus(container);
        assert!(builder.push(item, test_node()));

        // item 在栈顶；已聚焦的 container 是其祖先，
        // 因此该声明被接受。
        assert!(builder.focus_is_ancestor_of_current());
        builder.set_active_descendant(item);

        builder.pop(); // item
        builder.pop(); // container
        let update = builder.finalize();
        assert_eq!(update.focus, item);
    }

    #[test]
    fn active_descendant_honored_for_deep_descendant() {
        let mut builder = new_builder();
        let container = NodeId(1);
        let group = NodeId(2);
        let item = NodeId(3);

        assert!(builder.push(container, test_node()));
        builder.set_focus(container);
        assert!(builder.push(group, test_node()));
        assert!(builder.push(item, test_node()));

        // item 是已聚焦 container 的孙节点；深度无关紧要，
        // 已聚焦的祖先仍在栈上。
        assert!(builder.focus_is_ancestor_of_current());
        builder.set_active_descendant(item);

        builder.pop(); // item
        builder.pop(); // group
        builder.pop(); // container
        let update = builder.finalize();
        assert_eq!(update.focus, item);
    }

    #[test]
    fn active_descendant_ignored_when_focus_in_other_subtree() {
        let mut builder = new_builder();
        let focused_container = NodeId(1);
        let focused_leaf = NodeId(2);
        let other_container = NodeId(3);
        let other_item = NodeId(4);

        // 第一个子树持有真实焦点。
        assert!(builder.push(focused_container, test_node()));
        assert!(builder.push(focused_leaf, test_node()));
        builder.set_focus(focused_leaf);
        builder.pop(); // focused_leaf
        builder.pop(); // focused_container

        // 第二个子树：其 item 会声明活动后代，但焦点
        // 不在其任何祖先上，因此门控拒绝了该声明。
        assert!(builder.push(other_container, test_node()));
        assert!(builder.push(other_item, test_node()));
        assert!(!builder.focus_is_ancestor_of_current());
        builder.pop(); // other_item
        builder.pop(); // other_container

        let update = builder.finalize();
        assert_eq!(update.focus, focused_leaf);
    }

    #[test]
    fn active_descendant_ignored_when_nothing_focused() {
        let mut builder = new_builder();
        let container = NodeId(1);
        let item = NodeId(2);

        assert!(builder.push(container, test_node()));
        assert!(builder.push(item, test_node()));

        // 未聚焦任何节点（焦点默认为根窗口节点），
        // 因此门控拒绝了该声明。
        assert!(!builder.focus_is_ancestor_of_current());
        builder.pop();
        builder.pop();

        let update = builder.finalize();
        assert_eq!(update.focus, ROOT_NODE_ID);
    }

    #[test]
    fn regular_focus_used_when_no_active_descendant() {
        let mut builder = new_builder();
        let focused = NodeId(1);

        assert!(builder.push(focused, test_node()));
        builder.set_focus(focused);
        builder.pop();

        let update = builder.finalize();
        assert_eq!(update.focus, focused);
    }

    #[test]
    fn focus_is_ancestor_excludes_self_and_non_ancestors() {
        let mut builder = new_builder();
        let container = NodeId(1);
        let item = NodeId(2);

        assert!(builder.push(container, test_node()));
        builder.set_focus(container);

        // 当已聚焦的 container 自身在栈顶时，它不是自身的
        // （严格）祖先，因此门控为 false。
        assert!(!builder.focus_is_ancestor_of_current());

        assert!(builder.push(item, test_node()));
        // 现在已聚焦的 container 是栈顶 item 的严格祖先。
        assert!(builder.focus_is_ancestor_of_current());

        builder.pop();
        builder.pop();
    }

    // 双重声明防护仅在调试构建中 panic；在发布构建中
    // 回退为后声明者胜出并输出警告。
    #[test]
    #[cfg_attr(
        debug_assertions,
        should_panic(expected = "active descendant claimed by multiple nodes")
    )]
    fn multiple_active_descendant_claims_panic_in_debug() {
        let mut builder = new_builder();
        builder.set_active_descendant(NodeId(1));
        builder.set_active_descendant(NodeId(2));
    }

    // 在一帧内设置两次焦点意味着两个元素同时声明了
    // 窗口焦点；调试构建中 panic，发布构建中回退为后设置者胜出。
    #[test]
    #[cfg_attr(
        debug_assertions,
        should_panic(expected = "set_focus called more than once")
    )]
    fn setting_focus_twice_panics_in_debug() {
        let mut builder = new_builder();
        builder.set_focus(NodeId(1));
        builder.set_focus(NodeId(2));
    }

    // 聚焦一个从未注册为可聚焦的节点是 bug：调试构建中
    // panic，发布构建中输出警告。
    #[test]
    #[cfg_attr(
        debug_assertions,
        should_panic(expected = "was not registered with set_focusable")
    )]
    fn set_focus_without_set_focusable() {
        let mut a11y = new_a11y();
        let node = NodeId(1);
        assert!(a11y.nodes.push(node, test_node()));
        // set_focusable was never called for `node`.
        a11y.set_focus(node);
    }

    // 已聚焦的节点不能同时作为自身的活动后代：调试构建中
    // panic，发布构建中输出警告。
    #[test]
    #[cfg_attr(debug_assertions, should_panic(expected = "on the focused node"))]
    fn set_active_descendant_on_focused_node() {
        let mut a11y = new_a11y();
        let node = NodeId(1);
        assert!(a11y.nodes.push(node, test_node()));
        a11y.set_focusable(node, FocusId::default());
        a11y.set_focus(node);
        a11y.set_active_descendant(node);
    }

    // 已聚焦容器的两个同级子节点同时声明了活动后代
    // （两者都通过了焦点门控）。第二次声明是 bug：调试构建中
    // panic，发布构建中后声明者胜出并输出警告。
    #[test]
    #[cfg_attr(
        debug_assertions,
        should_panic(expected = "active descendant claimed by multiple nodes")
    )]
    fn two_siblings_claiming_active_descendant() {
        let mut a11y = new_a11y();
        let container = NodeId(1);
        let first = NodeId(2);
        let second = NodeId(3);

        assert!(a11y.nodes.push(container, test_node()));
        a11y.set_focusable(container, FocusId::default());
        a11y.set_focus(container);

        assert!(a11y.nodes.push(first, test_node()));
        a11y.set_active_descendant(first);
        a11y.nodes.pop(); // first

        assert!(a11y.nodes.push(second, test_node()));
        a11y.set_active_descendant(second);
        a11y.nodes.pop(); // second

        a11y.nodes.pop(); // container
    }

    // 节点 A 已聚焦；节点 C（未聚焦节点 B 的子节点）声明了
    // 活动后代。最终树仍必须将 A 报告为已聚焦。
    #[test]
    fn active_descendant_in_unfocused_subtree_keeps_real_focus() {
        let mut a11y = new_a11y();
        let a = NodeId(1);
        let b = NodeId(2);
        let c = NodeId(3);

        assert!(a11y.nodes.push(a, test_node()));
        a11y.set_focusable(a, FocusId::default());
        a11y.set_focus(a);
        a11y.nodes.pop(); // a

        assert!(a11y.nodes.push(b, test_node()));
        assert!(a11y.nodes.push(c, test_node()));
        a11y.set_active_descendant(c);
        a11y.nodes.pop(); // c
        a11y.nodes.pop(); // b

        let update = a11y.end_frame(Default::default());
        assert_eq!(update.focus, a);
    }
}
