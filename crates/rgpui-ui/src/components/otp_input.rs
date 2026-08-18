//! 一次性密码（OTP）输入：多位数字格子输入。

use std::rc::Rc;

use rgpui::{prelude::FluentBuilder as _, *};

actions!(
    otp_input,
    [
        OTPBackspace,
        OTPDelete,
        OTPLeft,
        OTPRight,
        OTPHome,
        OTPEnd,
        OTPPaste,
        OTPEscape,
    ]
);

/// 注册 OTP 输入的键盘绑定。
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", OTPBackspace, Some("OTPInput")),
        KeyBinding::new("delete", OTPDelete, Some("OTPInput")),
        KeyBinding::new("left", OTPLeft, Some("OTPInput")),
        KeyBinding::new("right", OTPRight, Some("OTPInput")),
        KeyBinding::new("home", OTPHome, Some("OTPInput")),
        KeyBinding::new("end", OTPEnd, Some("OTPInput")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-v", OTPPaste, Some("OTPInput")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-v", OTPPaste, Some("OTPInput")),
        KeyBinding::new("escape", OTPEscape, Some("OTPInput")),
    ]);
}

/// OTP 输入事件。
#[derive(Clone, Debug)]
pub enum OTPInputEvent {
    /// 值变化。
    Change(String),
    /// 输入完整。
    Complete(String),
    /// 获得焦点。
    Focus,
    /// 失去焦点。
    Blur,
}

/// OTP 输入尺寸。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OTPInputSize {
    Sm,
    Md,
    Lg,
}

impl Default for OTPInputSize {
    fn default() -> Self {
        Self::Md
    }
}

/// OTP 输入状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OTPInputState {
    Default,
    Error,
    Success,
}

impl Default for OTPInputState {
    fn default() -> Self {
        Self::Default
    }
}

/// OTP 状态实体：管理各格子的焦点与数字。
pub struct OTPState {
    /// 每个格子的焦点句柄。
    focus_handles: Vec<FocusHandle>,
    /// 各格子数字。
    digits: Vec<Option<char>>,
    /// 当前聚焦下标。
    focused_index: usize,
    /// 格子数量。
    digit_count: usize,
    /// 是否掩码显示。
    masked: bool,
    /// 是否禁用。
    disabled: bool,
    /// 输入状态。
    state: OTPInputState,
}

impl EventEmitter<OTPInputEvent> for OTPState {}

impl OTPState {
    /// 创建 OTP 状态，格子数限制在 4~8。
    pub fn new(cx: &mut Context<Self>, digit_count: usize) -> Self {
        let digit_count = digit_count.clamp(4, 8);
        let focus_handles: Vec<FocusHandle> = (0..digit_count).map(|_| cx.focus_handle()).collect();

        Self {
            focus_handles,
            digits: vec![None; digit_count],
            focused_index: 0,
            digit_count,
            masked: false,
            disabled: false,
            state: OTPInputState::Default,
        }
    }

    /// 设置格子数量（4~8）。
    pub fn digit_count(mut self, count: usize) -> Self {
        let count = count.clamp(4, 8);
        self.digit_count = count;
        self.digits.resize(count, None);
        self
    }

