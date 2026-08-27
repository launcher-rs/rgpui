//! rgpui Web DOM 后端（独立 crate）。
//!
//! 依赖 `rgpui` 的 `dom-backend` feature 提供的 DOM 数据模型
//! （[`rgpui::DomTree`]、[`rgpui::DomNode`]、[`rgpui::DomNodeKey`]、[`rgpui::DomStyle`]），
//! 在此之上提供：
//!
//! - [`reconcile`]：新旧两棵 DOM 树的增量对账，产出 [`DomPatch`] 序列；
//! - [`DomBackend`] trait：平台后端抽象（wasm 实现见 [`web::WebDomBackend`]）；
//! - [`to_html`]：把 DOM 树序列化为 HTML 字符串（SSR / 调试 / 预览）。
//!
//! 架构依据 `docs/web-dom-backend-plan.md` 与 `docs/web-dom-backend-analysis.md`：
//! 核心每帧构建一棵新鲜 DOM 树，本 crate 负责与平台侧上一帧树对账、增量应用，
//! 避免把 diff 逻辑塞进核心。v1 中 DOM 层作为 canvas 之上的绝对定位覆盖层渲染
//! （接受双重绘制），不依赖浏览器布局重排。

mod css;
mod diff;

pub use diff::{DomPatch, reconcile};
pub use rgpui::{DomDisplay, DomNode, DomNodeKey, DomNodeKind, DomOverflow, DomStyle, DomTree};

#[cfg(target_family = "wasm")]
pub mod web;
#[cfg(target_family = "wasm")]
pub use web::WebDomBackend;

/// 平台 DOM 后端抽象。
///
/// 核心框架通过 [`rgpui::PlatformWindow::dom_tree_update`] 每帧交付一棵新鲜
/// [`DomTree`]，后端用 [`reconcile`] 与自身维护的上一帧树对账，再 [`apply_patches`]
/// 增量更新真实 DOM；首帧或需要全量重建时用 [`rebuild`]。
///
/// [`apply_patches`]: Self::apply_patches
/// [`rebuild`]: Self::rebuild
pub trait DomBackend {
    /// 应用一批增量补丁到 DOM。
    fn apply_patches(&mut self, patches: &[DomPatch]);

    /// 用一棵新树全量重建 DOM（首帧 / 兜底）。
    fn rebuild(&mut self, tree: &DomTree);
}

/// 把一棵 DOM 树序列化为 HTML 字符串。
///
/// 递归渲染：元素节点输出 `<tag style="...">子节点</tag>`，文本节点输出为
/// 绝对定位的 `<span>` 包装 + HTML 转义后的文本。用于 SSR（Phase 4）、
/// 调试与静态预览，与浏览器 DOM 层的节点结构保持一致。
pub fn to_html(tree: &DomTree) -> String {
    let mut out = String::new();
    render_children(tree, &tree.root, &mut out);
    out
}

/// 把 DOM 节点渲染为 HTML 片段（元素或文本）。
fn render_node(tree: &DomTree, key: &DomNodeKey, out: &mut String) {
    let Some(node) = tree.nodes.get(key) else {
        return;
    };
    match &node.kind {
        DomNodeKind::Element { tag, attrs, .. } => {
            out.push('<');
            out.push_str(tag);
            for (name, value) in attrs {
                out.push(' ');
                out.push_str(name);
                out.push_str("=\"");
                out.push_str(&escape_attr(value));
                out.push('"');
            }
            if node.style != DomStyle::default() {
                out.push_str(" style=\"");
                out.push_str(&css::dom_style_to_css(&node.style));
                out.push('"');
            }
            out.push('>');
            render_children(tree, key, out);
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
        }
        DomNodeKind::Text { text } => {
            out.push_str("<span");
            if node.style != DomStyle::default() {
                out.push_str(" style=\"");
                out.push_str(&css::dom_style_to_css(&node.style));
                out.push('"');
            }
            out.push('>');
            out.push_str(&escape_html(text));
            out.push_str("</span>");
        }
    }
}

/// 渲染某节点的全部子节点。
fn render_children(tree: &DomTree, parent: &DomNodeKey, out: &mut String) {
    if let Some(children) = tree.children.get(parent) {
        for child in children {
            render_node(tree, child, out);
        }
    }
}

