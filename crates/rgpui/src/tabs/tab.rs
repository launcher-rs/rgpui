use std::{rc::Rc, time::Duration};

use crate::{
    Animation, AnimationExt as _, AnyElement, App, Background, ClickEvent, Div, Edges, ElementId,
    Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px, relative,
};

use crate::{
    ActiveTheme, ElementSize, Icon, IconName, Lerp, Selectable, Sizable, StyledExt,
    ease_in_out_cubic, h_flex, prelude::FluentBuilder as _,
};

/// Tab 的变体样式。
#[derive(Debug, Clone, Default, Copy, PartialEq, Eq, Hash)]
pub enum TabVariant {
    /// 默认的标签页样式。
    #[default]
    Tab,
    /// 描边样式，带圆角边框。
    Outline,
    /// 药丸样式，完全圆角。
    Pill,
    /// 分段样式，选中项带内嵌背景与阴影。
    Segmented,
    /// 下划线样式，选中项底部显示下划线。
    Underline,
}

impl TabVariant {
    /// 返回 Tab 的总高度。
    fn height(&self, size: ElementSize) -> Pixels {
        match size {
            ElementSize::XSmall => match self {
                TabVariant::Underline => px(26.),
                _ => px(20.),
            },
            ElementSize::Small => match self {
                TabVariant::Underline => px(30.),
                _ => px(24.),
            },
            ElementSize::Large => match self {
                TabVariant::Underline => px(44.),
                _ => px(36.),
            },
            _ => match self {
                TabVariant::Underline => px(36.),
                _ => px(32.),
            },
        }
    }

    /// 返回 Tab 内容区域的高度。
    pub(super) fn inner_height(&self, size: ElementSize) -> Pixels {
        match size {
            ElementSize::XSmall => match self {
                TabVariant::Tab | TabVariant::Outline | TabVariant::Pill => px(18.),
                TabVariant::Segmented => px(16.),
                TabVariant::Underline => px(20.),
            },
            ElementSize::Small => match self {
                TabVariant::Tab | TabVariant::Outline | TabVariant::Pill => px(22.),
                TabVariant::Segmented => px(18.),
                TabVariant::Underline => px(22.),
            },
            ElementSize::Large => match self {
                TabVariant::Tab | TabVariant::Outline | TabVariant::Pill => px(36.),
                TabVariant::Segmented => px(28.),
                TabVariant::Underline => px(32.),
            },
            _ => match self {
                TabVariant::Tab => px(30.),
                TabVariant::Outline | TabVariant::Pill => px(26.),
                TabVariant::Segmented => px(24.),
                TabVariant::Underline => px(26.),
            },
        }
    }

    /// 默认 px(12) 以匹配面板 px_3，参见 [`crate::tabs::TabBar`]。
    fn inner_paddings(&self, size: ElementSize) -> Edges<Pixels> {
        let mut padding_x = match size {
            ElementSize::XSmall => px(8.),
            ElementSize::Small => px(10.),
            ElementSize::Large => px(16.),
            _ => px(12.),
        };

        if matches!(self, TabVariant::Underline) {
            padding_x = px(0.);
        }

        Edges {
            left: padding_x,
            right: padding_x,
            ..Default::default()
        }
    }

    /// 返回 Tab 内容区域的外边距。
    fn inner_margins(&self, size: ElementSize) -> Edges<Pixels> {
        match size {
            ElementSize::XSmall => match self {
                TabVariant::Underline => Edges {
                    top: px(1.),
                    bottom: px(2.),
                    ..Default::default()
                },
                _ => Edges::all(px(0.)),
            },
            ElementSize::Small => match self {
                TabVariant::Underline => Edges {
                    top: px(2.),
                    bottom: px(3.),
                    ..Default::default()
                },
                _ => Edges::all(px(0.)),
            },
            ElementSize::Large => match self {
                TabVariant::Underline => Edges {
                    top: px(5.),
                    bottom: px(6.),
                    ..Default::default()
                },
                _ => Edges::all(px(0.)),
            },
            _ => match self {
                TabVariant::Underline => Edges {
                    top: px(3.),
                    bottom: px(4.),
                    ..Default::default()
                },
                _ => Edges::all(px(0.)),
            },
        }
    }

