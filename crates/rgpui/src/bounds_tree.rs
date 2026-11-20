//! 空间索引树：基于 R-tree 的区域查询数据结构，用于命中检测。

use crate::scene::PrimitiveKind;
use crate::{Bounds, Half};
use std::{
    cmp,
    fmt::Debug,
    ops::{Add, Sub},
    ptr::NonNull,
};

/// 内部节点最大子节点数（R-tree 风格的分支因子）。
/// 值越高 = 树越短 = 缓存未命中越少，但每个节点的工作量越大。
const MAX_CHILDREN: usize = 12;

/// 一种空间树，优化用于查找相交边界中的最大排序键。
///
/// 这是 R-tree 的变体，专为给重叠 UI 元素分配 z-order 的用例设计。每个边界带
/// 一个排序值与一个图元种类；共享排序值的边界按种类顺序绘制。关键优化：
/// - 跟踪具有全局最大排序键的叶节点，支持 O(1) 快速路径查询
/// - 使用更高的分支因子以降低树高度
/// - 基于 max_key 元数据在搜索过程中进行激进剪枝
#[derive(Debug)]
pub(crate) struct BoundsTree<U>
where
    U: Clone + Debug + Default + PartialEq,
{
    /// 所有节点连续存储以提高缓存效率。
    nodes: Vec<Node<U>>,
    /// 根节点索引（如果有）。
    root: Option<usize>,
    /// 具有最高排序键的叶节点索引（用于快速路径查询）。
    max_leaf: Option<usize>,
    /// 插入时用于树遍历的可复用栈。
    insert_path: Vec<usize>,
    /// 搜索操作的可复用栈。
    search_stack: Vec<NonNull<Node<U>>>,
}

/// 边界树中的节点。
#[derive(Debug, Clone)]
struct Node<U>
where
    U: Clone + Debug + Default + PartialEq,
{
    /// 包含此节点及其所有后代的边界框。
    bounds: Bounds<U>,
    /// 此子树中的最大排序键。
    max_key: Key,
    /// 节点特定数据。
    kind: NodeKind,
}

/// 边界的排序键：先按排序值，再按图元种类（决定共享同一排序值的边界的绘制顺序）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    ordering: u32,
    kind: PrimitiveKind,
}

#[derive(Debug, Clone)]
enum NodeKind {
    /// 包含实际边界数据的叶节点。
    Leaf {
        /// 分配给此边界的排序键。
        key: Key,
    },
    /// 具有子节点的内部节点。
    Internal {
        /// 子节点索引（2 到 MAX_CHILDREN）。
        children: NodeChildren,
    },
}

/// 用于子节点索引的固定大小数组，避免堆分配。
#[derive(Debug, Clone)]
struct NodeChildren {
    // 不变量：max_key 最大的子节点始终排在末尾
    indices: [usize; MAX_CHILDREN],
    len: u8,
}

impl NodeChildren {
    fn new() -> Self {
        Self {
            indices: [0; MAX_CHILDREN],
            len: 0,
        }
    }

    fn push(&mut self, index: usize) {
        debug_assert!((self.len as usize) < MAX_CHILDREN);
        self.indices[self.len as usize] = index;
        self.len += 1;
    }

    fn len(&self) -> usize {
        self.len as usize
    }

    fn as_slice(&self) -> &[usize] {
        &self.indices[..self.len as usize]
    }
}

