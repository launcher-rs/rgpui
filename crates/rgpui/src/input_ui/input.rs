use crate::prelude::FluentBuilder as _;
use crate::theme::ActiveTheme as _;
use crate::{
    AccessibleAction, AnyElement, App, Colorize as _, DefiniteLength, Edges, ElementSize, Entity,
    Hsla, IconName, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, StyledExt as _, TextAlign, Window, div, px, relative,
};
use crate::{
    Button, ButtonVariants as _, Selectable, Sizable, Spinner, StyleSized as _, h_flex, v_flex,
};

use super::{
    CONTEXT, InputContentType, InputState, content_type::sync_native_content_type,
    element::EditorScrollbar,
};

/// 返回输入类组件的 `(背景, 前景)` 颜色。
pub(crate) fn input_style(disabled: bool, cx: &App) -> (Hsla, Hsla) {
    if disabled {
        (
            cx.theme().input.mix_oklab(cx.theme().transparent, 0.8),
            cx.theme().muted_foreground,
        )
    } else {
        (cx.theme().input_background(), cx.theme().foreground)
    }
}

/// 绑定到一个 [`InputState`] 的文本输入元素。
#[derive(IntoElement)]
pub struct Input {
    state: Entity<InputState>,
    style: StyleRefinement,
    size: ElementSize,
    prefix: Option<AnyElement>,
    suffix: Option<AnyElement>,
    height: Option<DefiniteLength>,
    appearance: bool,
    cleanable: bool,
    mask_toggle: bool,
    disabled: bool,
    bordered: bool,
    focus_bordered: bool,
    tab_index: isize,
    selected: bool,
    content_type: Option<InputContentType>,
    role: Option<Role>,
    aria_label: Option<SharedString>,
}

impl Sizable for Input {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl Selectable for Input {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl Input {
    /// 创建一个绑定到 [`InputState`] 的 [`Input`] 元素。
    pub fn new(state: &Entity<InputState>) -> Self {
        Self {
            state: state.clone(),
            size: ElementSize::default(),
            style: StyleRefinement::default(),
            prefix: None,
            suffix: None,
            height: None,
            appearance: true,
            cleanable: false,
            mask_toggle: false,
            disabled: false,
            bordered: true,
            focus_bordered: true,
            tab_index: 0,
            selected: false,
            content_type: None,
            role: None,
            aria_label: None,
        }
    }

    /// 设置无障碍标签（aria-label）。
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// 设置前缀元素，渲染在输入框左侧。
    pub fn prefix(mut self, prefix: impl IntoElement) -> Self {
        self.prefix = Some(prefix.into_any_element());
        self
    }

    /// 设置后缀元素，渲染在输入框右侧。
    pub fn suffix(mut self, suffix: impl IntoElement) -> Self {
        self.suffix = Some(suffix.into_any_element());
        self
    }

    /// 设置输入框占满全部高度（仅多行模式）。
    pub fn h_full(mut self) -> Self {
        self.height = Some(relative(1.));
        self
    }

    /// 设置输入框高度（仅多行模式）。
    pub fn h(mut self, height: impl Into<DefiniteLength>) -> Self {
        self.height = Some(height.into());
        self
    }

    /// 设置输入框外观，为 false 时不显示边框和背景。
    pub fn appearance(mut self, appearance: bool) -> Self {
        self.appearance = appearance;
        self
    }

    /// 设置输入框是否显示边框，默认 true。
    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    /// 设置聚焦时是否显示边框，默认 true。
    pub fn focus_bordered(mut self, bordered: bool) -> Self {
        self.focus_bordered = bordered;
        self
    }

    /// 设置输入框非空时是否显示清除按钮，默认 false。
    pub fn cleanable(mut self, cleanable: bool) -> Self {
        self.cleanable = cleanable;
        self
    }

    /// 启用密码掩码状态的切换按钮。
    pub fn mask_toggle(mut self) -> Self {
        self.mask_toggle = true;
        self
    }

    /// 设置密码管理器和自动填充的语义内容类型。
    ///
    /// 这是组件级别的语义提示，不会改变文本值或掩码渲染状态。
    pub fn content_type(mut self, content_type: InputContentType) -> Self {
        self.content_type = Some(content_type);
        self
    }

