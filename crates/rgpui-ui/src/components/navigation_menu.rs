//! 导航菜单组件：支持层级结构与展开/收起状态的分级导航。

use rgpui::{prelude::FluentBuilder as _, *};
use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Arc;

/// 导航菜单排列方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NavigationMenuOrientation {
    /// 水平排列。
    #[default]
    Horizontal,
    /// 垂直排列。
    Vertical,
}

/// 导航菜单条目（可嵌套子条目）。
#[derive(Clone)]
pub struct NavigationMenuItem<T: Clone = SharedString> {
    /// 条目 ID。
    pub id: T,
    /// 条目标签。
    pub label: SharedString,
    /// 条目图标。
    pub icon: Option<Icon>,
    /// 是否禁用。
    pub disabled: bool,
    /// 子条目列表。
    pub children: Vec<NavigationMenuItem<T>>,
}

impl<T: Clone> NavigationMenuItem<T> {
    /// 创建条目。
    pub fn new(id: T, label: impl Into<SharedString>) -> Self {
        Self {
            id,
            label: label.into(),
            icon: None,
            disabled: false,
            children: Vec::new(),
        }
    }

    /// 设置条目图标。
    pub fn with_icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 设置是否禁用。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置子条目列表。
    pub fn with_children(mut self, children: Vec<NavigationMenuItem<T>>) -> Self {
        self.children = children;
        self
    }

    /// 是否有子条目。
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}