impl<U> BoundsTree<U>
where
    U: Clone
        + Debug
        + PartialEq
        + PartialOrd
        + Add<U, Output = U>
        + Sub<Output = U>
        + Half
        + Default,
{
    /// 清除树中的所有节点。
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.root = None;
        self.max_leaf = None;
        self.insert_path.clear();
        self.search_stack.clear();
    }

    /// 将给定种类的边界插入树中并返回其分配的排序值。
    ///
    /// 排序值是与新边界相交的任何现有边界最大排序值加一；但若相交边界里排序值
    /// 最大的那些种类都不超过 `max_shared_kind`（它们在同一排序值内会先被绘制），
    /// 新边界就直接共享该最大排序值。
    pub fn insert(
        &mut self,
        new_bounds: Bounds<U>,
        kind: PrimitiveKind,
        max_shared_kind: PrimitiveKind,
    ) -> u32 {
        let ordering = match self.find_max_key(&new_bounds) {
            None => 1,
            Some(max_key) if max_key.kind <= max_shared_kind => max_key.ordering,
            Some(max_key) => max_key.ordering + 1,
        };
        let key = Key { ordering, kind };

        // Insert the new leaf
        let new_leaf_idx = self.insert_leaf(new_bounds, key);

        // Update max_leaf tracking
        self.max_leaf = match self.max_leaf {
            None => Some(new_leaf_idx),
            Some(old_idx) if self.nodes[old_idx].max_key < key => Some(new_leaf_idx),
            some => some,
        };

        ordering
    }

    /// 查找与查询相交的所有边界中的最大排序键。
    fn find_max_key(&mut self, query: &Bounds<U>) -> Option<Key> {
        let root_idx = self.root?;

        // Fast path: check if the max-key leaf intersects
        if let Some(max_idx) = self.max_leaf {
            let max_node = &self.nodes[max_idx];
            if query.intersects(&max_node.bounds) {
                return Some(max_node.max_key);
            }
        }

        // Slow path: search the tree
        self.search_stack.clear();
        self.search_stack.push(NonNull::from(&self.nodes[root_idx]));

        // `None` 排序低于任何 key。
        let mut max_found = None;

        while let Some(node) = self.search_stack.pop() {
            // SAFETY: `node` 一定有效——`nodes` 在本函数中不会被修改，
            // `search_stack` 只持有本次调用产生的指针。
            let node = unsafe { node.as_ref() };

            // Pruning: skip if this subtree can't improve our result
            if Some(node.max_key) <= max_found {
                continue;
            }

            // Spatial pruning: skip if bounds don't intersect
            if !query.intersects(&node.bounds) {
                continue;
            }

            match &node.kind {
                NodeKind::Leaf { key } => {
                    max_found = cmp::max(max_found, Some(*key));
                }
                NodeKind::Internal { children } => {
                    // 子节点按 max_key 升序维护（最大在末尾）。
                    // 正序压栈，最大的（栈顶）先弹出。
                    self.search_stack.extend(
                        children
                            .as_slice()
                            .iter()
                            .map(|&child_idx| &self.nodes[child_idx])
                            .filter(|node| Some(node.max_key) > max_found)
                            .map(NonNull::from),
                    );
                }
            }
        }

        max_found
    }

    /// 插入一个具有给定边界和排序键的叶节点。
    /// 返回新叶节点的索引。
    fn insert_leaf(&mut self, bounds: Bounds<U>, key: Key) -> usize {
        let new_leaf_idx = self.nodes.len();
        self.nodes.push(Node {
            bounds: bounds.clone(),
            max_key: key,
            kind: NodeKind::Leaf { key },
        });

        let Some(root_idx) = self.root else {
            // Tree is empty, new leaf becomes root
            self.root = Some(new_leaf_idx);
            return new_leaf_idx;
        };

        // If root is a leaf, create internal node with both
        if matches!(self.nodes[root_idx].kind, NodeKind::Leaf { .. }) {
            let root_bounds = self.nodes[root_idx].bounds.clone();
            let root_key = self.nodes[root_idx].max_key;

            let mut children = NodeChildren::new();
            // Max end invariant
            if key > root_key {
                children.push(root_idx);
                children.push(new_leaf_idx);
            } else {
                children.push(new_leaf_idx);
                children.push(root_idx);
            }

            let new_root_idx = self.nodes.len();
            self.nodes.push(Node {
                bounds: root_bounds.union(&bounds),
                max_key: cmp::max(root_key, key),
                kind: NodeKind::Internal { children },
            });
            self.root = Some(new_root_idx);
            return new_leaf_idx;
        }

        // Descend to find the best internal node to insert into
        self.insert_path.clear();
        let mut current_idx = root_idx;

        loop {
            let current = &self.nodes[current_idx];
            let NodeKind::Internal { children } = &current.kind else {
                unreachable!("Should only traverse internal nodes");
            };

            self.insert_path.push(current_idx);

            // Find the best child to descend into
            let mut best_child_idx = children.as_slice()[0];
            let mut best_child_pos = 0;
            let mut best_cost = bounds
                .union(&self.nodes[best_child_idx].bounds)
                .half_perimeter();

            for (pos, &child_idx) in children.as_slice().iter().enumerate().skip(1) {
                let cost = bounds.union(&self.nodes[child_idx].bounds).half_perimeter();
                if cost < best_cost {
                    best_cost = cost;
                    best_child_idx = child_idx;
                    best_child_pos = pos;
                }
            }

            // Check if best child is a leaf or internal
            if matches!(self.nodes[best_child_idx].kind, NodeKind::Leaf { .. }) {
                // Best child is a leaf. Check if current node has room for another child.
                if children.len() < MAX_CHILDREN {
                    // Add new leaf directly to this node
                    let node = &mut self.nodes[current_idx];

                    if let NodeKind::Internal { children } = &mut node.kind {
                        children.push(new_leaf_idx);
                        // 仅当新叶的 max_key 最大时才交换到末尾
                        if key <= node.max_key {
                            let last = children.len() - 1;
                            children.indices.swap(last - 1, last);
                        }
                    }

                    node.bounds = node.bounds.union(&bounds);
                    node.max_key = cmp::max(node.max_key, key);
                    break;
                } else {
                    // Node is full, create new internal with [best_leaf, new_leaf]
                    let sibling_bounds = self.nodes[best_child_idx].bounds.clone();
                    let sibling_key = self.nodes[best_child_idx].max_key;

                    let mut new_children = NodeChildren::new();
                    // Max end invariant
                    if key > sibling_key {
                        new_children.push(best_child_idx);
                        new_children.push(new_leaf_idx);
                    } else {
                        new_children.push(new_leaf_idx);
                        new_children.push(best_child_idx);
                    }

                    let new_internal_idx = self.nodes.len();
                    let new_internal_max_key = cmp::max(sibling_key, key);
                    self.nodes.push(Node {
                        bounds: sibling_bounds.union(&bounds),
                        max_key: new_internal_max_key,
                        kind: NodeKind::Internal {
                            children: new_children,
                        },
                    });

                    // Replace the leaf with the new internal in parent
                    let parent = &mut self.nodes[current_idx];
                    if let NodeKind::Internal { children } = &mut parent.kind {
                        let children_len = children.len();

                        children.indices[best_child_pos] = new_internal_idx;

                        // 若新内部节点 max_key 最大，交换到末尾以保持排序不变量
                        if new_internal_max_key > parent.max_key {
                            children.indices.swap(best_child_pos, children_len - 1);
                        }
                    }
                    break;
                }
            } else {
                // Best child is internal, continue descent
                current_idx = best_child_idx;
            }
        }

        // 向上传播边界与 max_key 更新
        let mut updated_child_idx = None;
        for &node_idx in self.insert_path.iter().rev() {
            let node = &mut self.nodes[node_idx];
            node.bounds = node.bounds.union(&bounds);

            if node.max_key < key {
                node.max_key = key;

                // Swap updated child to end (skip first iteration since the invariant is already handled by previous cases)
                if let Some(child_idx) = updated_child_idx {
                    if let NodeKind::Internal { children } = &mut node.kind {
                        if let Some(pos) = children.as_slice().iter().position(|&c| c == child_idx)
                        {
                            let last = children.len() - 1;
                            if pos != last {
                                children.indices.swap(pos, last);
                            }
                        }
                    }
                }
            }

            updated_child_idx = Some(node_idx);
        }

        new_leaf_idx
    }
}

