//! 快捷键录制输入：点击后捕获按键组合。

use std::rc::Rc;

use crate::{prelude::FluentBuilder as _, *};

/// 快捷键值：按键 + 修饰键组合。
#[derive(Clone)]
pub struct HotkeyValue {
    /// 按键名。
    pub key: String,
    /// 修饰键。
    pub modifiers: Modifiers,
}

impl HotkeyValue {
    /// 创建快捷键值。
    pub fn new(key: impl Into<String>, modifiers: Modifiers) -> Self {
        Self {
            key: key.into(),
            modifiers,
        }
    }

    /// 从按键事件构造快捷键值；纯修饰键返回 None。
    pub fn from_keystroke(keystroke: &Keystroke) -> Option<Self> {
        let key = keystroke.key.as_str();
        if Self::is_modifier_only(key) {
            return None;
        }
        Some(Self {
            key: key.to_string(),
            modifiers: keystroke.modifiers,
        })
    }

    /// 是否为纯修饰键。
    fn is_modifier_only(key: &str) -> bool {
        matches!(
            key.to_lowercase().as_str(),
            "shift" | "control" | "alt" | "meta" | "cmd" | "command" | "ctrl" | "option"
        )
    }

    /// 格式化显示文本（macOS 用符号）。
    #[cfg(target_os = "macos")]
    pub fn format_display(&self) -> String {
        let mut result = String::new();
        if self.modifiers.control {
            result.push_str("⌃");
        }
        if self.modifiers.alt {
            result.push_str("⌥");
        }
        if self.modifiers.shift {
            result.push_str("⇧");
        }
        if self.modifiers.platform {
            result.push_str("⌘");
        }
        result.push_str(&self.format_key());
        result
    }

    /// 格式化显示文本（Windows/Linux 用 Ctrl+Alt 风格）。
    #[cfg(not(target_os = "macos"))]
    pub fn format_display(&self) -> String {
        let mut parts = Vec::new();
        if self.modifiers.control {
            parts.push("Ctrl".to_string());
        }
        if self.modifiers.alt {
            parts.push("Alt".to_string());
        }
        if self.modifiers.shift {
            parts.push("Shift".to_string());
        }
        if self.modifiers.platform {
            parts.push("Win".to_string());
        }
        parts.push(self.format_key());
        parts.join("+")
    }

    /// 格式化按键名（Space/Enter/Esc 等）。
    fn format_key(&self) -> String {
        match self.key.as_str() {
            "space" => "Space".to_string(),
            "enter" => "Enter".to_string(),
            "escape" => "Esc".to_string(),
            "tab" => "Tab".to_string(),
            "backspace" => "Backspace".to_string(),
            "delete" => "Del".to_string(),
            "up" => "Up".to_string(),
            "down" => "Down".to_string(),
            "left" => "Left".to_string(),
            "right" => "Right".to_string(),
            "home" => "Home".to_string(),
            "end" => "End".to_string(),
            "pageup" => "PgUp".to_string(),
            "pagedown" => "PgDn".to_string(),
            k if k.starts_with('f') && k.len() <= 3 => k.to_uppercase(),
            k => k.to_uppercase(),
        }
    }
}

/// 快捷键输入状态。
pub struct HotkeyInputState {
    /// 当前快捷键。
    hotkey: Option<HotkeyValue>,
    /// 是否正在录制。
    recording: bool,
    /// 焦点句柄。
    focus_handle: FocusHandle,
}

impl HotkeyInputState {
    /// 创建快捷键输入状态。
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            hotkey: None,
            recording: false,
            focus_handle: cx.focus_handle(),
        }
    }

    /// 以初始快捷键创建状态。
    pub fn with_hotkey(cx: &mut Context<Self>, hotkey: HotkeyValue) -> Self {
        Self {
            hotkey: Some(hotkey),
            recording: false,
            focus_handle: cx.focus_handle(),
        }
    }

    /// 获取当前快捷键。
    pub fn hotkey(&self) -> Option<&HotkeyValue> {
        self.hotkey.as_ref()
    }

    /// 设置快捷键。
    pub fn set_hotkey(&mut self, hotkey: Option<HotkeyValue>, cx: &mut Context<Self>) {
        self.hotkey = hotkey;
        self.recording = false;
        cx.notify();
    }

    /// 是否正在录制。
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// 开始录制。
    pub fn start_recording(&mut self, cx: &mut Context<Self>) {
        self.recording = true;
        cx.notify();
    }

    /// 停止录制。
    pub fn stop_recording(&mut self, cx: &mut Context<Self>) {
        self.recording = false;
        cx.notify();
    }

    /// 清空快捷键。
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.hotkey = None;
        self.recording = false;
        cx.notify();
    }

    /// 捕获按键事件；录制中且为有效组合时写入并返回 true。
    pub fn capture_keystroke(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) -> bool {
        if !self.recording {
            return false;
        }

        if keystroke.key.as_str() == "escape" {
            self.stop_recording(cx);
            return true;
        }

        if let Some(hotkey) = HotkeyValue::from_keystroke(keystroke) {
            self.hotkey = Some(hotkey);
            self.recording = false;
            cx.notify();
            return true;
        }

        false
    }
}

