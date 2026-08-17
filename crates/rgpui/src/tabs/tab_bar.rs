use std::{cell::RefCell, rc::Rc, time::Duration};

use crate::{
    Anchor, Animation, AnimationExt as _, AnyElement, App, Background, Bounds, Div, Edges,
    ElementId, InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce, Role,
    ScrollHandle, SharedString, Stateful, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder as _, px,
};
use smallvec::SmallVec;

use super::{Tab, TabVariant};
use crate::{
    ActiveTheme, Button, ButtonVariants, DropdownMenu, ElementExt, ElementSize, Icon, Lerp,
    PopupMenuItem, Selectable, Sizable, StyledExt, ease_in_out_cubic, h_flex,
};

/// Tab 指示器的边界缓存，用于滑动动画计算。
struct TabIndicatorBounds {
    container: Bounds<Pixels>,
    tabs: Vec<Bounds<Pixels>>,
}

impl TabIndicatorBounds {
    /// 创建指定 Tab 数量的边界缓存。
    fn new(num_tabs: usize) -> Self {
        Self {
            container: Bounds::default(),
            tabs: vec![Bounds::default(); num_tabs],
        }
    }

    /// 按当前 Tab 数量调整边界缓存的大小。
    fn resize(&mut self, num_tabs: usize) {
        self.tabs.resize(num_tabs, Bounds::default());
    }
}

/// 包含多个 [`Tab`] 项目的 TabBar 元素。
#[derive(IntoElement)]
pub struct TabBar {
    id: ElementId,
    base: Stateful<Div>,
    style: StyleRefinement,
    scroll_handle: Option<ScrollHandle>,
    prefix: Option<AnyElement>,
    suffix: Option<AnyElement>,
    children: SmallVec<[Tab; 2]>,
    last_empty_space: AnyElement,
    selected_index: Option<usize>,
    variant: TabVariant,
    size: ElementSize,
    menu: bool,
    on_click: Option<Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>>,
}