/// HTML 转义文本内容。
fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// 转义属性值。
fn escape_attr(value: &str) -> String {
    escape_html(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgpui::{ElementId, GlobalElementId, Hsla, px};

    fn key(path: &[ElementId], dom_path: &[u32]) -> DomNodeKey {
        DomNodeKey {
            global_id: GlobalElementId::from_ids(path.iter().cloned()),
            dom_path: dom_path.to_vec(),
        }
    }

    fn div_node() -> DomNode {
        DomNode {
            kind: DomNodeKind::Element {
                tag: "div",
                attrs: Vec::new(),
                children: Vec::new(),
            },
            style: DomStyle {
                left: px(10.0),
                top: px(20.0),
                width: px(100.0),
                height: px(50.0),
                ..Default::default()
            },
            scroll_handle: None,
        }
    }

    fn text_node(text: &str) -> DomNode {
        DomNode {
            kind: DomNodeKind::Text {
                text: rgpui::SharedString::from(text),
            },
            style: DomStyle {
                color: Some(Hsla {
                    h: 0.0,
                    s: 0.0,
                    l: 0.2,
                    a: 1.0,
                }),
                font_size: Some(px(14.0)),
                ..Default::default()
            },
            scroll_handle: None,
        }
    }

    fn build(children: &[(DomNodeKey, DomNode)]) -> DomTree {
        let mut tree = DomTree::default();
        for (k, n) in children {
            tree.nodes.insert(k.clone(), n.clone());
        }
        tree
    }

    #[test]
    fn test_reconcile_no_changes() {
        let a = key(&[ElementId::Name("a".into())], &[]);
        let old = build(&[(a.clone(), div_node())]);
        let new = old.clone();
        let patches = reconcile(&old, &new);
        assert!(patches.is_empty());
    }

    #[test]
    fn test_reconcile_add_sibling_and_child() {
        let a = key(&[ElementId::Name("a".into())], &[]);
        let b = key(&[ElementId::Name("b".into())], &[]);
        let root = DomNodeKey::root();

        // 旧树：根 > div#a
        let mut old = build(&[(a.clone(), div_node())]);
        old.children.insert(root.clone(), vec![a.clone()]);
        old.z_orders.insert(a.clone(), 0);

        // 新树：根 > div#b（新增兄弟）+ div#a（不变）> 文本
        let text = key(&[ElementId::Name("a".into())], &[1]);
        let mut new = build(&[
            (b.clone(), div_node()),
            (a.clone(), div_node()),
            (text.clone(), text_node("hi")),
        ]);
        new.children
            .insert(root.clone(), vec![b.clone(), a.clone()]);
        new.children.insert(a.clone(), vec![text.clone()]);
        new.z_orders.insert(b.clone(), 0);
        new.z_orders.insert(a.clone(), 1);
        new.z_orders.insert(text.clone(), 2);

        let patches = reconcile(&old, &new);
        // keyed 语义下 a 跨帧保留，不应被删除
        assert!(!patches.contains(&DomPatch::RemoveNode { key: a.clone() }));
        assert!(patches.contains(&DomPatch::CreateNode {
            key: b.clone(),
            parent: root,
            index: 0,
            node: div_node(),
        }));
        // 新文本节点应被创建（父为 a）
        assert!(patches.iter().any(|p| matches!(
            p,
            DomPatch::CreateNode { key: k, parent: p, .. } if *k == text && *p == a
        )));
    }

    #[test]
    fn test_reconcile_update_style() {
        let a = key(&[ElementId::Name("a".into())], &[]);
        let root = DomNodeKey::root();
        let mut old = build(&[(a.clone(), div_node())]);
        old.children.insert(root.clone(), vec![a.clone()]);
        old.z_orders.insert(a.clone(), 0);

        let mut changed = div_node();
        changed.style.width = px(200.0);
        let mut new = build(&[(a.clone(), changed.clone())]);
        new.children.insert(root.clone(), vec![a.clone()]);
        new.z_orders.insert(a.clone(), 0);

        let patches = reconcile(&old, &new);
        assert!(patches.contains(&DomPatch::UpdateNode {
            key: a.clone(),
            node: changed,
        }));
    }

    #[test]
    fn test_to_html_roundtrip() {
        let a = key(&[ElementId::Name("root".into())], &[]);
        let text = key(&[ElementId::Name("root".into())], &[1]);
        let root = DomNodeKey::root();
        let mut tree = build(&[
            (a.clone(), div_node()),
            (text.clone(), text_node("a <b> & c")),
        ]);
        tree.children.insert(root, vec![a.clone()]);
        tree.children.insert(a.clone(), vec![text.clone()]);
        tree.z_orders.insert(a.clone(), 0);
        tree.z_orders.insert(text.clone(), 1);

        let html = to_html(&tree);
        assert!(html.contains("<div"));
        assert!(html.contains("left:10px"));
        assert!(html.contains("top:20px"));
        assert!(html.contains("width:100px"));
        assert!(html.contains("height:50px"));
        assert!(html.contains("a &lt;b&gt; &amp; c"));
    }
}