impl Focusable for HotkeyInputState {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for HotkeyInputState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// 快捷键输入组件。
#[derive(IntoElement)]
pub struct HotkeyInput {
    /// 绑定状态实体。
    state: Entity<HotkeyInputState>,
    /// 占位文本。
    placeholder: SharedString,
    /// 是否禁用。
    disabled: bool,
    /// 快捷键变化回调。
    on_change: Option<Rc<dyn Fn(Option<&HotkeyValue>, &mut Window, &mut App)>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl HotkeyInput {
    /// 创建快捷键输入，默认占位 "Click to record"。
    pub fn new(state: Entity<HotkeyInputState>) -> Self {
        Self {
            state,
            placeholder: "Click to record".into(),
            disabled: false,
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置是否禁用。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置快捷键变化回调。
    pub fn on_change(
        mut self,
        handler: impl Fn(Option<&HotkeyValue>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Styled for HotkeyInput {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for HotkeyInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;

        let state_data = self.state.read(cx);
        let hotkey = state_data.hotkey.clone();
        let recording = state_data.recording;
        let focus_handle = state_data.focus_handle(cx);
        let is_focused = focus_handle.is_focused(window);

        let display_text: SharedString = if recording {
            "Press a key...".into()
        } else if let Some(ref hk) = hotkey {
            hk.format_display().into()
        } else {
            self.placeholder.clone()
        };

        let has_value = hotkey.is_some();
        let show_clear = has_value && !self.disabled && !recording;

        let state_for_click = self.state.clone();
        let state_for_keydown = self.state.clone();
        let state_for_clear = self.state.clone();

        let on_change_for_keydown = self.on_change.clone();
        let on_change_for_clear = self.on_change.clone();

        let border_color = if recording {
            theme.tokens.primary
        } else if is_focused {
            theme.tokens.ring
        } else {
            theme.tokens.input
        };

        // 聚焦外发光（替代旧库 focus_ring_light）。
        let focus_ring = BoxShadow::new(px(0.0), px(0.0), *theme.tokens.ring).blur_radius(px(6.0));
        let recording_ring = BoxShadow {
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(3.0),
            color: theme.tokens.primary.opacity(0.3),
            inset: false,
        };

        let text_color = if hotkey.is_some() && !recording {
            theme.tokens.foreground
        } else {
            theme.tokens.muted_foreground
        };

        let clear_button = if show_clear {
            Some(
                div()
                    .id("hotkey-clear")
                    .ml(px(8.0))
                    .px(px(6.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .text_color(theme.tokens.muted_foreground)
                    .hover(|s| s.bg(theme.tokens.muted).text_color(theme.tokens.foreground))
                    .on_click(move |_, window, cx| {
                        state_for_clear.update(cx, |state, cx| {
                            state.clear(cx);
                        });
                        if let Some(ref handler) = on_change_for_clear {
                            handler(None, window, cx);
                        }
                        cx.stop_propagation();
                    })
                    .child("×"),
            )
        } else {
            None
        };

        let mut root = div();
        root.style().refine(&user_style);

        root = root.child(
            div()
                .id(("hotkey-input", self.state.entity_id()))
                .track_focus(&focus_handle.tab_index(0).tab_stop(true))
                .h(px(40.0))
                .px(px(12.0))
                .flex()
                .items_center()
                .justify_between()
                .bg(theme.tokens.background)
                .border_1()
                .border_color(border_color)
                .rounded(theme.radius)
                .font_family(theme.mono_font_family.clone())
                .text_size(px(14.0))
                .when(self.disabled, |d| d.opacity(0.5).cursor_not_allowed())
                .when(!self.disabled, |d| d.cursor_pointer())
                .when(is_focused && !recording, |d| d.shadow(vec![focus_ring]))
                .when(recording, |d| {
                    d.shadow(vec![recording_ring])
                        .border_color(theme.tokens.primary)
                })
                .when(!self.disabled, |d| {
                    d.on_click(move |_, window, cx| {
                        state_for_click.update(cx, |state, cx| {
                            if !state.recording {
                                state.start_recording(cx);
                            }
                        });
                        window.refresh();
                    })
                })
                .when(!self.disabled, |d| {
                    d.on_key_down(move |event, window, cx| {
                        let (captured, hotkey) = state_for_keydown.update(cx, |state, cx| {
                            let captured = state.capture_keystroke(&event.keystroke, cx);
                            (captured, state.hotkey.clone())
                        });
                        if captured {
                            if let Some(ref handler) = on_change_for_keydown {
                                handler(hotkey.as_ref(), window, cx);
                            }
                            cx.stop_propagation();
                        }
                    })
                })
                .child(
                    div()
                        .flex_1()
                        .text_color(text_color)
                        .when(recording, |d| d.opacity(0.7))
                        .child(display_text),
                )
                .children(clear_button),
        );

        root
    }
}