    /// 设置是否掩码显示。
    pub fn masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }

    /// 获取当前输入的完整字符串。
    pub fn value(&self) -> String {
        self.digits.iter().filter_map(|d| *d).collect()
    }

    /// 是否已填满。
    pub fn is_complete(&self) -> bool {
        self.digits.iter().all(|d| d.is_some())
    }

    /// 以字符串设置数字（仅接受 ASCII 数字）。
    pub fn set_value(&mut self, value: &str, cx: &mut Context<Self>) {
        self.digits.fill(None);

        for (i, ch) in value.chars().take(self.digit_count).enumerate() {
            if ch.is_ascii_digit() {
                self.digits[i] = Some(ch);
            }
        }

        cx.emit(OTPInputEvent::Change(self.value()));

        if self.is_complete() {
            cx.emit(OTPInputEvent::Complete(self.value()));
        }

        cx.notify();
    }

    /// 清空所有数字。
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.digits.fill(None);
        self.focused_index = 0;
        cx.emit(OTPInputEvent::Change(String::new()));
        cx.notify();
    }

    /// 设置输入状态（默认/错误/成功）。
    pub fn set_state(&mut self, state: OTPInputState, cx: &mut Context<Self>) {
        self.state = state;
        cx.notify();
    }

    /// 设为错误状态。
    pub fn set_error(&mut self, cx: &mut Context<Self>) {
        self.state = OTPInputState::Error;
        cx.notify();
    }

    /// 设为成功状态。
    pub fn set_success(&mut self, cx: &mut Context<Self>) {
        self.state = OTPInputState::Success;
        cx.notify();
    }

    /// 聚焦第一个格子。
    pub fn focus_first(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focused_index = 0;
        if let Some(handle) = self.focus_handles.first() {
            window.focus(handle, cx);
        }
        cx.notify();
    }

    /// 在指定格子写入数字并自动推进焦点。
    fn set_digit(
        &mut self,
        index: usize,
        digit: char,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.digit_count || !digit.is_ascii_digit() {
            return;
        }

        self.digits[index] = Some(digit);
        cx.emit(OTPInputEvent::Change(self.value()));

        if self.is_complete() {
            cx.emit(OTPInputEvent::Complete(self.value()));
        } else if index + 1 < self.digit_count {
            self.focused_index = index + 1;
            window.focus(&self.focus_handles[index + 1], cx);
        }

        cx.notify();
    }

    /// 清除指定格子的数字。
    fn clear_digit(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.digit_count {
            return;
        }

        self.digits[index] = None;
        cx.emit(OTPInputEvent::Change(self.value()));
        cx.notify();
    }

    /// 焦点左移。
    fn move_left(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.focused_index > 0 {
            self.focused_index -= 1;
            window.focus(&self.focus_handles[self.focused_index], cx);
            cx.notify();
        }
    }

    /// 焦点右移。
    fn move_right(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.focused_index + 1 < self.digit_count {
            self.focused_index += 1;
            window.focus(&self.focus_handles[self.focused_index], cx);
            cx.notify();
        }
    }

    /// 焦点移到开头。
    fn move_home(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focused_index = 0;
        window.focus(&self.focus_handles[0], cx);
        cx.notify();
    }

    /// 焦点移到末尾。
    fn move_end(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focused_index = self.digit_count - 1;
        window.focus(&self.focus_handles[self.focused_index], cx);
        cx.notify();
    }

    /// 退格：清除当前格子或回退到前一格清除。
    pub fn backspace(&mut self, _: &OTPBackspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }

        if self.digits[self.focused_index].is_some() {
            self.clear_digit(self.focused_index, cx);
        } else if self.focused_index > 0 {
            self.focused_index -= 1;
            self.clear_digit(self.focused_index, cx);
            window.focus(&self.focus_handles[self.focused_index], cx);
        }
    }

    /// 删除当前格子数字。
    pub fn delete(&mut self, _: &OTPDelete, _window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }

        self.clear_digit(self.focused_index, cx);
    }

    /// 左移。
    pub fn left(&mut self, _: &OTPLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.move_left(window, cx);
    }

    /// 右移。
    pub fn right(&mut self, _: &OTPRight, window: &mut Window, cx: &mut Context<Self>) {
        self.move_right(window, cx);
    }

    /// 移到开头。
    pub fn home(&mut self, _: &OTPHome, window: &mut Window, cx: &mut Context<Self>) {
        self.move_home(window, cx);
    }

    /// 移到末尾。
    pub fn end(&mut self, _: &OTPEnd, window: &mut Window, cx: &mut Context<Self>) {
        self.move_end(window, cx);
    }

    /// 从剪贴板粘贴数字。
    pub fn paste(&mut self, _: &OTPPaste, _window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }

        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
            self.set_value(&digits, cx);
        }
    }

    /// 退出焦点。
    pub fn escape(&mut self, _: &OTPEscape, window: &mut Window, cx: &mut Context<Self>) {
        window.blur();
        cx.emit(OTPInputEvent::Blur);
        cx.notify();
    }

    /// 获取指定格子的焦点句柄。
    pub fn focus_handle(&self, index: usize, _: &App) -> Option<FocusHandle> {
        self.focus_handles.get(index).cloned()
    }
}

impl Focusable for OTPState {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handles
            .first()
            .cloned()
            .expect("OTPState must have at least one focus handle")
    }
}

