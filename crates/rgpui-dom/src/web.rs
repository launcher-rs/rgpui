//! Web（wasm）DOM 后端：把 [`rgpui::DomTree`] 同步到浏览器真实 DOM。
//!
//! DOM 层是 canvas 之上的绝对定位覆盖层（`position:absolute; inset:0;
//! pointer-events:none`）。**不做双重绘制**：canvas 负责全部形状（背景/边框/图标等），
//! DOM 层只负责文本 span（`pointer-events:auto`，浏览器原生提供选择/复制/IME），
//! 元素/容器节点只是透明的定位结构。每帧先与上一帧树对账（[`reconcile`]），
//! 再增量应用补丁。
//!
//! 为保证文本颜色/字形与应用一致，挂载覆盖层时会把应用通过
//! [`rgpui::set_dom_font_face`] 注册的内嵌字体注入为 `@font-face`；
//! 同时把落在覆盖层上的指针/滚轮事件转发到 canvas，保证应用交互不受覆盖层遮挡。

use crate::{DomBackend, DomPatch, css, reconcile};
use rgpui::{DomNode, DomNodeKey, DomNodeKind, DomTree};
use std::collections::HashMap;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, Element, Event, MouseEvent, Node, PointerEvent, WheelEvent};

/// 覆盖层宿主元素上的标识属性（便于调试/样式定位）。
const HOST_ATTR: &str = "data-gpui-dom-layer";
/// 每个节点上的反查属性（数值 id）。
const NODE_ATTR: &str = "data-gpui-id";
/// 需要从覆盖层转发到 canvas 的事件类型（指针/滚轮）。
const FORWARD_EVENTS: &[&str] = &["pointerdown", "pointerup", "pointermove", "wheel"];
/// 转发到 canvas 的合成事件标记（当前仅用于去重/排查）。
const FORWARD_MARK: &str = "__gpuiDomForwarded";

/// 按 RFC 4648 把字节编码为 base64（用于 `@font-face` 的 data URI）。
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let first = chunk[0] as u32;
        let second = *chunk.get(1).unwrap_or(&0) as u32;
        let third = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (first << 16) | (second << 8) | third;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Web DOM 后端。
pub struct WebDomBackend {
    document: Document,
    /// 覆盖层宿主元素（绝对定位，接收全部子节点）。
    host: Element,
    /// key -> 对应 DOM 元素（元素节点为标签元素，文本节点为 span 包装）。
    elements: HashMap<DomNodeKey, Element>,
    /// 上一帧树，用于对账。
    last_tree: DomTree,
    /// 事件转发闭包（需持有以保活）。
    _forwarder: Option<Closure<dyn FnMut(Event)>>,
}

impl WebDomBackend {
    /// 以给定的宿主元素创建后端（宿主需挂在 document 下）。
    pub fn new(host: Element) -> Self {
        let document = web_sys::window()
            .expect("WebDomBackend 只能在浏览器中创建")
            .document()
            .expect("浏览器没有 document");
        let mut backend = Self {
            document,
            host,
            elements: HashMap::new(),
            last_tree: DomTree::default(),
            _forwarder: None,
        };
        backend.inject_font_faces();
        backend.install_event_forwarder();
        backend
    }

    /// 便捷构造：创建并挂载覆盖层宿主到 `document.body`。
    pub fn attach_default() -> Self {
        let document = web_sys::window()
            .expect("WebDomBackend 只能在浏览器中创建")
            .document()
            .expect("浏览器没有 document");
        let host = document.create_element("div").expect("创建覆盖层宿主失败");
        host.set_attribute(HOST_ATTR, "true")
            .expect("设置宿主属性失败");
        host.set_attribute(
            "style",
            "position:absolute;inset:0;pointer-events:none;overflow:hidden;",
        )
        .expect("设置宿主样式失败");
        document
            .body()
            .expect("没有 body")
            .append_child(&host)
            .expect("挂载覆盖层宿主失败");
        Self::new(host)
    }

