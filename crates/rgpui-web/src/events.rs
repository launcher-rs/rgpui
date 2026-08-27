use std::rc::Rc;

use rgpui::{
    Capslock, DispatchEventResult, ExternalPaths, FileDropEvent, KeyDownEvent, KeyUpEvent,
    Keystroke, Modifiers, ModifiersChangedEvent, MouseButton, MouseDownEvent, MouseExitEvent,
    MouseMoveEvent, MouseUpEvent, NavigationDirection, Pixels, PlatformInput, Point, ScrollDelta,
    ScrollWheelEvent, TouchPhase, point, px,
};
use smallvec::smallvec;
use wasm_bindgen::prelude::*;

use crate::window::WebWindowInner;

/// Web 平台事件监听器集合，持有所有注册的 DOM 事件回调
pub struct WebEventListeners {
    #[allow(dead_code)]
    closures: Vec<Closure<dyn FnMut(JsValue)>>,
}

/// 鼠标点击状态跟踪器，用于计算双击等多击事件
pub(crate) struct ClickState {
    last_position: Point<Pixels>,
    last_time: f64,
    current_count: usize,
}

impl Default for ClickState {
    fn default() -> Self {
        Self {
            last_position: Point::default(),
            last_time: 0.0,
            current_count: 0,
        }
    }
}

impl ClickState {
    /// 注册一次点击并返回当前点击计数
    ///
    /// # 参数
    /// * `position` - 点击位置
    /// * `time` - 点击时间戳
    fn register_click(&mut self, position: Point<Pixels>, time: f64) -> usize {
        let distance = ((f32::from(position.x) - f32::from(self.last_position.x)).powi(2)
            + (f32::from(position.y) - f32::from(self.last_position.y)).powi(2))
        .sqrt();

        if (time - self.last_time) < 400.0 && distance < 5.0 {
            self.current_count += 1;
        } else {
            self.current_count = 1;
        }

        self.last_position = position;
        self.last_time = time;
        self.current_count
    }
}

impl WebWindowInner {
    pub fn register_event_listeners(self: &Rc<Self>) -> WebEventListeners {
        let mut closures = vec![
            self.register_pointer_down(),
            self.register_pointer_up(),
            self.register_pointer_move(),
            self.register_pointer_leave(),
            self.register_wheel(),
            self.register_dragover(),
            self.register_drop(),
            self.register_dragleave(),
            self.register_key_down(),
            self.register_key_up(),
            self.register_composition_start(),
            self.register_composition_update(),
            self.register_composition_end(),
            self.register_focus(),
            self.register_blur(),
            self.register_pointer_enter(),
            self.register_pointer_leave_hover(),
        ];
        closures.extend(self.register_visibility_change());
        closures.extend(self.register_appearance_change());
        closures.extend(self.register_context_menu());

        WebEventListeners { closures }
    }

    fn listen(
        self: &Rc<Self>,
        event_name: &str,
        handler: impl FnMut(JsValue) + 'static,
    ) -> Closure<dyn FnMut(JsValue)> {
        let closure = Closure::<dyn FnMut(JsValue)>::new(handler);
        self.canvas
            .add_event_listener_with_callback(event_name, closure.as_ref().unchecked_ref())
            .ok();
        closure
    }

    fn listen_input(
        self: &Rc<Self>,
        event_name: &str,
        handler: impl FnMut(JsValue) + 'static,
    ) -> Closure<dyn FnMut(JsValue)> {
        let closure = Closure::<dyn FnMut(JsValue)>::new(handler);
        self.input_element
            .add_event_listener_with_callback(event_name, closure.as_ref().unchecked_ref())
            .ok();
        closure
    }

    /// 在 `window` 上注册监听。键盘事件挂到 window 而非隐藏 `input_element`，
    /// 因为 DOM 模式点击后浏览器默认会把焦点移到 body，导致挂在 `input_element`
    /// 上的 keydown 收不到；而按键最终由核心按当前聚焦的编辑器路由，与 DOM 焦点无关。
    fn listen_window(
        self: &Rc<Self>,
        event_name: &str,
        handler: impl FnMut(JsValue) + 'static,
    ) -> Closure<dyn FnMut(JsValue)> {
        let closure = Closure::<dyn FnMut(JsValue)>::new(handler);
        if let Some(window) = web_sys::window() {
            window
                .add_event_listener_with_callback(event_name, closure.as_ref().unchecked_ref())
                .ok();
        }
        closure
    }

