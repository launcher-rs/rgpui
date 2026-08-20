//! DOM 树增量对账：比较新旧两棵 [`rgpui::DomTree`]，产出 [`DomPatch`] 序列。

use rgpui::{DomNode, DomNodeKey, DomTree};
use rustc_hash::FxHashSet;

/// 对一棵 DOM 树的增量修改。
///
/// 按序应用即可把平台侧上一帧的 DOM 同步到本帧状态：
/// - [`CreateNode`] 创建节点（含整棵新子树，后代以追加的 CreateNode 逐层创建）；
/// - [`UpdateNode`] 原地更新节点内容/样式；
/// - [`RemoveNode`] 删除节点及其整棵子树（平台侧递归清理）。
///
/// [`CreateNode`]: Self::CreateNode
/// [`UpdateNode`]: Self::UpdateNode
/// [`RemoveNode`]: Self::RemoveNode
#[derive(Clone, Debug, PartialEq)]
pub enum DomPatch {
    /// 在 `parent` 下的 `index` 位置创建 `key` 节点。
    CreateNode {
        /// 新节点 key。
        key: DomNodeKey,
        /// 父节点 key。
        parent: DomNodeKey,
        /// 插入位置（父的子节点列表下标）。
        index: usize,
        /// 新节点内容。
        node: DomNode,
    },
    /// 更新已有节点（key 不变，内容/样式可能变化）。
    UpdateNode {
        /// 目标节点 key。
        key: DomNodeKey,
        /// 最新内容。
        node: DomNode,
    },
    /// 删除节点（含整棵子树）。
    RemoveNode {
        /// 目标节点 key。
        key: DomNodeKey,
    },
}

/// 对账 `old` 与 `new` 两棵 DOM 树，返回按序应用的增量补丁。
///
/// 算法：按 `children` 表递归深度优先；两树同 key 的节点做 `DomNode` 相等比较
/// （不等则 UpdateNode），随后递归其子节点；仅存在于新树的节点产出 CreateNode
/// （并递归其新后代）；仅存在于旧树的节点产出 RemoveNode。key 跨帧稳定，因此
/// 语义等价于 React 的 keyed diff 简化版；v1 不做兄弟重排，顺序错位只走原地更新。
pub fn reconcile(old: &DomTree, new: &DomTree) -> Vec<DomPatch> {
    let mut patches = Vec::new();
    diff_children(old, new, &DomNodeKey::root(), &mut patches);
    patches
}

