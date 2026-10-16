//! 布局节点路径键（P3a）：元素树位置的跨帧稳定哈希。
//!
//! 对标 `gpui-fast` 的 `fast/layout_key.rs`。Taffy 节点按此键复用：
//! - 有 `ElementId` 的步进按 id 哈希（同级移动仍命中）；
//! - 无 id 的步进按其在同级中的序号（条件增删仅优雅降级为重建）。
//!
//! 键只决定“试哪块缓存”，正确性不依赖它：命中节点仍会全量比对样式、
//! 子节点与测量指纹后才复用（见 [`TaffyLayoutEngine`](crate::taffy::TaffyLayoutEngine)）。

use std::hash::{Hash, Hasher};

/// 元素树路径的哈希键（父键 + 步进的链式哈希）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct LayoutKey(u64);

/// 本帧内第 n 次使用的路径键（[`LayoutKey`] + 出现序号）。
///
/// 同一帧里同一条元素路径可能被请求多次：两处元素共享同一个 `ElementId`
/// （最常见的是把默认 id 取自调用点 `caller_element_id` 的组件放进循环、又没显式
/// 传 id，见 `Element::id` 文档），或同一元素的 `request_layout` 里连开两个节点。
///
/// 多次请求必须落到**两个** Taffy 节点：Taffy 里一个节点只能有一个父节点，而
/// `TaffyTree::new_with_children`（taffy 0.10 `taffy_tree.rs:598`）只把子节点的
/// parent 指针改指到新节点，**不会**把子节点从旧父节点的 `children` 列表里摘掉。
/// 让第二次请求复用第一次的节点、或与它共享子节点，都会留下「旧父节点仍列着这个
/// 子节点」的残留：先表现为 `layout_bounds` 把这个子节点当成根节点、子树整体错位
/// （内容画到容器外），之后清理节点时在 `TaffyTree::remove` 的
/// `parents[child] = None` 处索引到已失效的 SlotMap 键而 panic
/// （`invalid SlotMap key used`）。
///
/// 出现序号把多次请求分成各自独立、可跨帧复用的槽位：元素树结构稳定时序号也稳定，
/// 每个槽位完整复用（比一次性临时节点更省）；序号漂移只带来重建，不会错位。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct LayoutSlot {
    key: LayoutKey,
    occurrence: u32,
}

impl LayoutSlot {
    /// 由路径键与本帧内的出现序号（从 0 起）构造槽位。
    pub(crate) fn new(key: LayoutKey, occurrence: u32) -> Self {
        Self { key, occurrence }
    }

    /// 本帧内的出现序号（0 为首次；大于 0 表示该路径本帧被重复请求）。
    pub(crate) fn occurrence(&self) -> u32 {
        self.occurrence
    }
}

/// 根种子（栈底常驻帧的键，永不弹出）。
const ROOT_SEED: u64 = 0x9e37_79b9_7f4a_7c15;

/// 栈上一层：父键 + 已分配的子序号（仅无 id 步进消耗）。
#[derive(Debug)]
struct LayoutFrame {
    key: LayoutKey,
    child_index: u32,
}

/// 窗口级布局键栈（随 `Drawable::request_layout` 压栈／弹栈）。
#[derive(Debug, Default)]
pub(crate) struct WindowLayout {
    stack: Vec<LayoutFrame>,
}

impl WindowLayout {
    /// 创建带根种子帧的空栈。
    pub(crate) fn new() -> Self {
        let mut layout = Self { stack: Vec::new() };
        layout.begin_frame();
        layout
    }

    /// 帧开始：键栈复位为只有根种子帧的一帧。
    ///
    /// 根种子帧的同级序号只在本帧内有意义。若不复位，它会跨帧累加，使根级无 id
    /// 元素的键每帧都变、保留节点永不命中（同时每帧多删一次旧节点）；
    /// 顺带兜住上一帧若有未配对的压栈残留。
    pub(crate) fn begin_frame(&mut self) {
        self.stack.clear();
        self.stack.push(LayoutFrame {
            key: LayoutKey(ROOT_SEED),
            child_index: 0,
        });
    }

    /// 进入一个元素的布局请求，返回其路径键。
    ///
    /// 有 id 按 id 哈希；无 id 按其在同级中的序号（父帧计数器推进）。
    /// 与 [`end_node`](Self::end_node) 配对，`Drawable` 保证平衡。
    pub(crate) fn begin_node(&mut self, id: Option<&crate::ElementId>) -> LayoutKey {
        if self.stack.is_empty() {
            self.stack.push(LayoutFrame {
                key: LayoutKey(ROOT_SEED),
                child_index: 0,
            });
        }
        let parent_key = self
            .stack
            .last()
            .map(|frame| frame.key)
            .unwrap_or(LayoutKey(ROOT_SEED));
        let step = match id {
            Some(id) => hash_id_step(id),
            None => {
                let frame = self.stack.last_mut().expect("布局键栈在检查后为空");
                let index = frame.child_index;
                frame.child_index = frame.child_index.wrapping_add(1);
                hash_index_step(index)
            }
        };
        let key = LayoutKey(combine(parent_key.0, step));
        self.stack.push(LayoutFrame {
            key,
            child_index: 0,
        });
        key
    }