    /// 注册一个带有 `{passive: false}` 的监听器，使 `preventDefault()` 能够正常工作。
    /// 对于 `wheel` 等在现代浏览器中默认为被动的事件是必需的。
    fn listen_non_passive(
        self: &Rc<Self>,
        event_name: &str,
        handler: impl FnMut(JsValue) + 'static,
    ) -> Closure<dyn FnMut(JsValue)> {
        let closure = Closure::<dyn FnMut(JsValue)>::new(handler);
        let canvas_js: &JsValue = self.canvas.as_ref();
        let callback_js: &JsValue = closure.as_ref();
        let options = js_sys::Object::new();
        js_sys::Reflect::set(&options, &"passive".into(), &false.into()).ok();
        if let Ok(add_fn_val) = js_sys::Reflect::get(canvas_js, &"addEventListener".into()) {
            if let Ok(add_fn) = add_fn_val.dyn_into::<js_sys::Function>() {
                add_fn
                    .call3(canvas_js, &event_name.into(), callback_js, &options)
                    .ok();
            }
        }
        closure
    }

    fn dispatch_input(&self, input: PlatformInput) -> Option<DispatchEventResult> {
        // 取出回调后释放借用再调用，避免 gpui 输入回调内部再次借用 callbacks（如请求绘制）时重借。
        let mut callback = self.callbacks.borrow_mut().input.take();
        let result = callback.as_mut().map(|callback| callback(input));
        if let Some(callback) = callback {
            self.callbacks.borrow_mut().input = Some(callback);
        }
        result
    }

    /// 处理来自 DOM 覆盖层的委托事件（点击 DOM 元素时按 key 链回调）。
    ///
    /// 纯 DOM 模式下点击带 `data-gpui-id` 的元素会走到这里：把原始 DOM 事件转换
    /// 为 [`PlatformInput`]，连同反查出的 key 链一起交给核心按 key 命中
    /// （绕过坐标 hit-test）。位置用 client 坐标相对 canvas 计算，与 canvas
    /// 监听器使用的坐标空间一致。
    ///
    /// 指针事件不调用 `preventDefault`：保留浏览器原生行为（文本 span 的
    /// `user-select:text` 原生选择/复制）；wheel 仍要阻止默认滚动，避免与
    /// gpui 自绘滚动双重滚动。
    pub(crate) fn dispatch_dom_event(&self, keys: Vec<rgpui::DomNodeKey>, event: web_sys::Event) {
        let event_type = event.type_();
        let input = match event_type.as_str() {
            "pointerdown" => {
                let event: &web_sys::PointerEvent = event.unchecked_ref();
                self.input_element.focus().ok();
                let button = dom_mouse_button_to_gpui(event.button());
                let position = mouse_position_from_event(event.as_ref(), &self.canvas);
                let modifiers = modifiers_from_mouse_event(event, self.is_mac);
                let time = js_sys::Date::now();
                self.pressed_button.set(Some(button));
                let click_count = self.click_state.borrow_mut().register_click(position, time);
                {
                    let mut current_state = self.state.borrow_mut();
                    current_state.mouse_position = position;
                    current_state.modifiers = modifiers;
                }
                PlatformInput::MouseDown(MouseDownEvent {
                    button,
                    position,
                    modifiers,
                    click_count,
                    first_mouse: false,
                })
            }
            "pointerup" => {
                let event: &web_sys::PointerEvent = event.unchecked_ref();
                let button = dom_mouse_button_to_gpui(event.button());
                let position = mouse_position_from_event(event.as_ref(), &self.canvas);
                let modifiers = modifiers_from_mouse_event(event, self.is_mac);
                self.pressed_button.set(None);
                let click_count = self.click_state.borrow().current_count;
                {
                    let mut current_state = self.state.borrow_mut();
                    current_state.mouse_position = position;
                    current_state.modifiers = modifiers;
                }
                PlatformInput::MouseUp(MouseUpEvent {
                    button,
                    position,
                    modifiers,
                    click_count,
                })
            }
            "pointermove" => {
                let event: &web_sys::PointerEvent = event.unchecked_ref();
                let position = mouse_position_from_event(event.as_ref(), &self.canvas);
                let modifiers = modifiers_from_mouse_event(event, self.is_mac);
                let current_pressed = self.pressed_button.get();
                {
                    let mut current_state = self.state.borrow_mut();
                    current_state.mouse_position = position;
                    current_state.modifiers = modifiers;
                }
                PlatformInput::MouseMove(MouseMoveEvent {
                    position,
                    pressed_button: current_pressed,
                    modifiers,
                })
            }
            "wheel" => {
                let event: &web_sys::WheelEvent = event.unchecked_ref();
                event.prevent_default();
                let position = mouse_position_from_event(event.as_ref(), &self.canvas);
                let modifiers = modifiers_from_wheel_event(event.as_ref(), self.is_mac);
                let delta_mode = event.delta_mode();
                let delta = if delta_mode == 1 {
                    ScrollDelta::Lines(point(-event.delta_x() as f32, -event.delta_y() as f32))
                } else {
                    ScrollDelta::Pixels(point(
                        px(-event.delta_x() as f32),
                        px(-event.delta_y() as f32),
                    ))
                };
                {
                    let mut current_state = self.state.borrow_mut();
                    current_state.modifiers = modifiers;
                }
                PlatformInput::ScrollWheel(ScrollWheelEvent {
                    position,
                    delta,
                    modifiers,
                    touch_phase: TouchPhase::Moved,
                })
            }
            _ => return,
        };

        // 取出回调后释放借用再调用，避免 gpui 回调内部再次借用 callbacks 时重借。
        let callback = self.callbacks.borrow_mut().dom_event.take();
        if let Some(mut callback) = callback {
            callback(keys, input);
            self.callbacks.borrow_mut().dom_event = Some(callback);
        }
    }

