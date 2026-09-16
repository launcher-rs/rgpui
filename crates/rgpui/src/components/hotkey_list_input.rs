//! 多值快捷键录制输入：一行 chip，多绑定追加。
//!
//! 与单值的 [`HotkeyInput`](super::hotkey_input::HotkeyInput) 对应：
//! 点击空白处开始录制，按键后追加为新 chip（去重），每个 chip 自带
//! 删除钮；`Escape` 取消录制。值类型复用 [`HotkeyValue`]。

use std::rc::Rc;

use super::hotkey_input::HotkeyValue;
use crate::{prelude::FluentBuilder as _, *};

/// 按键捕获结果（调用方据此决定是否触发 `on_change`）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HotkeyCapture {
    /// 未在录制，事件未消费。
    Ignored,
    /// `Escape` 取消录制，列表未变。
    Cancelled,
    /// 已捕获有效组合（去重后列表可能不变）。
    Appended,
}
/// 多值快捷键输入状态。
pub struct HotkeyListInputState {
    /// 当前快捷键列表。
    hotkeys: Vec<HotkeyValue>,
    /// 是否正在录制。
    recording: bool,
    /// 焦点句柄。
    focus_handle: FocusHandle,
}

impl HotkeyListInputState {
    /// 创建空状态。
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            hotkeys: Vec::new(),
            recording: false,
            focus_handle: cx.focus_handle(),
        }
    }

    /// 以初始快捷键列表创建状态。
    pub fn with_hotkeys(cx: &mut Context<Self>, hotkeys: Vec<HotkeyValue>) -> Self {
        Self {
            hotkeys,
            recording: false,
            focus_handle: cx.focus_handle(),
        }
    }

    /// 获取当前快捷键列表。
    pub fn hotkeys(&self) -> &[HotkeyValue] {
        &self.hotkeys
    }

    /// 整体替换快捷键列表（同时结束录制）。
    pub fn set_hotkeys(&mut self, hotkeys: Vec<HotkeyValue>, cx: &mut Context<Self>) {
        self.hotkeys = hotkeys;
        self.recording = false;
        cx.notify();
    }

    /// 追加快捷键（键 + 修饰键完全相同时去重，返回是否新增）。
    pub fn add_hotkey(&mut self, hotkey: HotkeyValue, cx: &mut Context<Self>) -> bool {
        let duplicate = self
            .hotkeys
            .iter()
            .any(|existing| existing.key == hotkey.key && existing.modifiers == hotkey.modifiers);
        if duplicate {
            return false;
        }
        self.hotkeys.push(hotkey);
        cx.notify();
        true
    }

    /// 按下标删除快捷键（越界返回 false）。
    pub fn remove_hotkey(&mut self, index: usize, cx: &mut Context<Self>) -> bool {
        if index >= self.hotkeys.len() {
            return false;
        }
        self.hotkeys.remove(index);
        cx.notify();
        true
    }

    /// 清空快捷键列表。
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.hotkeys.clear();
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

    /// 捕获按键事件；录制中且为有效组合时追加。
    /// `Escape` 仅结束录制（列表不变，调用方不应触发 `on_change`）。
    pub fn capture_keystroke(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> HotkeyCapture {
        if !self.recording {
            return HotkeyCapture::Ignored;
        }

        if keystroke.key.as_str() == "escape" {
            self.stop_recording(cx);
            return HotkeyCapture::Cancelled;
        }

        if let Some(hotkey) = HotkeyValue::from_keystroke(keystroke) {
            self.add_hotkey(hotkey, cx);
            self.recording = false;
            cx.notify();
            return HotkeyCapture::Appended;
        }

        HotkeyCapture::Ignored
    }
}