impl<U> Default for BoundsTree<U>
where
    U: Clone + Debug + Default + PartialEq,
{
    fn default() -> Self {
        BoundsTree {
            nodes: Vec::new(),
            root: None,
            max_leaf: None,
            insert_path: Vec::new(),
            search_stack: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bounds, Point, Size};
    use rand::{RngExt, SeedableRng};

    #[test]
    fn test_insert() {
        let mut tree = BoundsTree::<f32>::default();
        let (quad, shadow) = (PrimitiveKind::Quad, PrimitiveKind::Shadow);
        let bounds1 = Bounds {
            origin: Point { x: 0.0, y: 0.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };
        let bounds2 = Bounds {
            origin: Point { x: 5.0, y: 5.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };
        let bounds3 = Bounds {
            origin: Point { x: 10.0, y: 10.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };

        // Insert the bounds into the tree and verify the order is correct
        assert_eq!(tree.insert(bounds1, quad, shadow), 1);
        assert_eq!(tree.insert(bounds2, quad, shadow), 2);
        assert_eq!(tree.insert(bounds3, quad, shadow), 3);

        // Insert non-overlapping bounds and verify they can reuse orders
        let bounds4 = Bounds {
            origin: Point { x: 20.0, y: 20.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };
        let bounds5 = Bounds {
            origin: Point { x: 40.0, y: 40.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };
        let bounds6 = Bounds {
            origin: Point { x: 25.0, y: 25.0 },
            size: Size {
                width: 10.0,
                height: 10.0,
            },
        };
        assert_eq!(tree.insert(bounds4, quad, shadow), 1); // bounds4 does not overlap with bounds1, bounds2, or bounds3
        assert_eq!(tree.insert(bounds5, quad, shadow), 1); // bounds5 does not overlap with any other bounds
        assert_eq!(tree.insert(bounds6, quad, shadow), 2); // bounds6 overlaps with bounds4, so it should have a different order
    }

    #[test]
    fn test_random_iterations() {
        let max_bounds = 100;
        for seed in 1..=1000 {
            // let seed = 44;
            let mut tree = BoundsTree::default();
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed as u64);
            let mut expected_quads: Vec<(Bounds<f32>, u32, PrimitiveKind)> = Vec::new();
            let kinds = [
                PrimitiveKind::Shadow,
                PrimitiveKind::Quad,
                PrimitiveKind::Path,
                PrimitiveKind::Underline,
            ];

            // Insert a random number of random AABBs into the tree.
            let num_bounds = rng.random_range(1..=max_bounds);
            for _ in 0..num_bounds {
                let min_x: f32 = rng.random_range(-100.0..100.0);
                let min_y: f32 = rng.random_range(-100.0..100.0);
                let width: f32 = rng.random_range(0.0..50.0);
                let height: f32 = rng.random_range(0.0..50.0);
                let bounds = Bounds {
                    origin: Point { x: min_x, y: min_y },
                    size: Size { width, height },
                };
                let kind = kinds[rng.random_range(0..kinds.len())];
                let max_shared_kind = kinds[rng.random_range(0..kinds.len())];

                // 朴素口径：相交边界里的最大 (排序值, 种类)；种类可共享则复用排序值。
                let max_intersecting = expected_quads
                    .iter()
                    .filter(|quad| quad.0.intersects(&bounds))
                    .map(|quad| (quad.1, quad.2))
                    .max();
                let expected_ordering = match max_intersecting {
                    None => 1,
                    Some((ordering, max_kind)) if max_kind <= max_shared_kind => ordering,
                    Some((ordering, _)) => ordering + 1,
                };
                expected_quads.push((bounds, expected_ordering, kind));

                // Insert the AABB into the tree and collect intersections.
                let actual_ordering = tree.insert(bounds, kind, max_shared_kind);
                assert_eq!(actual_ordering, expected_ordering);
            }
        }
    }
}