    /// 由 DOM 后端在可滚动容器发生原生滚动后回调：把 (key 链, scrollLeft, scrollTop)
    /// 交给核心，反查 `ScrollHandle` 并同步滚动偏移。
    #[cfg(feature = "dom-backend")]
    pub(crate) fn dispatch_dom_scroll(&self, keys: Vec<rgpui::DomNodeKey>, left: f64, top: f64) {
        // 取出回调后释放借用再调用，避免 gpui 回调内部再次借用 callbacks 时重借。
        let callback = self.callbacks.borrow_mut().dom_scroll.take();
        if let Some(mut callback) = callback {
            callback(keys, left, top);
            self.callbacks.borrow_mut().dom_scroll = Some(callback);
        }
    }

    fn register_pointer_down(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointerdown", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();
            // 不阻止右键默认行为（右键菜单由 contextmenu 事件处理）
            if event.button() != 2 {
                event.prevent_default();
            }
            this.input_element.focus().ok();

            let button = dom_mouse_button_to_gpui(event.button());
            let position = pointer_position_in_element(&event);
            let modifiers = modifiers_from_mouse_event(&event, this.is_mac);
            let time = js_sys::Date::now();

            this.pressed_button.set(Some(button));
            let click_count = this.click_state.borrow_mut().register_click(position, time);

            {
                let mut current_state = this.state.borrow_mut();
                current_state.mouse_position = position;
                current_state.modifiers = modifiers;
            }

            this.dispatch_input(PlatformInput::MouseDown(MouseDownEvent {
                button,
                position,
                modifiers,
                click_count,
                first_mouse: false,
            }));
        })
    }

    fn register_pointer_up(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointerup", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();
            event.prevent_default();

            let button = dom_mouse_button_to_gpui(event.button());
            let position = pointer_position_in_element(&event);
            let modifiers = modifiers_from_mouse_event(&event, this.is_mac);

            this.pressed_button.set(None);
            let click_count = this.click_state.borrow().current_count;

            {
                let mut current_state = this.state.borrow_mut();
                current_state.mouse_position = position;
                current_state.modifiers = modifiers;
            }

            this.dispatch_input(PlatformInput::MouseUp(MouseUpEvent {
                button,
                position,
                modifiers,
                click_count,
            }));
        })
    }

    fn register_pointer_move(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointermove", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();
            event.prevent_default();

            let position = pointer_position_in_element(&event);
            let modifiers = modifiers_from_mouse_event(&event, this.is_mac);
            let current_pressed = this.pressed_button.get();

            {
                let mut current_state = this.state.borrow_mut();
                current_state.mouse_position = position;
                current_state.modifiers = modifiers;
            }

            this.dispatch_input(PlatformInput::MouseMove(MouseMoveEvent {
                position,
                pressed_button: current_pressed,
                modifiers,
            }));
        })
    }

    fn register_pointer_leave(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointerleave", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();

            let position = pointer_position_in_element(&event);
            let modifiers = modifiers_from_mouse_event(&event, this.is_mac);
            let current_pressed = this.pressed_button.get();

            {
                let mut current_state = this.state.borrow_mut();
                current_state.mouse_position = position;
                current_state.modifiers = modifiers;
            }

            this.dispatch_input(PlatformInput::MouseExited(MouseExitEvent {
                position,
                pressed_button: current_pressed,
                modifiers,
            }));
        })
    }

    fn register_wheel(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_non_passive("wheel", move |event: JsValue| {
            let event: web_sys::WheelEvent = event.unchecked_into();
            event.prevent_default();

            let mouse_event: &web_sys::MouseEvent = event.as_ref();
            let position = mouse_position_in_element(mouse_event);
            let modifiers = modifiers_from_wheel_event(mouse_event, this.is_mac);

            let delta_mode = event.delta_mode();
            let delta = if delta_mode == 1 {
                ScrollDelta::Lines(point(-event.delta_x() as f32, -event.delta_y() as f32))
            } else {
                ScrollDelta::Pixels(point(
                    px(-event.delta_x() as f32),
                    px(-event.delta_y() as f32),
                ))
            };

            {
                let mut current_state = this.state.borrow_mut();
                current_state.modifiers = modifiers;
            }

            this.dispatch_input(PlatformInput::ScrollWheel(ScrollWheelEvent {
                position,
                delta,
                modifiers,
                touch_phase: TouchPhase::Moved,
            }));
        })
    }

    fn register_context_menu(self: &Rc<Self>) -> Vec<Closure<dyn FnMut(JsValue)>> {
        // 使用捕获阶段监听 contextmenu，在浏览器处理右键菜单之前拦截
        let this = Rc::clone(self);
        let closure = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
            let event: web_sys::Event = event.unchecked_into();
            event.prevent_default();
            event.stop_propagation();
        });
        let canvas_js: &JsValue = this.canvas.as_ref();
        let callback_js: &JsValue = closure.as_ref();
        let options = js_sys::Object::new();
        js_sys::Reflect::set(&options, &"capture".into(), &true.into()).ok();
        js_sys::Reflect::set(&options, &"passive".into(), &false.into()).ok();
        if let Ok(add_fn_val) = js_sys::Reflect::get(canvas_js, &"addEventListener".into()) {
            if let Ok(add_fn) = add_fn_val.dyn_into::<js_sys::Function>() {
                add_fn
                    .call3(canvas_js, &"contextmenu".into(), callback_js, &options)
                    .ok();
            }
        }
        // 同时在文档级别阻止冒泡，防止 body/document 的默认右键菜单。
        // 注意：doc_closure 也必须随闭包集合一起保留，否则函数返回后被 drop，
        // 后续 contextmenu 事件会触发已释放的 wasm-bindgen 闭包（"closure invoked
        // recursively or after being dropped"），导致事件循环中断。
        if let Some(document) = this.browser_window.document() {
            let doc_closure = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
                let event: web_sys::Event = event.unchecked_into();
                event.prevent_default();
            });
            let doc_js: &JsValue = document.as_ref();
            let doc_callback_js: &JsValue = doc_closure.as_ref();
            let doc_options = js_sys::Object::new();
            js_sys::Reflect::set(&doc_options, &"capture".into(), &true.into()).ok();
            if let Ok(add_fn_val) = js_sys::Reflect::get(doc_js, &"addEventListener".into()) {
                if let Ok(add_fn) = add_fn_val.dyn_into::<js_sys::Function>() {
                    add_fn
                        .call3(doc_js, &"contextmenu".into(), doc_callback_js, &doc_options)
                        .ok();
                }
            }
            return vec![closure, doc_closure];
        }
        vec![closure]
    }

    fn register_dragover(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("dragover", move |event: JsValue| {
            let event: web_sys::DragEvent = event.unchecked_into();
            event.prevent_default();

            let mouse_event: &web_sys::MouseEvent = event.as_ref();
            let position = mouse_position_in_element(mouse_event);

            {
                let mut current_state = this.state.borrow_mut();
                current_state.mouse_position = position;
            }

            this.dispatch_input(PlatformInput::FileDrop(FileDropEvent::Pending { position }));
        })
    }

    fn register_drop(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("drop", move |event: JsValue| {
            let event: web_sys::DragEvent = event.unchecked_into();
            event.prevent_default();

            let mouse_event: &web_sys::MouseEvent = event.as_ref();
            let position = mouse_position_in_element(mouse_event);

            {
                let mut current_state = this.state.borrow_mut();
                current_state.mouse_position = position;
            }

            let paths = extract_file_paths_from_drag(&event);

            this.dispatch_input(PlatformInput::FileDrop(FileDropEvent::Entered {
                position,
                paths: ExternalPaths(paths),
            }));

            this.dispatch_input(PlatformInput::FileDrop(FileDropEvent::Submit { position }));
        })
    }

    fn register_dragleave(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("dragleave", move |_event: JsValue| {
            this.dispatch_input(PlatformInput::FileDrop(FileDropEvent::Exited));
        })
    }

    fn register_key_down(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_window("keydown", move |event: JsValue| {
            let event: web_sys::KeyboardEvent = event.unchecked_into();

            let modifiers = modifiers_from_keyboard_event(&event, this.is_mac);
            let capslock = capslock_from_keyboard_event(&event);

            {
                let mut current_state = this.state.borrow_mut();
                current_state.modifiers = modifiers;
                current_state.capslock = capslock;
            }

            this.dispatch_input(PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                modifiers,
                capslock,
            }));

            let key = dom_key_to_gpui_key(&event);

            if is_modifier_only_key(&key) {
                return;
            }

            // 函数键（F1-F12）留给浏览器处理（F12 开发者工具等）
            if !is_function_key(&key) {
                event.prevent_default();
            }

            let is_held = event.repeat();
            let key_char = compute_key_char(&event, &key, &modifiers);

            let keystroke = Keystroke {
                modifiers,
                key,
                key_char: key_char.clone(),
            };

            let result = this.dispatch_input(PlatformInput::KeyDown(KeyDownEvent {
                keystroke,
                is_held,
                prefer_character_input: false,
            }));

            if let Some(result) = result {
                if !result.propagate {
                    return;
                }
            }

            if this.is_composing.get() || event.is_composing() {
                return;
            }

            if modifiers.is_subset_of(&Modifiers::shift()) {
                if let Some(text) = key_char {
                    this.with_input_handler(|handler| {
                        handler.replace_text_in_range(None, &text);
                    });
                }
            }
        })
    }

    fn register_key_up(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_window("keyup", move |event: JsValue| {
            let event: web_sys::KeyboardEvent = event.unchecked_into();

            let modifiers = modifiers_from_keyboard_event(&event, this.is_mac);
            let capslock = capslock_from_keyboard_event(&event);

            {
                let mut current_state = this.state.borrow_mut();
                current_state.modifiers = modifiers;
                current_state.capslock = capslock;
            }

            this.dispatch_input(PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                modifiers,
                capslock,
            }));

            let key = dom_key_to_gpui_key(&event);

            if is_modifier_only_key(&key) {
                return;
            }

            // 函数键（F1-F12）留给浏览器处理（F12 开发者工具等）
            if !is_function_key(&key) {
                event.prevent_default();
            }

            let key_char = compute_key_char(&event, &key, &modifiers);

            let keystroke = Keystroke {
                modifiers,
                key,
                key_char,
            };

            this.dispatch_input(PlatformInput::KeyUp(KeyUpEvent { keystroke }));
        })
    }

    fn register_composition_start(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("compositionstart", move |_event: JsValue| {
            this.is_composing.set(true);
        })
    }

    fn register_composition_update(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("compositionupdate", move |event: JsValue| {
            let event: web_sys::CompositionEvent = event.unchecked_into();
            let data = event.data().unwrap_or_default();
            this.is_composing.set(true);
            this.with_input_handler(|handler| {
                handler.replace_and_mark_text_in_range(None, &data, None);
            });
        })
    }

    fn register_composition_end(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("compositionend", move |event: JsValue| {
            let event: web_sys::CompositionEvent = event.unchecked_into();
            let data = event.data().unwrap_or_default();
            this.is_composing.set(false);
            this.with_input_handler(|handler| {
                handler.replace_text_in_range(None, &data);
                handler.unmark_text();
            });
            this.input_element.set_value("");
        })
    }

    fn register_focus(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("focus", move |_event: JsValue| {
            {
                let mut state = this.state.borrow_mut();
                state.is_active = true;
            }
            // 取出回调后释放借用再调用，避免 gpui 回调内部再次借用 callbacks 时重借。
            let callback = this.callbacks.borrow_mut().active_status_change.take();
            if let Some(mut callback) = callback {
                callback(true);
                this.callbacks.borrow_mut().active_status_change = Some(callback);
            }
        })
    }

    fn register_blur(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("blur", move |_event: JsValue| {
            {
                let mut state = this.state.borrow_mut();
                state.is_active = false;
            }
            // 取出回调后释放借用再调用，避免 gpui 回调内部再次借用 callbacks 时重借。
            let callback = this.callbacks.borrow_mut().active_status_change.take();
            if let Some(mut callback) = callback {
                callback(false);
                this.callbacks.borrow_mut().active_status_change = Some(callback);
            }
        })
    }

    fn register_pointer_enter(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointerenter", move |_event: JsValue| {
            {
                let mut state = this.state.borrow_mut();
                state.is_hovered = true;
            }
            // 取出回调后释放借用再调用，避免 gpui 回调内部再次借用 callbacks 时重借。
            let callback = this.callbacks.borrow_mut().hover_status_change.take();
            if let Some(mut callback) = callback {
                callback(true);
                this.callbacks.borrow_mut().hover_status_change = Some(callback);
            }
        })
    }

    fn register_pointer_leave_hover(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointerleave", move |_event: JsValue| {
            {
                let mut state = this.state.borrow_mut();
                state.is_hovered = false;
            }
            // 取出回调后释放借用再调用，避免 gpui 回调内部再次借用 callbacks 时重借。
            let callback = this.callbacks.borrow_mut().hover_status_change.take();
            if let Some(mut callback) = callback {
                callback(false);
                this.callbacks.borrow_mut().hover_status_change = Some(callback);
            }
        })
    }
}

