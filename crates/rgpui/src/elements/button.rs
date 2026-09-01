//! 按钮组件，支持多种样式、大小、图标和交互状态的可点击按钮。

use std::rc::Rc;

use crate::{
    ActiveTheme, AnyElement, App, ClickEvent, Colorize as _, Corners, Div, Edges, ElementId,
    ElementSize, Hsla, InteractiveElement, Interactivity, IntoElement, MouseButton, ParentElement,
    Pixels, RenderOnce, Role, SharedString, Stateful, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px, relative,
    transparent_white,
};
use crate::{
    ButtonIcon, Caret, Disableable, FocusableExt as _, ManagedTooltipExt as _, Selectable, Sizable,
    StyleSized, StyledExt, Tooltip,
};

/// 按钮圆角设置。
#[derive(Default, Clone, Copy)]
pub enum ButtonRounded {
    /// 无圆角
    None,
    /// 小圆角
    Small,
    #[default]
    /// 中等圆角
    Medium,
    /// 大圆角
    Large,
    /// 自定义像素圆角
    Size(Pixels),
}

impl From<Pixels> for ButtonRounded {
    fn from(px: Pixels) -> Self {
        ButtonRounded::Size(px)
    }
}

/// 自定义按钮变体样式。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ButtonCustomVariant {
    color: Hsla,
    foreground: Hsla,
    shadow: bool,
    hover: Hsla,
    active: Hsla,
}

/// 按钮变体 trait，提供设置各种变体样式的方法。
pub trait ButtonVariants: Sized {
    /// 设置按钮变体。
    fn with_variant(self, variant: ButtonVariant) -> Self;

    /// 使用主按钮样式。
    fn primary(self) -> Self {
        self.with_variant(ButtonVariant::Primary)
    }

    /// 使用次要按钮样式。
    fn secondary(self) -> Self {
        self.with_variant(ButtonVariant::Secondary)
    }

    /// 使用危险按钮样式。
    fn danger(self) -> Self {
        self.with_variant(ButtonVariant::Danger)
    }

    /// 使用警告按钮样式。
    fn warning(self) -> Self {
        self.with_variant(ButtonVariant::Warning)
    }

    /// 使用成功按钮样式。
    fn success(self) -> Self {
        self.with_variant(ButtonVariant::Success)
    }

    /// 使用信息按钮样式。
    fn info(self) -> Self {
        self.with_variant(ButtonVariant::Info)
    }

    /// 使用幽灵按钮样式。
    fn ghost(self) -> Self {
        self.with_variant(ButtonVariant::Ghost)
    }

    /// 使用链接按钮样式。
    fn link(self) -> Self {
        self.with_variant(ButtonVariant::Link)
    }

    /// 使用文本按钮样式，无内边距，看起来像普通文本。
    fn text(self) -> Self {
        self.with_variant(ButtonVariant::Text)
    }

    /// 使用自定义样式。
    fn custom(self, style: ButtonCustomVariant) -> Self {
        self.with_variant(ButtonVariant::Custom(style))
    }
}

impl ButtonCustomVariant {
    /// 使用当前主题创建自定义变体。
    pub fn new(cx: &App) -> Self {
        Self {
            color: cx.theme().transparent,
            foreground: cx.theme().foreground,
            hover: cx.theme().transparent,
            active: cx.theme().transparent,
            shadow: false,
        }
    }

    /// 设置背景色，默认为透明。
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// 设置前景色，默认为主题前景色。
    pub fn foreground(mut self, color: Hsla) -> Self {
        self.foreground = color;
        self
    }

    /// 设置悬停背景色，默认为透明。
    pub fn hover(mut self, color: Hsla) -> Self {
        self.hover = color;
        self
    }

    /// 设置激活背景色，默认为透明。
    pub fn active(mut self, color: Hsla) -> Self {
        self.active = color;
        self
    }

    /// 设置阴影，默认为 false。
    pub fn shadow(mut self, shadow: bool) -> Self {
        self.shadow = shadow;
        self
    }
}

/// 按钮变体。
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ButtonVariant {
    /// 默认样式
    #[default]
    Default,
    /// 主按钮样式
    Primary,
    /// 次要按钮样式
    Secondary,
    /// 危险按钮样式
    Danger,
    /// 信息按钮样式
    Info,
    /// 成功按钮样式
    Success,
    /// 警告按钮样式
    Warning,
    /// 幽灵按钮样式
    Ghost,
    /// 链接按钮样式
    Link,
    /// 文本按钮样式
    Text,
    /// 自定义样式
    Custom(ButtonCustomVariant),
}

