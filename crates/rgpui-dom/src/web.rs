//! Web（wasm）DOM 后端：把 [`rgpui::DomTree`] 同步到浏览器真实 DOM。
//!
//! DOM 层是 canvas 之上的绝对定位覆盖层（`position:absolute; inset:0;
//! pointer-events:auto`）。**不做双重绘制**：canvas 负责全部形状（背景/边框/图标等），
//! DOM 层只负责文本 span（浏览器原生提供选择/复制/IME），元素/容器节点只是
//! 透明的定位结构。每帧先与上一帧树对账（[`reconcile`]），再增量应用补丁。
//!
//! 覆盖层上所有节点均可命中（纯 DOM 模式下 DOM 层是主渲染器）：点击带
//! `data-gpui-id` 的节点时按 key 链**委托**给核心（绕过坐标 hit-test，避免滚动/
//! 缩放下的错位）；未命中委托的点击回退为转发到 canvas 做坐标命中。
//!
//! 为保证文本颜色/字形与应用一致，挂载覆盖层时会把应用通过
//! [`rgpui::set_dom_font_face`] 注册的内嵌字体注入为 `@font-face`。

use crate::{DomBackend, DomPatch, css, reconcile};
use rgpui::{DomNode, DomNodeKey, DomNodeKind, DomTree};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
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

/// 读取节点的 `data-gpui-id` 属性。
///
/// web-sys 在 wasm 与宿主（非 wasm 的 check/test）下返回类型不同：
/// wasm 下为 `Option<String>`，宿主下为 `Result<Option<String>, JsValue>`，
/// 用 cfg 分叉统一为 `Option<String>`。
#[cfg(target_family = "wasm")]
fn get_gpui_id(element: &Element) -> Option<String> {
    element.get_attribute(NODE_ATTR)
}

#[cfg(not(target_family = "wasm"))]
fn get_gpui_id(element: &Element) -> Option<String> {
    element.get_attribute(NODE_ATTR).ok().flatten()
}

/// Web DOM 后端。
pub struct WebDomBackend {
    document: Document,
    /// 覆盖层宿主元素（绝对定位，接收全部子节点）。
    host: Element,
    /// key -> 对应 DOM 元素（元素节点为标签元素，文本节点为 span 包装）。
    elements: HashMap<DomNodeKey, Element>,
    /// key -> 上次写入的 `style` 属性串（去重缓存，未变化时跳过 setAttribute）。
    styles: HashMap<DomNodeKey, String>,
    /// 上一帧树，用于对账。
    last_tree: DomTree,
    /// `data-gpui-id`（数值串） -> 反查的 DOM key（事件委托用，每帧刷新）。
    id_to_key: Rc<RefCell<HashMap<String, DomNodeKey>>>,
    /// 事件委托回调（点击 DOM 元素时按 key 链回调核心）。
    ///
    /// 由平台（`rgpui-web`）在创建后端后通过 [`Self::set_dom_event_handler`] 注入。
    handler: Rc<RefCell<Option<Box<dyn FnMut(Vec<DomNodeKey>, Event)>>>>,
    /// 原生滚动同步回调（可滚动容器 `scroll` 事件后回传 key 链与 scrollLeft/Top）。
    ///
    /// 由平台（`rgpui-web`）在创建后端后通过 [`Self::set_dom_scroll_handler`] 注入。
    scroll_handler: Rc<RefCell<Option<Box<dyn FnMut(Vec<DomNodeKey>, f64, f64)>>>>,
    /// 事件转发闭包（需持有以保活）。
    _forwarder: Option<Closure<dyn FnMut(Event)>>,
    /// 滚动监听闭包（需持有以保活）。
    _scroll_forwarder: Option<Closure<dyn FnMut(Event)>>,
    /// 复制监听闭包（需持有以保活）。
    _copy_handler: Option<Closure<dyn FnMut(Event)>>,
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
            styles: HashMap::new(),
            last_tree: DomTree::default(),
            id_to_key: Rc::new(RefCell::new(HashMap::new())),
            handler: Rc::new(RefCell::new(None)),
            scroll_handler: Rc::new(RefCell::new(None)),
            _forwarder: None,
            _scroll_forwarder: None,
            _copy_handler: None,
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
            "position:absolute;inset:0;pointer-events:auto;overflow:hidden;",
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

    /// 设置事件委托回调：点击覆盖层上的 DOM 元素时，按 `data-gpui-id` 反查 key 链并回调。
    ///
    /// `keys` 为文档序（根 → 叶）的 key 链，`event` 为原始 DOM 事件。
    pub fn set_dom_event_handler(&self, handler: Box<dyn FnMut(Vec<DomNodeKey>, Event)>) {
        *self.handler.borrow_mut() = Some(handler);
    }