fn dom_key_to_gpui_key(event: &web_sys::KeyboardEvent) -> String {
    let key = event.key();
    match key.as_str() {
        "Enter" => "enter".to_string(),
        "Backspace" => "backspace".to_string(),
        "Tab" => "tab".to_string(),
        "Escape" => "escape".to_string(),
        "Delete" => "delete".to_string(),
        " " => "space".to_string(),
        "ArrowLeft" => "left".to_string(),
        "ArrowRight" => "right".to_string(),
        "ArrowUp" => "up".to_string(),
        "ArrowDown" => "down".to_string(),
        "Home" => "home".to_string(),
        "End" => "end".to_string(),
        "PageUp" => "pageup".to_string(),
        "PageDown" => "pagedown".to_string(),
        "Insert" => "insert".to_string(),
        "Control" => "control".to_string(),
        "Alt" => "alt".to_string(),
        "Shift" => "shift".to_string(),
        "Meta" => "platform".to_string(),
        "CapsLock" => "capslock".to_string(),
        other => {
            if let Some(rest) = other.strip_prefix('F') {
                if let Ok(number) = rest.parse::<u8>() {
                    if (1..=35).contains(&number) {
                        return format!("f{number}");
                    }
                }
            }
            other.to_lowercase()
        }
    }
}