/// 导航菜单组件。
#[derive(IntoElement)]
pub struct NavigationMenu<T: Clone + PartialEq + Eq + Hash + 'static> {
    /// 排列方向。
    orientation: NavigationMenuOrientation,
    /// 条目列表。
    items: Vec<NavigationMenuItem<T>>,
    /// 选中条目 ID。
    selected_id: Option<T>,
    /// 展开的条目 ID 列表。
    expanded_ids: Vec<T>,
    /// 选中回调。
    on_select: Option<Arc<dyn Fn(&T, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 展开/收起回调。
    on_toggle: Option<Arc<dyn Fn(&T, bool, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl<T: Clone + PartialEq + Eq + Hash + 'static> NavigationMenu<T> {
    /// 创建导航菜单。
    pub fn new() -> Self {
        Self {
            orientation: NavigationMenuOrientation::default(),
            items: Vec::new(),
            selected_id: None,
            expanded_ids: Vec::new(),
            on_select: None,
            on_toggle: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置排列方向。
    pub fn orientation(mut self, orientation: NavigationMenuOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// 添加一个条目。
    pub fn item(mut self, item: NavigationMenuItem<T>) -> Self {
        self.items.push(item);
        self
    }

    /// 批量设置条目。
    pub fn items(mut self, items: Vec<NavigationMenuItem<T>>) -> Self {
        self.items = items;
        self
    }

    /// 设置选中条目 ID。
    pub fn selected_id(mut self, id: T) -> Self {
        self.selected_id = Some(id);
        self
    }

    /// 设置展开的条目 ID 列表。
    pub fn expanded_ids(mut self, ids: Vec<T>) -> Self {
        self.expanded_ids = ids;
        self
    }

    /// 设置选中回调。
    pub fn on_select<F>(mut self, f: F) -> Self
    where
        F: Fn(&T, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_select = Some(Arc::new(f));
        self
    }

    /// 设置展开/收起回调。
    pub fn on_toggle<F>(mut self, f: F) -> Self
    where
        F: Fn(&T, bool, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_toggle = Some(Arc::new(f));
        self
    }
}

impl<T: Clone + PartialEq + Eq + Hash + 'static> Default for NavigationMenu<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + PartialEq + Eq + Hash + 'static> Styled for NavigationMenu<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<T: Clone + PartialEq + Eq + Hash + 'static> RenderOnce for NavigationMenu<T> {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let orientation = self.orientation;

        let expanded_set: HashSet<T> = self.expanded_ids.into_iter().collect();
        let selected_id = self.selected_id;
        let on_select = self.on_select;
        let on_toggle = self.on_toggle;
        let user_style = self.style;

        div()
            .flex()
            .when(
                orientation == NavigationMenuOrientation::Horizontal,
                |this: Div| this.flex_row().items_center().gap(px(4.0)),
            )
            .when(
                orientation == NavigationMenuOrientation::Vertical,
                |this: Div| this.flex_col().gap(px(2.0)),
            )
            .children(self.items.into_iter().map(|item| {
                render_menu_item(
                    item,
                    orientation,
                    &theme,
                    0,
                    &expanded_set,
                    &selected_id,
                    &on_select,
                    &on_toggle,
                )
            }))
            .map(|this| {
                let mut div = this;
                div.style().refine(&user_style);
                div
            })
    }
}

/// 递归渲染单个菜单条目。
fn render_menu_item<T: Clone + PartialEq + Eq + Hash + 'static>(
    item: NavigationMenuItem<T>,
    orientation: NavigationMenuOrientation,
    theme: &rgpui::Theme,
    depth: usize,
    expanded_set: &HashSet<T>,
    selected_id: &Option<T>,
    on_select: &Option<Arc<dyn Fn(&T, &mut Window, &mut App) + Send + Sync + 'static>>,
    on_toggle: &Option<Arc<dyn Fn(&T, bool, &mut Window, &mut App) + Send + Sync + 'static>>,
) -> impl IntoElement {
    let has_children = item.has_children();
    let disabled = item.disabled;
    let is_expanded = expanded_set.contains(&item.id);
    let is_selected = selected_id.as_ref() == Some(&item.id);
    let indent = px(depth as f32 * 16.0);
    let radius = theme.radius;
    let accent = theme.tokens.accent;
    let accent_foreground = theme.tokens.accent_foreground;
    let muted = theme.tokens.muted;
    let muted_foreground = theme.tokens.muted_foreground;
    let foreground = theme.tokens.foreground;
    let border = theme.tokens.border;
    let popover = theme.tokens.popover;

    div()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .px(px(8.0))
                .py(px(8.0))
                .pl(when(
                    orientation == NavigationMenuOrientation::Vertical && depth > 0,
                    indent + px(8.0),
                    px(8.0),
                ))
                .rounded(radius)
                .text_size(px(14.0))
                .when(is_selected, |this: Div| this.bg(accent))
                .when(!is_selected && !disabled, |this: Div| {
                    this.hover(|style| style.bg(accent.opacity(0.1)))
                })
                .when(has_children, |this: Div| {
                    let item_id = item.id.clone();
                    let on_toggle = on_toggle.clone();
                    let is_expanded_copy = is_expanded;

                    this.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(20.0))
                            .h(px(20.0))
                            .rounded(px(4.0))
                            .cursor(if disabled {
                                CursorStyle::Arrow
                            } else {
                                CursorStyle::PointingHand
                            })
                            .when(!disabled && !is_selected, |this: Div| {
                                this.hover(|style| style.bg(muted.opacity(0.3)))
                            })
                            .when(!disabled && on_toggle.is_some(), |this: Div| {
                                let on_toggle = on_toggle.unwrap();
                                this.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    on_toggle(&item_id, !is_expanded_copy, window, cx);
                                })
                            })
                            .child(
                                Icon::new(if is_expanded {
                                    IconName::ArrowDown
                                } else {
                                    IconName::ArrowRight
                                })
                                .with_size(px(12.0))
                                .text_color(if is_selected {
                                    accent_foreground
                                } else {
                                    muted_foreground
                                }),
                            ),
                    )
                })
                .when(!has_children, |this: Div| this.child(div().w(px(20.0))))
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .items_center()
                        .gap(px(8.0))
                        .cursor(if disabled {
                            CursorStyle::Arrow
                        } else {
                            CursorStyle::PointingHand
                        })
                        .when(!disabled, |this: Div| {
                            let item_id = item.id.clone();
                            let on_select = on_select.clone();

                            this.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                if let Some(on_select) = on_select.as_ref() {
                                    on_select(&item_id, window, cx);
                                }
                            })
                        })
                        .when_some(item.icon.clone(), |this: Div, icon| {
                            this.child(Icon::new(icon).with_size(px(16.0)).text_color(
                                if is_selected {
                                    accent_foreground
                                } else if disabled {
                                    muted_foreground
                                } else {
                                    foreground
                                },
                            ))
                        })
                        .child(
                            div()
                                .flex_1()
                                .when(disabled, |this: Div| this.opacity(0.5))
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .font_weight(if is_selected {
                                            FontWeight::SEMIBOLD
                                        } else {
                                            FontWeight::NORMAL
                                        })
                                        .text_color(if is_selected {
                                            accent_foreground
                                        } else if disabled {
                                            muted_foreground
                                        } else {
                                            foreground
                                        })
                                        .child(item.label.clone()),
                                ),
                        ),
                ),
        )
        .when(has_children && is_expanded, |this: Div| {
            this.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .when(
                        orientation == NavigationMenuOrientation::Horizontal,
                        |this: Div| {
                            this.absolute()
                                .top_full()
                                .left_0()
                                .mt(px(4.0))
                                .min_w(px(200.0))
                                .bg(popover)
                                .border_1()
                                .border_color(border)
                                .rounded(radius)
                                .shadow(vec![BoxShadow {
                                    color: hsla(0.0, 0.0, 0.0, 0.1),
                                    offset: point(px(0.0), px(2.0)),
                                    blur_radius: px(8.0),
                                    spread_radius: px(0.0),
                                    inset: false,
                                }])
                                .p(px(4.0))
                        },
                    )
                    .when(
                        orientation == NavigationMenuOrientation::Vertical,
                        |this: Div| this.mt(px(2.0)),
                    )
                    .children(item.children.into_iter().map(|child| {
                        render_menu_item(
                            child,
                            orientation,
                            theme,
                            depth + 1,
                            expanded_set,
                            selected_id,
                            on_select,
                            on_toggle,
                        )
                    })),
            )
        })
}

/// 条件取值辅助函数。
fn when<T>(condition: bool, true_value: T, false_value: T) -> T {
    if condition { true_value } else { false_value }
}