impl Render for OTPState {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// 一次性密码输入组件。
#[derive(IntoElement)]
pub struct OTPInput {
    /// 绑定状态实体。
    state: Entity<OTPState>,
    /// 尺寸。
    size: OTPInputSize,
    /// 是否禁用。
    disabled: bool,
    /// 是否掩码显示。
    masked: bool,
    /// 分隔符文本。
    separator: Option<SharedString>,
    /// 分隔符位置（从 1 计）。
    separator_position: Option<usize>,
    /// 值变化回调。
    on_change: Option<Rc<dyn Fn(String, &mut App)>>,
    /// 输入完成回调。
    on_complete: Option<Rc<dyn Fn(String, &mut App)>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl OTPInput {
    /// 创建 OTP 输入组件。
    pub fn new(state: &Entity<OTPState>) -> Self {
        Self {
            state: state.clone(),
            size: OTPInputSize::default(),
            disabled: false,
            masked: false,
            separator: None,
            separator_position: None,
            on_change: None,
            on_complete: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置尺寸。
    pub fn size(mut self, size: OTPInputSize) -> Self {
        self.size = size;
        self
    }

    /// 设置是否禁用。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置是否掩码显示。
    pub fn masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }

    /// 设置分隔符。
    pub fn separator(mut self, separator: impl Into<SharedString>) -> Self {
        self.separator = Some(separator.into());
        self
    }

    /// 设置分隔符位置（从 1 计）。
    pub fn separator_position(mut self, position: usize) -> Self {
        self.separator_position = Some(position);
        self
    }

    /// 设置值变化回调。
    pub fn on_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(String, &mut App) + 'static,
    {
        self.on_change = Some(Rc::new(callback));
        self
    }

    /// 设置输入完成回调。
    pub fn on_complete<F>(mut self, callback: F) -> Self
    where
        F: Fn(String, &mut App) + 'static,
    {
        self.on_complete = Some(Rc::new(callback));
        self
    }

    /// 获取格子边长。
    fn box_size(&self) -> Pixels {
        match self.size {
            OTPInputSize::Sm => px(36.0),
            OTPInputSize::Md => px(44.0),
            OTPInputSize::Lg => px(52.0),
        }
    }

    /// 获取字号。
    fn font_size(&self) -> Pixels {
        match self.size {
            OTPInputSize::Sm => px(16.0),
            OTPInputSize::Md => px(20.0),
            OTPInputSize::Lg => px(24.0),
        }
    }

    /// 获取格子间隙。
    fn input_gap(&self) -> Pixels {
        match self.size {
            OTPInputSize::Sm => px(6.0),
            OTPInputSize::Md => px(8.0),
            OTPInputSize::Lg => px(10.0),
        }
    }
}

impl Styled for OTPInput {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for OTPInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // 先取出所有需要的主题值，避免 cx 借用与后续实体更新冲突。
        let theme = cx.theme();
        let border = theme.tokens.border;
        let ring = theme.tokens.ring;
        let muted = theme.tokens.muted;
        let background = theme.tokens.background;
        let muted_foreground = theme.tokens.muted_foreground;
        let foreground = theme.tokens.foreground;
        let primary = theme.tokens.primary;
        let radius = theme.radius;
        let mono_font_family = theme.mono_font_family.clone();
        let error_border = theme.highlight_theme.style.status.error_border(cx);

        let box_size = self.box_size();
        let font_size = self.font_size();
        let input_gap = self.input_gap();

        let otp_state = self.state.read(cx);
        let digit_count = otp_state.digit_count;
        let digits = otp_state.digits.clone();
        let _focused_index = otp_state.focused_index;
        let state = otp_state.state;
        let masked = self.masked || otp_state.masked;
        let disabled = self.disabled || otp_state.disabled;
        let focus_handles: Vec<FocusHandle> = otp_state.focus_handles.iter().cloned().collect();

        self.state.update(cx, |state, _| {
            state.disabled = disabled;
            state.masked = masked;
        });

        let on_change_callback = self.on_change.clone();
        let on_complete_callback = self.on_complete.clone();

        if on_change_callback.is_some() || on_complete_callback.is_some() {
            let state_entity = self.state.clone();
            cx.subscribe(
                &state_entity,
                move |_emitter: Entity<OTPState>, event: &OTPInputEvent, cx: &mut App| match event {
                    OTPInputEvent::Change(value) => {
                        if let Some(callback) = on_change_callback.as_ref() {
                            callback(value.clone(), cx);
                        }
                    }
                    OTPInputEvent::Complete(value) => {
                        if let Some(callback) = on_complete_callback.as_ref() {
                            callback(value.clone(), cx);
                        }
                    }
                    _ => {}
                },
            )
            .detach();
        }

        let (border_color, focus_border_color) = match state {
            OTPInputState::Default => (border.color, ring.color),
            OTPInputState::Error => (error_border, error_border),
            OTPInputState::Success => (primary.color, primary.color),
        };

        let user_style = self.style;
        let separator = self.separator.clone();
        let separator_position = self.separator_position.unwrap_or(digit_count / 2);

        let mut root = div()
            .id(("otp-input", self.state.entity_id()))
            .key_context("OTPInput")
            .flex()
            .items_center()
            .gap(input_gap)
            .when(!disabled, |this| {
                this.on_action(window.listener_for(&self.state, OTPState::backspace))
                    .on_action(window.listener_for(&self.state, OTPState::delete))
                    .on_action(window.listener_for(&self.state, OTPState::left))
                    .on_action(window.listener_for(&self.state, OTPState::right))
                    .on_action(window.listener_for(&self.state, OTPState::home))
                    .on_action(window.listener_for(&self.state, OTPState::end))
                    .on_action(window.listener_for(&self.state, OTPState::paste))
                    .on_action(window.listener_for(&self.state, OTPState::escape))
            })
            .children((0..digit_count).flat_map(|i| {
                let state_clone = self.state.clone();
                let focus_handle = focus_handles[i].clone();
                let focus_handle_for_track = focus_handle.clone();
                let focus_handle_for_click = focus_handle.clone();
                let is_focused = focus_handle.is_focused(window);
                let digit = digits[i];

                let display_char = if let Some(d) = digit {
                    if masked {
                        SharedString::from("●")
                    } else {
                        SharedString::from(d.to_string())
                    }
                } else {
                    SharedString::from("")
                };

                let digit_box = div()
                    .id(ElementId::NamedInteger("otp-digit".into(), i as u64))
                    .track_focus(&focus_handle_for_track.tab_index(0).tab_stop(true))
                    .size(box_size)
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(if disabled {
                        muted.opacity(0.5)
                    } else {
                        background.color
                    })
                    .border_1()
                    .border_color(if is_focused && !disabled {
                        focus_border_color
                    } else {
                        border_color
                    })
                    .rounded(radius)
                    .text_size(font_size)
                    .font_weight(FontWeight::SEMIBOLD)
                    .font_family(mono_font_family.clone())
                    .text_color(if disabled {
                        muted_foreground
                    } else {
                        foreground
                    })
                    .when(is_focused && !disabled, |this| {
                        // 聚焦时用主题色外发光代替旧库的 focus_ring 阴影。
                        this.shadow(vec![
                            BoxShadow::new(px(0.0), px(0.0), focus_border_color)
                                .blur_radius(px(6.0)),
                        ])
                    })
                    .when(!disabled, |this| {
                        this.cursor(CursorStyle::IBeam)
                            .hover(|style| style.border_color(focus_border_color))
                    })
                    .on_mouse_down(MouseButton::Left, {
                        let state = state_clone.clone();
                        move |_, window, cx| {
                            window.focus(&focus_handle_for_click, cx);
                            state.update(cx, |s, cx| {
                                s.focused_index = i;
                                cx.notify();
                            });
                        }
                    })
                    .on_key_down({
                        let state = state_clone.clone();
                        move |event, window, cx| {
                            if disabled {
                                return;
                            }

                            let key = &event.keystroke.key;
                            if key.len() == 1 {
                                if let Some(ch) = key.chars().next() {
                                    if ch.is_ascii_digit() {
                                        state.update(cx, |s, cx| {
                                            s.set_digit(i, ch, window, cx);
                                        });
                                        cx.stop_propagation();
                                    }
                                }
                            }
                        }
                    })
                    .child(display_char)
                    .into_any_element();

                let should_show_separator =
                    separator.is_some() && i == separator_position - 1 && i + 1 < digit_count;

                if should_show_separator {
                    vec![
                        digit_box,
                        div()
                            .text_size(font_size)
                            .text_color(muted_foreground)
                            .child(separator.clone().unwrap())
                            .into_any_element(),
                    ]
                } else {
                    vec![digit_box]
                }
            }));

        root.style().refine(&user_style);
        root
    }
}