fn dom_mouse_button_to_gpui(button: i16) -> MouseButton {
    match button {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        3 => MouseButton::Navigate(NavigationDirection::Back),
        4 => MouseButton::Navigate(NavigationDirection::Forward),
        _ => MouseButton::Left,
    }
}

fn modifiers_from_keyboard_event(event: &web_sys::KeyboardEvent, _is_mac: bool) -> Modifiers {
    Modifiers {
        control: event.ctrl_key(),
        alt: event.alt_key(),
        shift: event.shift_key(),
        platform: event.meta_key(),
        function: false,
    }
}

fn modifiers_from_mouse_event(event: &web_sys::PointerEvent, _is_mac: bool) -> Modifiers {
    let mouse_event: &web_sys::MouseEvent = event.as_ref();
    Modifiers {
        control: mouse_event.ctrl_key(),
        alt: mouse_event.alt_key(),
        shift: mouse_event.shift_key(),
        platform: mouse_event.meta_key(),
        function: false,
    }
}

fn modifiers_from_wheel_event(event: &web_sys::MouseEvent, _is_mac: bool) -> Modifiers {
    Modifiers {
        control: event.ctrl_key(),
        alt: event.alt_key(),
        shift: event.shift_key(),
        platform: event.meta_key(),
        function: false,
    }
}