    /// 覆盖输入框的无障碍角色。
    ///
    /// 如果未设置，则根据多行模式和内容类型推断角色。
    pub fn role(mut self, role: Role) -> Self {
        self.role = Some(role);
        self
    }

    /// 设置输入框为禁用状态。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置输入框的 tab index，默认 0。
    pub fn tab_index(mut self, index: isize) -> Self {
        self.tab_index = index;
        self
    }

    fn render_toggle_mask_button(state: &Entity<InputState>, cx: &App) -> impl IntoElement {
        let masked = state.read(cx).masked;
        Button::new("toggle-mask")
            .icon(if masked {
                IconName::Eye
            } else {
                IconName::EyeOff
            })
            .xsmall()
            .text()
            .tab_stop(false)
            .on_click({
                let state = state.clone();
                move |_, window, cx| {
                    state.update(cx, |state, cx| {
                        state.set_masked(!state.masked, window, cx);
                    })
                }
            })
    }

    fn mouse_down_handler(
        state: Entity<InputState>,
        content_type: Option<InputContentType>,
        disabled: bool,
    ) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
        move |event, window, cx| {
            sync_native_content_type(window, content_type, disabled);
            state.update(cx, |state, cx| state.on_mouse_down(event, window, cx));
        }
    }

    fn accessibility_role(
        is_multi_line: bool,
        content_type: Option<InputContentType>,
        role: Option<Role>,
    ) -> Role {
        if let Some(role) = role {
            return role;
        }

        if is_multi_line {
            return Role::MultilineTextInput;
        }

        match content_type {
            None => Role::TextInput,
            Some(InputContentType::TelephoneNumber) => Role::PhoneNumberInput,
            Some(InputContentType::EmailAddress) => Role::EmailInput,
            Some(InputContentType::Url) => Role::UrlInput,
            Some(InputContentType::Password | InputContentType::NewPassword) => Role::PasswordInput,
            Some(InputContentType::DateTime) => Role::DateTimeInput,
            Some(InputContentType::Birthdate) => Role::DateInput,
            Some(
                InputContentType::Name
                | InputContentType::NamePrefix
                | InputContentType::GivenName
                | InputContentType::MiddleName
                | InputContentType::FamilyName
                | InputContentType::NameSuffix
                | InputContentType::Nickname
                | InputContentType::JobTitle
                | InputContentType::OrganizationName
                | InputContentType::Location
                | InputContentType::FullStreetAddress
                | InputContentType::StreetAddressLine1
                | InputContentType::StreetAddressLine2
                | InputContentType::AddressCity
                | InputContentType::AddressState
                | InputContentType::AddressCityAndState
                | InputContentType::Sublocality
                | InputContentType::CountryName
                | InputContentType::PostalCode
                | InputContentType::CreditCardNumber
                | InputContentType::CreditCardName
                | InputContentType::CreditCardGivenName
                | InputContentType::CreditCardMiddleName
                | InputContentType::CreditCardFamilyName
                | InputContentType::CreditCardSecurityCode
                | InputContentType::CreditCardExpiration
                | InputContentType::CreditCardExpirationMonth
                | InputContentType::CreditCardExpirationYear
                | InputContentType::CreditCardType
                | InputContentType::Username
                | InputContentType::OneTimeCode
                | InputContentType::ShipmentTrackingNumber
                | InputContentType::FlightNumber
                | InputContentType::BirthdateDay
                | InputContentType::BirthdateMonth
                | InputContentType::BirthdateYear
                | InputContentType::CellularEid
                | InputContentType::CellularImei,
            ) => Role::TextInput,
        }
    }

    fn exposes_accessibility_value(masked: bool, content_type: Option<InputContentType>) -> bool {
        !masked
            && !matches!(
                content_type,
                Some(InputContentType::Password | InputContentType::NewPassword)
            )
    }

    fn handle_accessibility_set_value(
        state: &Entity<InputState>,
        data: Option<&accesskit::ActionData>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(accesskit::ActionData::Value(value)) = data else {
            return;
        };
        state.update(cx, |state, cx| {
            state.replace_all(value.to_string(), window, cx);
        });
    }