    /// 返回 Tab 正常状态下的样式。
    fn normal(&self, cx: &App) -> TabStyle {
        match self {
            TabVariant::Tab => TabStyle {
                fg: cx.theme().tab_foreground,
                bg: cx.theme().transparent.into(),
                borders: Edges {
                    left: px(1.),
                    right: px(1.),
                    ..Default::default()
                },
                border_color: cx.theme().transparent,
                ..Default::default()
            },
            TabVariant::Outline => TabStyle {
                fg: cx.theme().tab_foreground,
                bg: cx.theme().transparent.into(),
                borders: Edges::all(px(1.)),
                border_color: cx.theme().border,
                ..Default::default()
            },
            TabVariant::Pill => TabStyle {
                fg: cx.theme().tab_foreground,
                bg: cx.theme().transparent.into(),
                ..Default::default()
            },
            TabVariant::Segmented => TabStyle {
                fg: cx.theme().tab_foreground,
                bg: cx.theme().transparent.into(),
                ..Default::default()
            },
            TabVariant::Underline => TabStyle {
                fg: cx.theme().tab_foreground,
                bg: cx.theme().transparent.into(),
                inner_bg: cx.theme().transparent.into(),
                borders: Edges {
                    bottom: px(2.),
                    ..Default::default()
                },
                border_color: cx.theme().transparent,
                ..Default::default()
            },
        }
    }

    /// 返回 Tab 悬停状态下的样式。
    fn hovered(&self, selected: bool, cx: &App) -> TabStyle {
        match self {
            TabVariant::Tab => TabStyle {
                fg: cx.theme().tab_active_foreground,
                bg: cx.theme().transparent.into(),
                borders: Edges {
                    left: px(1.),
                    right: px(1.),
                    ..Default::default()
                },
                border_color: cx.theme().transparent,
                ..Default::default()
            },
            TabVariant::Outline => TabStyle {
                fg: cx.theme().secondary_foreground,
                bg: cx.theme().tokens.secondary_hover.into(),
                borders: Edges::all(px(1.)),
                border_color: cx.theme().border,
                ..Default::default()
            },
            TabVariant::Pill => TabStyle {
                fg: cx.theme().secondary_foreground,
                bg: cx.theme().tokens.secondary.into(),
                ..Default::default()
            },
            TabVariant::Segmented => TabStyle {
                fg: cx.theme().tab_active_foreground,
                bg: cx.theme().transparent.into(),
                inner_bg: if selected {
                    cx.theme().tokens.background.into()
                } else {
                    cx.theme().transparent.into()
                },
                ..Default::default()
            },
            TabVariant::Underline => TabStyle {
                fg: cx.theme().tab_active_foreground,
                bg: cx.theme().transparent.into(),
                inner_bg: cx.theme().transparent.into(),
                borders: Edges {
                    bottom: px(2.),
                    ..Default::default()
                },
                border_color: cx.theme().transparent,
                ..Default::default()
            },
        }
    }

    /// 返回 Tab 选中状态下的样式。
    fn selected(&self, cx: &App) -> TabStyle {
        match self {
            TabVariant::Tab => TabStyle {
                fg: cx.theme().tab_active_foreground,
                bg: cx.theme().tokens.tab_active.into(),
                borders: Edges {
                    left: px(1.),
                    right: px(1.),
                    ..Default::default()
                },
                border_color: cx.theme().border,
                ..Default::default()
            },
            TabVariant::Outline => TabStyle {
                fg: cx.theme().primary,
                bg: cx.theme().transparent.into(),
                borders: Edges::all(px(1.)),
                border_color: cx.theme().primary,
                ..Default::default()
            },
            TabVariant::Pill => TabStyle {
                fg: cx.theme().primary_foreground,
                bg: cx.theme().tokens.primary.into(),
                ..Default::default()
            },
            TabVariant::Segmented => TabStyle {
                fg: cx.theme().tab_active_foreground,
                bg: cx.theme().transparent.into(),
                inner_bg: cx.theme().tokens.background.into(),
                shadow: true,
                ..Default::default()
            },
            TabVariant::Underline => TabStyle {
                fg: cx.theme().tab_active_foreground,
                bg: cx.theme().transparent.into(),
                borders: Edges {
                    bottom: px(2.),
                    ..Default::default()
                },
                border_color: cx.theme().primary,
                ..Default::default()
            },
        }
    }