    /// 退出一个元素的布局请求（保留根种子帧）。
    pub(crate) fn end_node(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
        }
    }

    /// 当前栈顶键（元素请求节点时使用）。
    pub(crate) fn current_key(&self) -> LayoutKey {
        self.stack
            .last()
            .map(|frame| frame.key)
            .unwrap_or(LayoutKey(ROOT_SEED))
    }
}

/// 组合父键与步进（常量加法 + 旋转混合，防简单拼接碰撞）。
fn combine(parent: u64, step: u64) -> u64 {
    parent
        .wrapping_add(step)
        .wrapping_add(0x9e37_79b9_7f4a_7c15)
        .rotate_left(17)
}

/// 有 id 步进的哈希（标签区分变体）。
fn hash_id_step(id: &crate::ElementId) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hash_element_id(&mut hasher, id);
    hasher.finish()
}

/// 无 id 步进的哈希（标签 + 同级序号）。
fn hash_index_step(index: u32) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    0xFFu8.hash(&mut hasher);
    index.hash(&mut hasher);
    hasher.finish()
}

/// 逐变体手动哈希 `ElementId`（部分变体未实现 `Hash`，如代码位置）。
fn hash_element_id(state: &mut impl Hasher, id: &crate::ElementId) {
    match id {
        crate::ElementId::View(entity) => {
            0u8.hash(state);
            entity.as_u64().hash(state);
        }
        crate::ElementId::Integer(i) => {
            1u8.hash(state);
            i.hash(state);
        }
        crate::ElementId::Name(name) => {
            2u8.hash(state);
            name.hash(state);
        }
        crate::ElementId::Uuid(uuid) => {
            3u8.hash(state);
            uuid.hash(state);
        }
        crate::ElementId::FocusHandle(focus) => {
            4u8.hash(state);
            focus.hash(state);
        }
        crate::ElementId::NamedInteger(name, i) => {
            5u8.hash(state);
            name.hash(state);
            i.hash(state);
        }
        crate::ElementId::Path(path) => {
            6u8.hash(state);
            path.hash(state);
        }
        crate::ElementId::CodeLocation(location) => {
            7u8.hash(state);
            location.file().hash(state);
            location.line().hash(state);
            location.column().hash(state);
        }
        crate::ElementId::NamedChild(parent, name) => {
            8u8.hash(state);
            hash_element_id(state, parent);
            name.hash(state);
        }
        crate::ElementId::OpaqueId(bytes) => {
            9u8.hash(state);
            bytes.hash(state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EntityId;

    /// 有 id 键稳定且区分变体；无 id 键按序号推进；嵌套释放回父键。
    #[test]
    fn keys_are_stable_and_scoped() {
        let mut layout = WindowLayout::new();
        let root = layout.current_key();

        let view_id = crate::ElementId::View(EntityId::from(7u64));
        let first = layout.begin_node(Some(&view_id));
        assert_ne!(first, root);
        // 同级无 id 子节点序号推进，键各不同。
        let child_a = layout.begin_node(None);
        layout.end_node();
        let child_b = layout.begin_node(None);
        layout.end_node();
        assert_ne!(child_a, child_b);
        layout.end_node();
        assert_eq!(layout.current_key(), root);

        // 同一 id 再次进入得到同一键。
        let again = layout.begin_node(Some(&view_id));
        layout.end_node();
        assert_eq!(first, again);
    }

    /// 根种子帧的同级序号按帧复位：根级无 id 元素跨帧复用同一个键。
    #[test]
    fn frame_reset_keeps_root_level_keys_stable() {
        let mut layout = WindowLayout::new();

        let frame_one = layout.begin_node(None);
        layout.end_node();

        layout.begin_frame();
        let frame_two = layout.begin_node(None);
        layout.end_node();
        assert_eq!(
            frame_one, frame_two,
            "帧复位后根级无 id 元素必须得到同一个键，否则保留节点永不命中"
        );

        // 同一帧内仍按同级序号区分。
        layout.begin_frame();
        let sibling_a = layout.begin_node(None);
        layout.end_node();
        let sibling_b = layout.begin_node(None);
        layout.end_node();
        assert_ne!(sibling_a, sibling_b);
    }

    /// 出现序号区分同键的多次使用，序号相同的槽位相等（跨帧可复用）。
    #[test]
    fn slots_distinguish_repeated_use() {
        let key = LayoutKey(ROOT_SEED);
        let first = LayoutSlot::new(key, 0);
        let repeated = LayoutSlot::new(key, 1);

        assert_ne!(first, repeated);
        assert_eq!(first.occurrence(), 0);
        assert_eq!(repeated.occurrence(), 1);
        assert_eq!(first, LayoutSlot::new(key, 0));
    }
}