    /// 此方法必须在 `refine_style` 之后调用。
    fn render_editor(
        paddings: Edges<DefiniteLength>,
        input_state: &Entity<InputState>,
        state: &InputState,
        window: &Window,
    ) -> impl IntoElement {
        let base_size = window.text_style().font_size;
        let rem_size = window.rem_size();

        let paddings = Edges {
            left: paddings.left.to_pixels(base_size, rem_size),
            right: paddings.right.to_pixels(base_size, rem_size),
            top: paddings.top.to_pixels(base_size, rem_size),
            bottom: paddings.bottom.to_pixels(base_size, rem_size),
        };

        state.editor_scrollbar_paddings.set(paddings);
        state.editor_scrollbar_snapshot.set(None);

        v_flex().size_full().child(
            div()
                .relative()
                .flex_1()
                .child(input_state.clone())
                .child(EditorScrollbar::new(input_state.clone())),
        )
    }
}

impl Styled for Input {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Input {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        const LINE_HEIGHT: crate::Rems = crate::Rems(1.25);
        let text_align = self.style.text.text_align.unwrap_or(TextAlign::Left);

        self.state.update(cx, |state, _| {
            state.disabled = self.disabled;
            state.size = self.size;

            // 仅单行模式
            if state.mode.is_single_line() {
                state.text_align = text_align;
            }
        });

        let state = self.state.read(cx);
        let content_type = self.content_type;
        let disabled = self.disabled;
        let is_multi_line = state.mode.is_multi_line();
        let accessibility_role = Self::accessibility_role(is_multi_line, content_type, self.role);
        let accessibility_state = self.state.clone();
        // 物化整个 rope 只在无障碍树中可观测，
        // 因此当没有客户端监听时跳过它。
        let accessibility_value = (window.is_a11y_active()
            && Self::exposes_accessibility_value(state.masked, content_type))
        .then(|| state.text.to_string());
        let focused = state.focus_handle.is_focused(window) && !state.disabled;
        if focused {
            sync_native_content_type(window, content_type, state.disabled);
        }

        let gap_x = match self.size {
            ElementSize::Small => px(4.),
            ElementSize::Large => px(8.),
            _ => px(6.),
        };

        let (bg, _) = input_style(state.disabled, cx);
        let bg = if state.mode.is_code_editor() {
            cx.theme().editor_background()
        } else {
            bg
        };
        let bg = if state.disabled { bg.opacity(0.5) } else { bg };
        let border_color = if state.disabled {
            cx.theme().input.opacity(0.5)
        } else {
            cx.theme().input
        };

        let prefix = self.prefix;
        let suffix = self.suffix;
        let show_clear_button = self.cleanable
            && !state.disabled
            && !state.loading
            && state.text.len() > 0
            && state.mode.is_single_line();
        let has_suffix = suffix.is_some() || state.loading || self.mask_toggle || show_clear_button;

        let placeholder = Some(state.placeholder.clone()).filter(|p| !p.is_empty());

        // 不要把掩码派生的占位文本（"(___)___-___"）用作 aria_label 回退。
        let placeholder_is_mask =
            state.mask_pattern.placeholder().as_deref() == placeholder.as_deref();

        let aria_label = match self.aria_label {
            Some(label) => Some(label),
            None if placeholder_is_mask => None,
            None => placeholder.clone(),
        };