    /// 把应用注册的内嵌字体注入为 `@font-face`，使覆盖层文本与 canvas 使用同一字面。
    fn inject_font_faces(&self) {
        let faces = rgpui::dom_font_faces();
        if faces.is_empty() {
            return;
        }
        let mut css_text = String::new();
        for face in faces {
            css_text.push_str(&format!(
                "@font-face{{font-family:\"{}\";src:url(data:font/ttf;base64,{}) format(\"truetype\");font-weight:400;font-style:normal;font-display:swap;}}",
                face.family,
                base64_encode(&face.data)
            ));
        }
        if let Ok(style_el) = self.document.create_element("style") {
            style_el.set_text_content(Some(&css_text));
            if let Some(head) = self.document.head() {
                let _ = head.append_child(style_el.unchecked_ref());
            }
        }
    }

    /// 安装文档级捕获转发器：把落在覆盖层（文本 span）上的指针/滚轮事件转发到
    /// canvas，使应用交互不受覆盖层遮挡，同时保留 span 上的原生文本选择。
    fn install_event_forwarder(&mut self) {
        let document = self.document.clone();
        let host = self.host.clone();
        let closure = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            // 事件是否落在覆盖层内（文本 span 开启了 pointer-events:auto）。
            let Some(target) = event.target() else { return };
            let target_node: &Node = target.unchecked_ref();
            if !host.contains(Some(target_node)) {
                return;
            }
            // 找到应用 canvas；覆盖层文本必须与 canvas 同源定位。
            let Ok(Some(canvas)) = document.query_selector("canvas") else {
                return;
            };

            let event_type = event.type_();
            let mouse: &MouseEvent = event.unchecked_ref();
            let client_x = mouse.client_x();
            let client_y = mouse.client_y();
            let synthetic: Option<Event> = match event_type.as_str() {
                "pointerdown" | "pointerup" | "pointermove" => {
                    let init = web_sys::PointerEventInit::new();
                    init.set_client_x(client_x);
                    init.set_client_y(client_y);
                    init.set_button(mouse.button());
                    init.set_buttons(mouse.buttons());
                    init.set_bubbles(true);
                    init.set_cancelable(true);
                    PointerEvent::new_with_event_init_dict(&event_type, &init)
                        .ok()
                        .map(|e| e.unchecked_into())
                }
                "wheel" => {
                    let wheel: &WheelEvent = event.unchecked_ref();
                    let init = web_sys::WheelEventInit::new();
                    init.set_delta_x(wheel.delta_x());
                    init.set_delta_y(wheel.delta_y());
                    init.set_delta_mode(wheel.delta_mode());
                    init.set_client_x(client_x);
                    init.set_client_y(client_y);
                    init.set_bubbles(true);
                    init.set_cancelable(true);
                    WheelEvent::new_with_event_init_dict(&event_type, &init)
                        .ok()
                        .map(|e| e.unchecked_into())
                }
                _ => None,
            };
            if let Some(synthetic) = synthetic {
                js_sys::Reflect::set(synthetic.as_ref(), &FORWARD_MARK.into(), &JsValue::TRUE).ok();
                let _ = canvas.dispatch_event(&synthetic);
            }
        });

        for event_name in FORWARD_EVENTS {
            let _ = self
                .document
                .add_event_listener_with_callback(event_name, closure.as_ref().unchecked_ref());
        }
        self._forwarder = Some(closure);
    }

    /// 用一棵新树增量更新 DOM（对账 + 应用）。
    ///
    /// 首帧时 `last_tree` 为空，等价于全量创建。
    pub fn update(&mut self, tree: &DomTree) {
        let patches = reconcile(&self.last_tree, tree);
        self.apply_patches(&patches);
        self.last_tree = tree.clone();
    }

    /// 创建单个 DOM 元素并打上 `data-gpui-id`。
    fn create_dom_node(&self, node: &DomNode, key: &DomNodeKey) -> Element {
        match &node.kind {
            DomNodeKind::Element { tag, attrs } => {
                let element = self.document.create_element(tag).expect("创建元素节点失败");
                for (name, value) in attrs {
                    let _ = element.set_attribute(name, value);
                }
                element
                    .set_attribute(NODE_ATTR, &key.to_dom_id())
                    .expect("设置 data-gpui-id 失败");
                element
            }
            DomNodeKind::Text { text } => {
                let span = self
                    .document
                    .create_element("span")
                    .expect("创建文本包装节点失败");
                span.unchecked_ref::<Node>()
                    .set_text_content(Some(text.as_ref()));
                span.set_attribute(NODE_ATTR, &key.to_dom_id())
                    .expect("设置 data-gpui-id 失败");
                span
            }
        }
    }

    /// 把节点的样式写入元素的 `style` 属性。
    ///
    /// 元素/容器节点只输出结构样式（透明，视觉由 canvas 绘制）；文本 span 额外开启
    /// `pointer-events:auto` 与 `user-select:text`，使浏览器原生文本选择生效
    /// （其余覆盖层区域保持穿透到 canvas）。
    fn apply_style(element: &Element, node: &DomNode) {
        let mut css_text = match &node.kind {
            DomNodeKind::Element { .. } => css::dom_structure_to_css(&node.style),
            DomNodeKind::Text { .. } => css::dom_style_to_css(&node.style),
        };
        if matches!(&node.kind, DomNodeKind::Text { .. }) {
            css_text.push_str(";pointer-events:auto;user-select:text");
        }
        element
            .set_attribute("style", &css_text)
            .expect("写入内联样式失败");
    }

    /// 递归创建 `key` 的整棵子树并挂到 `parent` 下。
    fn build_subtree(&mut self, tree: &DomTree, key: &DomNodeKey, parent: &Node) {
        let Some(node) = tree.nodes.get(key) else {
            return;
        };
        let element = self.create_dom_node(node, key);
        Self::apply_style(&element, node);
        parent
            .append_child(element.unchecked_ref())
            .expect("挂载节点失败");
        self.elements.insert(key.clone(), element.clone());
        if let Some(children) = tree.children.get(key) {
            for child in children {
                self.build_subtree(tree, child, element.unchecked_ref());
            }
        }
    }
}