impl ButtonVariant {
    /// 是否为链接类型。
    #[inline]
    pub fn is_link(&self) -> bool {
        matches!(self, Self::Link)
    }

    /// 是否为文本类型。
    #[inline]
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text)
    }

    /// 是否为幽灵类型。
    #[inline]
    pub fn is_ghost(&self) -> bool {
        matches!(self, Self::Ghost)
    }

    /// 是否无内边距。
    #[inline]
    fn no_padding(&self) -> bool {
        self.is_link() || self.is_text()
    }

    /// 是否为默认类型。
    #[inline]
    fn is_default(&self) -> bool {
        matches!(self, Self::Default)
    }
}

/// 按钮元素。
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    base: Stateful<Div>,
    style: StyleRefinement,
    icon: Option<ButtonIcon>,
    label: Option<SharedString>,
    children: Vec<AnyElement>,
    disabled: bool,
    pub(crate) selected: bool,
    toggled: Option<bool>,
    variant: ButtonVariant,
    rounded: ButtonRounded,
    outline: bool,
    border_corners: Corners<bool>,
    border_edges: Edges<bool>,
    dropdown_caret: bool,
    size: ElementSize,
    compact: bool,
    tooltip: Option<(
        SharedString,
        Option<(Rc<Box<dyn crate::Action>>, Option<SharedString>)>,
    )>,
    tooltip_builder: Option<Rc<dyn Fn(&mut Window, &mut App) -> crate::AnyView>>,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    on_hover: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,
    loading: bool,
    loading_icon: Option<crate::Icon>,

    tab_index: isize,
    tab_stop: bool,
}

impl From<Button> for AnyElement {
    fn from(button: Button) -> Self {
        button.into_any_element()
    }
}

impl Button {
    /// 使用给定 ID 创建按钮。
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();

        Self {
            id: id.clone(),
            // ID 必须在 div 创建后设置；
            // `dropdown_menu` 使用此 id 创建弹出菜单。
            base: div().flex_shrink_0().id(id),
            style: StyleRefinement::default(),
            icon: None,
            label: None,
            disabled: false,
            selected: false,
            toggled: None,
            variant: ButtonVariant::default(),
            rounded: ButtonRounded::Medium,
            border_corners: Corners {
                top_left: true,
                top_right: true,
                bottom_right: true,
                bottom_left: true,
            },
            border_edges: Edges::all(true),
            size: ElementSize::Medium,
            tooltip: None,
            tooltip_builder: None,
            on_click: None,
            on_hover: None,
            loading: false,
            compact: false,
            outline: false,
            children: Vec::new(),
            loading_icon: None,
            dropdown_caret: false,
            tab_index: 0,
            tab_stop: true,
        }
    }

    /// 设置按钮的描边样式。
    pub fn outline(mut self) -> Self {
        self.outline = true;
        self
    }

    /// 设置按钮的边框圆角。
    pub fn rounded(mut self, rounded: impl Into<ButtonRounded>) -> Self {
        self.rounded = rounded.into();
        self
    }

    /// 设置按钮各角是否启用圆角。
    ///
    /// 默认四角全部启用。可用来让按钮贴合外部容器边框（如数字输入框
    /// 内部的加减按钮只保留外侧圆角）。
    pub fn border_corners(mut self, corners: Corners<bool>) -> Self {
        self.border_corners = corners;
        self
    }

    /// 设置按钮标签，如果未设置标签，则按钮为图标按钮模式。
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 设置按钮图标，如果按钮没有标签，则按钮为图标按钮模式。
    pub fn icon(mut self, icon: impl Into<ButtonIcon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 设置按钮的 tooltip。
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some((tooltip.into(), None));
        self
    }

    /// 设置按钮的 tooltip，并附带 action 以显示快捷键。
    pub fn tooltip_with_action(
        mut self,
        tooltip: impl Into<SharedString>,
        action: &dyn crate::Action,
        context: Option<&str>,
    ) -> Self {
        self.tooltip = Some((
            tooltip.into(),
            Some((
                Rc::new(action.boxed_clone()),
                context.map(|c| c.to_string().into()),
            )),
        ));
        self
    }

    /// 设置为 true 以显示加载指示器。
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// 设置按钮为紧凑模式，内边距将减小。
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// 添加点击处理函数。
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// 添加悬停处理函数，bool 参数表示鼠标是否正在悬停。
    pub fn on_hover(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(handler));
        self
    }

    /// 设置加载图标，当 loading 为 true 时使用。
    ///
    /// 默认是 spinner 图标。
    pub fn loading_icon(mut self, icon: impl Into<crate::Icon>) -> Self {
        self.loading_icon = Some(icon.into());
        self
    }

    /// 设置按钮的 tab index，用于通过 Tab 键聚焦按钮。
    ///
    /// 默认为 0。
    pub fn tab_index(mut self, tab_index: isize) -> Self {
        self.tab_index = tab_index;
        self
    }

    /// 设置按钮的 tab stop，如果为 true，按钮可通过 Tab 键聚焦。
    ///
    /// 默认为 true。
    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        self
    }

    /// 设置为在按钮末尾显示下拉箭头图标。
    pub fn dropdown_caret(mut self, dropdown_caret: bool) -> Self {
        self.dropdown_caret = dropdown_caret;
        self
    }

    /// 将此按钮作为辅助技术中的切换按钮，`toggled` 为其按下状态。
    ///
    /// 仅影响无障碍元数据。使用 [`Selectable::selected`] 用于选中样式，
    /// 当按钮确实是切换按钮时再调用此方法，否则按钮保持普通按钮。
    pub fn toggled(mut self, toggled: bool) -> Self {
        self.toggled = Some(toggled);
        self
    }

    #[inline]
    fn clickable(&self) -> bool {
        !(self.disabled || self.loading) && self.on_click.is_some()
    }

    #[inline]
    fn hoverable(&self) -> bool {
        !(self.disabled || self.loading) && self.on_hover.is_some()
    }
}