/// 对账父节点 `parent_key` 下的子节点列表。
fn diff_children(
    old: &DomTree,
    new: &DomTree,
    parent_key: &DomNodeKey,
    patches: &mut Vec<DomPatch>,
) {
    let old_children: Vec<DomNodeKey> = old.children.get(parent_key).cloned().unwrap_or_default();
    let new_children: Vec<DomNodeKey> = new.children.get(parent_key).cloned().unwrap_or_default();

    let new_set: FxHashSet<DomNodeKey> = new_children.iter().cloned().collect();
    let old_set: FxHashSet<DomNodeKey> = old_children.iter().cloned().collect();

    // 1) 删除与原地更新（仅遍历两树都存在的节点，避免对已删子树重复处理）。
    for child in &old_children {
        if !new_set.contains(child) {
            patches.push(DomPatch::RemoveNode { key: child.clone() });
        } else {
            let old_node = &old.nodes[child];
            let new_node = &new.nodes[child];
            if old_node != new_node {
                patches.push(DomPatch::UpdateNode {
                    key: child.clone(),
                    node: new_node.clone(),
                });
            }
            diff_children(old, new, child, patches);
        }
    }

    // 2) 创建（含新子树的整棵创建：递归发出后代 CreateNode）。
    for (index, child) in new_children.iter().enumerate() {
        if !old_set.contains(child) {
            if let Some(node) = new.nodes.get(child) {
                patches.push(DomPatch::CreateNode {
                    key: child.clone(),
                    parent: parent_key.clone(),
                    index,
                    node: node.clone(),
                });
            }
            diff_children(old, new, child, patches);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgpui::{DomNodeKind, DomStyle, ElementId, GlobalElementId, Hsla};

    fn key(path: &[ElementId], dom_path: &[u32]) -> DomNodeKey {
        DomNodeKey {
            global_id: GlobalElementId::from_ids(path.iter().cloned()),
            dom_path: dom_path.to_vec(),
        }
    }

    fn node(tag: &'static str) -> DomNode {
        DomNode {
            kind: DomNodeKind::Element {
                tag,
                attrs: Vec::new(),
            },
            style: DomStyle::default(),
        }
    }

    /// 构造一棵简单树：root > 若干子节点。
    fn tree(children: Vec<(DomNodeKey, DomNode)>) -> DomTree {
        let mut tree = DomTree::default();
        let mut ordered = Vec::new();
        for (k, n) in children {
            tree.nodes.insert(k.clone(), n);
            ordered.push(k);
        }
        tree.children.insert(DomNodeKey::root(), ordered);
        tree
    }

    #[test]
    fn test_reconcile_create() {
        let a = key(&[ElementId::Name("a".into())], &[]);
        let old = tree(Vec::new());
        let new = tree(vec![(a.clone(), node("div"))]);
        let patches = reconcile(&old, &new);
        assert_eq!(
            patches,
            vec![DomPatch::CreateNode {
                key: a.clone(),
                parent: DomNodeKey::root(),
                index: 0,
                node: node("div"),
            }]
        );
    }

    #[test]
    fn test_reconcile_remove() {
        let a = key(&[ElementId::Name("a".into())], &[]);
        let old = tree(vec![(a.clone(), node("div"))]);
        let new = tree(Vec::new());
        let patches = reconcile(&old, &new);
        assert_eq!(patches, vec![DomPatch::RemoveNode { key: a }]);
    }

    #[test]
    fn test_reconcile_identical_no_patches() {
        let a = key(&[ElementId::Name("a".into())], &[]);
        let b = key(&[ElementId::Name("b".into())], &[1]);
        let mut tree = DomTree::default();
        tree.nodes.insert(a.clone(), node("div"));
        tree.nodes.insert(b.clone(), node("div"));
        tree.children.insert(DomNodeKey::root(), vec![a.clone()]);
        tree.children.insert(a.clone(), vec![b.clone()]);
        let patches = reconcile(&tree, &tree);
        assert!(patches.is_empty());
    }

    #[test]
    fn test_reconcile_nested_create_and_remove() {
        let a = key(&[ElementId::Name("a".into())], &[]);
        let b = key(&[ElementId::Name("b".into())], &[1]);

        // 旧树：root > a > b
        let mut old = DomTree::default();
        old.nodes.insert(a.clone(), node("div"));
        old.nodes.insert(b.clone(), node("div"));
        old.children.insert(DomNodeKey::root(), vec![a.clone()]);
        old.children.insert(a.clone(), vec![b.clone()]);

        // 新树：root > a（无 b 子节点）
        let mut new = DomTree::default();
        new.nodes.insert(a.clone(), node("div"));
        new.children.insert(DomNodeKey::root(), vec![a.clone()]);

        let patches = reconcile(&old, &new);
        // 只应删除 b，不重复删除 a
        assert_eq!(patches, vec![DomPatch::RemoveNode { key: b }]);
    }

    #[test]
    fn test_reconcile_style_update_only() {
        let a = key(&[ElementId::Name("a".into())], &[]);
        let old = tree(vec![(a.clone(), node("div"))]);
        let mut new_node = node("div");
        new_node.style.background_color = Some(Hsla {
            h: 0.5,
            s: 0.5,
            l: 0.5,
            a: 1.0,
        });
        let new = tree(vec![(a.clone(), new_node.clone())]);
        let patches = reconcile(&old, &new);
        assert_eq!(
            patches,
            vec![DomPatch::UpdateNode {
                key: a,
                node: new_node,
            }]
        );
    }

    #[test]
    fn test_reconcile_text_content_update() {
        let a = key(&[ElementId::Name("a".into())], &[]);
        let mut t1 = node("div");
        t1.kind = DomNodeKind::Text { text: "old".into() };
        let mut t2 = node("div");
        t2.kind = DomNodeKind::Text { text: "new".into() };
        let old = tree(vec![(a.clone(), t1.clone())]);
        let new = tree(vec![(a.clone(), t2.clone())]);
        let patches = reconcile(&old, &new);
        assert_eq!(patches, vec![DomPatch::UpdateNode { key: a, node: t2 }]);
    }
}