        div()
            .id(("input", self.state.entity_id()))
            .role(accessibility_role)
            .when_some(aria_label, |this, label| this.aria_label(label))
            .when_some(placeholder, |this, placeholder| {
                this.aria_placeholder(placeholder)
            })
            .when_some(accessibility_value, |this, value| this.aria_value(value))
            .flex()
            .key_context(CONTEXT)
            .track_focus(&state.focus_handle.clone())
            .tab_index(self.tab_index)
            .when(!state.disabled, |this| {
                this.on_a11y_action(AccessibleAction::SetValue, move |data, window, cx| {
                    Self::handle_accessibility_set_value(&accessibility_state, data, window, cx);
                })
                .on_action(window.listener_for(&self.state, InputState::backspace))
                .on_action(window.listener_for(&self.state, InputState::delete))
                .on_action(
                    window.listener_for(&self.state, InputState::delete_to_beginning_of_line),
                )
                .on_action(window.listener_for(&self.state, InputState::delete_to_end_of_line))
                .on_action(window.listener_for(&self.state, InputState::delete_previous_word))
                .on_action(window.listener_for(&self.state, InputState::delete_next_word))
                .on_action(window.listener_for(&self.state, InputState::enter))
                .on_action(window.listener_for(&self.state, InputState::escape))
                .on_action(window.listener_for(&self.state, InputState::paste))
                .on_action(window.listener_for(&self.state, InputState::cut))
                .on_action(window.listener_for(&self.state, InputState::undo))
                .on_action(window.listener_for(&self.state, InputState::redo))
                .when(state.mode.is_multi_line(), |this| {
                    this.on_action(window.listener_for(&self.state, InputState::indent_inline))
                        .on_action(window.listener_for(&self.state, InputState::outdent_inline))
                        .on_action(window.listener_for(&self.state, InputState::indent_block))
                        .on_action(window.listener_for(&self.state, InputState::outdent_block))
                })
            })
            .on_action(window.listener_for(&self.state, InputState::left))
            .on_action(window.listener_for(&self.state, InputState::right))
            .on_action(window.listener_for(&self.state, InputState::select_left))
            .on_action(window.listener_for(&self.state, InputState::select_right))
            .when(state.mode.is_multi_line(), |this| {
                this.on_action(window.listener_for(&self.state, InputState::up))
                    .on_action(window.listener_for(&self.state, InputState::down))
                    .on_action(window.listener_for(&self.state, InputState::select_up))
                    .on_action(window.listener_for(&self.state, InputState::select_down))
                    .on_action(window.listener_for(&self.state, InputState::page_up))
                    .on_action(window.listener_for(&self.state, InputState::page_down))
            })
            .on_action(window.listener_for(&self.state, InputState::select_all))
            .on_action(window.listener_for(&self.state, InputState::select_to_start_of_line))
            .on_action(window.listener_for(&self.state, InputState::select_to_end_of_line))
            .on_action(window.listener_for(&self.state, InputState::select_to_previous_word))
            .on_action(window.listener_for(&self.state, InputState::select_to_next_word))
            .on_action(window.listener_for(&self.state, InputState::home))
            .on_action(window.listener_for(&self.state, InputState::end))
            .on_action(window.listener_for(&self.state, InputState::move_to_start))
            .on_action(window.listener_for(&self.state, InputState::move_to_end))
            .on_action(window.listener_for(&self.state, InputState::move_to_previous_word))
            .on_action(window.listener_for(&self.state, InputState::move_to_next_word))
            .on_action(window.listener_for(&self.state, InputState::select_to_start))
            .on_action(window.listener_for(&self.state, InputState::select_to_end))
            .on_action(window.listener_for(&self.state, InputState::show_character_palette))
            .on_action(window.listener_for(&self.state, InputState::copy))
            .on_key_down(window.listener_for(&self.state, InputState::on_key_down))
            .on_mouse_down(
                MouseButton::Left,
                Self::mouse_down_handler(self.state.clone(), content_type, disabled),
            )
            .on_mouse_down(
                MouseButton::Right,
                Self::mouse_down_handler(self.state.clone(), content_type, disabled),
            )
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&self.state, InputState::on_mouse_up),
            )
            .on_mouse_up(
                MouseButton::Right,
                window.listener_for(&self.state, InputState::on_mouse_up),
            )
            .on_mouse_move(window.listener_for(&self.state, InputState::on_mouse_move))
            .on_scroll_wheel(window.listener_for(&self.state, InputState::on_scroll_wheel))
            .size_full()
            .line_height(LINE_HEIGHT)
            .input_px(self.size)
            .input_py(self.size)
            .input_h(self.size)
            .input_text_size(self.size)
            .when(!self.disabled, |this| this.cursor_text())
            .items_center()
            .when(state.mode.is_multi_line(), |this| {
                this.h_auto()
                    .when_some(self.height, |this, height| this.h(height))
            })
            .when(self.appearance, |this| {
                this.bg(bg)
                    .rounded(cx.theme().radius)
                    .when(self.bordered, |this| {
                        this.border_color(border_color)
                            .border_1()
                            .when(focused && self.focus_bordered, |this| {
                                this.focused_border(cx)
                            })
                    })
            })
            .items_center()
            .gap(gap_x)
            .refine_style(&self.style)
            .children(prefix.map(|p| {
                div()
                    .when(state.disabled, |this| this.opacity(0.5))
                    .child(p)
            }))
            .when(state.mode.is_multi_line(), |mut this| {
                let paddings = this.style().padding.clone();
                this.child(Self::render_editor(
                    paddings.into(),
                    &self.state,
                    &state,
                    window,
                ))
            })
            .when(!state.mode.is_multi_line(), |this| {
                this.child(self.state.clone())
            })
            .when(has_suffix, |this| {
                this.pr(self.size.input_px()).child(
                    h_flex()
                        .id("suffix")
                        .gap(gap_x)
                        .items_center()
                        .cursor_default()
                        .when(state.disabled, |this| this.opacity(0.5))
                        .when(state.loading, |this| {
                            this.child(Spinner::new().color(cx.theme().muted_foreground))
                        })
                        .when(self.mask_toggle, |this| {
                            this.child(Self::render_toggle_mask_button(&self.state, cx))
                        })
                        .when(show_clear_button, |this| {
                            this.child(super::clear_button(cx).on_click({
                                let state = self.state.clone();
                                move |_, window, cx| {
                                    state.update(cx, |state, cx| {
                                        state.clean(window, cx);
                                        state.focus(window, cx);
                                    })
                                }
                            }))
                        })
                        .children(suffix),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_types_map_to_accessibility_roles() {
        let cases = [
            (None, Role::TextInput),
            (Some(InputContentType::Name), Role::TextInput),
            (Some(InputContentType::NamePrefix), Role::TextInput),
            (Some(InputContentType::GivenName), Role::TextInput),
            (Some(InputContentType::MiddleName), Role::TextInput),
            (Some(InputContentType::FamilyName), Role::TextInput),
            (Some(InputContentType::NameSuffix), Role::TextInput),
            (Some(InputContentType::Nickname), Role::TextInput),
            (Some(InputContentType::JobTitle), Role::TextInput),
            (Some(InputContentType::OrganizationName), Role::TextInput),
            (Some(InputContentType::Location), Role::TextInput),
            (Some(InputContentType::FullStreetAddress), Role::TextInput),
            (Some(InputContentType::StreetAddressLine1), Role::TextInput),
            (Some(InputContentType::StreetAddressLine2), Role::TextInput),
            (Some(InputContentType::AddressCity), Role::TextInput),
            (Some(InputContentType::AddressState), Role::TextInput),
            (Some(InputContentType::AddressCityAndState), Role::TextInput),
            (Some(InputContentType::Sublocality), Role::TextInput),
            (Some(InputContentType::CountryName), Role::TextInput),
            (Some(InputContentType::PostalCode), Role::TextInput),
            (
                Some(InputContentType::TelephoneNumber),
                Role::PhoneNumberInput,
            ),
            (Some(InputContentType::EmailAddress), Role::EmailInput),
            (Some(InputContentType::Url), Role::UrlInput),
            (Some(InputContentType::CreditCardNumber), Role::TextInput),
            (Some(InputContentType::CreditCardName), Role::TextInput),
            (Some(InputContentType::CreditCardGivenName), Role::TextInput),
            (
                Some(InputContentType::CreditCardMiddleName),
                Role::TextInput,
            ),
            (
                Some(InputContentType::CreditCardFamilyName),
                Role::TextInput,
            ),
            (
                Some(InputContentType::CreditCardSecurityCode),
                Role::TextInput,
            ),
            (
                Some(InputContentType::CreditCardExpiration),
                Role::TextInput,
            ),
            (
                Some(InputContentType::CreditCardExpirationMonth),
                Role::TextInput,
            ),
            (
                Some(InputContentType::CreditCardExpirationYear),
                Role::TextInput,
            ),
            (Some(InputContentType::CreditCardType), Role::TextInput),
            (Some(InputContentType::Username), Role::TextInput),
            (Some(InputContentType::Password), Role::PasswordInput),
            (Some(InputContentType::NewPassword), Role::PasswordInput),
            (Some(InputContentType::OneTimeCode), Role::TextInput),
            (
                Some(InputContentType::ShipmentTrackingNumber),
                Role::TextInput,
            ),
            (Some(InputContentType::FlightNumber), Role::TextInput),
            (Some(InputContentType::DateTime), Role::DateTimeInput),
            (Some(InputContentType::Birthdate), Role::DateInput),
            (Some(InputContentType::BirthdateDay), Role::TextInput),
            (Some(InputContentType::BirthdateMonth), Role::TextInput),
            (Some(InputContentType::BirthdateYear), Role::TextInput),
            (Some(InputContentType::CellularEid), Role::TextInput),
            (Some(InputContentType::CellularImei), Role::TextInput),
        ];

        for (content_type, role) in cases {
            assert_eq!(Input::accessibility_role(false, content_type, None), role);
        }
    }

    #[test]
    fn multiline_inputs_keep_multiline_accessibility_role() {
        assert_eq!(
            Input::accessibility_role(true, Some(InputContentType::Password), None),
            Role::MultilineTextInput
        );
    }

    #[test]
    fn explicit_accessibility_role_overrides_defaults() {
        assert_eq!(
            Input::accessibility_role(
                false,
                Some(InputContentType::Password),
                Some(Role::TextInput)
            ),
            Role::TextInput
        );
        assert_eq!(
            Input::accessibility_role(
                true,
                Some(InputContentType::Password),
                Some(Role::TextInput)
            ),
            Role::TextInput
        );
    }

    #[rgpui::test]
    fn editable_input_offers_accessibility_write_action(cx: &mut rgpui::TestAppContext) {
        use crate::ElementExt as _;
        use rgpui::{AppContext as _, Element as _, IntoElement as _, RenderOnce};
        use std::sync::{Arc, Mutex};

        type EmittedState = Option<(Option<String>, bool)>;

        struct InputA11yProbe {
            state: Entity<InputState>,
            emitted: Arc<Mutex<EmittedState>>,
        }

        impl crate::Render for InputA11yProbe {
            fn render(
                &mut self,
                _window: &mut Window,
                _cx: &mut crate::Context<Self>,
            ) -> impl IntoElement {
                let state = self.state.clone();
                let emitted = self.emitted.clone();
                div().on_prepaint(move |_, window, cx| {
                    let input = Input::new(&state).render(window, cx).into_element();
                    let mut node = accesskit::Node::new(Role::TextInput);
                    input.write_a11y_info(&mut node);
                    *emitted.lock().unwrap() = Some((
                        node.value().map(ToOwned::to_owned),
                        node.supports_action(AccessibleAction::SetValue),
                    ));
                })
            }
        }

        cx.update(super::super::init);
        cx.update(crate::theme::init);
        let emitted = Arc::new(Mutex::new(None));
        let captured = emitted.clone();
        let (probe, cx) = cx.add_window_view(move |window, cx| InputA11yProbe {
            state: cx.new(|cx| InputState::new(window, cx).default_value("initial")),
            emitted,
        });
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        // 测试中没有附加辅助技术，因此 value 保持未物化，
        // 而 `SetValue` 仍然被声明。
        assert_eq!(*captured.lock().unwrap(), Some((None, true)));

        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|window, cx| {
            Input::handle_accessibility_set_value(&state, None, window, cx);
        });
        assert_eq!(state.read_with(cx, |state, _| state.value()), "initial");

        let action = accesskit::ActionData::Value("updated".into());
        cx.update(|window, cx| {
            Input::handle_accessibility_set_value(&state, Some(&action), window, cx);
        });
        assert_eq!(state.read_with(cx, |state, _| state.value()), "updated");
    }

    #[test]
    fn accessibility_value_is_hidden_for_secret_inputs() {
        assert!(Input::exposes_accessibility_value(false, None));
        assert!(!Input::exposes_accessibility_value(true, None));
        assert!(!Input::exposes_accessibility_value(
            false,
            Some(InputContentType::Password)
        ));
        assert!(!Input::exposes_accessibility_value(
            false,
            Some(InputContentType::NewPassword)
        ));
    }
}