impl TabBar {
    /// 创建一个新的 TabBar。
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();
        Self {
            id: id.clone(),
            base: div().id(id).px(px(-1.)),
            style: StyleRefinement::default(),
            children: SmallVec::new(),
            scroll_handle: None,
            prefix: None,
            suffix: None,
            variant: TabVariant::default(),
            size: ElementSize::default(),
            last_empty_space: div().w_3().into_any_element(),
            selected_index: None,
            on_click: None,
            menu: false,
        }
    }

    /// 设置 Tab 变体样式，所有子元素将继承该变体。
    pub fn with_variant(mut self, variant: TabVariant) -> Self {
        self.variant = variant;
        self
    }

    /// 设置 Tab 变体为药丸样式，所有子元素将继承该变体。
    pub fn pill(mut self) -> Self {
        self.variant = TabVariant::Pill;
        self
    }

    /// 设置 Tab 变体为描边样式，所有子元素将继承该变体。
    pub fn outline(mut self) -> Self {
        self.variant = TabVariant::Outline;
        self
    }

    /// 设置 Tab 变体为分段样式，所有子元素将继承该变体。
    pub fn segmented(mut self) -> Self {
        self.variant = TabVariant::Segmented;
        self
    }

    /// 设置 Tab 变体为下划线样式，所有子元素将继承该变体。
    pub fn underline(mut self) -> Self {
        self.variant = TabVariant::Underline;
        self
    }

    /// 设置 Tab 溢出时是否显示菜单按钮，默认为 false。
    pub fn menu(mut self, menu: bool) -> Self {
        self.menu = menu;
        self
    }

    /// 跟踪 TabBar 的滚动。
    pub fn track_scroll(mut self, scroll_handle: &ScrollHandle) -> Self {
        self.scroll_handle = Some(scroll_handle.clone());
        self
    }

    /// 设置 TabBar 的前缀元素。
    pub fn prefix(mut self, prefix: impl IntoElement) -> Self {
        self.prefix = Some(prefix.into_any_element());
        self
    }

    /// 设置 TabBar 的后缀元素。
    pub fn suffix(mut self, suffix: impl IntoElement) -> Self {
        self.suffix = Some(suffix.into_any_element());
        self
    }

    /// 添加多个 Tab 子元素，所有子元素将继承变体样式。
    pub fn children(mut self, children: impl IntoIterator<Item = impl Into<Tab>>) -> Self {
        self.children.extend(children.into_iter().map(Into::into));
        self
    }

    /// 添加单个 Tab 子元素，子元素将继承变体样式。
    pub fn child(mut self, child: impl Into<Tab>) -> Self {
        self.children.push(child.into());
        self
    }

    /// 设置 TabBar 的选中索引。
    pub fn selected_index(mut self, index: usize) -> Self {
        self.selected_index = Some(index);
        self
    }

    /// 设置 TabBar 末尾的空位元素。
    pub fn last_empty_space(mut self, last_empty_space: impl IntoElement) -> Self {
        self.last_empty_space = last_empty_space.into_any_element();
        self
    }

    /// 设置 TabBar 的点击回调，第一个参数是被点击 Tab 的索引。
    ///
    /// 设置后，子元素的 on_click 将被忽略。
    pub fn on_click<F>(mut self, on_click: F) -> Self
    where
        F: Fn(&usize, &mut Window, &mut App) + 'static,
    {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    /// 渲染 Tab 切换的滑动指示器元素。
    ///
    /// 返回指示器元素以及当前的动画 `epoch`，每次切换 Tab 时递增。
    /// Tab 以该纪元为键来触发自身过渡（例如文字颜色渐变），使其与指示器
    /// 滑动同步重启。
    fn render_indicator(
        &self,
        bounds_rc: &Option<Rc<RefCell<TabIndicatorBounds>>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<(AnyElement, u64)> {
        let has_indicator = matches!(
            self.variant,
            TabVariant::Segmented | TabVariant::Pill | TabVariant::Underline
        );
        let num_tabs = self.children.len();
        let selected_ix = self.selected_index.unwrap_or(usize::MAX);

        if !(has_indicator && num_tabs > 0 && selected_ix < num_tabs) {
            return None;
        }

        let prev_key = format!("{}-tab-prev", self.id);
        let anim_key = format!("{}-tab-anim", self.id);
        let init_key = format!("{}-tab-init", self.id);

        let prev_selected = window.use_keyed_state(prev_key, cx, |_, _| selected_ix);
        // (from_left, from_width, to_left, to_width, epoch)
        let anim_params =
            window.use_keyed_state(anim_key, cx, |_, _| (px(0.), px(0.), px(0.), px(0.), 0u64));
        let initialized = window.use_keyed_state(init_key, cx, |_, _| false);

        // 首帧：触发重渲染以通过 on_prepaint 捕获边界。
        if !*initialized.read(cx) {
            initialized.update(cx, |v, _| *v = true);
        }

        self.update_anim_params(selected_ix, bounds_rc, &prev_selected, &anim_params, cx);

        let (from_left, from_width, to_left, to_width, epoch) = *anim_params.read(cx);
        if to_width <= px(0.) {
            return None;
        }

        let variant = self.variant;
        let size = self.size;
        let inner_height = variant.inner_height(size);
        let inner_radius = variant.inner_radius(size, cx);

        let indicator = div()
            .absolute()
            .top_0()
            .bottom_0()
            .map(|el| match variant {
                TabVariant::Segmented => el.flex().items_center().child(
                    div()
                        .w_full()
                        .h(inner_height)
                        .bg(cx.theme().tokens.background)
                        .rounded(inner_radius)
                        .shadow_sm(),
                ),
                TabVariant::Pill => el.flex().items_center().child(
                    div()
                        .size_full()
                        .bg(cx.theme().tokens.primary)
                        .rounded(px(99.)),
                ),
                TabVariant::Underline => el.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(px(2.))
                        .bg(cx.theme().tokens.primary),
                ),
                _ => el,
            })
            .with_animation(
                ElementId::NamedInteger("tab-ind".into(), epoch),
                Animation::new(Duration::from_millis(200)).with_easing(ease_in_out_cubic),
                move |el, delta| {
                    let left = Lerp::lerp(&from_left, &to_left, delta);
                    let width = Lerp::lerp(&from_width, &to_width, delta);
                    el.left(left).w(width)
                },
            );

        Some((indicator.into_any_element(), epoch))
    }

    /// 根据当前与上一次的选择更新动画参数。
    fn update_anim_params(
        &self,
        selected_ix: usize,
        bounds_rc: &Option<Rc<RefCell<TabIndicatorBounds>>>,
        prev_selected: &crate::Entity<usize>,
        anim_params: &crate::Entity<(Pixels, Pixels, Pixels, Pixels, u64)>,
        cx: &mut App,
    ) {
        let rc = match bounds_rc {
            Some(rc) => rc,
            None => return,
        };

        let prev_ix = *prev_selected.read(cx);
        let bounds = rc.borrow();
        let container = bounds.container;

        if container.size.width == px(0.) {
            if prev_ix != selected_ix {
                prev_selected.update(cx, |v, _| *v = selected_ix);
            }
            return;
        }

        if prev_ix != selected_ix {
            let from_b = bounds.tabs.get(prev_ix);
            let to_b = bounds.tabs.get(selected_ix);
            match (from_b, to_b) {
                (Some(from_b), Some(to_b)) => {
                    let from_left = from_b.origin.x - container.origin.x;
                    let from_width = from_b.size.width;
                    let to_left = to_b.origin.x - container.origin.x;
                    let to_width = to_b.size.width;
                    let epoch = anim_params.read(cx).4 + 1;
                    anim_params.update(cx, |v, _| {
                        *v = (from_left, from_width, to_left, to_width, epoch)
                    });
                }
                (None, Some(to_b)) => {
                    let left = to_b.origin.x - container.origin.x;
                    let width = to_b.size.width;
                    anim_params.update(cx, |v, _| *v = (left, width, left, width, v.4));
                }
                _ => {}
            }
            drop(bounds);
            prev_selected.update(cx, |v, _| *v = selected_ix);
            return;
        }

        if let Some(to_b) = bounds.tabs.get(selected_ix) {
            let left = to_b.origin.x - container.origin.x;
            let width = to_b.size.width;
            let (_, _, to_left, to_width, epoch) = *anim_params.read(cx);

            if to_width == px(0.) {
                anim_params.update(cx, |v, _| *v = (left, width, left, width, epoch));
                return;
            }

            if left != to_left || width != to_width {
                anim_params.update(cx, |v, _| *v = (left, width, left, width, epoch));
            }
        }
    }
}

