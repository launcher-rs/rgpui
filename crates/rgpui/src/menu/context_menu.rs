use std::{cell::RefCell, rc::Rc};

use crate::{
    Anchor, AnyElement, App, Context, DismissEvent, Element, ElementId, Entity, Focusable,
    GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, ParentElement, Pixels, Point, StyleRefinement, Styled,
    Subscription, Window, anchored, deferred, div, prelude::FluentBuilder, px,
};

use crate::menu::PopupMenu;

/// 为元素添加右键菜单的扩展 trait
pub trait ContextMenuExt: InteractiveElement + ParentElement + Styled {
    /// 为元素添加右键菜单
    ///
    /// 这将把元素改为 `relative` 定位，并添加一个子 `ContextMenu` 元素。
    /// 因为 `ContextMenu` 元素是 `absolute` 定位，所以不会影响父元素的布局。
    #[track_caller]
    fn context_menu(
        mut self,
        f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> ContextMenu<Self>
    where
        Self: Sized,
    {
        // ID 必须跨渲染保持稳定，否则元素状态（打开的菜单）会在每次重新渲染时丢失。
        let caller = std::panic::Location::caller();
        let id = self
            .interactivity()
            .element_id
            .clone()
            .map(|id| ElementId::Name(format!("context-menu-{:?}", id).into()))
            .unwrap_or_else(|| ElementId::CodeLocation(*caller));
        ContextMenu::new(id, self).menu(f)
    }
}

impl<E: InteractiveElement + ParentElement + Styled> ContextMenuExt for E {}

/// 可在右键时显示的右键菜单
pub struct ContextMenu<E: ParentElement + Styled + Sized> {
    id: ElementId,
    element: Option<E>,
    menu: Option<Rc<dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>>,
    // 未使用，仅用于样式细化转发
    _ignore_style: StyleRefinement,
    anchor: Anchor,
}

impl<E: ParentElement + Styled> ContextMenu<E> {
    /// 使用给定的 ID 创建新的右键菜单
    pub fn new(id: impl Into<ElementId>, element: E) -> Self {
        Self {
            id: id.into(),
            element: Some(element),
            menu: None,
            anchor: Anchor::TopLeft,
            _ignore_style: StyleRefinement::default(),
        }
    }

    /// 使用给定的构建器构建右键菜单
    #[must_use]
    fn menu<F>(mut self, builder: F) -> Self
    where
        F: Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    {
        self.menu = Some(Rc::new(builder));
        self
    }

    fn with_element_state<R>(
        &mut self,
        id: &GlobalElementId,
        window: &mut Window,
        cx: &mut App,
        f: impl FnOnce(&mut Self, &mut ContextMenuState, &mut Window, &mut App) -> R,
    ) -> R {
        window.with_optional_element_state::<ContextMenuState, _>(
            Some(id),
            |element_state, window| {
                let mut element_state = element_state.unwrap().unwrap_or_default();
                let result = f(self, &mut element_state, window, cx);
                (result, Some(element_state))
            },
        )
    }
}

impl<E: ParentElement + Styled> ParentElement for ContextMenu<E> {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        if let Some(element) = &mut self.element {
            element.extend(elements);
        }
    }
}

impl<E: ParentElement + Styled> Styled for ContextMenu<E> {
    fn style(&mut self) -> &mut StyleRefinement {
        if let Some(element) = &mut self.element {
            element.style()
        } else {
            &mut self._ignore_style
        }
    }
}

impl<E: ParentElement + Styled + IntoElement + 'static> IntoElement for ContextMenu<E> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// 右键菜单共享状态
struct ContextMenuSharedState {
    menu_view: Option<Entity<PopupMenu>>,
    open: bool,
    position: Point<Pixels>,
    _subscription: Option<Subscription>,
}

/// 右键菜单元素状态
pub struct ContextMenuState {
    element: Option<AnyElement>,
    shared_state: Rc<RefCell<ContextMenuSharedState>>,
}

impl Default for ContextMenuState {
    fn default() -> Self {
        Self {
            element: None,
            shared_state: Rc::new(RefCell::new(ContextMenuSharedState {
                menu_view: None,
                open: false,
                position: Default::default(),
                _subscription: None,
            })),
        }
    }
}