fn capslock_from_keyboard_event(event: &web_sys::KeyboardEvent) -> Capslock {
    Capslock {
        on: event.get_modifier_state("CapsLock"),
    }
}

pub(crate) fn is_mac_platform(browser_window: &web_sys::Window) -> bool {
    let navigator = browser_window.navigator();

    #[allow(deprecated)]
    // navigator.platform() 已弃用，但 navigator.userAgentData 尚未广泛可用
    if let Ok(platform) = navigator.platform() {
        if platform.contains("Mac") {
            return true;
        }
    }

    if let Ok(user_agent) = navigator.user_agent() {
        return user_agent.contains("Mac");
    }

    false
}

fn is_modifier_only_key(key: &str) -> bool {
    matches!(
        key,
        "control" | "alt" | "shift" | "platform" | "capslock" | "compose" | "process"
    )
}

/// 检查是否为函数键（F1-F12），这些键应留给浏览器默认行为
fn is_function_key(key: &str) -> bool {
    if let Some(rest) = key.strip_prefix('f') {
        if let Ok(number) = rest.parse::<u8>() {
            return (1..=12).contains(&number);
        }
    }
    false
}

fn compute_key_char(
    event: &web_sys::KeyboardEvent,
    gpui_key: &str,
    modifiers: &Modifiers,
) -> Option<String> {
    if modifiers.platform || modifiers.control {
        return None;
    }

    if is_modifier_only_key(gpui_key) {
        return None;
    }

    if gpui_key == "space" {
        return Some(" ".to_string());
    }

    let raw_key = event.key();

    if raw_key.len() == 1 {
        return Some(raw_key);
    }

    None
}