    /// 返回 Tab 禁用状态下的样式。
    fn disabled(&self, selected: bool, cx: &App) -> TabStyle {
        match self {
            TabVariant::Tab => TabStyle {
                fg: cx.theme().muted_foreground,
                bg: cx.theme().transparent.into(),
                border_color: if selected {
                    cx.theme().border
                } else {
                    cx.theme().transparent
                },
                borders: Edges {
                    left: px(1.),
                    right: px(1.),
                    ..Default::default()
                },
                ..Default::default()
            },
            TabVariant::Outline => TabStyle {
                fg: cx.theme().muted_foreground,
                bg: cx.theme().transparent.into(),
                borders: Edges::all(px(1.)),
                border_color: if selected {
                    cx.theme().primary
                } else {
                    cx.theme().border
                },
                ..Default::default()
            },
            TabVariant::Pill => TabStyle {
                fg: if selected {
                    cx.theme().primary_foreground.opacity(0.5)
                } else {
                    cx.theme().muted_foreground
                },
                bg: if selected {
                    cx.theme().primary.opacity(0.5).into()
                } else {
                    cx.theme().transparent.into()
                },
                ..Default::default()
            },
            TabVariant::Segmented => TabStyle {
                fg: cx.theme().muted_foreground,
                bg: cx.theme().tokens.tab_bar.into(),
                inner_bg: if selected {
                    cx.theme().tokens.background.into()
                } else {
                    cx.theme().transparent.into()
                },
                ..Default::default()
            },
            TabVariant::Underline => TabStyle {
                fg: cx.theme().muted_foreground,
                bg: cx.theme().transparent.into(),
                border_color: if selected {
                    cx.theme().border
                } else {
                    cx.theme().transparent
                },
                borders: Edges {
                    bottom: px(2.),
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    }

    /// 返回 TabBar 的整体圆角半径。
    pub(super) fn tab_bar_radius(&self, size: ElementSize, cx: &App) -> Pixels {
        if *self != TabVariant::Segmented {
            return px(0.);
        }

        match size {
            ElementSize::XSmall | ElementSize::Small => cx.theme().radius,
            ElementSize::Large => cx.theme().radius_lg,
            _ => cx.theme().radius_lg,
        }
    }

    /// 返回 Tab 自身的圆角半径。
    fn radius(&self, size: ElementSize, cx: &App) -> Pixels {
        match self {
            TabVariant::Outline | TabVariant::Pill => px(99.),
            TabVariant::Segmented => match size {
                ElementSize::XSmall | ElementSize::Small => cx.theme().radius,
                ElementSize::Large => cx.theme().radius_lg,
                _ => cx.theme().radius_lg,
            },
            _ => px(0.),
        }
    }

    /// 返回 Tab 内容区域的圆角半径。
    pub(super) fn inner_radius(&self, size: ElementSize, cx: &App) -> Pixels {
        match self {
            TabVariant::Segmented => match size {
                ElementSize::Large => self.tab_bar_radius(size, cx) - px(3.),
                _ => self.tab_bar_radius(size, cx) - px(2.),
            },
            _ => px(0.),
        }
    }
}

/// Tab 的样式信息集合。
struct TabStyle {
    borders: Edges<Pixels>,
    border_color: Hsla,
    bg: Background,
    fg: Hsla,
    shadow: bool,
    inner_bg: Background,
}

impl Default for TabStyle {
    fn default() -> Self {
        TabStyle {
            borders: Edges::all(px(0.)),
            border_color: crate::transparent_white(),
            bg: crate::transparent_white().into(),
            fg: crate::transparent_white(),
            shadow: false,
            inner_bg: crate::transparent_white().into(),
        }
    }
}

/// 供 [`crate::tabs::TabBar`] 使用的 Tab 元素。
#[derive(IntoElement)]
pub struct Tab {
    ix: usize,
    base: Div,
    pub(super) label: Option<SharedString>,
    pub(super) icon: Option<Icon>,
    prefix: Option<AnyElement>,
    pub(super) tab_bar_prefix: Option<bool>,
    suffix: Option<AnyElement>,
    children: Vec<AnyElement>,
    variant: TabVariant,
    size: ElementSize,
    pub(super) disabled: bool,
    pub(super) selected: bool,
    pub(super) indicator_active: bool,
    pub(super) indicator_ready: bool,
    /// [`crate::tabs::TabBar`] 指示器的动画纪元；每次切换 Tab 时递增。
    /// 用于为选中 Tab 的文字颜色渐变作键，使其与指示器滑动同步重启。
    pub(super) indicator_epoch: u64,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
}

impl From<&'static str> for Tab {
    fn from(label: &'static str) -> Self {
        Self::new().label(label)
    }
}

impl From<String> for Tab {
    fn from(label: String) -> Self {
        Self::new().label(label)
    }
}

impl From<SharedString> for Tab {
    fn from(label: SharedString) -> Self {
        Self::new().label(label)
    }
}

impl From<Icon> for Tab {
    fn from(icon: Icon) -> Self {
        Self::default().icon(icon)
    }
}

impl From<IconName> for Tab {
    fn from(icon_name: IconName) -> Self {
        Self::default().icon(Icon::new(icon_name))
    }
}

impl Default for Tab {
    fn default() -> Self {
        Self {
            ix: 0,
            base: div(),
            label: None,
            icon: None,
            tab_bar_prefix: None,
            children: Vec::new(),
            disabled: false,
            selected: false,
            indicator_active: false,
            indicator_ready: true,
            indicator_epoch: 0,
            prefix: None,
            suffix: None,
            variant: TabVariant::default(),
            size: ElementSize::default(),
            on_click: None,
        }
    }
}

impl Tab {
    /// 创建一个新的 Tab。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置 Tab 的标签文本。
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 设置 Tab 的图标。
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 设置 Tab 的变体样式。
    pub fn with_variant(mut self, variant: TabVariant) -> Self {
        self.variant = variant;
        self
    }

    /// 使用药丸变体样式。
    pub fn pill(mut self) -> Self {
        self.variant = TabVariant::Pill;
        self
    }

    /// 使用描边变体样式。
    pub fn outline(mut self) -> Self {
        self.variant = TabVariant::Outline;
        self
    }

    /// 使用分段变体样式。
    pub fn segmented(mut self) -> Self {
        self.variant = TabVariant::Segmented;
        self
    }

    /// 使用下划线变体样式。
    pub fn underline(mut self) -> Self {
        self.variant = TabVariant::Underline;
        self
    }

    /// 设置 Tab 左侧的前缀元素。
    pub fn prefix(mut self, prefix: impl IntoElement) -> Self {
        self.prefix = Some(prefix.into_any_element());
        self
    }

    /// 设置 Tab 右侧的后缀元素。
    pub fn suffix(mut self, suffix: impl IntoElement) -> Self {
        self.suffix = Some(suffix.into_any_element());
        self
    }

    /// 设置 Tab 的禁用状态，默认为 false。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置 Tab 的点击处理回调。
    pub fn on_click(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    /// 设置 Tab 的索引。
    pub(crate) fn ix(mut self, ix: usize) -> Self {
        self.ix = ix;
        self
    }

    /// 设置 TabBar 是否带有前缀元素。
    pub(crate) fn tab_bar_prefix(mut self, tab_bar_prefix: bool) -> Self {
        self.tab_bar_prefix = Some(tab_bar_prefix);
        self
    }
}

impl ParentElement for Tab {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Selectable for Tab {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl InteractiveElement for Tab {
    fn interactivity(&mut self) -> &mut crate::Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for Tab {}

impl Styled for Tab {
    fn style(&mut self) -> &mut crate::StyleRefinement {
        self.base.style()
    }
}

impl Sizable for Tab {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for Tab {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut tab_style = if self.selected {
            self.variant.selected(cx)
        } else {
            self.variant.normal(cx)
        };
        let mut hover_style = self.variant.hovered(self.selected, cx);
        if self.disabled {
            tab_style = self.variant.disabled(self.selected, cx);
            hover_style = self.variant.disabled(self.selected, cx);
        }
        let tab_bar_prefix = self.tab_bar_prefix.unwrap_or_default();
        if !tab_bar_prefix {
            if self.ix == 0 && self.variant == TabVariant::Tab {
                tab_style.borders.left = px(0.);
                hover_style.borders.left = px(0.);
            }
        }
        let radius = self.variant.radius(self.size, cx);
        let inner_radius = self.variant.inner_radius(self.size, cx);
        let inner_paddings = self.variant.inner_paddings(self.size);
        let inner_margins = self.variant.inner_margins(self.size);
        let inner_height = self.variant.inner_height(self.size);
        let height = self.variant.height(self.size);

        let segmented_indicator_active =
            self.variant == TabVariant::Segmented && self.indicator_active;
        let has_inline_inner_bg =
            self.selected && segmented_indicator_active && !self.indicator_ready;
        let inline_inner_bg = tab_style.inner_bg;
        let (inner_bg, hover_inner_bg) = if segmented_indicator_active && self.indicator_ready {
            (cx.theme().transparent.into(), cx.theme().transparent.into())
        } else if has_inline_inner_bg {
            (inline_inner_bg, inline_inner_bg)
        } else {
            (tab_style.inner_bg, hover_style.inner_bg)
        };
        let inner_shadow = tab_style.shadow && !segmented_indicator_active;

        // 当滑动指示器处于激活且就绪状态时，它单独代表选中状态。
        // 抑制选中 Tab 自身的激活背景/边框，使二者在切换动画期间不重叠
        // （Segmented 已对其 `inner_bg` 执行此操作）。跳过禁用的 Tab，
        // 使已禁用的选中 Tab 保持暗淡样式而不是指示器的全强度颜色。
        let suppress_active_visual =
            self.selected && !self.disabled && self.indicator_active && self.indicator_ready;
        // Pill 通过外层 `bg` 绘制其激活状态。
        let outer_bg = if suppress_active_visual && self.variant == TabVariant::Pill {
            cx.theme().transparent.into()
        } else {
            tab_style.bg
        };
        // Underline 通过底部 `border_color` 绘制其激活状态。
        let outer_border_color = if suppress_active_visual && self.variant == TabVariant::Underline
        {
            cx.theme().transparent
        } else {
            tab_style.border_color
        };

        // 对于 Pill，新选中 Tab 的文字颜色（`primary_foreground`）
        // 会在指示器仍滑动到位时瞬间变成白色。使其从正常颜色随指示器
        // 滑动同步淡入（以指示器纪元为键，这样每次切换都会重新开始）。
        // `epoch == 0` 是初始布局（无滑动），因此跳过它。
        let animate_fg = self.selected
            && !self.disabled
            && self.variant == TabVariant::Pill
            && self.indicator_active
            && self.indicator_ready
            && self.indicator_epoch > 0;
        let fg_from = self.variant.normal(cx).fg;
        let fg_to = tab_style.fg;

        let inner_content = h_flex()
            .flex_1()
            .h(inner_height)
            .line_height(relative(1.))
            .whitespace_nowrap()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .margins(inner_margins)
            .flex_shrink_0()
            .map(|this| match self.icon {
                Some(icon) => this
                    .w(inner_height * 1.25)
                    .child(icon.map(|this| match self.size {
                        ElementSize::XSmall => this.size_2p5(),
                        ElementSize::Small => this.size_3p5(),
                        ElementSize::Large => this.size_4(),
                        _ => this.size_4(),
                    })),
                None => this
                    .paddings(inner_paddings)
                    .map(|this| match self.label {
                        Some(label) => this.child(label),
                        None => this,
                    })
                    .children(self.children),
            })
            .bg(inner_bg)
            .rounded(inner_radius)
            .when(inner_shadow, |this| this.shadow_xs())
            .hover(|this| this.bg(hover_inner_bg).rounded(inner_radius));

        let inner_element = if animate_fg {
            inner_content
                .with_animation(
                    ElementId::NamedInteger("tab-fg".into(), self.indicator_epoch),
                    Animation::new(Duration::from_millis(200)).with_easing(ease_in_out_cubic),
                    move |this, delta| this.text_color(Lerp::lerp(&fg_from, &fg_to, delta)),
                )
                .into_any_element()
        } else {
            inner_content.into_any_element()
        };

        self.base
            .id(self.ix)
            .relative()
            .flex()
            .flex_wrap()
            .gap_1()
            .items_center()
            .flex_shrink_0()
            .h(height)
            .overflow_hidden()
            .text_color(tab_style.fg)
            .map(|this| match self.size {
                ElementSize::XSmall => this.text_xs(),
                ElementSize::Large => this.text_base(),
                _ => this.text_sm(),
            })
            .bg(outer_bg)
            .border_l(tab_style.borders.left)
            .border_r(tab_style.borders.right)
            .border_t(tab_style.borders.top)
            .border_b(tab_style.borders.bottom)
            .border_color(outer_border_color)
            .rounded(radius)
            .when(!self.selected && !self.disabled, |this| {
                this.hover(|this| {
                    this.text_color(hover_style.fg)
                        .bg(hover_style.bg)
                        .border_l(hover_style.borders.left)
                        .border_r(hover_style.borders.right)
                        .border_t(hover_style.borders.top)
                        .border_b(hover_style.borders.bottom)
                        .border_color(hover_style.border_color)
                        .rounded(radius)
                })
            })
            .when(has_inline_inner_bg, |this| {
                this.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top_0()
                        .bottom_0()
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .w_full()
                                .h(inner_height)
                                .bg(inline_inner_bg)
                                .rounded(inner_radius)
                                .when(tab_style.shadow, |this| this.shadow_xs()),
                        ),
                )
            })
            .when_some(self.prefix, |this, prefix| this.child(prefix))
            .child(inner_element)
            .when_some(self.suffix, |this, suffix| this.child(suffix))
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                // 阻止事件传播，使其在 TitleBar 上正常工作。
                cx.stop_propagation();
            })
            .when(!self.disabled, |this| {
                this.when_some(self.on_click.clone(), |this, on_click| {
                    this.on_click(move |event, window, cx| on_click(event, window, cx))
                })
            })
    }
}