impl Styled for TabBar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Sizable for TabBar {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for TabBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let default_gap = match self.size {
            ElementSize::Small | ElementSize::XSmall => px(8.),
            ElementSize::Large => px(16.),
            _ => px(12.),
        };
        let (bg, paddings, gap): (Background, _, _) = match self.variant {
            TabVariant::Tab => {
                let padding = Edges::all(px(0.));
                (cx.theme().tokens.tab_bar.into(), padding, px(0.))
            }
            TabVariant::Outline => {
                let padding = Edges::all(px(0.));
                (cx.theme().transparent.into(), padding, default_gap)
            }
            TabVariant::Pill => {
                let padding = Edges::all(px(0.));
                (cx.theme().transparent.into(), padding, px(4.))
            }
            TabVariant::Segmented => {
                let padding_x = match self.size {
                    ElementSize::XSmall => px(2.),
                    ElementSize::Small => px(3.),
                    _ => px(4.),
                };
                let padding = Edges {
                    left: padding_x,
                    right: padding_x,
                    ..Default::default()
                };

                (cx.theme().tokens.tab_bar_segmented.into(), padding, px(2.))
            }
            TabVariant::Underline => {
                // 此间距与 Tab 的 inner_paddings 相同。
                let gap = match self.size {
                    ElementSize::XSmall => px(10.),
                    ElementSize::Small => px(12.),
                    ElementSize::Large => px(20.),
                    _ => px(16.),
                };

                (cx.theme().transparent.into(), Edges::all(px(0.)), gap)
            }
        };

        let has_indicator = matches!(
            self.variant,
            TabVariant::Segmented | TabVariant::Pill | TabVariant::Underline
        );
        let num_tabs = self.children.len();

        // 为 Tab 指示器动画跟踪边界。
        // 使用 Rc<RefCell> 以避免 prepaint 写入触发重渲染。
        let bounds_rc = if has_indicator && num_tabs > 0 {
            let rc: Rc<RefCell<TabIndicatorBounds>> = window
                .use_keyed_state(format!("{}-tab-bounds", self.id), cx, |_, _| {
                    Rc::new(RefCell::new(TabIndicatorBounds::new(num_tabs)))
                })
                .read(cx)
                .clone();
            rc.borrow_mut().resize(num_tabs);
            Some(rc)
        } else {
            None
        };