    /// 设置原生滚动同步回调：可滚动容器发生 `scroll` 事件后，回传 (key 链, scrollLeft, scrollTop)。
    pub fn set_dom_scroll_handler(&self, handler: Box<dyn FnMut(Vec<DomNodeKey>, f64, f64)>) {
        *self.scroll_handler.borrow_mut() = Some(handler);
    }

    /// 从事件目标向上收集 `data-gpui-id` 链（根 → 叶），并反查为 DOM key 链。
    ///
    /// 目标必须位于覆盖层宿主内；没有带 `data-gpui-id` 的祖先（如点击 canvas 本身）
    /// 时返回 `None`，调用方回退到坐标转发。
    fn collect_key_chain(
        target: &Node,
        host: &Element,
        id_to_key: &Rc<RefCell<HashMap<String, DomNodeKey>>>,
    ) -> Option<Vec<DomNodeKey>> {
        let mut ids = Vec::<String>::new();
        let mut current = Some(target.clone());
        while let Some(node) = current {
            if !host.contains(Some(&node)) {
                break;
            }
            if let Some(element) = node.dyn_ref::<Element>()
                && let Some(id) = get_gpui_id(element)
            {
                ids.push(id);
            }
            current = node.parent_node();
        }
        if ids.is_empty() {
            return None;
        }
        // 反转为文档序（根 → 叶）并把 id 映射回 key。
        let map = id_to_key.borrow();
        let mut keys = Vec::new();
        for id in ids.iter().rev() {
            if let Some(key) = map.get(id) {
                keys.push(key.clone());
            }
        }
        if keys.is_empty() { None } else { Some(keys) }
    }

    /// 判断事件目标（或祖先）是否存在可原生滚动的元素（`overflow: scroll/auto` 且内容可溢出）。
    fn is_scrollable_target(target: &Node, host: &Element) -> bool {
        let mut current = Some(target.clone());
        while let Some(node) = current {
            if !host.contains(Some(&node)) {
                break;
            }
            if let Some(el) = node.dyn_ref::<Element>() {
                if let Some(win) = web_sys::window() {
                    if let Ok(Some(style)) = win.get_computed_style(el) {
                        let oy = style.get_property_value("overflow-y").unwrap_or_default();
                        let ox = style.get_property_value("overflow-x").unwrap_or_default();
                        let scrollable_y = (oy == "scroll" || oy == "auto")
                            && el.scroll_height() > el.client_height();
                        let scrollable_x = (ox == "scroll" || ox == "auto")
                            && el.scroll_width() > el.client_width();
                        if scrollable_y || scrollable_x {
                            return true;
                        }
                    }
                }
            }
            current = node.parent_node();
        }
        false
    }