impl<E: ParentElement + Styled + IntoElement + 'static> Element for ContextMenu<E> {
    type RequestLayoutState = ContextMenuState;
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&crate::GlobalElementId>,
        _: Option<&crate::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (crate::LayoutId, Self::RequestLayoutState) {
        let anchor = self.anchor;

        self.with_element_state(
            id.unwrap(),
            window,
            cx,
            |this, state: &mut ContextMenuState, window, cx| {
                let (position, open) = {
                    let shared_state = state.shared_state.borrow();
                    (shared_state.position, shared_state.open)
                };
                let menu_view = state.shared_state.borrow().menu_view.clone();
                let mut menu_element = None;
                if open {
                    let has_menu_item = menu_view
                        .as_ref()
                        .map(|menu| !menu.read(cx).is_empty())
                        .unwrap_or(false);

                    if has_menu_item {
                        menu_element = Some(
                            deferred(
                                anchored().child(
                                    div()
                                        .w(window.bounds().size.width)
                                        .h(window.bounds().size.height)
                                        .on_scroll_wheel(|_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .child(
                                            anchored()
                                                .position(position)
                                                .snap_to_window_with_margin(px(8.))
                                                .anchor(anchor)
                                                .when_some(menu_view, |this, menu| {
                                                    // 聚焦菜单，以便其处理动作
                                                    if !menu
                                                        .focus_handle(cx)
                                                        .contains_focused(window, cx)
                                                    {
                                                        menu.focus_handle(cx).focus(window, cx);
                                                    }

                                                    this.child(menu)
                                                }),
                                        ),
                                ),
                            )
                            .with_priority(1)
                            .into_any(),
                        );
                    }
                }

                let mut element = this
                    .element
                    .take()
                    .expect("Element should exists.")
                    .children(menu_element)
                    .into_any_element();

                let layout_id = element.request_layout(window, cx);

                (
                    layout_id,
                    ContextMenuState {
                        element: Some(element),
                        ..Default::default()
                    },
                )
            },
        )
    }

    fn prepaint(
        &mut self,
        _: Option<&crate::GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: crate::Bounds<crate::Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        if let Some(element) = &mut request_layout.element {
            element.prepaint(window, cx);
        }
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        id: Option<&crate::GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: crate::Bounds<crate::Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(element) = &mut request_layout.element {
            element.paint(window, cx);
        }

        // 在设置元素状态之前取出构建器以避免借用问题
        let builder = self.menu.clone();

        self.with_element_state(
            id.unwrap(),
            window,
            cx,
            |_view, state: &mut ContextMenuState, window, _| {
                let shared_state = state.shared_state.clone();

                let hitbox = hitbox.clone();
                // 当右键点击时，构建内容菜单并在鼠标位置显示
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                    if phase.bubble()
                        && event.button == MouseButton::Right
                        && hitbox.is_hovered(window)
                    {
                        // 捕获聚焦元素以在关闭时恢复焦点。
                        // 如果焦点仍在之前的菜单上，则保留其捕获的焦点。
                        let previous_focus_handle = window.focused(cx).and_then(|focused| {
                            let shared_state = shared_state.borrow();
                            match shared_state.menu_view.as_ref() {
                                Some(menu) if menu.read(cx).focus_handle == focused => {
                                    menu.read(cx).previous_focus_handle.clone()
                                }
                                _ => Some(focused),
                            }
                        });

                        {
                            let mut shared_state = shared_state.borrow_mut();
                            // 清除现有菜单视图以允许立即替换
                            // 设置新位置并打开菜单
                            shared_state.menu_view = None;
                            shared_state._subscription = None;
                            shared_state.position = event.position;
                            shared_state.open = true;
                        }

                        // 使用 defer 在下一帧构建菜单，避免竞态条件
                        window.defer(cx, {
                            let shared_state = shared_state.clone();
                            let builder = builder.clone();
                            move |window, cx| {
                                let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
                                    let Some(build) = &builder else {
                                        return menu;
                                    };
                                    build(menu, window, cx)
                                });
                                menu.update(cx, |menu, cx| {
                                    menu.set_previous_focus(previous_focus_handle, cx);
                                });

                                // 设置关闭处理的订阅
                                let _subscription = window.subscribe(&menu, cx, {
                                    let shared_state = shared_state.clone();
                                    move |_, _: &DismissEvent, window, _cx| {
                                        shared_state.borrow_mut().open = false;
                                        window.refresh();
                                    }
                                });

                                // 使用构建的菜单和订阅更新共享状态
                                {
                                    let mut state = shared_state.borrow_mut();
                                    state.menu_view = Some(menu.clone());
                                    state._subscription = Some(_subscription);
                                    window.refresh();
                                }
                            }
                        });
                    }
                });
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::{
        Context, FocusHandle, IntoElement, Render, TestAppContext, VisualTestContext, point, px,
    };
    use std::cell::Cell;

    actions!(context_menu_test, [RemoveTab]);

    /// 回归测试形态：动作处理器位于触发器的祖先（如动作栏）上，
    /// 当焦点位于内容区时，该祖先不在焦点路径上。
    struct TestRoot {
        content_focus: FocusHandle,
        received: Rc<Cell<bool>>,
    }

    impl Render for TestRoot {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let received = self.received.clone();
            div()
                .size_full()
                .child(
                    div()
                        .id("content")
                        .h(px(40.))
                        .track_focus(&self.content_focus),
                )
                .child(
                    div()
                        .id("action-bar")
                        .h(px(60.))
                        .on_action(move |_: &RemoveTab, _, _| received.set(true))
                        .child(
                            div()
                                .id("tab")
                                .size_full()
                                .context_menu(|menu, _, _| menu.menu("Close", Box::new(RemoveTab))),
                        ),
                )
        }
    }

    #[rgpui::test]
    fn action_bubbles_from_trigger_and_focus_restores_on_dismiss(cx: &mut TestAppContext) {
        cx.update(|cx| {
            cx.set_global(Theme::default());
            super::super::popup_menu::init(cx);
        });

        let received = Rc::new(Cell::new(false));
        let (root, cx) = cx.add_window_view({
            let received = received.clone();
            move |window, cx| {
                let content_focus = cx.focus_handle();
                content_focus.focus(window, cx);
                TestRoot {
                    content_focus,
                    received,
                }
            }
        });
        let content_focus = root.read_with(cx, |root, _| root.content_focus.clone());
        let cx: &mut VisualTestContext = cx;
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });

        // 在标签页内右键打开上下文菜单。
        cx.simulate_event(MouseDownEvent {
            button: MouseButton::Right,
            position: point(px(50.), px(70.)),
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        });
        // 菜单实体在延迟回调中构建，然后在下次绘制时渲染（并聚焦）。
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });

        // 选择 "Close" 并确认。键盘确认和鼠标点击共享相同的 `confirm` 路径。
        cx.simulate_keystrokes("down enter");
        cx.run_until_parked();

        // 动作必须到达触发器祖先链上的处理器，即使动作栏从未在焦点路径上。
        assert!(received.get());
        // 关闭时必须将焦点恢复到菜单打开前的位置。
        cx.update(|window, cx| {
            assert_eq!(window.focused(cx).as_ref(), Some(&content_focus));
        });
    }
}