impl Disableable for Button {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Selectable for Button {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl Sizable for Button {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl ButtonVariants for Button {
    fn with_variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for Button {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements)
    }
}

impl InteractiveElement for Button {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let style: ButtonVariant = self.variant;
        let clickable = self.clickable();
        let is_disabled = self.disabled;
        let hoverable = self.hoverable();
        let normal_style = style.normal(self.outline, cx);
        let icon_size = match self.size {
            ElementSize::ElementSize(v) => ElementSize::ElementSize(v * 0.75),
            _ => self.size,
        };
        let has_content = self.icon.is_some() || self.label.is_some() || !self.children.is_empty();

        let focus_handle = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let is_focused = focus_handle.is_focused(window);

        let rounding = match self.rounded {
            ButtonRounded::Small => cx.theme().radius * 0.5,
            ButtonRounded::Medium => cx.theme().radius,
            ButtonRounded::Large => cx.theme().radius * 2.0,
            ButtonRounded::Size(px) => px,
            ButtonRounded::None => Pixels::ZERO,
        };

        self.base
            .role(if self.variant.is_link() {
                Role::Link
            } else {
                Role::Button
            })
            .when_some(self.label.as_ref(), |this, label| {
                this.aria_label(label.clone())
            })
            .when_some(self.toggled, |this, toggled| {
                this.aria_toggled(if toggled {
                    crate::accesskit::Toggled::True
                } else {
                    crate::accesskit::Toggled::False
                })
            })
            .when(!self.disabled, |this| {
                this.track_focus(
                    &focus_handle
                        .tab_index(self.tab_index)
                        .tab_stop(self.tab_stop),
                )
            })
            .cursor_default()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .cursor_default()
            .when(
                !self.disabled && (self.variant.is_link() || self.variant.is_text()),
                |this| this.cursor_pointer(),
            )
            .when(cx.theme().shadow && normal_style.shadow, |this| {
                this.shadow_xs()
            })
            .when(!style.no_padding(), |this| {
                if self.label.is_none() && self.children.is_empty() {
                    // 图标按钮
                    match self.size {
                        ElementSize::ElementSize(px) => this.size(px),
                        ElementSize::XSmall => this.size_5(),
                        ElementSize::Small => this.size_6(),
                        ElementSize::Large | ElementSize::Medium => this.size_8(),
                    }
                } else {
                    // 普通按钮
                    match self.size {
                        ElementSize::ElementSize(size) => this.px(size * 0.2),
                        ElementSize::XSmall => {
                            this.h_5().px_1().when(self.compact, |this| this.min_w_5())
                        }
                        ElementSize::Small => this
                            .h_6()
                            .px_2()
                            .when(self.compact, |this| this.min_w_6().px_1p5()),
                        ElementSize::Medium => this
                            .h_8()
                            .px_2p5()
                            .when(self.compact, |this| this.min_w_8().px_2()),
                        ElementSize::Large => this
                            .h_8()
                            .px_3()
                            .when(self.compact, |this| this.min_w_8().px_2()),
                    }
                }
            })
            .when(self.border_corners.top_left, |this| {
                this.rounded_tl(rounding)
            })
            .when(self.border_corners.top_right, |this| {
                this.rounded_tr(rounding)
            })
            .when(self.border_corners.bottom_left, |this| {
                this.rounded_bl(rounding)
            })
            .when(self.border_corners.bottom_right, |this| {
                this.rounded_br(rounding)
            })
            .when(self.variant.is_default() || self.outline, |this| {
                this.when(self.border_edges.left, |this| this.border_l_1())
                    .when(self.border_edges.right, |this| this.border_r_1())
                    .when(self.border_edges.top, |this| this.border_t_1())
                    .when(self.border_edges.bottom, |this| this.border_b_1())
            })
            .text_color(normal_style.fg)
            .when(self.selected, |this| {
                let selected_style = style.selected(self.outline, cx);
                this.bg(selected_style.bg)
                    .border_color(selected_style.border)
                    .text_color(selected_style.fg)
            })
            .when(!self.disabled && !self.selected, |this| {
                this.border_color(normal_style.border)
                    .bg(normal_style.bg)
                    .when(normal_style.underline, |this| this.text_decoration_1())
                    .hover(|this| {
                        let hover_style = style.hovered(self.outline, cx);
                        this.bg(hover_style.bg)
                            .border_color(hover_style.border)
                            .text_color(hover_style.fg)
                    })
                    .active(|this| {
                        let active_style = style.active(self.outline, cx);
                        this.bg(active_style.bg)
                            .border_color(active_style.border)
                            .text_color(active_style.fg)
                    })
            })
            .when(self.disabled, |this| {
                let disabled_style = style.disabled(self.outline, cx);
                this.bg(disabled_style.bg)
                    .text_color(disabled_style.fg)
                    .border_color(disabled_style.border)
                    .shadow_none()
            })
            .refine_style(&self.style)
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                // 禁用时停止处理任何点击事件。
                // 避免按钮禁用时打开下拉菜单。
                if is_disabled {
                    cx.stop_propagation();
                    return;
                }