    /// 安装文档级捕获转发器：把落在覆盖层（文本 span）上的指针/滚轮事件转发到
    /// canvas，使应用交互不受覆盖层遮挡，同时保留 span 上的原生文本选择。
    ///
    /// 若目标（或祖先）带 `data-gpui-id` 且已注入事件委托回调，则直接按 key 链
    /// 委托给核心（跳过坐标命中，避免滚动/缩放下的错位），不再转发到 canvas。
    fn install_event_forwarder(&mut self) {
        let document = self.document.clone();
        let host = self.host.clone();
        let id_to_key = self.id_to_key.clone();
        let handler = self.handler.clone();
        // 重入保护：核心同步处理委托事件时可能再次派发指针事件并回到本转发器闭包，
        // 此时若仍持有 `handler` 借用会触发 RefCell 恐慌（wasm 直接 abort，整页卡死）。
        // 用原子布尔标记拦截重入调用，使其直接跳过委托。
        let reentrancy = Rc::new(Cell::new(false));
        let closure = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            // 转发器自身派发的合成事件带 FORWARD_MARK 标记，直接跳过，
            // 避免在 bubble 阶段再次进入本闭包（wasm-bindgen FnMut 闭包禁止重入）。
            if js_sys::Reflect::get(event.as_ref(), &FORWARD_MARK.into())
                .map(|v| v.is_truthy())
                .unwrap_or(false)
            {
                return;
            }
            // 事件是否落在覆盖层内（文本 span 开启了 pointer-events:auto）。
            let Some(target) = event.target() else { return };
            let target_node: &Node = target.unchecked_ref();
            if !host.contains(Some(target_node)) {
                return;
            }
            // 滚轮落在可滚动元素上时，交给浏览器原生滚动（不拦截、不转发），
            // 再由 `scroll` 监听把位置同步回 Rust；Ctrl/Meta+滚轮保留给应用做缩放。
            let event_type = event.type_();
            if event_type == "wheel" {
                let zoom = event
                    .dyn_ref::<web_sys::WheelEvent>()
                    .map(|w| w.ctrl_key() || w.meta_key())
                    .unwrap_or(false);
                if !zoom && Self::is_scrollable_target(target_node, &host) {
                    return;
                }
            }
            // 事件委托：点击 DOM 元素时按 key 链直接命中核心 hitbox。
            if let Some(keys) = Self::collect_key_chain(target_node, &host, &id_to_key) {
                // 重入保护：若正在处理一次委托事件（核心同步派发的新事件回到本闭包），
                // 直接跳过，避免二次借用 `handler` 触发 RefCell 恐慌导致整页卡死。
                if reentrancy.replace(true) {
                    return;
                }
                if let Some(on_dom_event) = handler.borrow_mut().as_mut() {
                    on_dom_event(keys, event);
                }
                reentrancy.set(false);
                return;
            }
            // 未命中委托（点击 canvas 本身等），回退到坐标转发。
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
                    // 合成事件不需要向上冒泡：直接派发到 canvas，监听器在 canvas 上，
                    // 不冒泡到 document 才不会重入本转发器闭包。
                    init.set_bubbles(false);
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
                    init.set_bubbles(false);
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

        // 非被动注册：wheel 在 document 上默认被动（passive），无法对其
        // preventDefault（会触发 "[Intervention] Unable to preventDefault inside
        // passive event listener"）。事件委托/转发都需要阻止默认行为（wheel 交给
        // rgpui 滚动），必须显式声明 passive:false。保持冒泡阶段注册，避免
        // capture+冒泡导致同一事件被本闭包触发两次。
        let options = js_sys::Object::new();
        js_sys::Reflect::set(&options, &"passive".into(), &false.into()).ok();
        let document_js: &JsValue = self.document.as_ref();
        let callback_js: &JsValue = closure.as_ref();
        for event_name in FORWARD_EVENTS {
            if let Ok(add_fn_val) = js_sys::Reflect::get(document_js, &"addEventListener".into()) {
                if let Ok(add_fn) = add_fn_val.dyn_into::<js_sys::Function>() {
                    add_fn
                        .call3(document_js, &(*event_name).into(), callback_js, &options)
                        .ok();
                }
            }
        }
        self._forwarder = Some(closure);

        // 安装 `scroll` 监听：可滚动容器原生滚动后，把位置同步回 Rust 的 `ScrollHandle`。
        // 用 capture 阶段在宿主上捕获（元素 `scroll` 事件在现代浏览器会冒泡，但
        // capture 更稳妥），避免漏掉嵌套滚动容器的事件。
        let scroll_handler = self.scroll_handler.clone();
        let host_for_scroll = self.host.clone();
        let id_to_key_scroll = self.id_to_key.clone();
        let scroll_closure = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let target = match event.target() {
                Some(t) => t,
                None => return,
            };
            let target_node: &Node = target.unchecked_ref();
            if !host_for_scroll.contains(Some(target_node)) {
                return;
            }
            let keys = Self::collect_key_chain(target_node, &host_for_scroll, &id_to_key_scroll);
            let Some(keys) = keys else { return };
            let el: &Element = target_node.unchecked_ref();
            let left = el.scroll_left() as f64;
            let top = el.scroll_top() as f64;
            if let Some(h) = scroll_handler.borrow_mut().as_mut() {
                h(keys, left, top);
            }
        });
        let scroll_fn: &js_sys::Function = scroll_closure.as_ref().unchecked_ref();
        let _ = self
            .host
            .add_event_listener_with_callback_and_bool("scroll", scroll_fn, true)
            .map_err(|e| {
                web_sys::console::error_2(
                    &"安装 scroll 监听失败".into(),
                    &format!("{:?}", e).into(),
                )
            });
        self._scroll_forwarder = Some(scroll_closure);

        // 安装 `copy` 监听：DOM 覆盖层中文本被复制时，从选区逐文本节点构造
        // 带换行的 HTML/纯文本。由于 DOM 后端用绝对定位渲染，相邻文本 span 在
        // 选区里被浏览器当作连续内联内容，原生 copy 不会插入换行；此处按各文本
        // 节点的纵向位置变化手动补 `\n`（纯文本）与 `<br>`（HTML）。
        let host_for_copy = self.host.clone();
        let copy_handler = move |event: Event| {
            let window = match web_sys::window() {
                Some(w) => w,
                None => return,
            };
            let selection = js_sys::Reflect::get(window.as_ref(), &"getSelection".into())
                .ok()
                .and_then(|f| f.dyn_ref::<js_sys::Function>().cloned())
                .and_then(|f| f.call0(window.as_ref()).ok());
            let Some(selection) = selection else { return };
            let range_count = js_sys::Reflect::get(&selection, &"rangeCount".into())
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            if range_count < 1.0 {
                return;
            }
            let anchor = js_sys::Reflect::get(&selection, &"anchorNode".into())
                .ok()
                .filter(|v| !v.is_null() && !v.is_undefined());
            let Some(anchor) = anchor else { return };
            let anchor_node: &Node = anchor.unchecked_ref();
            if !host_for_copy.contains(Some(anchor_node)) {
                return;
            }
            // 用注入的 JS 函数遍历选区文本节点并按纵向位置补换行。
            let copy_fn = js_sys::Function::new_with_args(
                "event, host",
                r#"
                const sel = window.getSelection();
                if (!sel || sel.rangeCount === 0) return;
                const range = sel.getRangeAt(0);
                if (!host.contains(sel.anchorNode)) return;
                const walker = document.createTreeWalker(
                    host, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT, null);
                let plain = '', html = '';
                let lastTop = null;
                let n;
                while ((n = walker.nextNode())) {
                    if (!range.intersectsNode(n)) continue;
                    if (n.nodeType === 3) {
                        const top = Math.round(n.parentElement.getBoundingClientRect().top);
                        if (lastTop !== null && Math.abs(top - lastTop) > 2) {
                            plain += '\n'; html += '<br>';
                        }
                        lastTop = top;
                        const t = n.textContent;
                        plain += t;
                        const esc = t.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
                        const style = n.parentElement.getAttribute('style') || '';
                        html += '<span style="' + style + '">' + esc.replace(/\n/g,'<br>') + '</span>';
                    } else if (n.nodeName === 'BR') {
                        plain += '\n'; html += '<br>';
                        lastTop = null;
                    }
                }
                const data = event.clipboardData;
                if (data) {
                    data.setData('text/plain', plain);
                    data.setData('text/html', html);
                    event.preventDefault();
                }
                "#,
            );
            let _ = copy_fn.call2(
                &JsValue::NULL,
                event.unchecked_ref(),
                host_for_copy.unchecked_ref(),
            );
        };
        let copy_closure = Closure::<dyn FnMut(Event)>::new(copy_handler);
        {
            let doc_js: &JsValue = self.document.as_ref();
            let callback_js: &JsValue = copy_closure.as_ref();
            let options = js_sys::Object::new();
            js_sys::Reflect::set(&options, &"passive".into(), &false.into()).ok();
            if let Ok(add_fn_val) = js_sys::Reflect::get(doc_js, &"addEventListener".into()) {
                if let Ok(add_fn) = add_fn_val.dyn_into::<js_sys::Function>() {
                    let _ = add_fn.call3(doc_js, &"copy".into(), callback_js, &options);
                }
            }
        }
        self._copy_handler = Some(copy_closure);
    }

    /// 用一棵新树增量更新 DOM（对账 + 应用）。
    ///
    /// 首帧时 `last_tree` 为空，等价于全量创建。
    pub fn update(&mut self, tree: &DomTree) {
        let patches = reconcile(&self.last_tree, tree);
        self.apply_patches(&patches);
        self.last_tree = tree.clone();
        // 刷新 data-gpui-id -> key 反查表（事件委托用）。
        {
            let mut map = self.id_to_key.borrow_mut();
            map.clear();
            for key in self.last_tree.nodes.keys() {
                map.insert(key.to_dom_id(), key.clone());
            }
        }

        // 把 Rust 侧程序化滚动（如 `ScrollHandle::scroll_to`）推回 DOM：
        // 设置可滚动容器的 `scrollLeft/scrollTop`。仅在不一致时写入，避免每帧触发
        // 多余的 `scroll` 事件与回写循环。浏览器原生滚动时此处值与 DOM 一致，故为空操作。
        for (key, node) in &self.last_tree.nodes {
            if let Some(handle) = &node.scroll_handle {
                if let Some(el) = self.elements.get(key) {
                    let offset = handle.offset();
                    let want_top = -(f32::from(offset.y)) as i32;
                    let want_left = -(f32::from(offset.x)) as i32;
                    if el.scroll_top() != want_top {
                        el.set_scroll_top(want_top);
                    }
                    if el.scroll_left() != want_left {
                        el.set_scroll_left(want_left);
                    }
                }
            }
        }
    }

    /// 创建单个 DOM 元素并打上 `data-gpui-id`。
    fn create_dom_node(&self, node: &DomNode, key: &DomNodeKey) -> Element {
        match &node.kind {
            DomNodeKind::Element { tag, attrs, .. } => {
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
                self.set_span_text(&span, text.as_ref());
                span.set_attribute(NODE_ATTR, &key.to_dom_id())
                    .expect("设置 data-gpui-id 失败");
                span
            }
        }
    }

    /// 设置文本 span 的内容：把 `\n` 转换为真实的 `<br>` DOM 元素，
    /// 使浏览器原生复制在 `text/plain` 放 `\n`、在 `text/html` 放 `<br>`。
    ///
    /// 创建与每帧更新（[`Self::apply_patches`]）都走这里，保证 `<br>` 不被
    /// `set_text_content` 覆盖。
    fn set_span_text(&self, span: &Element, text: &str) {
        let node: &Node = span.unchecked_ref();
        node.set_text_content(None);
        if text.contains('\n') {
            for (i, line) in text.split('\n').enumerate() {
                if i > 0 {
                    let br = self.document.create_element("br").expect("创建 br 失败");
                    span.append_child(br.unchecked_ref()).ok();
                }
                if !line.is_empty() {
                    let text_node = self.document.create_text_node(line);
                    span.append_child(text_node.unchecked_ref()).ok();
                }
            }
        } else {
            node.set_text_content(Some(text));
        }
    }

    /// 把节点的样式写入元素的 `style` 属性。
    ///
    /// 纯 DOM 渲染模式下，DOM 层是**主渲染器**：元素节点输出完整视觉样式
    /// （背景/边框/阴影/文本），canvas 已隐藏，不存在双重渲染。覆盖层宿主
    /// `pointer-events:auto`，全部节点都可命中，点击时按 `data-gpui-id` 反查
    /// key 链委托给核心（事件委托）；文本节点额外开启 `user-select:text`，
    /// 使浏览器原生文本选择生效。
    ///
    /// 样式串按 key 去重缓存：与上一帧一致时跳过 `setAttribute`，
    /// 避免每帧对每个节点做字符串分配与 DOM 属性写入（大量静态样式的主要开销）。
    fn apply_style(&mut self, element: &Element, node: &DomNode, key: &DomNodeKey) {
        let mut css_text = css::dom_style_to_css(&node.style);
        if matches!(&node.kind, DomNodeKind::Text { .. }) {
            css_text.push_str(";user-select:text;white-space:pre-wrap");
        }
        if self.styles.get(key).map(String::as_str) == Some(css_text.as_str()) {
            return;
        }
        element
            .set_attribute("style", &css_text)
            .expect("写入内联样式失败");
        self.styles.insert(key.clone(), css_text);
    }

    /// 递归创建 `key` 的整棵子树并挂到 `parent` 下。
    fn build_subtree(&mut self, tree: &DomTree, key: &DomNodeKey, parent: &Node) {
        let Some(node) = tree.nodes.get(key) else {
            return;
        };
        let element = self.create_dom_node(node, key);
        self.apply_style(&element, node, key);
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
                    self.apply_style(&element, node, key);
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
                    if let Some(element) = self.elements.get(key).cloned() {
                        self.apply_style(&element, node, key);
                        match &node.kind {
                            DomNodeKind::Element { attrs, .. } => {
                                for (name, value) in attrs {
                                    let _ = element.set_attribute(name, value);
                                }
                            }
                            DomNodeKind::Text { text } => {
                                self.set_span_text(&element, text.as_ref());
                            }
                        }
                    }
                }
                DomPatch::RemoveNode { key } => {
                    if let Some(element) = self.elements.remove(key) {
                        self.styles.remove(key);
                        element.remove();
                    }
                }
            }
        }
    }

    fn rebuild(&mut self, tree: &DomTree) {
        self.host.unchecked_ref::<Node>().set_text_content(Some(""));
        self.elements.clear();
        self.styles.clear();
        let host = self.host.clone();
        self.build_subtree(tree, &tree.root, &host);
        self.last_tree = tree.clone();
        // 刷新 data-gpui-id -> key 反查表（事件委托用）。
        {
            let mut map = self.id_to_key.borrow_mut();
            map.clear();
            for key in self.last_tree.nodes.keys() {
                map.insert(key.to_dom_id(), key.clone());
            }
        }
    }
}