fn pointer_position_in_element(event: &web_sys::PointerEvent) -> Point<Pixels> {
    let mouse_event: &web_sys::MouseEvent = event.as_ref();
    mouse_position_in_element(mouse_event)
}

/// 从 DOM 事件计算窗口内坐标（相对 canvas 位置）。
///
/// 委托事件的 `offset_x/offset_y` 是相对点击目标元素（DOM 覆盖层节点）的，
/// 不能直接用作 canvas 命中坐标，故改用 client 坐标减去 canvas 的边界矩形。
fn mouse_position_from_event(
    event: &web_sys::MouseEvent,
    canvas: &web_sys::HtmlCanvasElement,
) -> Point<Pixels> {
    let rect = canvas.get_bounding_client_rect();
    point(
        px(event.client_x() as f32 - rect.left() as f32),
        px(event.client_y() as f32 - rect.top() as f32),
    )
}

fn mouse_position_in_element(event: &web_sys::MouseEvent) -> Point<Pixels> {
    // offset_x/offset_y give position relative to the target element's padding edge
    point(px(event.offset_x() as f32), px(event.offset_y() as f32))
}

fn extract_file_paths_from_drag(
    event: &web_sys::DragEvent,
) -> smallvec::SmallVec<[std::path::PathBuf; 2]> {
    let mut paths = smallvec![];
    let Some(data_transfer) = event.data_transfer() else {
        return paths;
    };
    let file_list = data_transfer.files();
    let Some(files) = file_list else {
        return paths;
    };
    for index in 0..files.length() {
        if let Some(file) = files.get(index) {
            paths.push(std::path::PathBuf::from(file.name()));
        }
    }
    paths
}