                // 避免在鼠标按下时获得焦点。
                window.prevent_default();
            })
            .when_some(self.on_click, |this, on_click| {
                this.on_click(move |event, window, cx| {
                    // 禁用时停止处理任何点击事件。
                    // 避免按钮禁用时打开下拉菜单。
                    if !clickable {
                        cx.stop_propagation();
                        return;
                    }

                    on_click(event, window, cx);
                })
            })
            .when_some(self.on_hover.filter(|_| hoverable), |this, on_hover| {
                this.on_hover(move |hovered, window, cx| {
                    on_hover(hovered, window, cx);
                })
            })
            .child({
                crate::h_flex()
                    .id("label")
                    .size_full()
                    .items_center()
                    .justify_center()
                    .button_text_size(self.size)
                    .map(|this| match self.size {
                        ElementSize::XSmall => this.gap_1(),
                        ElementSize::Small => this.gap_1(),
                        _ => this.gap_2(),
                    })
                    .when_some(self.icon, |this, icon| {
                        this.child(
                            icon.loading_icon(self.loading_icon)
                                .loading(self.loading)
                                .with_size(icon_size),
                        )
                    })
                    .when_some(self.label, |this, label| {
                        this.child(div().flex_none().line_height(relative(1.)).child(label))
                    })
                    .children(self.children)
                    .when(self.dropdown_caret, |this| {
                        this.when(has_content, |this| this.justify_between())
                            .child(Caret::new(self.size).text_color(normal_style.fg.opacity(0.75)))
                    })
            })
            .when(self.loading && !self.disabled, |this| {
                this.bg(normal_style.bg.opacity(0.8))
                    .border_color(normal_style.border.opacity(0.8))
                    .text_color(normal_style.fg.opacity(0.8))
            })
            .map(|this| {
                if let Some(builder) = self.tooltip_builder {
                    this.managed_tooltip(move |window, cx| builder(window, cx))
                } else if let Some((tooltip, action)) = self.tooltip {
                    this.managed_tooltip(move |window, cx| {
                        Tooltip::new(tooltip.clone())
                            .when_some(action.clone(), |this, (action, context)| {
                                this.action(
                                    action.boxed_clone().as_ref(),
                                    context.as_ref().map(|c| c.as_ref()),
                                )
                            })
                            .build(window, cx)
                    })
                } else {
                    this
                }
            })
            .focus_ring(is_focused, px(0.), window, cx)
    }
}