impl DomBackend for WebDomBackend {
    fn apply_patches(&mut self, patches: &[DomPatch]) {
        for patch in patches {
            match patch {
                DomPatch::CreateNode {
                    key,
                    parent,
                    index,
                    node,
                } => {
                    let element = self.create_dom_node(node, key);
                    Self::apply_style(&element, node);
                    let parent_element = self
                        .elements
                        .get(parent)
                        .cloned()
                        .unwrap_or_else(|| self.host.clone());
                    let parent_node = parent_element.unchecked_ref::<Node>();
                    let child_nodes = parent_node.child_nodes();
                    if let Some(reference) = child_nodes.get(*index as u32) {
                        parent_node
                            .insert_before(element.unchecked_ref(), Some(&reference))
                            .expect("插入节点失败");
                    } else {
                        parent_node
                            .append_child(element.unchecked_ref())
                            .expect("追加节点失败");
                    }
                    self.elements.insert(key.clone(), element);
                }
                DomPatch::UpdateNode { key, node } => {
                    if let Some(element) = self.elements.get(key) {
                        Self::apply_style(element, node);
                        if let DomNodeKind::Text { text } = &node.kind {
                            element
                                .unchecked_ref::<Node>()
                                .set_text_content(Some(text.as_ref()));
                        }
                    }
                }
                DomPatch::RemoveNode { key } => {
                    if let Some(element) = self.elements.remove(key) {
                        element.remove();
                    }
                }
            }
        }
    }

    fn rebuild(&mut self, tree: &DomTree) {
        self.host.unchecked_ref::<Node>().set_text_content(Some(""));
        self.elements.clear();
        let host = self.host.clone();
        self.build_subtree(tree, &tree.root, &host);
        self.last_tree = tree.clone();
    }
}