        let indicator = self.render_indicator(&bounds_rc, window, cx);
        let indicator_epoch = indicator.as_ref().map(|(_, epoch)| *epoch).unwrap_or(0);
        let indicator_element = indicator.map(|(el, _)| el);
        let indicator_ready = indicator_element.is_some();

        let has_suffix_or_menu = self.suffix.is_some() || self.menu;
        let mut item_metas: Vec<(Option<SharedString>, Option<Icon>, bool)> = Vec::new();
        let selected_index = self.selected_index;
        let on_click = self.on_click.clone();

        self.base
            .role(Role::TabList)
            .group("tab-bar")
            .relative()
            .flex()
            .items_center()
            .bg(bg)
            .text_color(cx.theme().tab_foreground)
            .when(
                self.variant == TabVariant::Underline || self.variant == TabVariant::Tab,
                |this| {
                    this.child(
                        div()
                            .id("border-b")
                            .absolute()
                            .left_0()
                            .bottom_0()
                            .size_full()
                            .border_b_1()
                            .border_color(cx.theme().border),
                    )
                },
            )
            .rounded(self.variant.tab_bar_radius(self.size, cx))
            .paddings(paddings)
            .refine_style(&self.style)
            .when_some(self.prefix, |this, prefix| this.child(prefix))
            .child(
                h_flex().id("tabs").flex_1().overflow_x_hidden().child(
                    h_flex()
                        .id("tabs-inner")
                        .relative()
                        .gap(gap)
                        .overflow_x_scroll()
                        .restrict_scroll_to_axis()
                        .when_some(self.scroll_handle, |this, scroll_handle| {
                            this.track_scroll(&scroll_handle)
                        })
                        .when_some(bounds_rc.clone(), |this, rc| {
                            this.on_prepaint(move |bounds, _, _| {
                                rc.borrow_mut().container = bounds;
                            })
                        })
                        .when_some(indicator_element, |this, ind| this.child(ind))
                        .children(self.children.into_iter().enumerate().map(|(ix, child)| {
                            item_metas.push((
                                child.label.clone(),
                                child.icon.clone(),
                                child.disabled,
                            ));
                            let tab_bar_prefix = child.tab_bar_prefix.unwrap_or(true);
                            let mut tab = child
                                .ix(ix)
                                .tab_bar_prefix(tab_bar_prefix)
                                .with_variant(self.variant)
                                .with_size(self.size);
                            tab.indicator_active = has_indicator;
                            tab.indicator_ready = indicator_ready;
                            tab.indicator_epoch = indicator_epoch;
                            let tab = tab
                                .when_some(self.selected_index, |this, selected_ix| {
                                    this.selected(selected_ix == ix)
                                })
                                .when_some(self.on_click.clone(), move |this, on_click| {
                                    this.on_click(move |_, window, cx| on_click(&ix, window, cx))
                                });

                            if let Some(ref rc) = bounds_rc {
                                let rc = rc.clone();
                                div()
                                    .flex_shrink_0()
                                    .on_prepaint(move |bounds, _, _| {
                                        if let Some(slot) = rc.borrow_mut().tabs.get_mut(ix) {
                                            *slot = bounds;
                                        }
                                    })
                                    .child(tab)
                                    .into_any_element()
                            } else {
                                tab.into_any_element()
                            }
                        }))
                        .when(has_suffix_or_menu, |this| this.child(self.last_empty_space)),
                ),
            )
            .when(self.menu, |this| {
                this.child(
                    Button::new("more")
                        .xsmall()
                        .ghost()
                        .dropdown_caret(true)
                        .dropdown_menu(move |mut this, _, _| {
                            this = this.scrollable(true);
                            for (ix, (label, icon, disabled)) in item_metas.iter().enumerate() {
                                let base = if let Some(label) = label.clone() {
                                    PopupMenuItem::new(label)
                                } else if let Some(icon) = icon.clone() {
                                    PopupMenuItem::element(move |_, _| icon.clone())
                                } else {
                                    PopupMenuItem::new("Unnamed")
                                };
                                this = this.item(
                                    base.checked(selected_index == Some(ix))
                                        .disabled(*disabled)
                                        .when_some(on_click.clone(), |this, on_click| {
                                            this.on_click(move |_, window, cx| {
                                                on_click(&ix, window, cx)
                                            })
                                        }),
                                );
                            }

                            this
                        })
                        .anchor(Anchor::TopRight),
                )
            })
            .when_some(self.suffix, |this, suffix| this.child(suffix))
    }
}