struct ButtonVariantStyle {
    bg: crate::Background,
    border: Hsla,
    fg: Hsla,
    underline: bool,
    shadow: bool,
}

#[derive(Clone, Copy)]
enum ButtonStyleState {
    Normal,
    Hovered,
    Active,
}

impl ButtonVariant {
    fn outline_background(&self, state: ButtonStyleState, cx: &mut App) -> crate::Background {
        match (self, state) {
            (Self::Default, ButtonStyleState::Normal) => cx.theme().input_background().into(),
            (Self::Default, ButtonStyleState::Hovered) => cx
                .theme()
                .input
                .mix_oklab(cx.theme().transparent, 0.5)
                .into(),
            (Self::Default, ButtonStyleState::Active) => cx
                .theme()
                .input
                .mix_oklab(cx.theme().transparent, 0.7)
                .into(),
            (Self::Primary, ButtonStyleState::Normal) => {
                cx.theme().tokens.primary.background.opacity(0.1)
            }
            (Self::Primary, ButtonStyleState::Hovered) => {
                cx.theme().tokens.primary_hover.background.opacity(0.2)
            }
            (Self::Primary, ButtonStyleState::Active) => {
                cx.theme().tokens.primary_active.background.opacity(0.4)
            }
            (Self::Secondary, ButtonStyleState::Normal) => {
                cx.theme().tokens.secondary.background.opacity(0.1)
            }
            (Self::Secondary, ButtonStyleState::Hovered) => {
                cx.theme().tokens.secondary_hover.background.opacity(0.2)
            }
            (Self::Secondary, ButtonStyleState::Active) => {
                cx.theme().tokens.secondary_active.background.opacity(0.4)
            }
            (Self::Danger, ButtonStyleState::Normal) => {
                cx.theme().tokens.danger.background.opacity(0.1)
            }
            (Self::Danger, ButtonStyleState::Hovered) => {
                cx.theme().tokens.danger_hover.background.opacity(0.2)
            }
            (Self::Danger, ButtonStyleState::Active) => {
                cx.theme().tokens.danger_active.background.opacity(0.4)
            }
            (Self::Warning, ButtonStyleState::Normal) => {
                cx.theme().tokens.warning.background.opacity(0.1)
            }
            (Self::Warning, ButtonStyleState::Hovered) => {
                cx.theme().tokens.warning_hover.background.opacity(0.2)
            }
            (Self::Warning, ButtonStyleState::Active) => {
                cx.theme().tokens.warning_active.background.opacity(0.4)
            }
            (Self::Success, ButtonStyleState::Normal) => {
                cx.theme().tokens.success.background.opacity(0.1)
            }
            (Self::Success, ButtonStyleState::Hovered) => {
                cx.theme().tokens.success_hover.background.opacity(0.2)
            }
            (Self::Success, ButtonStyleState::Active) => {
                cx.theme().tokens.success_active.background.opacity(0.4)
            }
            (Self::Info, ButtonStyleState::Normal) => {
                cx.theme().tokens.info.background.opacity(0.1)
            }
            (Self::Info, ButtonStyleState::Hovered) => {
                cx.theme().tokens.info_hover.background.opacity(0.2)
            }
            (Self::Info, ButtonStyleState::Active) => {
                cx.theme().tokens.info_active.background.opacity(0.4)
            }
            (Self::Ghost | Self::Link | Self::Text, _) => cx.theme().transparent.into(),
            (Self::Custom(colors), _) => colors.color.mix_oklab(cx.theme().transparent, 0.2).into(),
        }
    }

    fn bg_color(&self, outline: bool, cx: &mut App) -> crate::Background {
        if outline {
            return self.outline_background(ButtonStyleState::Normal, cx);
        }

        match self {
            Self::Default => cx.theme().tokens.button.into(),
            Self::Primary => cx.theme().tokens.button_primary.into(),
            Self::Secondary => cx.theme().tokens.button_secondary.into(),
            Self::Danger => cx.theme().tokens.button_danger.into(),
            Self::Warning => cx.theme().tokens.button_warning.into(),
            Self::Success => cx.theme().tokens.button_success.into(),
            Self::Info => cx.theme().tokens.button_info.into(),
            Self::Ghost | Self::Link | Self::Text => cx.theme().transparent.into(),
            Self::Custom(colors) => colors.color.mix_oklab(cx.theme().transparent, 0.2).into(),
        }
    }