impl Focusable for HotkeyListInputState {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for HotkeyListInputState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// 多值快捷键输入组件。
#[derive(IntoElement)]
pub struct HotkeyListInput {
    /// 绑定状态实体。
    state: Entity<HotkeyListInputState>,
    /// 空列表占位文本。
    placeholder: SharedString,
    /// 录制中提示文本。
    recording_text: SharedString,
    /// 是否禁用。
    disabled: bool,
    /// 快捷键列表变化回调。
    on_change: Option<Rc<dyn Fn(&[HotkeyValue], &mut Window, &mut App)>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl HotkeyListInput {
    /// 创建多值快捷键输入，默认占位 "Click to add"。
    pub fn new(state: Entity<HotkeyListInputState>) -> Self {
        Self {
            state,
            placeholder: "Click to add".into(),
            recording_text: "Press a key...".into(),
            disabled: false,
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置空列表占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置录制中的提示文本（默认 "Press a key..."，多语言应用请覆盖）。
    pub fn recording_text(mut self, text: impl Into<SharedString>) -> Self {
        self.recording_text = text.into();
        self
    }

    /// 设置是否禁用。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置快捷键列表变化回调。
    pub fn on_change(
        mut self,
        handler: impl Fn(&[HotkeyValue], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Styled for HotkeyListInput {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for HotkeyListInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;

        let state_data = self.state.read(cx);
        let hotkeys = state_data.hotkeys.clone();
        let recording = state_data.recording;
        let focus_handle = state_data.focus_handle(cx);
        let is_focused = focus_handle.is_focused(window);

        let state_for_add = self.state.clone();
        let state_for_keydown = self.state.clone();

        let on_change_for_keydown = self.on_change.clone();
        let on_change_for_remove = self.on_change.clone();

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

        let mut root = div();
        root.style().refine(&user_style);

        root.child(
            div()
                .id(("hotkey-list-input", self.state.entity_id()))
                .track_focus(&focus_handle.tab_index(0).tab_stop(true))
                .min_h(px(40.0))
                .px(px(12.0))
                .py(px(6.0))
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(6.0))
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
                        state_for_add.update(cx, |state, cx| {
                            if !state.recording {
                                state.start_recording(cx);
                            }
                        });
                        window.refresh();
                    })
                })
                .when(!self.disabled, |d| {
                    d.on_key_down(move |event, window, cx| {
                        if event.is_held {
                            return;
                        }
                        let (outcome, hotkeys) = state_for_keydown.update(cx, |state, cx| {
                            let outcome = state.capture_keystroke(&event.keystroke, cx);
                            (outcome, state.hotkeys.clone())
                        });
                        match outcome {
                            HotkeyCapture::Ignored => {}
                            HotkeyCapture::Cancelled => cx.stop_propagation(),
                            HotkeyCapture::Appended => {
                                if let Some(ref handler) = on_change_for_keydown {
                                    handler(&hotkeys, window, cx);
                                }
                                cx.stop_propagation();
                            }
                        }
                    })
                })
                .children(hotkeys.iter().enumerate().map(|(ix, hotkey)| {
                    let state_for_remove = self.state.clone();
                    let on_change_for_remove = on_change_for_remove.clone();
                    let label: SharedString = hotkey.format_display().into();
                    h_flex()
                        .id(("hotkey-list-chip", ix))
                        .items_center()
                        .gap(px(4.0))
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .bg(theme.tokens.muted)
                        .text_color(theme.tokens.foreground)
                        .child(label)
                        .child(
                            div()
                                .id(("hotkey-list-remove", ix))
                                .text_color(theme.tokens.muted_foreground)
                                .hover(|s| s.text_color(theme.tokens.foreground))
                                .on_click(move |_, window, cx| {
                                    let hotkeys = state_for_remove.update(cx, |state, cx| {
                                        state.remove_hotkey(ix, cx);
                                        state.hotkeys.clone()
                                    });
                                    if let Some(ref handler) = on_change_for_remove {
                                        handler(&hotkeys, window, cx);
                                    }
                                    cx.stop_propagation();
                                })
                                .child("×"),
                        )
                        .into_any_element()
                }))
                .when(hotkeys.is_empty() && !recording, |d| {
                    d.child(
                        div()
                            .text_color(theme.tokens.muted_foreground)
                            .child(self.placeholder.clone()),
                    )
                })
                .when(recording, |d| {
                    d.child(
                        div()
                            .text_color(theme.tokens.muted_foreground)
                            .opacity(0.7)
                            .child(self.recording_text.clone()),
                    )
                }),
        )
    }
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本文件有 `use crate::*`，会把根导出的
    // `test` 过程宏引进作用域，遮蔽内置 `#[test]` 导致宏无限递归。
    use super::super::hotkey_input::HotkeyValue;
    use super::{HotkeyCapture, HotkeyListInput, HotkeyListInputState};
    use crate::{AppContext as _, Context, Entity, Keystroke, Render, Window};

    /// 测试宿主视图。
    struct Probe {
        state: Entity<HotkeyListInputState>,
    }

    impl Render for Probe {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> impl crate::IntoElement {
            HotkeyListInput::new(self.state.clone())
        }
    }

    /// 追加去重、按下标删除、清空。
    #[rgpui::test]
    fn list_add_remove_clear(cx: &mut crate::TestAppContext) {
        let state = cx.new(HotkeyListInputState::new);
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                let a = HotkeyValue::new("a", Default::default());
                let b = HotkeyValue::new("b", Default::default());
                assert!(state.add_hotkey(a.clone(), cx));
                assert!(!state.add_hotkey(a, cx));
                assert!(state.add_hotkey(b, cx));
                assert_eq!(state.hotkeys().len(), 2);
                assert!(!state.remove_hotkey(7, cx));
                assert!(state.remove_hotkey(0, cx));
                assert_eq!(state.hotkeys()[0].key, "b");
                state.clear(cx);
                assert!(state.hotkeys().is_empty());
            });
        });
    }

    /// 录制捕获追加并结束录制；`Escape` 只结束录制不改动列表。
    #[rgpui::test]
    fn capture_appends_and_escape_only_stops(cx: &mut crate::TestAppContext) {
        let state = cx.new(HotkeyListInputState::new);
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                assert_eq!(
                    state.capture_keystroke(&Keystroke::parse("ctrl-a").unwrap(), cx),
                    HotkeyCapture::Ignored
                );
                state.start_recording(cx);
                assert_eq!(
                    state.capture_keystroke(&Keystroke::parse("ctrl-a").unwrap(), cx),
                    HotkeyCapture::Appended
                );
                assert!(!state.is_recording());
                assert_eq!(state.hotkeys().len(), 1);

                state.start_recording(cx);
                assert_eq!(
                    state.capture_keystroke(&Keystroke::parse("escape").unwrap(), cx),
                    HotkeyCapture::Cancelled
                );
                assert!(!state.is_recording());
                assert_eq!(state.hotkeys().len(), 1);

                state.set_hotkeys(Vec::new(), cx);
                assert!(state.hotkeys().is_empty());
            });
        });
    }

    /// 带 chip 与录制态可绘制（冒烟测试）。
    #[rgpui::test]
    fn renders_chips_and_recording_without_panic(cx: &mut crate::TestAppContext) {
        let state = cx.new(HotkeyListInputState::new);
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                state.set_hotkeys(vec![HotkeyValue::new("a", Default::default())], cx);
                state.start_recording(cx);
            });
        });
        let (_view, cx) = cx.add_window_view(|_, _| Probe {
            state: state.clone(),
        });
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    }
}