    fn text_color(&self, outline: bool, cx: &mut App) -> Hsla {
        match self {
            Self::Default => cx.theme().button_foreground,
            Self::Primary => {
                if outline {
                    cx.theme().primary
                } else {
                    cx.theme().button_primary_foreground
                }
            }
            Self::Secondary => {
                if outline {
                    cx.theme().secondary_foreground
                } else {
                    cx.theme().button_secondary_foreground
                }
            }
            Self::Ghost => cx.theme().secondary_foreground,
            Self::Danger => {
                if outline {
                    cx.theme().danger
                } else {
                    cx.theme().button_danger_foreground
                }
            }
            Self::Warning => {
                if outline {
                    cx.theme().warning
                } else {
                    cx.theme().button_warning_foreground
                }
            }
            Self::Success => {
                if outline {
                    cx.theme().success
                } else {
                    cx.theme().button_success_foreground
                }
            }
            Self::Info => {
                if outline {
                    cx.theme().info
                } else {
                    cx.theme().button_info_foreground
                }
            }
            Self::Link => cx.theme().link,
            Self::Text => cx.theme().foreground.opacity(0.9),
            Self::Custom(colors) => colors.color,
        }
    }

    fn border_color(&self, outline: bool, cx: &mut App) -> Hsla {
        match self {
            Self::Default => cx.theme().input,
            Self::Secondary => cx.theme().border,
            Self::Primary => cx.theme().primary,
            Self::Danger => {
                if outline {
                    cx.theme().danger.mix_oklab(transparent_white(), 0.4)
                } else {
                    cx.theme().button_danger
                }
            }
            Self::Info => {
                if outline {
                    cx.theme().info.mix_oklab(transparent_white(), 0.4)
                } else {
                    cx.theme().button_info
                }
            }
            Self::Warning => {
                if outline {
                    cx.theme().warning.mix_oklab(transparent_white(), 0.4)
                } else {
                    cx.theme().button_warning
                }
            }
            Self::Success => {
                if outline {
                    cx.theme().success.mix_oklab(transparent_white(), 0.4)
                } else {
                    cx.theme().button_success
                }
            }
            Self::Ghost | Self::Link | Self::Text => cx.theme().transparent,
            Self::Custom(colors) => {
                if outline {
                    colors.color.mix_oklab(transparent_white(), 0.4)
                } else {
                    colors.color
                }
            }
        }
    }

    fn underline(&self, _: &App) -> bool {
        match self {
            Self::Link => true,
            _ => false,
        }
    }

    fn shadow(&self, _outline: bool, _: &App) -> bool {
        match self {
            Self::Custom(c) => c.shadow,
            _ => false,
        }
    }

    fn normal(&self, outline: bool, cx: &mut App) -> ButtonVariantStyle {
        let bg = self.bg_color(outline, cx);
        let border = self.border_color(outline, cx);
        let fg = self.text_color(outline, cx);
        let underline = self.underline(cx);
        let shadow = self.shadow(outline, cx);

        ButtonVariantStyle {
            bg,
            border,
            fg,
            underline,
            shadow,
        }
    }

    fn hovered(&self, outline: bool, cx: &mut App) -> ButtonVariantStyle {
        let bg: crate::Background = match self {
            Self::Default => {
                if outline {
                    self.outline_background(ButtonStyleState::Hovered, cx)
                } else {
                    cx.theme().tokens.button_hover.into()
                }
            }
            Self::Primary => {
                if outline {
                    self.outline_background(ButtonStyleState::Hovered, cx)
                } else {
                    cx.theme().tokens.button_primary_hover.into()
                }
            }
            Self::Secondary => {
                if outline {
                    self.outline_background(ButtonStyleState::Hovered, cx)
                } else {
                    cx.theme().tokens.button_secondary_hover.into()
                }
            }
            Self::Danger => {
                if outline {
                    self.outline_background(ButtonStyleState::Hovered, cx)
                } else {
                    cx.theme().tokens.button_danger_hover.into()
                }
            }
            Self::Warning => {
                if outline {
                    self.outline_background(ButtonStyleState::Hovered, cx)
                } else {
                    cx.theme().tokens.button_warning_hover.into()
                }
            }
            Self::Success => {
                if outline {
                    self.outline_background(ButtonStyleState::Hovered, cx)
                } else {
                    cx.theme().tokens.button_success_hover.into()
                }
            }
            Self::Info => {
                if outline {
                    self.outline_background(ButtonStyleState::Hovered, cx)
                } else {
                    cx.theme().tokens.button_info_hover.into()
                }
            }
            Self::Custom(colors) => {
                if outline {
                    colors.color.mix_oklab(cx.theme().transparent, 0.2)
                } else {
                    colors.color.mix_oklab(cx.theme().transparent, 0.3)
                }
            }
            .into(),
            Self::Ghost => {
                if cx.theme().mode.is_dark() {
                    cx.theme().secondary.lighten(0.1).opacity(0.8)
                } else {
                    cx.theme().secondary.darken(0.1).opacity(0.8)
                }
            }
            .into(),
            Self::Link => cx.theme().transparent.into(),
            Self::Text => cx.theme().transparent.into(),
        };

        let border = self.border_color(outline, cx);
        let fg = match self {
            Self::Link => cx.theme().link_hover,
            Self::Text => cx.theme().foreground,
            _ => self.text_color(outline, cx),
        };

        let underline = self.underline(cx);
        let shadow = self.shadow(outline, cx);

        ButtonVariantStyle {
            bg,
            border,
            fg,
            underline,
            shadow,
        }
    }

    fn active(&self, outline: bool, cx: &mut App) -> ButtonVariantStyle {
        let bg = match self {
            Self::Default => {
                if outline {
                    self.outline_background(ButtonStyleState::Active, cx)
                } else {
                    cx.theme().tokens.button_active.into()
                }
            }
            Self::Primary => {
                if outline {
                    self.outline_background(ButtonStyleState::Active, cx)
                } else {
                    cx.theme().tokens.button_primary_active.into()
                }
            }
            Self::Secondary => {
                if outline {
                    self.outline_background(ButtonStyleState::Active, cx)
                } else {
                    cx.theme().tokens.button_secondary_active.into()
                }
            }
            Self::Ghost => {
                if cx.theme().mode.is_dark() {
                    cx.theme().secondary.lighten(0.2).opacity(0.8)
                } else {
                    cx.theme().secondary.darken(0.2).opacity(0.8)
                }
            }
            .into(),
            Self::Danger => {
                if outline {
                    self.outline_background(ButtonStyleState::Active, cx)
                } else {
                    cx.theme().tokens.button_danger_active.into()
                }
            }
            Self::Warning => {
                if outline {
                    self.outline_background(ButtonStyleState::Active, cx)
                } else {
                    cx.theme().tokens.button_warning_active.into()
                }
            }
            Self::Success => {
                if outline {
                    self.outline_background(ButtonStyleState::Active, cx)
                } else {
                    cx.theme().tokens.button_success_active.into()
                }
            }
            Self::Info => {
                if outline {
                    self.outline_background(ButtonStyleState::Active, cx)
                } else {
                    cx.theme().tokens.button_info_active.into()
                }
            }
            Self::Custom(colors) => colors.color.mix_oklab(cx.theme().transparent, 0.4).into(),
            Self::Link => cx.theme().transparent.into(),
            Self::Text => cx.theme().transparent.into(),
        };
        let border = self.border_color(outline, cx);
        let fg = match self {
            Self::Link => cx.theme().link_active,
            Self::Text => cx.theme().foreground.opacity(0.7),
            _ => self.text_color(outline, cx),
        };
        let underline = self.underline(cx);
        let shadow = self.shadow(outline, cx);

        ButtonVariantStyle {
            bg,
            border,
            fg,
            underline,
            shadow,
        }
    }

    fn selected(&self, outline: bool, cx: &mut App) -> ButtonVariantStyle {
        if outline {
            let active_style = self.active(outline, cx);

            return ButtonVariantStyle {
                fg: self.text_color(outline, cx),
                ..active_style
            };
        }

        let bg = match self {
            Self::Default => cx.theme().tokens.button_active.into(),
            Self::Primary => cx.theme().tokens.button_primary_active.into(),
            Self::Secondary => cx.theme().tokens.button_secondary_active.into(),
            Self::Ghost => cx.theme().tokens.secondary_active.into(),
            Self::Danger => cx.theme().tokens.button_danger_active.into(),
            Self::Warning => cx.theme().tokens.button_warning_active.into(),
            Self::Success => cx.theme().tokens.button_success_active.into(),
            Self::Info => cx.theme().tokens.button_info_active.into(),
            Self::Link => cx.theme().transparent.into(),
            Self::Text => cx.theme().transparent.into(),
            Self::Custom(colors) => colors.active.into(),
        };

        let border = self.border_color(outline, cx);
        let fg = match self {
            Self::Link => cx.theme().link_active,
            Self::Text => cx.theme().foreground.opacity(0.7),
            _ => self.text_color(false, cx),
        };
        let underline = self.underline(cx);
        let shadow = self.shadow(outline, cx);

        ButtonVariantStyle {
            bg,
            border,
            fg,
            underline,
            shadow,
        }
    }

    fn disabled(&self, outline: bool, cx: &mut App) -> ButtonVariantStyle {
        let bg = match self {
            Self::Default | Self::Link | Self::Ghost | Self::Text => cx.theme().transparent.into(),
            Self::Primary => cx.theme().tokens.button_primary.background.opacity(0.15),
            Self::Danger => cx.theme().tokens.button_danger.background.opacity(0.15),
            Self::Warning => cx.theme().tokens.button_warning.background.opacity(0.15),
            Self::Success => cx.theme().tokens.button_success.background.opacity(0.15),
            Self::Info => cx.theme().tokens.button_info.background.opacity(0.15),
            Self::Secondary => cx.theme().tokens.button_secondary.background.opacity(1.5),
            Self::Custom(style) => style.color.opacity(0.15).into(),
        };
        let fg = cx.theme().muted_foreground.opacity(0.5);
        let (bg, border) = if outline {
            (
                self.outline_background(ButtonStyleState::Normal, cx)
                    .opacity(0.5),
                self.border_color(true, cx).opacity(0.5),
            )
        } else if let Self::Default = self {
            (
                cx.theme().input_background().opacity(0.5).into(),
                cx.theme().input.opacity(0.5),
            )
        } else {
            let border = match self {
                Self::Primary => cx.theme().button_primary.opacity(0.15),
                Self::Secondary => cx.theme().button_secondary.opacity(1.5),
                Self::Danger => cx.theme().button_danger.opacity(0.15),
                Self::Warning => cx.theme().button_warning.opacity(0.15),
                Self::Success => cx.theme().button_success.opacity(0.15),
                Self::Info => cx.theme().button_info.opacity(0.15),
                Self::Custom(style) => style.color.opacity(0.15),
                Self::Default | Self::Link | Self::Ghost | Self::Text => cx.theme().transparent,
            };
            (bg, border)
        };

        let underline = self.underline(cx);
        let shadow = false;

        ButtonVariantStyle {
            bg,
            border,
            fg,
            underline,
            shadow,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ElementSize;

    /// 测试 Button 构造器
    #[test]
    fn test_button_builder() {
        let button = Button::new("complex-button")
            .label("Save Changes")
            .primary()
            .outline()
            .large()
            .tooltip("Click to save")
            .compact()
            .loading(false)
            .disabled(false)
            .selected(false)
            .tab_index(1)
            .tab_stop(true)
            .dropdown_caret(false)
            .rounded(ButtonRounded::Medium)
            .on_click(|_, _, _| {});

        assert_eq!(button.label, Some("Save Changes".into()));
        assert_eq!(button.variant, ButtonVariant::Primary);
        assert!(button.outline);
        assert_eq!(button.size, ElementSize::Large);
        assert!(button.tooltip.is_some());
        assert!(button.compact);
        assert!(!button.loading);
        assert!(!button.disabled);
        assert!(!button.selected);
        assert_eq!(button.toggled, None);
        assert_eq!(button.tab_index, 1);
        assert!(button.tab_stop);
        assert!(!button.dropdown_caret);
        assert!(matches!(button.rounded, ButtonRounded::Medium));
    }

    /// 测试按钮可点击逻辑
    #[test]
    fn test_button_clickable_logic() {
        // 带点击处理函数的按钮应可点击
        let clickable = Button::new("test").on_click(|_, _, _| {});
        assert!(clickable.clickable());

        // 禁用按钮不可点击
        let disabled = Button::new("test").disabled(true).on_click(|_, _, _| {});
        assert!(!disabled.clickable());

        // 加载中按钮不可点击
        let loading = Button::new("test").loading(true).on_click(|_, _, _| {});
        assert!(!loading.clickable());
    }

    /// 测试按钮变体方法
    #[test]
    fn test_button_variant_methods() {
        // 测试变体检查方法
        assert!(ButtonVariant::Link.is_link());
        assert!(ButtonVariant::Text.is_text());
        assert!(ButtonVariant::Ghost.is_ghost());

        // 测试 no_padding 逻辑
        assert!(ButtonVariant::Link.no_padding());
        assert!(ButtonVariant::Text.no_padding());
        assert!(!ButtonVariant::Ghost.no_padding());
    }
}
