//! Div 是核心的、可复用的元素，大多数 RGPUI 树都将基于它构建。
//! 它作为其他元素的容器，提供了许多用于布局和样式化子元素的实用功能，
//! 以及绑定鼠标事件和动作处理器。它的设计类似于 HTML 中的 `<div>` 元素，
//! 但适用于 RGPUI。
//!
//! # 构建你自己的 div
//!
//! RGPUI 没有直接提供有状态的、多步骤事件（如 `click` 和 `drag`）的 API。
//! 我们希望 RGPUI 用户能够根据自己的需求构建自己的抽象。然而，作为 UI 框架，
//! 我们也有义务提供一些构建块，使构建自定义元素的过程更加容易。为此，我们提供了
//! [`Interactivity`] 和 [`StyleRefinement`] 结构体，以及若干相关的 trait。
//! 它们共同提供了完整的类 Dom 事件和类 Tailwind 样式能力，你可以用它们来构建
//! 自定义元素。Div 通过将这两个系统组合成一个全能元素来构建。

use crate::PinchEvent;
use crate::collections::HashMap;
use crate::refineable::Refineable;
use crate::rgpui_util::ResultExt;
use crate::{
    Action, AnyDrag, AnyElement, AnyTooltip, AnyView, App, Bounds, ClickEvent, DispatchPhase,
    Display, Element, ElementId, Entity, EntityId, FocusHandle, Global, GlobalElementId, Hitbox,
    HitboxBehavior, HitboxId, InspectorElementId, IntoElement, IsZero, KeyContext, KeyDownEvent,
    KeyUpEvent, KeyboardButton, KeyboardClickEvent, LayoutId, ModifiersChangedEvent, MouseButton,
    MouseClickEvent, MouseDownEvent, MouseExitEvent, MouseMoveEvent, MousePressureEvent,
    MouseUpEvent, Overflow, ParentElement, Pixels, Point, Render, ScrollWheelEvent, SharedString,
    Size, Style, StyleRefinement, Styled, Task, TooltipId, Visibility, Window, WindowControlArea,
    point, px, size,
};
use smallvec::SmallVec;
use stacksafe::{StackSafe, stacksafe};
use std::{
    any::{Any, TypeId},
    cell::RefCell,
    cmp::Ordering,
    fmt::Debug,
    marker::PhantomData,
    mem,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use super::ImageCacheProvider;

const DRAG_THRESHOLD: f64 = 2.;
const DEFAULT_TOOLTIP_SHOW_DELAY: Duration = Duration::from_millis(500);
const HOVERABLE_TOOLTIP_HIDE_DELAY: Duration = Duration::from_millis(500);

/// 给定分组的样式信息。
pub struct GroupStyle {
    /// 该分组的标识符。
    pub group: SharedString,

    /// 该分组将应用到其子元素的具体样式细化。
    pub style: Box<StyleRefinement>,
}

/// 当拖拽移动经过此元素时触发的事件，包含给定的状态类型。
pub struct DragMoveEvent<T> {
    /// 触发此拖拽移动事件的鼠标移动事件。
    pub event: MouseMoveEvent,

    /// 此元素的边界矩形。
    pub bounds: Bounds<Pixels>,
    drag: PhantomData<T>,
    dragged_item: Arc<dyn Any>,
}

impl<T: 'static> DragMoveEvent<T> {
    /// 返回此事件的拖拽状态。
    pub fn drag<'b>(&self, cx: &'b App) -> &'b T {
        cx.active_drag
            .as_ref()
            .and_then(|drag| drag.value.downcast_ref::<T>())
            .expect("DragMoveEvent is only valid when the stored active drag is of the same type.")
    }

    /// 即将被释放（drop）的项目。
    pub fn dragged_item(&self) -> &dyn Any {
        self.dragged_item.as_ref()
    }
}

impl Interactivity {
    /// 创建一个 `Interactivity`，在调试模式下捕获调用位置。
    #[cfg(any(feature = "inspector", debug_assertions))]
    #[track_caller]
    pub fn new() -> Interactivity {
        Interactivity {
            source_location: Some(core::panic::Location::caller()),
            ..Default::default()
        }
    }

    /// 创建一个 `Interactivity`，在调试模式下捕获调用位置。
    #[cfg(not(any(feature = "inspector", debug_assertions)))]
    pub fn new() -> Interactivity {
        Interactivity::default()
    }

    /// 获取构造的源代码位置。非调试模式下返回 `None`。
    pub fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        #[cfg(any(feature = "inspector", debug_assertions))]
        {
            self.source_location
        }

        #[cfg(not(any(feature = "inspector", debug_assertions)))]
        {
            None
        }
    }

    /// 在冒泡阶段将给定回调绑定到指定鼠标按钮的鼠标按下事件。
    /// [`InteractiveElement::on_mouse_down`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_mouse_down(
        &mut self,
        button: MouseButton,
        listener: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) {
        self.mouse_down_listeners
            .push(Box::new(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Bubble
                    && event.button == button
                    && hitbox.is_hovered(window)
                {
                    (listener)(event, window, cx)
                }
            }));
    }

    /// 在捕获阶段将给定回调绑定到任意按钮的鼠标按下事件。
    /// [`InteractiveElement::capture_any_mouse_down`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn capture_any_mouse_down(
        &mut self,
        listener: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) {
        self.mouse_down_listeners
            .push(Box::new(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Capture && hitbox.is_hovered(window) {
                    (listener)(event, window, cx)
                }
            }));
    }

    /// 在冒泡阶段将给定回调绑定到任意按钮的鼠标按下事件。
    /// [`InteractiveElement::on_any_mouse_down`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_any_mouse_down(
        &mut self,
        listener: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) {
        self.mouse_down_listeners
            .push(Box::new(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Bubble && hitbox.is_hovered(window) {
                    (listener)(event, window, cx)
                }
            }));
    }

    /// 在冒泡阶段将给定回调绑定到鼠标按压事件。
    /// [`InteractiveElement::on_mouse_pressure`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_mouse_pressure(
        &mut self,
        listener: impl Fn(&MousePressureEvent, &mut Window, &mut App) + 'static,
    ) {
        self.mouse_pressure_listeners
            .push(Box::new(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Bubble && hitbox.is_hovered(window) {
                    (listener)(event, window, cx)
                }
            }));
    }

    /// 在捕获阶段将给定回调绑定到鼠标按压事件。
    /// [`InteractiveElement::on_mouse_pressure`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn capture_mouse_pressure(
        &mut self,
        listener: impl Fn(&MousePressureEvent, &mut Window, &mut App) + 'static,
    ) {
        self.mouse_pressure_listeners
            .push(Box::new(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Capture && hitbox.is_hovered(window) {
                    (listener)(event, window, cx)
                }
            }));
    }

    /// 在冒泡阶段将给定回调绑定到指定按钮的鼠标释放事件。
    /// [`InteractiveElement::on_mouse_up`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_mouse_up(
        &mut self,
        button: MouseButton,
        listener: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
    ) {
        self.mouse_up_listeners
            .push(Box::new(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Bubble
                    && event.button == button
                    && hitbox.is_hovered(window)
                {
                    (listener)(event, window, cx)
                }
            }));
    }

    /// 在捕获阶段将给定回调绑定到任意按钮的鼠标释放事件。
    /// [`InteractiveElement::capture_any_mouse_up`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn capture_any_mouse_up(
        &mut self,
        listener: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
    ) {
        self.mouse_up_listeners
            .push(Box::new(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Capture && hitbox.is_hovered(window) {
                    (listener)(event, window, cx)
                }
            }));
    }

    /// 在冒泡阶段将给定回调绑定到任意按钮的鼠标释放事件。
    /// [`Interactivity::on_any_mouse_up`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_any_mouse_up(
        &mut self,
        listener: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
    ) {
        self.mouse_up_listeners
            .push(Box::new(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Bubble && hitbox.is_hovered(window) {
                    (listener)(event, window, cx)
                }
            }));
    }

    /// 在捕获阶段，当鼠标位于此元素边界之外时，将给定回调绑定到任意按钮的鼠标按下事件。
    /// [`InteractiveElement::on_mouse_down_out`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_mouse_down_out(
        &mut self,
        listener: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) {
        self.mouse_down_listeners
            .push(Box::new(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Capture && !hitbox.contains(&window.mouse_position()) {
                    (listener)(event, window, cx)
                }
            }));
    }

    /// 在捕获阶段，当鼠标位于此元素边界之外时，将给定回调绑定到指定按钮的鼠标释放事件。
    /// [`InteractiveElement::on_mouse_up_out`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_mouse_up_out(
        &mut self,
        button: MouseButton,
        listener: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
    ) {
        self.mouse_up_listeners
            .push(Box::new(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Capture
                    && event.button == button
                    && !hitbox.is_hovered(window)
                {
                    (listener)(event, window, cx);
                }
            }));
    }

    /// 在冒泡阶段将给定回调绑定到鼠标移动事件。
    /// [`InteractiveElement::on_mouse_move`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_mouse_move(
        &mut self,
        listener: impl Fn(&MouseMoveEvent, &mut Window, &mut App) + 'static,
    ) {
        self.mouse_move_listeners
            .push(Box::new(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Bubble && hitbox.is_hovered(window) {
                    (listener)(event, window, cx);
                }
            }));
    }

    /// 在冒泡阶段将给定回调绑定到鼠标离开事件。
    /// [`InteractiveElement::on_mouse_exit`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_mouse_exit(
        &mut self,
        listener: impl Fn(&MouseExitEvent, &mut Window, &mut App) + 'static,
    ) {
        self.mouse_exit_listeners
            .push(Box::new(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Bubble && hitbox.is_hovered(window) {
                    (listener)(event, window, cx);
                }
            }));
    }

    /// 将给定回调绑定到指定类型的鼠标拖拽移动事件。注意此回调
    /// 会在所有移动事件中被调用，无论鼠标在元素内部还是外部，只要拖拽
    /// 是由此元素开始的。适用于实现不符合拖放交互样式的可拖拽 UI，
    /// 例如调整大小。
    /// [`InteractiveElement::on_drag_move`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_drag_move<T>(
        &mut self,
        listener: impl Fn(&DragMoveEvent<T>, &mut Window, &mut App) + 'static,
    ) where
        T: 'static,
    {
        self.mouse_move_listeners
            .push(Box::new(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Capture
                    && let Some(drag) = &cx.active_drag
                    && drag.value.as_ref().type_id() == TypeId::of::<T>()
                {
                    (listener)(
                        &DragMoveEvent {
                            event: event.clone(),
                            bounds: hitbox.bounds,
                            drag: PhantomData,
                            dragged_item: Arc::clone(&drag.value),
                        },
                        window,
                        cx,
                    );
                }
            }));
    }

    /// 在冒泡阶段将给定回调绑定到滚轮事件。
    /// [`InteractiveElement::on_scroll_wheel`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_scroll_wheel(
        &mut self,
        listener: impl Fn(&ScrollWheelEvent, &mut Window, &mut App) + 'static,
    ) {
        self.scroll_wheel_listeners
            .push(Box::new(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Bubble && hitbox.should_handle_scroll(window) {
                    (listener)(event, window, cx);
                }
            }));
    }

    /// 在冒泡阶段将给定回调绑定到捏合手势事件。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_pinch(&mut self, listener: impl Fn(&PinchEvent, &mut Window, &mut App) + 'static) {
        self.pinch_listeners
            .push(Box::new(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Bubble && hitbox.is_hovered(window) {
                    (listener)(event, window, cx);
                }
            }));
    }

    /// 在捕获阶段将给定回调绑定到捏合手势事件。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn capture_pinch(
        &mut self,
        listener: impl Fn(&PinchEvent, &mut Window, &mut App) + 'static,
    ) {
        self.pinch_listeners
            .push(Box::new(move |event, phase, _hitbox, window, cx| {
                if phase == DispatchPhase::Capture {
                    (listener)(event, window, cx);
                } else {
                    cx.propagate();
                }
            }));
    }

    /// 在捕获阶段将给定回调绑定到动作分发。
    /// [`InteractiveElement::capture_action`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn capture_action<A: Action>(
        &mut self,
        listener: impl Fn(&A, &mut Window, &mut App) + 'static,
    ) {
        self.action_listeners.push((
            TypeId::of::<A>(),
            Box::new(move |action, phase, window, cx| {
                let action = action.downcast_ref().unwrap();
                if phase == DispatchPhase::Capture {
                    (listener)(action, window, cx)
                } else {
                    cx.propagate();
                }
            }),
        ));
    }

    /// 在冒泡阶段将给定回调绑定到动作分发。
    /// [`InteractiveElement::on_action`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    #[track_caller]
    pub fn on_action<A: Action>(&mut self, listener: impl Fn(&A, &mut Window, &mut App) + 'static) {
        self.action_listeners.push((
            TypeId::of::<A>(),
            Box::new(move |action, phase, window, cx| {
                let action = action.downcast_ref().unwrap();
                if phase == DispatchPhase::Bubble {
                    (listener)(action, window, cx)
                }
            }),
        ));
    }

    /// 将给定回调绑定到动作分发，基于动态动作参数而非类型参数。
    /// 适用于希望向用户暴露动作绑定的组件库。
    /// [`InteractiveElement::on_boxed_action`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_boxed_action(
        &mut self,
        action: &dyn Action,
        listener: impl Fn(&dyn Action, &mut Window, &mut App) + 'static,
    ) {
        let action = action.boxed_clone();
        self.action_listeners.push((
            (*action).type_id(),
            Box::new(move |_, phase, window, cx| {
                if phase == DispatchPhase::Bubble {
                    (listener)(&*action, window, cx)
                }
            }),
        ));
    }

    /// 在冒泡阶段将给定回调绑定到按键按下事件。
    /// [`InteractiveElement::on_key_down`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_key_down(
        &mut self,
        listener: impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static,
    ) {
        self.key_down_listeners
            .push(Box::new(move |event, phase, window, cx| {
                if phase == DispatchPhase::Bubble {
                    (listener)(event, window, cx)
                }
            }));
    }

    /// 在捕获阶段将给定回调绑定到按键按下事件。
    /// [`InteractiveElement::capture_key_down`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn capture_key_down(
        &mut self,
        listener: impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static,
    ) {
        self.key_down_listeners
            .push(Box::new(move |event, phase, window, cx| {
                if phase == DispatchPhase::Capture {
                    listener(event, window, cx)
                }
            }));
    }

    /// 在冒泡阶段将给定回调绑定到按键释放事件。
    /// [`InteractiveElement::on_key_up`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_key_up(&mut self, listener: impl Fn(&KeyUpEvent, &mut Window, &mut App) + 'static) {
        self.key_up_listeners
            .push(Box::new(move |event, phase, window, cx| {
                if phase == DispatchPhase::Bubble {
                    listener(event, window, cx)
                }
            }));
    }

    /// 在捕获阶段将给定回调绑定到按键释放事件。
    /// [`InteractiveElement::on_key_up`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn capture_key_up(
        &mut self,
        listener: impl Fn(&KeyUpEvent, &mut Window, &mut App) + 'static,
    ) {
        self.key_up_listeners
            .push(Box::new(move |event, phase, window, cx| {
                if phase == DispatchPhase::Capture {
                    listener(event, window, cx)
                }
            }));
    }

    /// 将给定回调绑定到修饰键变更事件。
    /// [`InteractiveElement::on_modifiers_changed`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_modifiers_changed(
        &mut self,
        listener: impl Fn(&ModifiersChangedEvent, &mut Window, &mut App) + 'static,
    ) {
        self.modifiers_changed_listeners
            .push(Box::new(move |event, window, cx| {
                listener(event, window, cx)
            }));
    }

    /// 将给定回调绑定到指定类型的放置（drop）事件，无论拖拽是否从此元素开始。
    /// [`InteractiveElement::on_drop`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_drop<T: 'static>(&mut self, listener: impl Fn(&T, &mut Window, &mut App) + 'static) {
        self.drop_listeners.push((
            TypeId::of::<T>(),
            Box::new(move |dragged_value, window, cx| {
                listener(dragged_value.downcast_ref().unwrap(), window, cx);
            }),
        ));
    }

    /// 使用给定的谓词判断是否应向此元素分发放置事件。
    /// [`InteractiveElement::can_drop`] 的命令式 API 等价物。
    pub fn can_drop(
        &mut self,
        predicate: impl Fn(&dyn Any, &mut Window, &mut App) -> bool + 'static,
    ) {
        self.can_drop_predicate = Some(Box::new(predicate));
    }

    /// 将给定回调绑定到此元素的点击事件。
    /// [`StatefulInteractiveElement::on_click`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_click(&mut self, listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static)
    where
        Self: Sized,
    {
        self.click_listeners.push(Rc::new(move |event, window, cx| {
            listener(event, window, cx)
        }));
    }

    /// 将给定回调绑定到此元素的非主按钮点击事件。
    /// [`StatefulInteractiveElement::on_aux_click`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_aux_click(&mut self, listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static)
    where
        Self: Sized,
    {
        self.aux_click_listeners
            .push(Rc::new(move |event, window, cx| {
                listener(event, window, cx)
            }));
    }

    /// 在拖拽启动时，此回调用于创建一个新视图来渲染拖拽值，用于拖放操作。
    /// 此 API 也应作为 [`Self::on_drag_move`] API 的"拖拽开始"等价物使用。
    /// [`StatefulInteractiveElement::on_drag`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_drag<T, W>(
        &mut self,
        value: T,
        constructor: impl Fn(&T, Point<Pixels>, &mut Window, &mut App) -> Entity<W> + 'static,
    ) where
        Self: Sized,
        T: 'static,
        W: 'static + Render,
    {
        debug_assert!(
            self.drag_listener.is_none(),
            "calling on_drag more than once on the same element is not supported"
        );
        self.drag_listener = Some((
            Arc::new(value),
            Box::new(move |value, offset, window, cx| {
                constructor(value.downcast_ref().unwrap(), offset, window, cx).into()
            }),
        ));
    }

    /// 将给定回调绑定到此元素的悬停开始和结束事件。注意传入回调的布尔值
    /// 在悬停开始时为 true，结束时为 false。
    /// 鼠标静止时由布局变化引起的过渡也会触发回调。
    /// [`StatefulInteractiveElement::on_hover`] 的命令式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    pub fn on_hover(&mut self, listener: impl Fn(&bool, &mut Window, &mut App) + 'static)
    where
        Self: Sized,
    {
        debug_assert!(
            self.hover_listener.is_none(),
            "calling on_hover more than once on the same element is not supported"
        );
        self.hover_listener = Some(Box::new(listener));
    }

    /// 使用给定回调在鼠标悬停于此元素时构建新的工具提示视图。
    /// [`StatefulInteractiveElement::tooltip`] 的命令式 API 等价物。
    pub fn tooltip(&mut self, build_tooltip: impl Fn(&mut Window, &mut App) -> AnyView + 'static)
    where
        Self: Sized,
    {
        debug_assert!(
            self.tooltip_builder.is_none(),
            "calling tooltip more than once on the same element is not supported"
        );
        self.tooltip_builder = Some(TooltipBuilder {
            build: Rc::new(build_tooltip),
            hoverable: false,
        });
    }

    /// 使用给定回调在鼠标悬停于此元素时构建新的工具提示视图。
    /// 工具提示本身也可悬停，当用户将鼠标移入工具提示时不会消失。
    /// [`StatefulInteractiveElement::hoverable_tooltip`] 的命令式 API 等价物。
    pub fn hoverable_tooltip(
        &mut self,
        build_tooltip: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
    ) where
        Self: Sized,
    {
        debug_assert!(
            self.tooltip_builder.is_none(),
            "calling tooltip more than once on the same element is not supported"
        );
        self.tooltip_builder = Some(TooltipBuilder {
            build: Rc::new(build_tooltip),
            hoverable: true,
        });
    }

    /// 设置此元素的工具提示显示前的延迟时间。
    /// [`StatefulInteractiveElement::tooltip_show_delay`] 的命令式 API 等价物。
    pub fn tooltip_show_delay(&mut self, delay: Duration) {
        self.tooltip_show_delay = Some(delay);
    }

    /// 阻止鼠标与此元素 hitbox 后方元素的所有交互。通常应优先使用
    /// `block_mouse_except_scroll`。
    ///
    /// [`InteractiveElement::occlude`] 的命令式 API 等价物。
    pub fn occlude_mouse(&mut self) {
        self.hitbox_behavior = HitboxBehavior::BlockMouse;
    }

    /// 将此元素的边界设置为平台窗口的窗口控制区域。
    /// [`InteractiveElement::window_control_area`] 的命令式 API 等价物。
    pub fn window_control_area(&mut self, area: WindowControlArea) {
        self.window_control = Some(area);
    }

    /// 阻止鼠标与此元素 hitbox 后方元素的非滚动交互。
    /// [`InteractiveElement::block_mouse_except_scroll`] 的命令式 API 等价物。
    ///
    /// 参见 [`Hitbox::is_hovered`] 了解详情。
    pub fn block_mouse_except_scroll(&mut self) {
        self.hitbox_behavior = HitboxBehavior::BlockMouseExceptScroll;
    }

    fn has_pinch_listeners(&self) -> bool {
        !self.pinch_listeners.is_empty()
    }
}

/// 希望使用标准 RGPUI 事件处理器且不需要任何状态的元素的 trait。
pub trait InteractiveElement: Sized {
    /// 获取与此元素关联的交互状态
    fn interactivity(&mut self) -> &mut Interactivity;

    /// 将此元素分配到可一起设置样式的分组中
    fn group(mut self, group: impl Into<SharedString>) -> Self {
        self.interactivity().group = Some(group.into());
        self
    }

    /// 为元素分配 ID，使其可用于交互功能
    fn id(mut self, id: impl Into<ElementId>) -> Stateful<Self> {
        self.interactivity().element_id = Some(id.into());

        Stateful { element: self }
    }

    /// 跟踪此元素上给定焦点句柄的焦点状态。
    /// 如果焦点句柄被应用程序聚焦，此元素将应用其聚焦样式。
    fn track_focus(mut self, focus_handle: &FocusHandle) -> Self {
        self.interactivity().focusable = true;
        self.interactivity().tracked_focus_handle = Some(focus_handle.clone());
        self
    }

    /// 设置此元素是否为制表停靠点。
    ///
    /// 为 false 时，元素仍保持在制表索引顺序中，但无法通过键盘导航到达。
    /// 适用于容器元素：聚焦容器后调用 `window.focus_next(cx)` 可聚焦容器内的
    /// 第一个制表停靠点，同时容器元素本身通过键盘不可达。
    /// 仅应与 `tab_index` 配合使用。
    fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.interactivity().tab_stop = tab_stop;
        self
    }

    /// 设置制表停靠顺序的索引，并将此节点设为制表停靠点。
    /// 这将默认使元素成为制表停靠点。参见 [`Self::tab_stop`] 了解更多信息。
    /// 仅应与 `tab_group` 配合使用，
    /// 以免干扰其他元素的制表索引。
    fn tab_index(mut self, index: isize) -> Self {
        self.interactivity().focusable = true;
        self.interactivity().tab_index = Some(index);
        self.interactivity().tab_stop = true;
        self
    }

    /// 将此 div 指定为"制表分组"。制表分组在制表索引顺序中有自己的位置，
    /// 但对于分组的子元素，制表索引重置为 0。这在交换分组内制表停靠点顺序时
    /// 非常有用，无需重新编号整个应用中的所有制表停靠点。
    fn tab_group(mut self) -> Self {
        self.interactivity().tab_group = true;
        if self.interactivity().tab_index.is_none() {
            self.interactivity().tab_index = Some(0);
        }
        self
    }

    /// 设置此元素的按键映射上下文。这将用于确定从按键映射分发哪个动作。
    fn key_context<C, E>(mut self, key_context: C) -> Self
    where
        C: TryInto<KeyContext, Error = E>,
        E: std::fmt::Display,
    {
        if let Some(key_context) = key_context.try_into().log_err() {
            self.interactivity().key_context = Some(key_context);
        }
        self
    }

    /// 当鼠标悬停于此元素时应用给定样式
    fn hover(mut self, f: impl FnOnce(StyleRefinement) -> StyleRefinement) -> Self {
        debug_assert!(
            self.interactivity().hover_style.is_none(),
            "hover style already set"
        );
        self.interactivity().hover_style = Some(Box::new(f(StyleRefinement::default())));
        self
    }

    /// 当鼠标悬停于分组成员时应用给定样式
    fn group_hover(
        mut self,
        group_name: impl Into<SharedString>,
        f: impl FnOnce(StyleRefinement) -> StyleRefinement,
    ) -> Self {
        self.interactivity().group_hover_style = Some(GroupStyle {
            group: group_name.into(),
            style: Box::new(f(StyleRefinement::default())),
        });
        self
    }

    /// 将给定回调绑定到指定鼠标按钮的按下事件。
    /// [`Interactivity::on_mouse_down`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_mouse_down(
        mut self,
        button: MouseButton,
        listener: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().on_mouse_down(button, listener);
        self
    }

    #[cfg(any(test, feature = "test-support"))]
    /// 设置一个可用于在 [`crate::VisualTestContext::debug_bounds`] 映射中
    /// 查找此元素边界的键。
    /// 在 release 构建中为空操作。
    fn debug_selector(mut self, f: impl FnOnce() -> String) -> Self {
        self.interactivity().debug_selector = Some(f());
        self
    }

    #[cfg(not(any(test, feature = "test-support")))]
    /// 设置一个可用于在 [`crate::VisualTestContext::debug_bounds`] 映射中
    /// 查找此元素边界的键。
    /// 在 release 构建中为空操作。
    #[inline]
    fn debug_selector(self, _: impl FnOnce() -> String) -> Self {
        self
    }

    /// 在捕获阶段将给定回调绑定到任意按钮的鼠标按下事件。
    /// [`Interactivity::capture_any_mouse_down`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn capture_any_mouse_down(
        mut self,
        listener: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().capture_any_mouse_down(listener);
        self
    }

    /// 在捕获阶段将给定回调绑定到任意按钮的鼠标按下事件。
    /// [`Interactivity::on_any_mouse_down`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_any_mouse_down(
        mut self,
        listener: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().on_any_mouse_down(listener);
        self
    }

    /// 在冒泡阶段将给定回调绑定到指定按钮的鼠标释放事件。
    /// [`Interactivity::on_mouse_up`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_mouse_up(
        mut self,
        button: MouseButton,
        listener: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().on_mouse_up(button, listener);
        self
    }

    /// 在捕获阶段将给定回调绑定到任意按钮的鼠标释放事件。
    /// [`Interactivity::capture_any_mouse_up`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn capture_any_mouse_up(
        mut self,
        listener: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().capture_any_mouse_up(listener);
        self
    }

    /// 在冒泡阶段将给定回调绑定到鼠标按压事件。
    /// [`Interactivity::on_mouse_pressure`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_mouse_pressure(
        mut self,
        listener: impl Fn(&MousePressureEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().on_mouse_pressure(listener);
        self
    }

    /// 在捕获阶段将给定回调绑定到鼠标按压事件。
    /// [`Interactivity::on_mouse_pressure`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn capture_mouse_pressure(
        mut self,
        listener: impl Fn(&MousePressureEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().capture_mouse_pressure(listener);
        self
    }

    /// 在捕获阶段，当鼠标位于此元素边界之外时，将给定回调绑定到任意按钮的鼠标按下事件。
    /// [`Interactivity::on_mouse_down_out`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_mouse_down_out(
        mut self,
        listener: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().on_mouse_down_out(listener);
        self
    }

    /// 在捕获阶段，当鼠标位于此元素边界之外时，将给定回调绑定到指定按钮的鼠标释放事件。
    /// [`Interactivity::on_mouse_up_out`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_mouse_up_out(
        mut self,
        button: MouseButton,
        listener: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().on_mouse_up_out(button, listener);
        self
    }

    /// 在冒泡阶段将给定回调绑定到鼠标移动事件。
    /// [`Interactivity::on_mouse_move`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_mouse_move(
        mut self,
        listener: impl Fn(&MouseMoveEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().on_mouse_move(listener);
        self
    }

    /// 在冒泡阶段将给定回调绑定到鼠标离开事件。
    /// [`Interactivity::on_mouse_exit`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_mouse_exit(
        mut self,
        listener: impl Fn(&MouseExitEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().on_mouse_exit(listener);
        self
    }

    /// 将给定回调绑定到指定类型的鼠标拖拽移动事件。注意此回调
    /// 会在所有移动事件中被调用，无论鼠标在元素内部还是外部，只要拖拽
    /// 是由此元素开始的。适用于实现不符合拖放交互样式的可拖拽 UI，
    /// 例如调整大小。
    /// [`Interactivity::on_drag_move`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_drag_move<T: 'static>(
        mut self,
        listener: impl Fn(&DragMoveEvent<T>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().on_drag_move(listener);
        self
    }

    /// 在冒泡阶段将给定回调绑定到滚轮事件。
    /// [`Interactivity::on_scroll_wheel`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_scroll_wheel(
        mut self,
        listener: impl Fn(&ScrollWheelEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().on_scroll_wheel(listener);
        self
    }

    /// 在冒泡阶段将给定回调绑定到捏合手势事件。
    /// [`Interactivity::on_pinch`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_pinch(mut self, listener: impl Fn(&PinchEvent, &mut Window, &mut App) + 'static) -> Self {
        self.interactivity().on_pinch(listener);
        self
    }

    /// 在捕获阶段将给定回调绑定到捏合手势事件。
    /// [`Interactivity::capture_pinch`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn capture_pinch(
        mut self,
        listener: impl Fn(&PinchEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().capture_pinch(listener);
        self
    }
    /// 在常规动作分发触发之前捕获给定动作。
    /// [`Interactivity::capture_action`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn capture_action<A: Action>(
        mut self,
        listener: impl Fn(&A, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().capture_action(listener);
        self
    }

    /// 在冒泡阶段将给定回调绑定到动作分发。
    /// [`Interactivity::on_action`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    #[track_caller]
    fn on_action<A: Action>(
        mut self,
        listener: impl Fn(&A, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().on_action(listener);
        self
    }

    /// 将给定回调绑定到动作分发，基于动态动作参数而非类型参数。
    /// 适用于希望向用户暴露动作绑定的组件库。
    /// [`Interactivity::on_boxed_action`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_boxed_action(
        mut self,
        action: &dyn Action,
        listener: impl Fn(&dyn Action, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().on_boxed_action(action, listener);
        self
    }

    /// 在冒泡阶段将给定回调绑定到按键按下事件。
    /// [`Interactivity::on_key_down`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_key_down(
        mut self,
        listener: impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().on_key_down(listener);
        self
    }

    /// 在捕获阶段将给定回调绑定到按键按下事件。
    /// [`Interactivity::capture_key_down`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn capture_key_down(
        mut self,
        listener: impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().capture_key_down(listener);
        self
    }

    /// 在冒泡阶段将给定回调绑定到按键释放事件。
    /// [`Interactivity::on_key_up`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_key_up(
        mut self,
        listener: impl Fn(&KeyUpEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().on_key_up(listener);
        self
    }

    /// 在捕获阶段将给定回调绑定到按键释放事件。
    /// [`Interactivity::capture_key_up`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn capture_key_up(
        mut self,
        listener: impl Fn(&KeyUpEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().capture_key_up(listener);
        self
    }

    /// 将给定回调绑定到修饰键变更事件。
    /// [`Interactivity::on_modifiers_changed`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_modifiers_changed(
        mut self,
        listener: impl Fn(&ModifiersChangedEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().on_modifiers_changed(listener);
        self
    }

    /// 当给定数据类型被拖拽到此元素上时应用给定样式
    fn drag_over<S: 'static>(
        mut self,
        f: impl 'static + Fn(StyleRefinement, &S, &mut Window, &mut App) -> StyleRefinement,
    ) -> Self {
        self.interactivity().drag_over_styles.push((
            TypeId::of::<S>(),
            Box::new(move |currently_dragged: &dyn Any, window, cx| {
                f(
                    StyleRefinement::default(),
                    currently_dragged.downcast_ref::<S>().unwrap(),
                    window,
                    cx,
                )
            }),
        ));
        self
    }

    /// 当给定数据类型被拖拽到此元素的分组上时应用给定样式
    fn group_drag_over<S: 'static>(
        mut self,
        group_name: impl Into<SharedString>,
        f: impl FnOnce(StyleRefinement) -> StyleRefinement,
    ) -> Self {
        self.interactivity().group_drag_over_styles.push((
            TypeId::of::<S>(),
            GroupStyle {
                group: group_name.into(),
                style: Box::new(f(StyleRefinement::default())),
            },
        ));
        self
    }

    /// 将给定回调绑定到指定类型的放置（drop）事件，无论拖拽是否从此元素开始。
    /// [`Interactivity::on_drop`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_drop<T: 'static>(
        mut self,
        listener: impl Fn(&T, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.interactivity().on_drop(listener);
        self
    }

    /// 使用给定的谓词判断是否应向此元素分发放置事件。
    /// [`Interactivity::can_drop`] 的流式 API 等价物。
    fn can_drop(
        mut self,
        predicate: impl Fn(&dyn Any, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.interactivity().can_drop(predicate);
        self
    }

    /// 阻止鼠标与此元素 hitbox 后方元素的所有交互。通常应优先使用
    /// `block_mouse_except_scroll`。
    /// [`Interactivity::occlude_mouse`] 的流式 API 等价物。
    fn occlude(mut self) -> Self {
        self.interactivity().occlude_mouse();
        self
    }

    /// 将此元素的边界设置为平台窗口的窗口控制区域。
    /// [`Interactivity::window_control_area`] 的流式 API 等价物。
    fn window_control_area(mut self, area: WindowControlArea) -> Self {
        self.interactivity().window_control_area(area);
        self
    }

    /// 阻止鼠标与此元素 hitbox 后方元素的非滚动交互。
    /// [`Interactivity::block_mouse_except_scroll`] 的流式 API 等价物。
    ///
    /// 参见 [`Hitbox::is_hovered`] 了解详情。
    fn block_mouse_except_scroll(mut self) -> Self {
        self.interactivity().block_mouse_except_scroll();
        self
    }

    /// 设置此元素被聚焦时应用的给定样式。
    /// 要求元素可聚焦。可使用 [`InteractiveElement::track_focus`] 使元素可聚焦。
    fn focus(mut self, f: impl FnOnce(StyleRefinement) -> StyleRefinement) -> Self
    where
        Self: Sized,
    {
        self.interactivity().focus_style = Some(Box::new(f(StyleRefinement::default())));
        self
    }

    /// 设置此元素位于另一个被聚焦的元素内部时应用的给定样式。
    /// 要求元素可聚焦。可使用 [`InteractiveElement::track_focus`] 使元素可聚焦。
    fn in_focus(mut self, f: impl FnOnce(StyleRefinement) -> StyleRefinement) -> Self
    where
        Self: Sized,
    {
        self.interactivity().in_focus_style = Some(Box::new(f(StyleRefinement::default())));
        self
    }

    /// 设置此元素通过键盘导航聚焦时应用的给定样式。
    /// 类似于 CSS 的 `:focus-visible` 伪类——仅在元素被聚焦且用户通过键盘导航
    /// （而非鼠标点击）时应用。
    /// 要求元素可聚焦。可使用 [`InteractiveElement::track_focus`] 使元素可聚焦。
    fn focus_visible(mut self, f: impl FnOnce(StyleRefinement) -> StyleRefinement) -> Self
    where
        Self: Sized,
    {
        self.interactivity().focus_visible_style = Some(Box::new(f(StyleRefinement::default())));
        self
    }
}

/// 希望使用需要状态的标准 RGPUI 交互功能的元素的 trait。
pub trait StatefulInteractiveElement: InteractiveElement {
    /// 设置此元素的无障碍角色。
    ///
    /// 参见[无障碍指南](crate::_accessibility)了解概述。
    fn role(mut self, role: accesskit::Role) -> Self {
        debug_assert!(
            role != accesskit::Role::GenericContainer,
            "GenericContainer is filtered out of the a11y tree and has no effect"
        );
        self.interactivity().override_role = Some(role);
        self
    }

    /// 设置此元素的无障碍标签。
    fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.interactivity().aria.label = Some(label.into());
        self
    }

    /// 设置此元素的无障碍描述。与标签（命名元素）不同，描述提供辅助技术
    /// 在名称、角色和值之后公布的补充信息——例如设置子标题或提示。
    fn aria_description(mut self, description: impl Into<SharedString>) -> Self {
        self.interactivity().aria.description = Some(description.into());
        self
    }

    /// 设置激活此元素的键盘快捷键，由辅助技术公布
    /// （映射到 AccessKit 的 `keyboard_shortcut`）。
    ///
    /// 注意这不会创建按键映射，只是告知辅助技术按键映射是什么。
    fn aria_keyshortcuts(mut self, keyshortcuts: impl Into<SharedString>) -> Self {
        self.interactivity().aria.keyshortcuts = Some(keyshortcuts.into());
        self
    }

    /// 将此元素报告为无障碍树中的聚焦节点，覆盖实际持有键盘焦点的元素
    /// ——但仅在其某个祖先实际持有焦点时。
    ///
    /// 这实现了 `aria-activedescendant` 模式，用于将键盘焦点保持在容器上
    /// （如菜单或列表框）而子元素被"选中"的复合组件：在选中的子元素上设置
    /// 此属性，使辅助技术将其宣布并高亮为聚焦。
    ///
    /// 元素还必须有 [`role`][Self::role]（和 id），以便生成无障碍节点。
    /// 与网页的容器端 `aria-activedescendant` 不同，这是设置在后代上的；
    /// RGPUI 仅在树中存在聚焦祖先时才将其视为有效，因此可以无条件地设置在
    /// 选中的子元素上——如果容器未聚焦，该声明将被忽略。
    fn aria_active_descendant(mut self) -> Self {
        self.interactivity().report_active_descendant_focus = true;
        self
    }

    /// 贡献合成无障碍节点——不对应任何元素的节点——作为此元素无障碍节点的子节点。
    /// 例如描述编辑器文本内容的文本运行。
    ///
    /// 闭包在此元素预绘制后调用，且仅在它向无障碍树贡献了节点（即有 id 和
    /// [`role`][StatefulInteractiveElement::role]）时才调用。
    ///
    /// 参见 [`Element::a11y_synthetic_children`] 了解详情。
    fn a11y_synthetic_children(
        mut self,
        f: impl FnOnce(&mut crate::A11ySubtreeBuilder) + 'static,
    ) -> Self {
        self.interactivity().a11y_synthetic_children = Some(Box::new(f));
        self
    }

    /// 设置此元素的选中状态。
    fn aria_selected(mut self, selected: bool) -> Self {
        self.interactivity().aria.selected = Some(selected);
        self
    }

    /// 设置此元素的展开状态。
    fn aria_expanded(mut self, expanded: bool) -> Self {
        self.interactivity().aria.expanded = Some(expanded);
        self
    }

    /// 设置此元素的切换状态。
    fn aria_toggled(mut self, toggled: accesskit::Toggled) -> Self {
        self.interactivity().aria.toggled = Some(toggled);
        self
    }

    /// 设置此元素的数值。
    fn aria_numeric_value(mut self, value: f64) -> Self {
        self.interactivity().aria.numeric_value = Some(value);
        self
    }

    /// 设置辅助技术应预期此元素数值变化的步长（例如递增微调按钮时）。
    fn aria_numeric_value_step(mut self, step: f64) -> Self {
        self.interactivity().aria.numeric_value_step = Some(step);
        self
    }

    /// 设置此元素的字符串值，例如简单文本输入框的文本内容。
    fn aria_value(mut self, value: impl Into<SharedString>) -> Self {
        self.interactivity().aria.value = Some(value.into());
        self
    }

    /// 设置向辅助技术报告的占位符文本，在文本输入为空时显示。
    fn aria_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.interactivity().aria.placeholder = Some(placeholder.into());
        self
    }

    /// 设置此元素的最小数值。
    fn aria_min_numeric_value(mut self, value: f64) -> Self {
        self.interactivity().aria.min_numeric_value = Some(value);
        self
    }

    /// 设置此元素的最大数值。
    fn aria_max_numeric_value(mut self, value: f64) -> Self {
        self.interactivity().aria.max_numeric_value = Some(value);
        self
    }

    /// 设置此元素的方向。
    fn aria_orientation(mut self, orientation: accesskit::Orientation) -> Self {
        self.interactivity().aria.orientation = Some(orientation);
        self
    }

    /// 设置此元素的标题级别。
    fn aria_level(mut self, level: usize) -> Self {
        self.interactivity().aria.level = Some(level);
        self
    }

    /// 设置此元素在集合中的位置。
    fn aria_position_in_set(mut self, position: usize) -> Self {
        self.interactivity().aria.position_in_set = Some(position);
        self
    }

    /// 设置此元素的集合大小。
    fn aria_size_of_set(mut self, size: usize) -> Self {
        self.interactivity().aria.size_of_set = Some(size);
        self
    }

    /// 设置此元素的行索引。
    fn aria_row_index(mut self, index: usize) -> Self {
        self.interactivity().aria.row_index = Some(index);
        self
    }

    /// 设置此元素的列索引。
    fn aria_column_index(mut self, index: usize) -> Self {
        self.interactivity().aria.column_index = Some(index);
        self
    }

    /// 设置此元素的行数。
    fn aria_row_count(mut self, count: usize) -> Self {
        self.interactivity().aria.row_count = Some(count);
        self
    }

    /// 设置此元素的列数。
    fn aria_column_count(mut self, count: usize) -> Self {
        self.interactivity().aria.column_count = Some(count);
        self
    }

    /// 为此元素注册无障碍动作的处理器。
    /// 当屏幕阅读器请求给定动作时调用处理器。
    ///
    /// 参见[无障碍指南](crate::_accessibility)了解概述。
    fn on_a11y_action(
        mut self,
        action: accesskit::Action,
        listener: impl FnMut(Option<&accesskit::ActionData>, &mut crate::Window, &mut crate::App)
        + 'static,
    ) -> Self {
        self.interactivity()
            .a11y_action_listeners
            .push((action, Box::new(listener)));
        self
    }

    /// 将此元素设为可聚焦。
    fn focusable(mut self) -> Self {
        self.interactivity().focusable = true;
        self
    }

    /// 将 x 和 y 溢出设置为滚动。
    fn overflow_scroll(mut self) -> Self {
        self.interactivity().base_style.overflow.x = Some(Overflow::Scroll);
        self.interactivity().base_style.overflow.y = Some(Overflow::Scroll);
        self
    }

    /// 将 x 溢出设置为滚动。
    fn overflow_x_scroll(mut self) -> Self {
        self.interactivity().base_style.overflow.x = Some(Overflow::Scroll);
        self
    }

    /// 将 y 溢出设置为滚动。
    fn overflow_y_scroll(mut self) -> Self {
        self.interactivity().base_style.overflow.y = Some(Overflow::Scroll);
        self
    }

    /// 将滚动限制为输入手势的方向轴。
    ///
    /// 参见 [`Style::restrict_scroll_to_axis`](crate::Style::restrict_scroll_to_axis) 的说明。
    fn restrict_scroll_to_axis(mut self) -> Self {
        self.interactivity().base_style.restrict_scroll_to_axis = Some(true);
        self
    }

    /// 使用给定句柄跟踪此元素的滚动状态。
    fn track_scroll(mut self, scroll_handle: &ScrollHandle) -> Self {
        self.interactivity().tracked_scroll_handle = Some(scroll_handle.clone());
        self
    }

    /// 使用给定锚点跟踪此元素的滚动状态。
    fn anchor_scroll(mut self, scroll_anchor: Option<ScrollAnchor>) -> Self {
        self.interactivity().scroll_anchor = scroll_anchor;
        self
    }

    /// 设置此元素处于激活状态时应用的给定样式。
    fn active(mut self, f: impl FnOnce(StyleRefinement) -> StyleRefinement) -> Self
    where
        Self: Sized,
    {
        self.interactivity().active_style = Some(Box::new(f(StyleRefinement::default())));
        self
    }

    /// 设置此元素的分组处于激活状态时应用的给定样式。
    fn group_active(
        mut self,
        group_name: impl Into<SharedString>,
        f: impl FnOnce(StyleRefinement) -> StyleRefinement,
    ) -> Self
    where
        Self: Sized,
    {
        self.interactivity().group_active_style = Some(GroupStyle {
            group: group_name.into(),
            style: Box::new(f(StyleRefinement::default())),
        });
        self
    }

    /// 将给定回调绑定到此元素的点击事件。
    /// [`Interactivity::on_click`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_click(mut self, listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self
    where
        Self: Sized,
    {
        self.interactivity().on_click(listener);
        self
    }

    /// 将给定回调绑定到此元素的非主按钮点击事件。
    /// [`Interactivity::on_aux_click`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_aux_click(
        mut self,
        listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self
    where
        Self: Sized,
    {
        self.interactivity().on_aux_click(listener);
        self
    }

    /// 在拖拽启动时，此回调用于创建一个新视图来渲染拖拽值，用于拖放操作。
    /// 此 API 也应作为 [`InteractiveElement::on_drag_move`] API 的"拖拽开始"等价物使用。
    /// 回调还可以访问触发点击相对于父元素原点的偏移量。
    /// [`Interactivity::on_drag`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_drag<T, W>(
        mut self,
        value: T,
        constructor: impl Fn(&T, Point<Pixels>, &mut Window, &mut App) -> Entity<W> + 'static,
    ) -> Self
    where
        Self: Sized,
        T: 'static,
        W: 'static + Render,
    {
        self.interactivity().on_drag(value, constructor);
        self
    }

    /// 将给定回调绑定到此元素的悬停开始和结束事件。注意传入回调的布尔值
    /// 在悬停开始时为 true，结束时为 false。
    /// 鼠标静止时由布局变化引起的过渡也会触发回调。
    /// [`Interactivity::on_hover`] 的流式 API 等价物。
    ///
    /// 参见 [`Context::listener`](crate::Context::listener) 了解如何从此回调访问视图状态。
    fn on_hover(mut self, listener: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self
    where
        Self: Sized,
    {
        self.interactivity().on_hover(listener);
        self
    }

    /// 使用给定回调在鼠标悬停于此元素时构建新的工具提示视图。
    /// [`Interactivity::tooltip`] 的流式 API 等价物。
    fn tooltip(mut self, build_tooltip: impl Fn(&mut Window, &mut App) -> AnyView + 'static) -> Self
    where
        Self: Sized,
    {
        self.interactivity().tooltip(build_tooltip);
        self
    }

    /// 使用给定回调在鼠标悬停于此元素时构建新的工具提示视图。
    /// 工具提示本身也可悬停，当用户将鼠标移入工具提示时不会消失。
    /// [`Interactivity::hoverable_tooltip`] 的流式 API 等价物。
    fn hoverable_tooltip(
        mut self,
        build_tooltip: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
    ) -> Self
    where
        Self: Sized,
    {
        self.interactivity().hoverable_tooltip(build_tooltip);
        self
    }

    /// 设置此元素的工具提示显示前的延迟时间。
    /// [`Interactivity::tooltip_show_delay`] 的流式 API 等价物。
    fn tooltip_show_delay(mut self, delay: Duration) -> Self
    where
        Self: Sized,
    {
        self.interactivity().tooltip_show_delay(delay);
        self
    }
}

pub(crate) type MouseDownListener =
    Box<dyn Fn(&MouseDownEvent, DispatchPhase, &Hitbox, &mut Window, &mut App) + 'static>;
pub(crate) type MouseUpListener =
    Box<dyn Fn(&MouseUpEvent, DispatchPhase, &Hitbox, &mut Window, &mut App) + 'static>;
pub(crate) type MousePressureListener =
    Box<dyn Fn(&MousePressureEvent, DispatchPhase, &Hitbox, &mut Window, &mut App) + 'static>;
pub(crate) type MouseMoveListener =
    Box<dyn Fn(&MouseMoveEvent, DispatchPhase, &Hitbox, &mut Window, &mut App) + 'static>;
pub(crate) type MouseExitListener =
    Box<dyn Fn(&MouseExitEvent, DispatchPhase, &Hitbox, &mut Window, &mut App) + 'static>;

pub(crate) type ScrollWheelListener =
    Box<dyn Fn(&ScrollWheelEvent, DispatchPhase, &Hitbox, &mut Window, &mut App) + 'static>;

pub(crate) type PinchListener =
    Box<dyn Fn(&PinchEvent, DispatchPhase, &Hitbox, &mut Window, &mut App) + 'static>;

pub(crate) type ClickListener = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

pub(crate) type DragListener =
    Box<dyn Fn(&dyn Any, Point<Pixels>, &mut Window, &mut App) -> AnyView + 'static>;

type DropListener = Box<dyn Fn(&dyn Any, &mut Window, &mut App) + 'static>;

type CanDropPredicate = Box<dyn Fn(&dyn Any, &mut Window, &mut App) -> bool + 'static>;

pub(crate) struct TooltipBuilder {
    build: Rc<dyn Fn(&mut Window, &mut App) -> AnyView + 'static>,
    hoverable: bool,
}

pub(crate) type KeyDownListener =
    Box<dyn Fn(&KeyDownEvent, DispatchPhase, &mut Window, &mut App) + 'static>;

pub(crate) type KeyUpListener =
    Box<dyn Fn(&KeyUpEvent, DispatchPhase, &mut Window, &mut App) + 'static>;

pub(crate) type ModifiersChangedListener =
    Box<dyn Fn(&ModifiersChangedEvent, &mut Window, &mut App) + 'static>;

pub(crate) type ActionListener =
    Box<dyn Fn(&dyn Any, DispatchPhase, &mut Window, &mut App) + 'static>;

/// 构建一个新的 [`Div`] 元素
#[track_caller]
pub fn div() -> Div {
    Div {
        interactivity: Interactivity::new(),
        children: SmallVec::default(),
        prepaint_listener: None,
        image_cache: None,
        prepaint_order_fn: None,
    }
}

/// [`Div`] 元素，用于在 RGPUI 中构建复杂 UI 的一体化元素
pub struct Div {
    interactivity: Interactivity,
    children: SmallVec<[StackSafe<AnyElement>; 2]>,
    prepaint_listener: Option<Box<dyn Fn(Vec<Bounds<Pixels>>, &mut Window, &mut App) + 'static>>,
    image_cache: Option<Box<dyn ImageCacheProvider>>,
    prepaint_order_fn: Option<Box<dyn Fn(&mut Window, &mut App) -> SmallVec<[usize; 8]>>>,
}

impl Div {
    /// 添加一个监听器，在此 `Div` 的子元素预绘制时被调用。
    /// 这允许你存储子元素的 [`Bounds`] 以供后续使用。
    pub fn on_children_prepainted(
        mut self,
        listener: impl Fn(Vec<Bounds<Pixels>>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.prepaint_listener = Some(Box::new(listener));
        self
    }

    /// 在此元素树中此 div 的位置添加图像缓存。
    pub fn image_cache(mut self, cache: impl ImageCacheProvider) -> Self {
        self.image_cache = Some(Box::new(cache));
        self
    }

    /// 指定一个函数来确定子元素的预绘制顺序。
    ///
    /// 该函数在预绘制时调用，应返回一个子元素索引向量，按所需的预绘制顺序排列。
    /// 每个索引应恰好出现一次。
    ///
    /// 当一个子元素的预绘制影响另一个子元素读取的状态时，这非常有用。
    /// 例如，在分割编辑器视图中，具有自动滚动请求的编辑器应先预绘制，
    /// 使其滚动位置更新对另一个编辑器可见。
    pub fn with_dynamic_prepaint_order(
        mut self,
        order_fn: impl Fn(&mut Window, &mut App) -> SmallVec<[usize; 8]> + 'static,
    ) -> Self {
        self.prepaint_order_fn = Some(Box::new(order_fn));
        self
    }
}

/// `Div` 元素的帧状态，包含其子元素的布局 ID。
///
/// 此结构体由 `Div` 元素内部使用，用于管理 UI 更新周期中子元素的布局状态。
/// 它持有一个小型 `LayoutId` 值向量，每个值对应 `Div` 的一个子元素。
/// 这些 ID 用于在布局阶段完成后查询布局引擎以获取子元素的计算边界。
pub struct DivFrameState {
    child_layout_ids: SmallVec<[LayoutId; 2]>,
}

/// 在检查器中显示和操作的交互状态。
#[derive(Clone)]
pub struct DivInspectorState {
    /// 被检查元素的基础样式。这用于检查和修改状态。将来应分离读写，
    /// 可能跟踪修改。
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub base_style: Box<StyleRefinement>,
    /// 检查元素的边界。
    pub bounds: Bounds<Pixels>,
    /// 元素子内容的大小，若无子元素则为 `bounds.size`。
    pub content_size: Size<Pixels>,
}

impl Styled for Div {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.interactivity.base_style
    }
}

impl InteractiveElement for Div {
    fn interactivity(&mut self) -> &mut Interactivity {
        &mut self.interactivity
    }
}

impl ParentElement for Div {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children
            .extend(elements.into_iter().map(StackSafe::new))
    }
}

impl Element for Div {
    type RequestLayoutState = DivFrameState;
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        self.interactivity.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.interactivity.source_location()
    }

    fn a11y_role(&self) -> Option<accesskit::Role> {
        // Nodes with `GenericContainer` should never be reported to accesskit.
        // Equivalent to an HTML div with no role.
        self.interactivity
            .override_role
            .filter(|role| *role != accesskit::Role::GenericContainer)
    }

    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        self.interactivity.write_a11y_info(node);
    }

    fn a11y_synthetic_children(
        &mut self,
        _prepaint: &mut Self::PrepaintState,
        builder: &mut crate::A11ySubtreeBuilder,
    ) {
        if let Some(f) = self.interactivity.a11y_synthetic_children.take() {
            f(builder);
        }
    }

    #[stacksafe]
    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut child_layout_ids = SmallVec::new();
        let image_cache = self
            .image_cache
            .as_mut()
            .map(|provider| provider.provide(window, cx));

        let layout_id = window.with_image_cache(image_cache, |window| {
            self.interactivity.request_layout(
                global_id,
                inspector_id,
                window,
                cx,
                |style, window, cx| {
                    window.with_text_style(style.text_style().cloned(), |window| {
                        child_layout_ids = self
                            .children
                            .iter_mut()
                            .map(|child| child.request_layout(window, cx))
                            .collect::<SmallVec<_>>();
                        window.request_layout(style, child_layout_ids.iter().copied(), cx)
                    })
                },
            )
        });

        (layout_id, DivFrameState { child_layout_ids })
    }

    #[stacksafe]
    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Hitbox> {
        let image_cache = self
            .image_cache
            .as_mut()
            .map(|provider| provider.provide(window, cx));

        let has_prepaint_listener = self.prepaint_listener.is_some();
        let mut children_bounds = Vec::with_capacity(if has_prepaint_listener {
            request_layout.child_layout_ids.len()
        } else {
            0
        });

        let mut child_min = point(Pixels::MAX, Pixels::MAX);
        let mut child_max = Point::default();
        if let Some(handle) = self.interactivity.scroll_anchor.as_ref() {
            *handle.last_origin.borrow_mut() = bounds.origin - window.element_offset();
        }
        let content_size = if request_layout.child_layout_ids.is_empty() {
            bounds.size
        } else if let Some(scroll_handle) = self.interactivity.tracked_scroll_handle.as_ref() {
            let mut state = scroll_handle.0.borrow_mut();
            state.child_bounds = Vec::with_capacity(request_layout.child_layout_ids.len());
            for child_layout_id in &request_layout.child_layout_ids {
                let child_bounds = window.layout_bounds(*child_layout_id);
                child_min = child_min.min(&child_bounds.origin);
                child_max = child_max.max(&child_bounds.bottom_right());
                state.child_bounds.push(child_bounds);
            }
            (child_max - child_min).into()
        } else {
            for child_layout_id in &request_layout.child_layout_ids {
                let child_bounds = window.layout_bounds(*child_layout_id);
                child_min = child_min.min(&child_bounds.origin);
                child_max = child_max.max(&child_bounds.bottom_right());

                if has_prepaint_listener {
                    children_bounds.push(child_bounds);
                }
            }
            (child_max - child_min).into()
        };

        if let Some(scroll_handle) = self.interactivity.tracked_scroll_handle.as_ref() {
            scroll_handle.scroll_to_active_item();
        }

        self.interactivity.prepaint(
            global_id,
            inspector_id,
            bounds,
            content_size,
            window,
            cx,
            |style, scroll_offset, hitbox, window, cx| {
                // skip children
                if style.display == Display::None {
                    return hitbox;
                }

                window.with_image_cache(image_cache, |window| {
                    // DOM 模式下由浏览器原生滚动处理：子元素按布局坐标（不叠加滚动偏移）
                    // 渲染，浏览器 `overflow:scroll` 容器负责真实滚动，滚动位置再经
                    // `scroll` 事件同步回 Rust 的 `ScrollHandle`。
                    #[cfg(feature = "dom-backend")]
                    let paint_offset = if window.dom_builder_active() {
                        Point::default()
                    } else {
                        scroll_offset
                    };
                    #[cfg(not(feature = "dom-backend"))]
                    let paint_offset = scroll_offset;
                    window.with_element_offset(paint_offset, |window| {
                        if let Some(order_fn) = &self.prepaint_order_fn {
                            let order = order_fn(window, cx);
                            for idx in order {
                                if let Some(child) = self.children.get_mut(idx) {
                                    child.prepaint(window, cx);
                                }
                            }
                        } else {
                            for child in &mut self.children {
                                child.prepaint(window, cx);
                            }
                        }
                    });

                    if let Some(listener) = self.prepaint_listener.as_ref() {
                        listener(children_bounds, window, cx);
                    }
                });

                hitbox
            },
        )
    }

    #[stacksafe]
    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        hitbox: &mut Option<Hitbox>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let image_cache = self
            .image_cache
            .as_mut()
            .map(|provider| provider.provide(window, cx));

        window.with_image_cache(image_cache, |window| {
            self.interactivity.paint(
                global_id,
                inspector_id,
                bounds,
                hitbox.as_ref(),
                window,
                cx,
                |style, window, cx| {
                    // skip children
                    if style.display == Display::None {
                        return;
                    }

                    for child in &mut self.children {
                        child.paint(window, cx);
                    }
                },
            )
        });
    }

    /// Web DOM 后端：把 Div 映射为一个绝对定位的 `<div>` 节点。
    ///
    /// 基于基础样式（`base_style` 解析后的 [`Style`]）与 Taffy 布局 bounds 生成。
    /// v1 不传 `global_id`/`hitbox`，因此 hover/focus/drag 等交互态样式不会反映到
    /// DOM 层（DOM 层尚未桥接指针事件，属已知限制，见 `docs/web-dom-backend-analysis.md`）。
    #[cfg(feature = "dom-backend")]
    fn dom(
        &self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<crate::DomNode> {
        use crate::{
            BackgroundTag, Corners, DomBoxShadow, DomDisplay, DomGradient, DomGradientKind,
            DomNode, DomNodeKind, DomOverflow, DomStyle, Overflow,
        };

        let style = self.interactivity.compute_style(None, None, window, cx);
        if style.visibility == Visibility::Hidden {
            return None;
        }

        let mut dom_style = DomStyle::from_bounds(bounds);

        // 布局由 Taffy 完成，DOM 只负责绝对定位呈现，因此 Block/Flex/Grid 统一映射为 block。
        dom_style.display = match style.display {
            crate::Display::None => DomDisplay::None,
            _ => DomDisplay::Block,
        };

        if let Some(fill) = style.background.as_ref()
            && let Some(background) = fill.color()
        {
            // 渐变背景：把 Background 的渐变数据映射为 DOM 渐变（v1 支持线性/径向/锥形）。
            match background.tag {
                BackgroundTag::Solid => {
                    if !background.solid.is_transparent() {
                        dom_style.background_color = Some(background.solid);
                    }
                }
                BackgroundTag::LinearGradient
                | BackgroundTag::RadialGradient
                | BackgroundTag::ConicGradient => {
                    let count = (background.stop_count as usize).clamp(1, 4);
                    let stops = background.colors[..count]
                        .iter()
                        .map(|stop| (stop.color, stop.percentage))
                        .collect();
                    let kind = match background.tag {
                        BackgroundTag::LinearGradient => DomGradientKind::Linear,
                        BackgroundTag::RadialGradient => DomGradientKind::Radial,
                        _ => DomGradientKind::Conic,
                    };
                    dom_style.background_gradient = Some(DomGradient {
                        kind,
                        angle: background.gradient_angle_or_pattern_height,
                        stops,
                    });
                }
                BackgroundTag::PatternSlash | BackgroundTag::Checkerboard => {
                    // 图案背景 v1 降级为纯色（浏览器无等效 CSS）。
                    if !background.solid.is_transparent() {
                        dom_style.background_color = Some(background.solid);
                    }
                }
            }
        }

        // 圆角：四角相等时映射为统一的 border-radius（不等时 v1 降级为 0）。
        let radii: Corners<crate::Pixels> = style.corner_radii.to_pixels(window.rem_size());
        if radii.top_left == radii.top_right
            && radii.top_right == radii.bottom_right
            && radii.bottom_right == radii.bottom_left
        {
            dom_style.border_radius = Some(radii.top_left);
        }

        // 边框：颜色 + 统一宽度（v1 不支持逐边宽度）。
        if let Some(color) = style.border_color {
            let widths = style.border_widths.to_pixels(window.rem_size());
            let max_width = widths.max();
            if max_width > Pixels::ZERO && !color.is_transparent() {
                dom_style.border_color = Some(color);
                dom_style.border_width = Some(max_width);
                dom_style.border_style = Some(style.border_style);
            }
        }

        // 盒阴影：映射为 CSS box-shadow（内/外阴影均支持）。
        if !style.box_shadow.is_empty() {
            dom_style.box_shadows = style
                .box_shadow
                .iter()
                .map(|shadow| DomBoxShadow {
                    color: shadow.color,
                    offset_x: shadow.offset.x,
                    offset_y: shadow.offset.y,
                    blur_radius: shadow.blur_radius,
                    spread_radius: shadow.spread_radius,
                    inset: shadow.inset,
                })
                .collect();
        }

        dom_style.opacity = style.opacity;
        dom_style.cursor = style.mouse_cursor;
        // 同时考虑 x/y 两个方向的溢出设置：任一方向为 `Scroll` 即视为可滚动容器
        // （DOM 侧用 `overflow:auto` 承载），否则 `overflow_y_scroll()` 等只设置单轴时
        // 因只看 `overflow.x` 而被错误映射成 `Visible`，导致 DOM 层无法原生滚动。
        dom_style.overflow = match (style.overflow.x, style.overflow.y) {
            (Overflow::Scroll, _) | (_, Overflow::Scroll) => DomOverflow::Scroll,
            (Overflow::Clip | Overflow::Hidden, _) | (_, Overflow::Clip | Overflow::Hidden) => {
                DomOverflow::Hidden
            }
            _ => DomOverflow::Visible,
        };

        // DOM 模式下，真正的「用户可滚动」容器（`overflow: scroll/auto`）携带 `ScrollHandle`，
        // 用于把浏览器原生滚动位置同步回 Rust（`crate::Window::dispatch_dom_scroll`），
        // 以及把程序化滚动推回 DOM。仅当样式为 `Overflow::Scroll` 时才挂载——输入框等仅用
        // `scroll_offset` 做光标自动滚动、或 `overflow:hidden` 裁剪的元素不在此列，避免每帧
        // 把 `ScrollHandle` 偏移写入 DOM 造成光标跳动/对话框错位。
        let scroll_handle = if dom_style.overflow == DomOverflow::Scroll {
            self.interactivity.tracked_scroll_handle.clone()
        } else {
            None
        };

        Some(DomNode {
            kind: DomNodeKind::Element {
                tag: "div",
                attrs: Vec::new(),
                children: Vec::new(),
            },
            style: dom_style,
            scroll_handle,
        })
    }
}

impl IntoElement for Div {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[derive(Default)]
pub(crate) struct AriaProperties {
    pub(crate) label: Option<SharedString>,
    pub(crate) description: Option<SharedString>,
    pub(crate) keyshortcuts: Option<SharedString>,
    pub(crate) selected: Option<bool>,
    pub(crate) expanded: Option<bool>,
    pub(crate) toggled: Option<accesskit::Toggled>,
    pub(crate) numeric_value: Option<f64>,
    pub(crate) min_numeric_value: Option<f64>,
    pub(crate) max_numeric_value: Option<f64>,
    pub(crate) numeric_value_step: Option<f64>,
    pub(crate) value: Option<SharedString>,
    pub(crate) placeholder: Option<SharedString>,
    pub(crate) orientation: Option<accesskit::Orientation>,
    pub(crate) level: Option<usize>,
    pub(crate) position_in_set: Option<usize>,
    pub(crate) size_of_set: Option<usize>,
    pub(crate) row_index: Option<usize>,
    pub(crate) column_index: Option<usize>,
    pub(crate) row_count: Option<usize>,
    pub(crate) column_count: Option<usize>,
}

/// 交互状态结构体。驱动 `Div` 元素中所有通用交互功能。
#[derive(Default)]
pub struct Interactivity {
    /// 元素的 ID。需要 ID 才能支持交互的有状态子集，如 on_click。
    pub element_id: Option<ElementId>,
    /// 元素是否被点击。仅在布局后存在。
    pub active: Option<bool>,
    /// 元素是否被悬停。仅在绘制后存在（如果为交互元素创建了 hitbox）。
    pub hovered: Option<bool>,
    pub(crate) tooltip_id: Option<TooltipId>,
    pub(crate) content_size: Size<Pixels>,
    pub(crate) key_context: Option<KeyContext>,
    pub(crate) focusable: bool,
    pub(crate) tracked_focus_handle: Option<FocusHandle>,
    pub(crate) tracked_scroll_handle: Option<ScrollHandle>,
    pub(crate) scroll_anchor: Option<ScrollAnchor>,
    pub(crate) scroll_offset: Option<Rc<RefCell<Point<Pixels>>>>,
    pub(crate) group: Option<SharedString>,
    /// 元素的基础样式，在聚焦、激活等修改应用之前。
    pub base_style: Box<StyleRefinement>,
    pub(crate) focus_style: Option<Box<StyleRefinement>>,
    pub(crate) in_focus_style: Option<Box<StyleRefinement>>,
    pub(crate) focus_visible_style: Option<Box<StyleRefinement>>,
    pub(crate) hover_style: Option<Box<StyleRefinement>>,
    pub(crate) group_hover_style: Option<GroupStyle>,
    pub(crate) active_style: Option<Box<StyleRefinement>>,
    pub(crate) group_active_style: Option<GroupStyle>,
    pub(crate) drag_over_styles: Vec<(
        TypeId,
        Box<dyn Fn(&dyn Any, &mut Window, &mut App) -> StyleRefinement>,
    )>,
    pub(crate) group_drag_over_styles: Vec<(TypeId, GroupStyle)>,
    pub(crate) mouse_down_listeners: Vec<MouseDownListener>,
    pub(crate) mouse_up_listeners: Vec<MouseUpListener>,
    pub(crate) mouse_pressure_listeners: Vec<MousePressureListener>,
    pub(crate) mouse_move_listeners: Vec<MouseMoveListener>,
    pub(crate) mouse_exit_listeners: Vec<MouseExitListener>,
    pub(crate) scroll_wheel_listeners: Vec<ScrollWheelListener>,
    pub(crate) pinch_listeners: Vec<PinchListener>,
    pub(crate) key_down_listeners: Vec<KeyDownListener>,
    pub(crate) key_up_listeners: Vec<KeyUpListener>,
    pub(crate) modifiers_changed_listeners: Vec<ModifiersChangedListener>,
    pub(crate) action_listeners: Vec<(TypeId, ActionListener)>,
    pub(crate) drop_listeners: Vec<(TypeId, DropListener)>,
    pub(crate) can_drop_predicate: Option<CanDropPredicate>,
    pub(crate) click_listeners: Vec<ClickListener>,
    pub(crate) aux_click_listeners: Vec<ClickListener>,
    pub(crate) drag_listener: Option<(Arc<dyn Any>, DragListener)>,
    pub(crate) hover_listener: Option<Box<dyn Fn(&bool, &mut Window, &mut App)>>,
    pub(crate) tooltip_builder: Option<TooltipBuilder>,
    pub(crate) tooltip_show_delay: Option<Duration>,
    pub(crate) window_control: Option<WindowControlArea>,
    pub(crate) hitbox_behavior: HitboxBehavior,
    pub(crate) tab_index: Option<isize>,
    pub(crate) tab_group: bool,
    pub(crate) tab_stop: bool,

    pub(crate) a11y_action_listeners:
        Vec<(accesskit::Action, crate::window::a11y::A11yActionListener)>,
    pub(crate) a11y_synthetic_children: Option<Box<dyn FnOnce(&mut crate::A11ySubtreeBuilder)>>,
    pub(crate) report_active_descendant_focus: bool,
    pub(crate) override_role: Option<accesskit::Role>,
    pub(crate) aria: AriaProperties,

    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(crate) source_location: Option<&'static core::panic::Location<'static>>,

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) debug_selector: Option<String>,
}

impl Interactivity {
    /// 根据此交互状态配置的样式布局此元素
    pub fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
        f: impl FnOnce(Style, &mut Window, &mut App) -> LayoutId,
    ) -> LayoutId {
        #[cfg(any(feature = "inspector", debug_assertions))]
        window.with_inspector_state(
            _inspector_id,
            cx,
            |inspector_state: &mut Option<DivInspectorState>, _window| {
                if let Some(inspector_state) = inspector_state {
                    self.base_style = inspector_state.base_style.clone();
                } else {
                    *inspector_state = Some(DivInspectorState {
                        base_style: self.base_style.clone(),
                        bounds: Default::default(),
                        content_size: Default::default(),
                    })
                }
            },
        );

        window.with_optional_element_state::<InteractiveElementState, _>(
            global_id,
            |element_state, window| {
                let mut element_state =
                    element_state.map(|element_state| element_state.unwrap_or_default());

                if let Some(element_state) = element_state.as_ref()
                    && cx.has_active_drag()
                {
                    if let Some(pending_mouse_down) = element_state.pending_mouse_down.as_ref() {
                        *pending_mouse_down.borrow_mut() = None;
                    }
                    if let Some(clicked_state) = element_state.clicked_state.as_ref() {
                        *clicked_state.borrow_mut() = ElementClickedState::default();
                    }
                }

                // Ensure we store a focus handle in our element state if we're focusable.
                // If there's an explicit focus handle we're tracking, use that. Otherwise
                // create a new handle and store it in the element state, which lives for as
                // as frames contain an element with this id.
                if self.focusable
                    && self.tracked_focus_handle.is_none()
                    && let Some(element_state) = element_state.as_mut()
                {
                    let mut handle = element_state
                        .focus_handle
                        .get_or_insert_with(|| cx.focus_handle())
                        .clone()
                        .tab_stop(self.tab_stop);

                    if let Some(index) = self.tab_index {
                        handle = handle.tab_index(index);
                    }

                    self.tracked_focus_handle = Some(handle);
                }

                if let Some(scroll_handle) = self.tracked_scroll_handle.as_ref() {
                    self.scroll_offset = Some(scroll_handle.0.borrow().offset.clone());
                } else if (self.base_style.overflow.x == Some(Overflow::Scroll)
                    || self.base_style.overflow.y == Some(Overflow::Scroll))
                    && let Some(element_state) = element_state.as_mut()
                {
                    self.scroll_offset = Some(
                        element_state
                            .scroll_offset
                            .get_or_insert_with(Rc::default)
                            .clone(),
                    );
                }

                let style = self.compute_style_internal(None, element_state.as_mut(), window, cx);
                let layout_id = f(style, window, cx);
                (layout_id, element_state)
            },
        )
    }

    /// 根据此交互状态配置的样式提交此元素的边界。
    pub fn prepaint<R>(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        content_size: Size<Pixels>,
        window: &mut Window,
        cx: &mut App,
        f: impl FnOnce(&Style, Point<Pixels>, Option<Hitbox>, &mut Window, &mut App) -> R,
    ) -> R {
        self.content_size = content_size;

        #[cfg(any(feature = "inspector", debug_assertions))]
        window.with_inspector_state(
            _inspector_id,
            cx,
            |inspector_state: &mut Option<DivInspectorState>, _window| {
                if let Some(inspector_state) = inspector_state {
                    inspector_state.bounds = bounds;
                    inspector_state.content_size = content_size;
                }
            },
        );

        if let Some(focus_handle) = self.tracked_focus_handle.as_ref() {
            window.set_focus_handle(focus_handle, cx);

            if window.a11y.is_active() {
                if let Some(global_id) = global_id {
                    let node_id = global_id.accesskit_node_id();
                    window.a11y.set_focusable(node_id, focus_handle.id);
                    if focus_handle.is_focused(window) {
                        window.a11y.set_focus(node_id);
                    }
                } else if focus_handle.is_focused(window) {
                    // Focusable, but with no element id it can't have an
                    // accessibility node, so screen readers fall back to the
                    // whole window.
                    window
                        .a11y
                        .note_focus_without_node(focus_handle.id, "it has no element id");
                }
            }
        }

        if self.report_active_descendant_focus && window.a11y.is_active() {
            if let Some(global_id) = global_id {
                window
                    .a11y
                    .set_active_descendant(global_id.accesskit_node_id());
            }
        }
        window.with_optional_element_state::<InteractiveElementState, _>(
            global_id,
            |element_state, window| {
                let mut element_state =
                    element_state.map(|element_state| element_state.unwrap_or_default());
                let style = self.compute_style_internal(None, element_state.as_mut(), window, cx);

                if let Some(element_state) = element_state.as_mut() {
                    if let Some(clicked_state) = element_state.clicked_state.as_ref() {
                        let clicked_state = clicked_state.borrow();
                        self.active = Some(clicked_state.element);
                    }
                    if self.hover_style.is_some() || self.group_hover_style.is_some() {
                        element_state
                            .hover_state
                            .get_or_insert_with(Default::default);
                    }
                    if let Some(active_tooltip) = element_state.active_tooltip.as_ref() {
                        if self.tooltip_builder.is_some() {
                            self.tooltip_id = set_tooltip_on_window(active_tooltip, window);
                        } else {
                            // If there is no longer a tooltip builder, remove the active tooltip.
                            element_state.active_tooltip.take();
                        }
                    }
                }

                window.with_text_style(style.text_style().cloned(), |window| {
                    window.with_content_mask(
                        style.overflow_mask(bounds, window.rem_size()),
                        |window| {
                            let hitbox = if self.should_insert_hitbox(&style, window, cx) {
                                Some(window.insert_hitbox(bounds, self.hitbox_behavior))
                            } else {
                                None
                            };

                            let scroll_offset =
                                self.clamp_scroll_position(bounds, &style, window, cx);
                            let result = f(&style, scroll_offset, hitbox, window, cx);
                            (result, element_state)
                        },
                    )
                })
            },
        )
    }

    fn should_insert_hitbox(&self, style: &Style, window: &Window, cx: &App) -> bool {
        self.hitbox_behavior != HitboxBehavior::Normal
            || self.window_control.is_some()
            || style.mouse_cursor.is_some()
            || self.group.is_some()
            || self.scroll_offset.is_some()
            || self.tracked_focus_handle.is_some()
            || self.hover_style.is_some()
            || self.group_hover_style.is_some()
            || self.hover_listener.is_some()
            || !self.mouse_up_listeners.is_empty()
            || !self.mouse_pressure_listeners.is_empty()
            || !self.mouse_down_listeners.is_empty()
            || !self.mouse_move_listeners.is_empty()
            || !self.mouse_exit_listeners.is_empty()
            || !self.click_listeners.is_empty()
            || !self.aux_click_listeners.is_empty()
            || !self.scroll_wheel_listeners.is_empty()
            || self.has_pinch_listeners()
            || self.drag_listener.is_some()
            || !self.drop_listeners.is_empty()
            || self.tooltip_builder.is_some()
            || window.is_inspector_picking(cx)
    }

    fn clamp_scroll_position(
        &self,
        bounds: Bounds<Pixels>,
        style: &Style,
        window: &mut Window,
        _cx: &mut App,
    ) -> Point<Pixels> {
        fn round_to_two_decimals(pixels: Pixels) -> Pixels {
            const ROUNDING_FACTOR: f32 = 100.0;
            (pixels * ROUNDING_FACTOR).round() / ROUNDING_FACTOR
        }

        if let Some(scroll_offset) = self.scroll_offset.as_ref() {
            let mut scroll_to_bottom = false;
            let mut tracked_scroll_handle = self
                .tracked_scroll_handle
                .as_ref()
                .map(|handle| handle.0.borrow_mut());
            if let Some(mut scroll_handle_state) = tracked_scroll_handle.as_deref_mut() {
                scroll_handle_state.overflow = style.overflow;
                scroll_to_bottom = mem::take(&mut scroll_handle_state.scroll_to_bottom);
            }

            let rem_size = window.rem_size();
            let padding = style.padding.to_pixels(bounds.size.into(), rem_size);
            let padding_size = size(padding.left + padding.right, padding.top + padding.bottom);
            // The floating point values produced by Taffy and ours often vary
            // slightly after ~5 decimal places. This can lead to cases where after
            // subtracting these, the container becomes scrollable for less than
            // 0.00000x pixels. As we generally don't benefit from a precision that
            // high for the maximum scroll, we round the scroll max to 2 decimal
            // places here.
            let padded_content_size = self.content_size + padding_size;
            let scroll_max = Point::from(padded_content_size - bounds.size)
                .map(round_to_two_decimals)
                .max(&Default::default());
            // Clamp scroll offset in case scroll max is smaller now (e.g., if children
            // were removed or the bounds became larger).
            let mut scroll_offset = scroll_offset.borrow_mut();

            scroll_offset.x = scroll_offset.x.clamp(-scroll_max.x, px(0.));
            if scroll_to_bottom {
                scroll_offset.y = -scroll_max.y;
            } else {
                scroll_offset.y = scroll_offset.y.clamp(-scroll_max.y, px(0.));
            }

            if let Some(mut scroll_handle_state) = tracked_scroll_handle {
                scroll_handle_state.max_offset = scroll_max;
                scroll_handle_state.bounds = bounds;
            }

            *scroll_offset
        } else {
            Point::default()
        }
    }

    /// 根据此交互状态配置的样式绘制此元素，并绑定元素的鼠标和键盘事件。
    ///
    /// content_size 是元素内容的大小，如果元素可滚动，可能大于元素的边界。
    ///
    /// 最终计算的样式将传递给提供的函数，以及当前的滚动偏移量。
    pub fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        hitbox: Option<&Hitbox>,
        window: &mut Window,
        cx: &mut App,
        f: impl FnOnce(&Style, &mut Window, &mut App),
    ) {
        self.hovered = hitbox.map(|hitbox| hitbox.is_hovered(window));
        window.with_optional_element_state::<InteractiveElementState, _>(
            global_id,
            |element_state, window| {
                let mut element_state =
                    element_state.map(|element_state| element_state.unwrap_or_default());

                let style = self.compute_style_internal(hitbox, element_state.as_mut(), window, cx);

                #[cfg(any(feature = "test-support", test))]
                if let Some(debug_selector) = &self.debug_selector {
                    window
                        .next_frame
                        .debug_bounds
                        .insert(debug_selector.clone(), bounds);
                }

                self.paint_hover_group_handler(window, cx);

                if style.visibility == Visibility::Hidden {
                    return ((), element_state);
                }

                let mut tab_group = None;
                if self.tab_group {
                    tab_group = self.tab_index;
                }

                window.with_element_opacity(style.opacity, |window| {
                    style.paint(bounds, window, cx, |window: &mut Window, cx: &mut App| {
                        window.with_text_style(style.text_style().cloned(), |window| {
                            window.with_content_mask(
                                style.overflow_mask(bounds, window.rem_size()),
                                |window| {
                                    window.with_tab_group(tab_group, |window| {
                                        // Register the container's own focus handle *inside* its
                                        // tab group, so that focusing the container and then
                                        // calling `focus_next` descends into this group's first
                                        // item. Inserting it before `with_tab_group` would give the
                                        // container a shallower tab path than its children; with
                                        // sibling groups every container would then sort ahead of
                                        // every item, and `focus_next` from a container would jump
                                        // to the first item in the whole window instead of its own.
                                        if let Some(focus_handle) = &self.tracked_focus_handle {
                                            window.next_frame.tab_stops.insert(focus_handle);
                                        }
                                        if let Some(hitbox) = hitbox {
                                            #[cfg(debug_assertions)]
                                            self.paint_debug_info(
                                                global_id, hitbox, &style, window, cx,
                                            );

                                            if let Some(drag) = cx.active_drag.as_ref() {
                                                if let Some(mouse_cursor) = drag.cursor_style {
                                                    window.set_window_cursor_style(mouse_cursor);
                                                }
                                            } else {
                                                if let Some(mouse_cursor) = style.mouse_cursor {
                                                    window.set_cursor_style(mouse_cursor, hitbox);
                                                }
                                            }

                                            if let Some(group) = self.group.clone() {
                                                GroupHitboxes::push(group, hitbox.id, cx);
                                            }

                                            if let Some(area) = self.window_control {
                                                window.insert_window_control_hitbox(
                                                    area,
                                                    hitbox.clone(),
                                                );
                                            }

                                            self.paint_mouse_listeners(
                                                hitbox,
                                                element_state.as_mut(),
                                                window,
                                                cx,
                                            );
                                            self.paint_scroll_listener(hitbox, &style, window, cx);
                                        }

                                        self.paint_keyboard_listeners(window, cx);

                                        if window.a11y.is_active() {
                                            if let Some(global_id) = global_id {
                                                if !self.a11y_action_listeners.is_empty() {
                                                    let node_id = global_id.accesskit_node_id();
                                                    for (action, listener) in
                                                        self.a11y_action_listeners.drain(..)
                                                    {
                                                        window.on_a11y_action(
                                                            node_id, action, listener,
                                                        );
                                                    }
                                                }
                                            }
                                        }

                                        f(&style, window, cx);

                                        if let Some(_hitbox) = hitbox {
                                            #[cfg(any(feature = "inspector", debug_assertions))]
                                            window.insert_inspector_hitbox(
                                                _hitbox.id,
                                                _inspector_id,
                                                cx,
                                            );

                                            if let Some(group) = self.group.as_ref() {
                                                GroupHitboxes::pop(group, cx);
                                            }
                                        }
                                    })
                                },
                            );
                        });
                    });
                });

                ((), element_state)
            },
        );
    }

    #[cfg(debug_assertions)]
    fn paint_debug_info(
        &self,
        global_id: Option<&GlobalElementId>,
        hitbox: &Hitbox,
        style: &Style,
        window: &mut Window,
        cx: &mut App,
    ) {
        use crate::{BorderStyle, TextAlign};

        if let Some(global_id) = global_id
            && (style.debug || style.debug_below || cx.has_global::<crate::DebugBelow>())
            && hitbox.is_hovered(window)
        {
            const FONT_SIZE: crate::Pixels = crate::Pixels(10.);
            let element_id = format!("{global_id:?}");
            let str_len = element_id.len();

            let render_debug_text = |window: &mut Window| {
                if let Some(text) = window
                    .text_system()
                    .shape_text(
                        element_id.into(),
                        FONT_SIZE,
                        &[window.text_style().to_run(str_len)],
                        None,
                        None,
                    )
                    .ok()
                    .and_then(|mut text| text.pop())
                {
                    text.paint(hitbox.origin, FONT_SIZE, TextAlign::Left, None, window, cx)
                        .ok();

                    let text_bounds = crate::Bounds {
                        origin: hitbox.origin,
                        size: text.size(FONT_SIZE),
                    };
                    if let Some(source_location) = self.source_location
                        && text_bounds.contains(&window.mouse_position())
                        && window.modifiers().secondary()
                    {
                        let secondary_held = window.modifiers().secondary();
                        window.on_key_event({
                            move |e: &crate::ModifiersChangedEvent, _phase, window, _cx| {
                                if e.modifiers.secondary() != secondary_held
                                    && text_bounds.contains(&window.mouse_position())
                                {
                                    window.refresh();
                                }
                            }
                        });

                        let was_hovered = hitbox.is_hovered(window);
                        let current_view = window.current_view();
                        window.on_mouse_event({
                            let hitbox = hitbox.clone();
                            move |_: &MouseMoveEvent, phase, window, cx| {
                                if phase == DispatchPhase::Capture {
                                    let hovered = hitbox.is_hovered(window);
                                    if hovered != was_hovered {
                                        cx.notify(current_view)
                                    }
                                }
                            }
                        });

                        window.on_mouse_event({
                            let hitbox = hitbox.clone();
                            move |e: &crate::MouseDownEvent, phase, window, cx| {
                                if text_bounds.contains(&e.position)
                                    && phase.capture()
                                    && hitbox.is_hovered(window)
                                {
                                    cx.stop_propagation();
                                    let Ok(dir) = std::env::current_dir() else {
                                        return;
                                    };

                                    eprintln!(
                                        "This element was created at:\n{}:{}:{}",
                                        dir.join(source_location.file()).to_string_lossy(),
                                        source_location.line(),
                                        source_location.column()
                                    );
                                }
                            }
                        });
                        window.paint_quad(crate::outline(
                            crate::Bounds {
                                origin: hitbox.origin
                                    + crate::point(crate::px(0.), FONT_SIZE - px(2.)),
                                size: crate::Size {
                                    width: text_bounds.size.width,
                                    height: crate::px(1.),
                                },
                            },
                            crate::red(),
                            BorderStyle::default(),
                        ))
                    }
                }
            };

            window.with_text_style(
                Some(crate::TextStyleRefinement {
                    color: Some(crate::red()),
                    line_height: Some(FONT_SIZE.into()),
                    background_color: Some(crate::white()),
                    ..Default::default()
                }),
                render_debug_text,
            )
        }
    }

    fn paint_mouse_listeners(
        &mut self,
        hitbox: &Hitbox,
        element_state: Option<&mut InteractiveElementState>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let is_focused = self
            .tracked_focus_handle
            .as_ref()
            .map(|handle| handle.is_focused(window))
            .unwrap_or(false);

        // If this element can be focused, register a mouse down listener
        // that will automatically transfer focus when hitting the element.
        // This behavior can be suppressed by using `cx.prevent_default()`.
        if let Some(focus_handle) = self.tracked_focus_handle.clone() {
            let hitbox = hitbox.clone();
            window.on_mouse_event(move |_: &MouseDownEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble
                    && hitbox.is_hovered(window)
                    && !window.default_prevented()
                {
                    window.focus(&focus_handle, cx);
                    // If there is a parent that is also focusable, prevent it
                    // from transferring focus because we already did so.
                    window.prevent_default();
                }
            });
        }

        for listener in self.mouse_down_listeners.drain(..) {
            let hitbox = hitbox.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                listener(event, phase, &hitbox, window, cx);
            })
        }

        for listener in self.mouse_up_listeners.drain(..) {
            let hitbox = hitbox.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                listener(event, phase, &hitbox, window, cx);
            })
        }

        for listener in self.mouse_pressure_listeners.drain(..) {
            let hitbox = hitbox.clone();
            window.on_mouse_event(move |event: &MousePressureEvent, phase, window, cx| {
                listener(event, phase, &hitbox, window, cx);
            })
        }

        for listener in self.mouse_move_listeners.drain(..) {
            let hitbox = hitbox.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                listener(event, phase, &hitbox, window, cx);
            })
        }

        for listener in self.mouse_exit_listeners.drain(..) {
            let hitbox = hitbox.clone();
            window.on_mouse_event(move |event: &MouseExitEvent, phase, window, cx| {
                listener(event, phase, &hitbox, window, cx);
            })
        }

        for listener in self.scroll_wheel_listeners.drain(..) {
            let hitbox = hitbox.clone();
            window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
                listener(event, phase, &hitbox, window, cx);
            })
        }

        for listener in self.pinch_listeners.drain(..) {
            let hitbox = hitbox.clone();
            window.on_mouse_event(move |event: &PinchEvent, phase, window, cx| {
                listener(event, phase, &hitbox, window, cx);
            })
        }

        if self.hover_style.is_some()
            || self.base_style.mouse_cursor.is_some()
            || cx.active_drag.is_some() && !self.drag_over_styles.is_empty()
        {
            let hitbox = hitbox.clone();
            let hover_state = self.hover_style.as_ref().and_then(|_| {
                element_state
                    .as_ref()
                    .and_then(|state| state.hover_state.as_ref())
                    .cloned()
            });
            let current_view = window.current_view();

            window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, cx| {
                let hovered = hitbox.is_hovered(window);
                let was_hovered = hover_state
                    .as_ref()
                    .is_some_and(|state| state.borrow().element);
                if phase == DispatchPhase::Capture && hovered != was_hovered {
                    if let Some(hover_state) = &hover_state {
                        hover_state.borrow_mut().element = hovered;
                        cx.notify(current_view);
                    }
                }
            });
        }

        if let Some(group_hover) = self.group_hover_style.as_ref() {
            if let Some(group_hitbox_id) = GroupHitboxes::get(&group_hover.group, cx) {
                let hover_state = element_state
                    .as_ref()
                    .and_then(|element| element.hover_state.as_ref())
                    .cloned();
                let current_view = window.current_view();

                window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, cx| {
                    let group_hovered = group_hitbox_id.is_hovered(window);
                    let was_group_hovered = hover_state
                        .as_ref()
                        .is_some_and(|state| state.borrow().group);
                    if phase == DispatchPhase::Capture && group_hovered != was_group_hovered {
                        if let Some(hover_state) = &hover_state {
                            hover_state.borrow_mut().group = group_hovered;
                        }
                        cx.notify(current_view);
                    }
                });
            }
        }

        let drag_cursor_style = self.base_style.as_ref().mouse_cursor;

        let mut drag_listener = mem::take(&mut self.drag_listener);
        let drop_listeners = mem::take(&mut self.drop_listeners);
        let click_listeners = mem::take(&mut self.click_listeners);
        let aux_click_listeners = mem::take(&mut self.aux_click_listeners);
        let can_drop_predicate = mem::take(&mut self.can_drop_predicate);

        if !drop_listeners.is_empty() {
            let hitbox = hitbox.clone();
            window.on_mouse_event({
                move |_: &MouseUpEvent, phase, window, cx| {
                    if let Some(drag) = &cx.active_drag
                        && phase == DispatchPhase::Bubble
                        && hitbox.is_hovered(window)
                    {
                        let drag_state_type = drag.value.as_ref().type_id();
                        for (drop_state_type, listener) in &drop_listeners {
                            if *drop_state_type == drag_state_type {
                                let drag = cx
                                    .active_drag
                                    .take()
                                    .expect("checked for type drag state type above");

                                let mut can_drop = true;
                                if let Some(predicate) = &can_drop_predicate {
                                    can_drop = predicate(drag.value.as_ref(), window, cx);
                                }

                                if can_drop {
                                    listener(drag.value.as_ref(), window, cx);
                                    window.refresh();
                                    cx.stop_propagation();
                                }
                            }
                        }
                    }
                }
            });
        }

        if let Some(element_state) = element_state {
            if !click_listeners.is_empty()
                || !aux_click_listeners.is_empty()
                || drag_listener.is_some()
            {
                let pending_mouse_down = element_state
                    .pending_mouse_down
                    .get_or_insert_with(Default::default)
                    .clone();

                let pending_keyboard_down = element_state
                    .pending_keyboard_down
                    .get_or_insert_with(Default::default)
                    .clone();

                let clicked_state = element_state
                    .clicked_state
                    .get_or_insert_with(Default::default)
                    .clone();

                window.on_mouse_event({
                    let pending_mouse_down = pending_mouse_down.clone();
                    let hitbox = hitbox.clone();
                    let has_aux_click_listeners = !aux_click_listeners.is_empty();
                    move |event: &MouseDownEvent, phase, window, _cx| {
                        if phase == DispatchPhase::Bubble
                            && (event.button == MouseButton::Left || has_aux_click_listeners)
                            && hitbox.is_hovered(window)
                        {
                            *pending_mouse_down.borrow_mut() = Some(event.clone());
                            window.refresh();
                        }
                    }
                });

                window.on_mouse_event({
                    let pending_mouse_down = pending_mouse_down.clone();
                    let hitbox = hitbox.clone();
                    move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Capture {
                            return;
                        }

                        let mut pending_mouse_down = pending_mouse_down.borrow_mut();
                        if let Some(mouse_down) = pending_mouse_down.clone()
                            && !cx.has_active_drag()
                            && (event.position - mouse_down.position).magnitude() > DRAG_THRESHOLD
                            && let Some((drag_value, drag_listener)) = drag_listener.take()
                            && mouse_down.button == MouseButton::Left
                        {
                            *clicked_state.borrow_mut() = ElementClickedState::default();
                            let cursor_offset = event.position - hitbox.origin;
                            let drag =
                                (drag_listener)(drag_value.as_ref(), cursor_offset, window, cx);
                            cx.active_drag = Some(AnyDrag {
                                view: drag,
                                value: drag_value,
                                cursor_offset,
                                cursor_style: drag_cursor_style,
                            });
                            pending_mouse_down.take();
                            window.refresh();
                            cx.stop_propagation();
                        }
                    }
                });

                if is_focused {
                    // Record the focus generation at which an enter/space key
                    // down event happened on this element. The next key up
                    // event will be mapped to a click event if both of the
                    // following are true:
                    // - no other key events happen in between
                    // - the focus generation is the same (implying focus did not move)
                    //
                    // This design avoids an ABA problem that happens if you
                    // store the focus handle that registered the keypress.
                    window.on_key_event({
                        let pending_keyboard_down = pending_keyboard_down.clone();
                        move |event: &KeyDownEvent, phase, window, _cx| {
                            if phase.bubble() && !window.default_prevented() {
                                let stroke = &event.keystroke;
                                let is_activation_key = (stroke.key.eq("enter")
                                    || stroke.key.eq("space"))
                                    && !stroke.modifiers.modified();
                                *pending_keyboard_down.borrow_mut() =
                                    is_activation_key.then_some(window.focus_generation);
                            }
                        }
                    });

                    // Press enter, space to trigger click, when the element is focused.
                    window.on_key_event({
                        let click_listeners = click_listeners.clone();
                        let hitbox = hitbox.clone();
                        move |event: &KeyUpEvent, phase, window, cx| {
                            if phase.bubble() && !window.default_prevented() {
                                let stroke = &event.keystroke;
                                let keyboard_button = if stroke.key.eq("enter") {
                                    Some(KeyboardButton::Enter)
                                } else if stroke.key.eq("space") {
                                    Some(KeyboardButton::Space)
                                } else {
                                    None
                                };

                                if let Some(button) = keyboard_button
                                    && !stroke.modifiers.modified()
                                {
                                    let pending =
                                        std::mem::take(&mut *pending_keyboard_down.borrow_mut());
                                    if pending != Some(window.focus_generation) {
                                        return;
                                    }

                                    let click_event = ClickEvent::Keyboard(KeyboardClickEvent {
                                        button,
                                        bounds: hitbox.bounds,
                                    });

                                    for listener in &click_listeners {
                                        listener(&click_event, window, cx);
                                    }
                                } else {
                                    // Releasing any other key mid-press means
                                    // this isn't a clean activation, so cancel
                                    // the pending keydown.
                                    *pending_keyboard_down.borrow_mut() = None;
                                }
                            }
                        }
                    });
                }

                window.on_mouse_event({
                    let mut captured_mouse_down = None;
                    let hitbox = hitbox.clone();
                    move |event: &MouseUpEvent, phase, window, cx| match phase {
                        // Clear the pending mouse down during the capture phase,
                        // so that it happens even if another event handler stops
                        // propagation.
                        DispatchPhase::Capture => {
                            let mut pending_mouse_down = pending_mouse_down.borrow_mut();
                            if pending_mouse_down.is_some() && hitbox.is_hovered(window) {
                                captured_mouse_down = pending_mouse_down.take();
                                window.refresh();
                            } else if pending_mouse_down.is_some() {
                                // Clear the pending mouse down event (without firing click handlers)
                                // if the hitbox is not being hovered.
                                // This avoids dragging elements that changed their position
                                // immediately after being clicked.
                                // See https://github.com/zed-industries/zed/issues/24600 for more details
                                pending_mouse_down.take();
                                window.refresh();
                            }
                        }
                        // Fire click handlers during the bubble phase.
                        DispatchPhase::Bubble => {
                            if let Some(mouse_down) = captured_mouse_down.take() {
                                let btn = mouse_down.button;

                                let mouse_click = ClickEvent::Mouse(MouseClickEvent {
                                    down: mouse_down,
                                    up: event.clone(),
                                });

                                match btn {
                                    MouseButton::Left => {
                                        for listener in &click_listeners {
                                            listener(&mouse_click, window, cx);
                                        }
                                    }
                                    _ => {
                                        for listener in &aux_click_listeners {
                                            listener(&mouse_click, window, cx);
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
            }

            if let Some(hover_listener) = self.hover_listener.take() {
                let was_hovered = element_state
                    .hover_listener_state
                    .get_or_insert_with(Default::default)
                    .clone();
                let has_mouse_down = element_state
                    .pending_mouse_down
                    .get_or_insert_with(Default::default)
                    .clone();
                let hover_listener = Rc::new(hover_listener);
                let hover_listener_state = was_hovered.clone();
                let update_hover = move |is_hovered: bool, window: &mut Window, cx: &mut App| {
                    let mut was_hovered = hover_listener_state.borrow_mut();
                    if is_hovered != *was_hovered {
                        *was_hovered = is_hovered;
                        drop(was_hovered);
                        hover_listener(&is_hovered, window, cx);
                    }
                };

                if has_mouse_down.borrow().is_none() {
                    let is_hovered = !cx.has_active_drag() && hitbox.is_hovered(window);
                    if is_hovered != *was_hovered.borrow() {
                        let update_hover = update_hover.clone();
                        window.defer(cx, move |window, cx| {
                            update_hover(is_hovered, window, cx);
                        });
                    }
                }

                window.on_mouse_event({
                    let update_hover = update_hover.clone();
                    let hitbox = hitbox.clone();
                    move |_: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble {
                            let is_hovered = has_mouse_down.borrow().is_none()
                                && !cx.has_active_drag()
                                && hitbox.is_hovered(window);
                            update_hover(is_hovered, window, cx);
                        }
                    }
                });

                // The pointer can leave the window without a final MouseMove, so also
                // clear hover on MouseExited.
                window.on_mouse_event(move |_: &MouseExitEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble {
                        update_hover(false, window, cx);
                    }
                });
            }

            if let Some(tooltip_builder) = self.tooltip_builder.take() {
                let active_tooltip = element_state
                    .active_tooltip
                    .get_or_insert_with(Default::default)
                    .clone();
                let pending_mouse_down = element_state
                    .pending_mouse_down
                    .get_or_insert_with(Default::default)
                    .clone();

                let tooltip_is_hoverable = tooltip_builder.hoverable;
                let build_tooltip = Rc::new(move |window: &mut Window, cx: &mut App| {
                    Some(((tooltip_builder.build)(window, cx), tooltip_is_hoverable))
                });
                // Use bounds instead of testing hitbox since this is called during prepaint.
                let check_is_hovered_during_prepaint = Rc::new({
                    let pending_mouse_down = pending_mouse_down.clone();
                    let source_bounds = hitbox.bounds;
                    move |window: &Window| {
                        !window.last_input_was_keyboard()
                            && pending_mouse_down.borrow().is_none()
                            && source_bounds.contains(&window.mouse_position())
                    }
                });
                let check_is_hovered = Rc::new({
                    let hitbox = hitbox.clone();
                    move |window: &Window| {
                        pending_mouse_down.borrow().is_none() && hitbox.is_hovered(window)
                    }
                });
                register_tooltip_mouse_handlers(
                    &active_tooltip,
                    self.tooltip_id,
                    build_tooltip,
                    check_is_hovered,
                    check_is_hovered_during_prepaint,
                    self.tooltip_show_delay,
                    window,
                );
            }

            // We unconditionally bind both the mouse up and mouse down active state handlers
            // Because we might not get a chance to render a frame before the mouse up event arrives.
            let active_state = element_state
                .clicked_state
                .get_or_insert_with(Default::default)
                .clone();

            {
                let active_state = active_state.clone();
                window.on_mouse_event(move |_: &MouseUpEvent, phase, window, _cx| {
                    if phase == DispatchPhase::Capture && active_state.borrow().is_clicked() {
                        *active_state.borrow_mut() = ElementClickedState::default();
                        window.refresh();
                    }
                });
            }

            {
                let active_group_hitbox = self
                    .group_active_style
                    .as_ref()
                    .and_then(|group_active| GroupHitboxes::get(&group_active.group, cx));
                let hitbox = hitbox.clone();
                window.on_mouse_event(move |_: &MouseDownEvent, phase, window, _cx| {
                    if phase == DispatchPhase::Bubble && !window.default_prevented() {
                        let group_hovered = active_group_hitbox
                            .is_some_and(|group_hitbox_id| group_hitbox_id.is_hovered(window));
                        let element_hovered = hitbox.is_hovered(window);
                        if group_hovered || element_hovered {
                            *active_state.borrow_mut() = ElementClickedState {
                                group: group_hovered,
                                element: element_hovered,
                            };
                            window.refresh();
                        }
                    }
                });
            }
        }
    }

    fn paint_keyboard_listeners(&mut self, window: &mut Window, _cx: &mut App) {
        let key_down_listeners = mem::take(&mut self.key_down_listeners);
        let key_up_listeners = mem::take(&mut self.key_up_listeners);
        let modifiers_changed_listeners = mem::take(&mut self.modifiers_changed_listeners);
        let action_listeners = mem::take(&mut self.action_listeners);
        if let Some(context) = self.key_context.clone() {
            window.set_key_context(context);
        }

        for listener in key_down_listeners {
            window.on_key_event(move |event: &KeyDownEvent, phase, window, cx| {
                listener(event, phase, window, cx);
            })
        }

        for listener in key_up_listeners {
            window.on_key_event(move |event: &KeyUpEvent, phase, window, cx| {
                listener(event, phase, window, cx);
            })
        }

        for listener in modifiers_changed_listeners {
            window.on_modifiers_changed(move |event: &ModifiersChangedEvent, window, cx| {
                listener(event, window, cx);
            })
        }

        for (action_type, listener) in action_listeners {
            window.on_action(action_type, listener)
        }
    }

    fn paint_hover_group_handler(&self, window: &mut Window, cx: &mut App) {
        let group_hitbox = self
            .group_hover_style
            .as_ref()
            .and_then(|group_hover| GroupHitboxes::get(&group_hover.group, cx));

        if let Some(group_hitbox) = group_hitbox {
            let was_hovered = group_hitbox.is_hovered(window);
            let current_view = window.current_view();
            window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, cx| {
                let hovered = group_hitbox.is_hovered(window);
                if phase == DispatchPhase::Capture && hovered != was_hovered {
                    cx.notify(current_view);
                }
            });
        }
    }

    fn paint_scroll_listener(
        &self,
        hitbox: &Hitbox,
        style: &Style,
        window: &mut Window,
        _cx: &mut App,
    ) {
        if let Some(scroll_offset) = self.scroll_offset.clone() {
            let overflow = style.overflow;
            let allow_concurrent_scroll = style.allow_concurrent_scroll;
            let restrict_scroll_to_axis = style.restrict_scroll_to_axis;
            let line_height = window.line_height();
            let hitbox = hitbox.clone();
            let current_view = window.current_view();
            window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble && hitbox.should_handle_scroll(window) {
                    let mut scroll_offset = scroll_offset.borrow_mut();
                    let old_scroll_offset = *scroll_offset;
                    let delta = event.delta.pixel_delta(line_height);

                    let mut delta_x = Pixels::ZERO;
                    if overflow.x == Overflow::Scroll {
                        if !delta.x.is_zero() {
                            delta_x = delta.x;
                        } else if !restrict_scroll_to_axis && overflow.y != Overflow::Scroll {
                            delta_x = delta.y;
                        }
                    }
                    let mut delta_y = Pixels::ZERO;
                    if overflow.y == Overflow::Scroll {
                        if !delta.y.is_zero() {
                            delta_y = delta.y;
                        } else if !restrict_scroll_to_axis && overflow.x != Overflow::Scroll {
                            delta_y = delta.x;
                        }
                    }
                    if !allow_concurrent_scroll && !delta_x.is_zero() && !delta_y.is_zero() {
                        if delta_x.abs() > delta_y.abs() {
                            delta_y = Pixels::ZERO;
                        } else {
                            delta_x = Pixels::ZERO;
                        }
                    }
                    scroll_offset.y += delta_y;
                    scroll_offset.x += delta_x;
                    if *scroll_offset != old_scroll_offset {
                        cx.notify(current_view);
                    }
                }
            });
        }
    }

    /// 根据当前边界和元素状态计算此元素的视觉样式。
    pub fn compute_style(
        &self,
        global_id: Option<&GlobalElementId>,
        hitbox: Option<&Hitbox>,
        window: &mut Window,
        cx: &mut App,
    ) -> Style {
        window.with_optional_element_state(global_id, |element_state, window| {
            let mut element_state =
                element_state.map(|element_state| element_state.unwrap_or_default());
            let style = self.compute_style_internal(hitbox, element_state.as_mut(), window, cx);
            (style, element_state)
        })
    }

    /// 从已调用 with_element_state 的内部方法中调用。
    fn compute_style_internal(
        &self,
        hitbox: Option<&Hitbox>,
        element_state: Option<&mut InteractiveElementState>,
        window: &mut Window,
        cx: &mut App,
    ) -> Style {
        let mut style = Style::default();
        style.refine(&self.base_style);

        if let Some(focus_handle) = self.tracked_focus_handle.as_ref() {
            if let Some(in_focus_style) = self.in_focus_style.as_ref()
                && focus_handle.within_focused(window, cx)
            {
                style.refine(in_focus_style);
            }

            if let Some(focus_style) = self.focus_style.as_ref()
                && focus_handle.is_focused(window)
            {
                style.refine(focus_style);
            }

            if let Some(focus_visible_style) = self.focus_visible_style.as_ref()
                && focus_handle.is_focused(window)
                && window.last_input_was_keyboard()
            {
                style.refine(focus_visible_style);
            }
        }

        if !cx.has_active_drag() {
            if let Some(group_hover) = self.group_hover_style.as_ref() {
                let is_group_hovered =
                    if let Some(group_hitbox_id) = GroupHitboxes::get(&group_hover.group, cx) {
                        group_hitbox_id.is_hovered(window)
                    } else if let Some(element_state) = element_state.as_ref() {
                        element_state
                            .hover_state
                            .as_ref()
                            .map(|state| state.borrow().group)
                            .unwrap_or(false)
                    } else {
                        false
                    };

                if is_group_hovered {
                    style.refine(&group_hover.style);
                }
            }

            if let Some(hover_style) = self.hover_style.as_ref() {
                let is_hovered = if let Some(hitbox) = hitbox {
                    hitbox.is_hovered(window)
                } else if let Some(element_state) = element_state.as_ref() {
                    element_state
                        .hover_state
                        .as_ref()
                        .map(|state| state.borrow().element)
                        .unwrap_or(false)
                } else {
                    false
                };

                if is_hovered {
                    style.refine(hover_style);
                }
            }
        }

        if let Some(hitbox) = hitbox {
            if let Some(drag) = cx.active_drag.take() {
                let mut can_drop = true;
                if let Some(can_drop_predicate) = &self.can_drop_predicate {
                    can_drop = can_drop_predicate(drag.value.as_ref(), window, cx);
                }

                if can_drop {
                    for (state_type, group_drag_style) in &self.group_drag_over_styles {
                        if let Some(group_hitbox_id) =
                            GroupHitboxes::get(&group_drag_style.group, cx)
                            && *state_type == drag.value.as_ref().type_id()
                            && group_hitbox_id.is_hovered(window)
                        {
                            style.refine(&group_drag_style.style);
                        }
                    }

                    for (state_type, build_drag_over_style) in &self.drag_over_styles {
                        if *state_type == drag.value.as_ref().type_id() && hitbox.is_hovered(window)
                        {
                            style.refine(&build_drag_over_style(drag.value.as_ref(), window, cx));
                        }
                    }
                }

                style.mouse_cursor = drag.cursor_style;
                cx.active_drag = Some(drag);
            }
        }

        if let Some(element_state) = element_state {
            let clicked_state = element_state
                .clicked_state
                .get_or_insert_with(Default::default)
                .borrow();
            if clicked_state.group
                && let Some(group) = self.group_active_style.as_ref()
            {
                style.refine(&group.style)
            }

            if let Some(active_style) = self.active_style.as_ref()
                && clicked_state.element
            {
                style.refine(active_style)
            }
        }

        style
    }

    pub(crate) fn write_a11y_info(&self, node: &mut accesskit::Node) {
        if let Some(label) = &self.aria.label {
            node.set_label(label.to_string());
        }
        if let Some(description) = &self.aria.description {
            node.set_description(description.to_string());
        }
        if let Some(keyshortcuts) = &self.aria.keyshortcuts {
            node.set_keyboard_shortcut(keyshortcuts.to_string());
        }
        if let Some(selected) = self.aria.selected {
            node.set_selected(selected);
        }
        if let Some(expanded) = self.aria.expanded {
            node.set_expanded(expanded);
        }
        if let Some(toggled) = self.aria.toggled {
            node.set_toggled(toggled);
        }
        if let Some(value) = self.aria.numeric_value {
            node.set_numeric_value(value);
        }
        if let Some(value) = self.aria.min_numeric_value {
            node.set_min_numeric_value(value);
        }
        if let Some(value) = self.aria.max_numeric_value {
            node.set_max_numeric_value(value);
        }
        if let Some(step) = self.aria.numeric_value_step {
            node.set_numeric_value_step(step);
        }
        if let Some(value) = &self.aria.value {
            node.set_value(value.to_string());
        }
        if let Some(placeholder) = &self.aria.placeholder {
            node.set_placeholder(placeholder.to_string());
        }
        if let Some(orientation) = self.aria.orientation {
            node.set_orientation(orientation);
        }
        if let Some(level) = self.aria.level {
            node.set_level(level);
        }
        if let Some(position) = self.aria.position_in_set {
            node.set_position_in_set(position);
        }
        if let Some(size) = self.aria.size_of_set {
            node.set_size_of_set(size);
        }
        if let Some(index) = self.aria.row_index {
            node.set_row_index(index);
        }
        if let Some(index) = self.aria.column_index {
            node.set_column_index(index);
        }
        if let Some(count) = self.aria.row_count {
            node.set_row_count(count);
        }
        if let Some(count) = self.aria.column_count {
            node.set_column_count(count);
        }
        if !self.click_listeners.is_empty() {
            node.add_action(accesskit::Action::Click);
        }
        if self.tracked_focus_handle.is_some() || self.focusable {
            node.add_action(accesskit::Action::Focus);
        }
        for (action, _) in &self.a11y_action_listeners {
            node.add_action(*action);
        }
    }
}

/// 交互元素的每帧状态。用于跟踪有状态交互，如点击和滚动偏移量。
#[derive(Default)]
pub struct InteractiveElementState {
    pub(crate) focus_handle: Option<FocusHandle>,
    pub(crate) clicked_state: Option<Rc<RefCell<ElementClickedState>>>,
    pub(crate) hover_state: Option<Rc<RefCell<ElementHoverState>>>,
    pub(crate) hover_listener_state: Option<Rc<RefCell<bool>>>,
    pub(crate) pending_mouse_down: Option<Rc<RefCell<Option<MouseDownEvent>>>>,
    /// 当此元素聚焦时收到 Enter/Space 按键按下时，设置为窗口的
    /// [`focus_generation`](crate::Window::focus_generation)，记录我们正在
    /// 等待匹配的按键释放来触发键盘点击。在按键释放时，仅当存储的生成仍匹配
    /// 窗口当前生成时才触发点击，即焦点在按键期间未移动（镜像浏览器在失焦时
    /// 清除控件按下状态的行为）。`None` 表示没有待处理的激活键。
    pub(crate) pending_keyboard_down: Option<Rc<RefCell<Option<u64>>>>,
    pub(crate) scroll_offset: Option<Rc<RefCell<Point<Pixels>>>>,
    pub(crate) active_tooltip: Option<Rc<RefCell<Option<ActiveTooltip>>>>,
}

/// 元素或包含它的分组是否被鼠标点击。
#[derive(Copy, Clone, Default, Eq, PartialEq)]
pub struct ElementClickedState {
    /// 如果此元素的分组被点击则为 true，否则为 false
    pub group: bool,

    /// 如果此元素被点击则为 true，否则为 false
    pub element: bool,
}

impl ElementClickedState {
    fn is_clicked(&self) -> bool {
        self.group || self.element
    }
}

/// 元素或包含它的分组是否被悬停。
#[derive(Copy, Clone, Default, Eq, PartialEq)]
pub struct ElementHoverState {
    /// 如果此元素的分组被悬停则为 true，否则为 false
    pub group: bool,

    /// 如果此元素被悬停则为 true，否则为 false
    pub element: bool,
}

pub(crate) enum ActiveTooltip {
    /// 当前正在延迟显示工具提示。
    WaitingForShow { _task: Task<()> },
    /// 工具提示可见，元素被悬停或对于可悬停工具提示，工具提示被悬停。
    Visible {
        tooltip: AnyTooltip,
        is_hoverable: bool,
    },
    /// 工具提示可见且可悬停，但鼠标不再悬停。当前正在延迟隐藏。
    WaitingForHide {
        tooltip: AnyTooltip,
        _task: Task<()>,
    },
}

pub(crate) fn clear_active_tooltip(
    active_tooltip: &Rc<RefCell<Option<ActiveTooltip>>>,
    window: &mut Window,
) {
    match active_tooltip.borrow_mut().take() {
        None => {}
        Some(ActiveTooltip::WaitingForShow { .. }) => {}
        Some(ActiveTooltip::Visible { .. }) => window.refresh(),
        Some(ActiveTooltip::WaitingForHide { .. }) => window.refresh(),
    }
}

pub(crate) fn clear_active_tooltip_if_not_hoverable(
    active_tooltip: &Rc<RefCell<Option<ActiveTooltip>>>,
    window: &mut Window,
) {
    let should_clear = match active_tooltip.borrow().as_ref() {
        None => false,
        Some(ActiveTooltip::WaitingForShow { .. }) => false,
        Some(ActiveTooltip::Visible { is_hoverable, .. }) => !is_hoverable,
        Some(ActiveTooltip::WaitingForHide { .. }) => false,
    };
    if should_clear {
        active_tooltip.borrow_mut().take();
        window.refresh();
    }
}

pub(crate) fn set_tooltip_on_window(
    active_tooltip: &Rc<RefCell<Option<ActiveTooltip>>>,
    window: &mut Window,
) -> Option<TooltipId> {
    let tooltip = match active_tooltip.borrow().as_ref() {
        None => return None,
        Some(ActiveTooltip::WaitingForShow { .. }) => return None,
        Some(ActiveTooltip::Visible { tooltip, .. }) => tooltip.clone(),
        Some(ActiveTooltip::WaitingForHide { tooltip, .. }) => tooltip.clone(),
    };
    Some(window.set_tooltip(tooltip))
}

pub(crate) fn register_tooltip_mouse_handlers(
    active_tooltip: &Rc<RefCell<Option<ActiveTooltip>>>,
    tooltip_id: Option<TooltipId>,
    build_tooltip: Rc<dyn Fn(&mut Window, &mut App) -> Option<(AnyView, bool)>>,
    check_is_hovered: Rc<dyn Fn(&Window) -> bool>,
    check_is_hovered_during_prepaint: Rc<dyn Fn(&Window) -> bool>,
    show_delay: Option<Duration>,
    window: &mut Window,
) {
    let current_view = window.current_view();
    let show_delay = show_delay.unwrap_or(DEFAULT_TOOLTIP_SHOW_DELAY);

    window.on_mouse_event({
        let active_tooltip = active_tooltip.clone();
        let build_tooltip = build_tooltip.clone();
        let check_is_hovered = check_is_hovered.clone();
        move |_: &MouseMoveEvent, phase, window, cx| {
            handle_tooltip_mouse_move(
                &active_tooltip,
                &build_tooltip,
                &check_is_hovered,
                &check_is_hovered_during_prepaint,
                tooltip_id,
                current_view,
                phase,
                show_delay,
                window,
                cx,
            )
        }
    });

    window.on_mouse_event({
        let active_tooltip = active_tooltip.clone();
        move |_: &MouseDownEvent, _phase, window: &mut Window, _cx| {
            if !tooltip_id.is_some_and(|tooltip_id| tooltip_id.is_hovered(window)) {
                clear_active_tooltip_if_not_hoverable(&active_tooltip, window);
            }
        }
    });

    window.on_mouse_event({
        let active_tooltip = active_tooltip.clone();
        move |_: &ScrollWheelEvent, _phase, window: &mut Window, _cx| {
            if !tooltip_id.is_some_and(|tooltip_id| tooltip_id.is_hovered(window)) {
                clear_active_tooltip_if_not_hoverable(&active_tooltip, window);
            }
        }
    });
}

/// 处理元素悬停时的工具提示显示。
///
/// 处理 tooltip 的鼠标移动事件。
///
/// 在 prepaint 阶段（hitbox 信息不可用时），使用 `check_is_hovered_during_prepaint`
/// 基于元素绝对边界判断是否悬停。由于无法获取 hitbox 信息，此方法无法检测元素是否被
/// 其他元素遮挡（occluded）。如果 tooltip 显示后被新出现的元素遮挡，tooltip 会持续
/// 显示直到鼠标移出悬停边界。这是已知的轻微视觉缺陷，修复需要 hitbox 遮挡检测支持。
fn handle_tooltip_mouse_move(
    active_tooltip: &Rc<RefCell<Option<ActiveTooltip>>>,
    build_tooltip: &Rc<dyn Fn(&mut Window, &mut App) -> Option<(AnyView, bool)>>,
    check_is_hovered: &Rc<dyn Fn(&Window) -> bool>,
    check_is_hovered_during_prepaint: &Rc<dyn Fn(&Window) -> bool>,
    tooltip_id: Option<TooltipId>,
    current_view: EntityId,
    phase: DispatchPhase,
    show_delay: Duration,
    window: &mut Window,
    cx: &mut App,
) {
    // Separates logic for what mutation should occur from applying it, to avoid overlapping
    // RefCell borrows.
    enum Action {
        None,
        CancelShow,
        ScheduleShow,
        CheckVisible,
    }

    let action = match active_tooltip.borrow().as_ref() {
        None => {
            let is_hovered = check_is_hovered(window);
            if is_hovered && phase.bubble() {
                Action::ScheduleShow
            } else {
                Action::None
            }
        }
        Some(ActiveTooltip::WaitingForShow { .. }) => {
            let is_hovered = check_is_hovered(window);
            if is_hovered {
                Action::None
            } else {
                Action::CancelShow
            }
        }
        Some(ActiveTooltip::Visible { is_hoverable, .. }) => {
            if phase.capture()
                && !check_is_hovered(window)
                && (!*is_hoverable
                    || !tooltip_id.is_some_and(|tooltip_id| tooltip_id.is_hovered(window)))
            {
                Action::CheckVisible
            } else {
                Action::None
            }
        }
        Some(ActiveTooltip::WaitingForHide { .. }) => {
            if phase.capture()
                && (check_is_hovered(window)
                    || tooltip_id.is_some_and(|tooltip_id| tooltip_id.is_hovered(window)))
            {
                Action::CheckVisible
            } else {
                Action::None
            }
        }
    };

    match action {
        Action::None => {}
        Action::CancelShow => {
            // Cancel waiting to show tooltip when it is no longer hovered.
            active_tooltip.borrow_mut().take();
        }
        Action::ScheduleShow => {
            let delayed_show_task = window.spawn(cx, {
                let weak_active_tooltip = Rc::downgrade(active_tooltip);
                let build_tooltip = build_tooltip.clone();
                let check_is_hovered_during_prepaint = check_is_hovered_during_prepaint.clone();
                async move |cx| {
                    cx.background_executor().timer(show_delay).await;
                    let Some(active_tooltip) = weak_active_tooltip.upgrade() else {
                        return;
                    };
                    cx.update(|window, cx| {
                        let new_tooltip =
                            build_tooltip(window, cx).map(|(view, tooltip_is_hoverable)| {
                                let weak_active_tooltip = Rc::downgrade(&active_tooltip);
                                ActiveTooltip::Visible {
                                    tooltip: AnyTooltip {
                                        view,
                                        mouse_position: window.mouse_position(),
                                        check_visible_and_update: Rc::new(
                                            move |tooltip_bounds, window, cx| {
                                                let Some(active_tooltip) =
                                                    weak_active_tooltip.upgrade()
                                                else {
                                                    return false;
                                                };
                                                handle_tooltip_check_visible_and_update(
                                                    &active_tooltip,
                                                    tooltip_is_hoverable,
                                                    &check_is_hovered_during_prepaint,
                                                    tooltip_bounds,
                                                    window,
                                                    cx,
                                                )
                                            },
                                        ),
                                    },
                                    is_hoverable: tooltip_is_hoverable,
                                }
                            });
                        *active_tooltip.borrow_mut() = new_tooltip;
                        window.refresh();
                    })
                    .ok();
                }
            });
            active_tooltip
                .borrow_mut()
                .replace(ActiveTooltip::WaitingForShow {
                    _task: delayed_show_task,
                });
        }
        Action::CheckVisible => cx.notify(current_view),
    }
}

/// 返回一个回调，由窗口预绘制调用以更新工具提示可见性。
/// 在此处而非鼠标移动处理器中执行此逻辑的原因是，当元素未被绘制时
/// （例如使用 `visible_on_hover`），鼠标移动处理器不会被调用。
fn handle_tooltip_check_visible_and_update(
    active_tooltip: &Rc<RefCell<Option<ActiveTooltip>>>,
    tooltip_is_hoverable: bool,
    check_is_hovered: &Rc<dyn Fn(&Window) -> bool>,
    tooltip_bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    // Separates logic for what mutation should occur from applying it, to avoid overlapping RefCell
    // borrows.
    enum Action {
        None,
        Hide,
        ScheduleHide(AnyTooltip),
        CancelHide(AnyTooltip),
    }

    let is_hovered = check_is_hovered(window)
        || (tooltip_is_hoverable && tooltip_bounds.contains(&window.mouse_position()));
    let action = match active_tooltip.borrow().as_ref() {
        Some(ActiveTooltip::Visible { tooltip, .. }) => {
            if is_hovered {
                Action::None
            } else {
                if tooltip_is_hoverable {
                    Action::ScheduleHide(tooltip.clone())
                } else {
                    Action::Hide
                }
            }
        }
        Some(ActiveTooltip::WaitingForHide { tooltip, .. }) => {
            if is_hovered {
                Action::CancelHide(tooltip.clone())
            } else {
                Action::None
            }
        }
        None | Some(ActiveTooltip::WaitingForShow { .. }) => Action::None,
    };

    match action {
        Action::None => {}
        Action::Hide => clear_active_tooltip(active_tooltip, window),
        Action::ScheduleHide(tooltip) => {
            let delayed_hide_task = window.spawn(cx, {
                let weak_active_tooltip = Rc::downgrade(active_tooltip);
                async move |cx| {
                    cx.background_executor()
                        .timer(HOVERABLE_TOOLTIP_HIDE_DELAY)
                        .await;
                    let Some(active_tooltip) = weak_active_tooltip.upgrade() else {
                        return;
                    };
                    if active_tooltip.borrow_mut().take().is_some() {
                        cx.update(|window, _cx| window.refresh()).ok();
                    }
                }
            });
            active_tooltip
                .borrow_mut()
                .replace(ActiveTooltip::WaitingForHide {
                    tooltip,
                    _task: delayed_hide_task,
                });
        }
        Action::CancelHide(tooltip) => {
            // Cancel waiting to hide tooltip when it becomes hovered.
            active_tooltip.borrow_mut().replace(ActiveTooltip::Visible {
                tooltip,
                is_hoverable: true,
            });
        }
    }

    active_tooltip.borrow().is_some()
}

#[derive(Default)]
pub(crate) struct GroupHitboxes(HashMap<SharedString, SmallVec<[HitboxId; 1]>>);

impl Global for GroupHitboxes {}

impl GroupHitboxes {
    pub fn get(name: &SharedString, cx: &mut App) -> Option<HitboxId> {
        cx.default_global::<Self>()
            .0
            .get(name)
            .and_then(|bounds_stack| bounds_stack.last())
            .cloned()
    }

    pub fn push(name: SharedString, hitbox_id: HitboxId, cx: &mut App) {
        cx.default_global::<Self>()
            .0
            .entry(name)
            .or_default()
            .push(hitbox_id);
    }

    pub fn pop(name: &SharedString, cx: &mut App) {
        cx.default_global::<Self>().0.get_mut(name).unwrap().pop();
    }
}

/// 可以存储状态的元素包装器，在分配 ElementId 后生成。
pub struct Stateful<E> {
    pub(crate) element: E,
}

impl<E> Styled for Stateful<E>
where
    E: Styled,
{
    fn style(&mut self) -> &mut StyleRefinement {
        self.element.style()
    }
}

impl<E> StatefulInteractiveElement for Stateful<E>
where
    E: Element,
    Self: InteractiveElement,
{
}

impl<E> InteractiveElement for Stateful<E>
where
    E: InteractiveElement,
{
    fn interactivity(&mut self) -> &mut Interactivity {
        self.element.interactivity()
    }
}

impl<E> Element for Stateful<E>
where
    E: Element,
{
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = E::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        self.element.id()
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        self.element.source_location()
    }

    fn a11y_role(&self) -> Option<accesskit::Role> {
        self.element.a11y_role()
    }

    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        self.element.write_a11y_info(node);
    }

    fn a11y_synthetic_children(
        &mut self,
        prepaint: &mut Self::PrepaintState,
        builder: &mut crate::A11ySubtreeBuilder,
    ) {
        self.element.a11y_synthetic_children(prepaint, builder);
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.element.request_layout(id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> E::PrepaintState {
        self.element
            .prepaint(id, inspector_id, bounds, state, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.element.paint(
            id,
            inspector_id,
            bounds,
            request_layout,
            prepaint,
            window,
            cx,
        );
    }

    /// Web DOM 后端：委托给内部元素（`Stateful<Div>` → `Div::dom`）。
    ///
    /// `button`/`checkbox`/`radio` 等交互组件的渲染根部通常是 `Stateful<Div>`；
    /// 若此层不实现 `dom()`，这些组件的形状（背景/边框等）在纯 DOM 模式下
    /// 不会进入 DOM，而 canvas 已被隐藏，组件形状就会消失。委托后与直接使用
    /// `div` 一致，输出完整的视觉样式。
    #[cfg(feature = "dom-backend")]
    fn dom(
        &self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<crate::DomNode> {
        self.element.dom(bounds, window, cx)
    }
}

impl<E> IntoElement for Stateful<E>
where
    E: Element,
{
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<E> ParentElement for Stateful<E>
where
    E: ParentElement,
{
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.element.extend(elements)
    }
}

/// 可以在父元素中滚动*到*的元素。
/// 与 [ScrollHandle::scroll_to_active_item] 不同，锚定元素不必是父元素的直接子元素。
#[derive(Clone)]
pub struct ScrollAnchor {
    handle: ScrollHandle,
    last_origin: Rc<RefCell<Point<Pixels>>>,
}

impl ScrollAnchor {
    /// 创建与给定 [ScrollHandle] 关联的 [ScrollAnchor]。
    pub fn for_handle(handle: ScrollHandle) -> Self {
        Self {
            handle,
            last_origin: Default::default(),
        }
    }
    /// 请求在下一帧滚动到此项。
    pub fn scroll_to(&self, window: &mut Window, _cx: &mut App) {
        let this = self.clone();

        window.on_next_frame(move |_, _| {
            let viewport_bounds = this.handle.bounds();
            let self_bounds = *this.last_origin.borrow();
            this.handle.set_offset(viewport_bounds.origin - self_bounds);
        });
    }
}

#[derive(Default, Debug)]
struct ScrollHandleState {
    offset: Rc<RefCell<Point<Pixels>>>,
    bounds: Bounds<Pixels>,
    max_offset: Point<Pixels>,
    child_bounds: Vec<Bounds<Pixels>>,
    scroll_to_bottom: bool,
    overflow: Point<Overflow>,
    active_item: Option<ScrollActiveItem>,
}

#[derive(Default, Debug, Clone, Copy)]
struct ScrollActiveItem {
    index: usize,
    strategy: ScrollStrategy,
}

#[derive(Default, Debug, Clone, Copy)]
enum ScrollStrategy {
    #[default]
    FirstVisible,
    Top,
}

/// 元素可滚动方面的句柄。
/// 用于访问滚动状态（如当前滚动偏移量）和修改滚动状态（如滚动到特定子元素）。
#[derive(Clone, Debug)]
pub struct ScrollHandle(Rc<RefCell<ScrollHandleState>>);

impl Default for ScrollHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollHandle {
    /// 构建一个新的滚动句柄。
    pub fn new() -> Self {
        Self(Rc::default())
    }

    /// 获取当前滚动偏移量。
    pub fn offset(&self) -> Point<Pixels> {
        *self.0.borrow().offset.borrow()
    }

    /// 获取最大滚动偏移量。
    pub fn max_offset(&self) -> Point<Pixels> {
        self.0.borrow().max_offset
    }

    /// 获取滚动到视图顶部的子元素索引。
    pub fn top_item(&self) -> usize {
        let state = self.0.borrow();
        let top = state.bounds.top() - state.offset.borrow().y;

        match state.child_bounds.binary_search_by(|bounds| {
            if top < bounds.top() {
                Ordering::Greater
            } else if top > bounds.bottom() {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        }) {
            Ok(ix) => ix,
            Err(ix) => ix.min(state.child_bounds.len().saturating_sub(1)),
        }
    }

    /// 获取滚动到视图底部的子元素索引。
    pub fn bottom_item(&self) -> usize {
        let state = self.0.borrow();
        let bottom = state.bounds.bottom() - state.offset.borrow().y;

        match state.child_bounds.binary_search_by(|bounds| {
            if bottom < bounds.top() {
                Ordering::Greater
            } else if bottom > bounds.bottom() {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        }) {
            Ok(ix) => ix,
            Err(ix) => ix.min(state.child_bounds.len().saturating_sub(1)),
        }
    }

    /// 返回此子元素被绘制到的边界
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.0.borrow().bounds
    }

    /// 获取特定子元素的边界。
    pub fn bounds_for_item(&self, ix: usize) -> Option<Bounds<Pixels>> {
        self.0.borrow().child_bounds.get(ix).cloned()
    }

    /// 更新 [ScrollHandleState] 的活动项，以便在预绘制时滚动到
    pub fn scroll_to_item(&self, ix: usize) {
        let mut state = self.0.borrow_mut();
        state.active_item = Some(ScrollActiveItem {
            index: ix,
            strategy: ScrollStrategy::default(),
        });
    }

    /// 更新 [ScrollHandleState] 的活动项，以便在预绘制时滚动到
    /// 此方法滚动最小量以确保子元素是第一个可见元素
    pub fn scroll_to_top_of_item(&self, ix: usize) {
        let mut state = self.0.borrow_mut();
        state.active_item = Some(ScrollActiveItem {
            index: ix,
            strategy: ScrollStrategy::Top,
        });
    }

    /// 滚动最小量以确保子元素完全可见或视图的顶部元素取决于滚动策略
    fn scroll_to_active_item(&self) {
        let mut state = self.0.borrow_mut();

        let Some(active_item) = state.active_item else {
            return;
        };

        let active_item = match state.child_bounds.get(active_item.index) {
            Some(bounds) => {
                let mut scroll_offset = state.offset.borrow_mut();

                match active_item.strategy {
                    ScrollStrategy::FirstVisible => {
                        if state.overflow.y == Overflow::Scroll {
                            let child_height = bounds.size.height;
                            let viewport_height = state.bounds.size.height;
                            if child_height > viewport_height {
                                scroll_offset.y = state.bounds.top() - bounds.top();
                            } else if bounds.top() + scroll_offset.y < state.bounds.top() {
                                scroll_offset.y = state.bounds.top() - bounds.top();
                            } else if bounds.bottom() + scroll_offset.y > state.bounds.bottom() {
                                scroll_offset.y = state.bounds.bottom() - bounds.bottom();
                            }
                        }
                    }
                    ScrollStrategy::Top => {
                        scroll_offset.y = state.bounds.top() - bounds.top();
                    }
                }

                if state.overflow.x == Overflow::Scroll {
                    let child_width = bounds.size.width;
                    let viewport_width = state.bounds.size.width;
                    if child_width > viewport_width {
                        scroll_offset.x = state.bounds.left() - bounds.left();
                    } else if bounds.left() + scroll_offset.x < state.bounds.left() {
                        scroll_offset.x = state.bounds.left() - bounds.left();
                    } else if bounds.right() + scroll_offset.x > state.bounds.right() {
                        scroll_offset.x = state.bounds.right() - bounds.right();
                    }
                }
                None
            }
            None => Some(active_item),
        };
        state.active_item = active_item;
    }

    /// 滚动到底部。
    pub fn scroll_to_bottom(&self) {
        let mut state = self.0.borrow_mut();
        state.scroll_to_bottom = true;
    }

    /// 显式设置偏移量。偏移量是父容器左上角到第一个子元素左上角的距离。
    /// 随着向下滚动，偏移量变得更负。
    pub fn set_offset(&self, mut position: Point<Pixels>) {
        let state = self.0.borrow();
        *state.offset.borrow_mut() = position;
    }

    /// 获取逻辑滚动顶部，基于子元素索引和像素偏移量。
    pub fn logical_scroll_top(&self) -> (usize, Pixels) {
        let ix = self.top_item();
        let state = self.0.borrow();

        if let Some(child_bounds) = state.child_bounds.get(ix) {
            (
                ix,
                child_bounds.top() + state.offset.borrow().y - state.bounds.top(),
            )
        } else {
            (ix, px(0.))
        }
    }

    /// 获取逻辑滚动底部，基于子元素索引和像素偏移量。
    pub fn logical_scroll_bottom(&self) -> (usize, Pixels) {
        let ix = self.bottom_item();
        let state = self.0.borrow();

        if let Some(child_bounds) = state.child_bounds.get(ix) {
            (
                ix,
                child_bounds.bottom() + state.offset.borrow().y - state.bounds.bottom(),
            )
        } else {
            (ix, px(0.))
        }
    }

    /// 获取可滚动项的子元素计数。
    pub fn children_count(&self) -> usize {
        self.0.borrow().child_bounds.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AnyWindowHandle, AppContext as _, Context, InputEvent, Keystroke, MouseMoveEvent,
        TestAppContext, util::FluentBuilder as _,
    };
    use std::rc::Weak;

    struct TestTooltipView;

    impl Render for TestTooltipView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().w(px(20.)).h(px(20.)).child("tooltip")
        }
    }

    type CapturedActiveTooltip = Rc<RefCell<Option<Weak<RefCell<Option<ActiveTooltip>>>>>>;

    struct TooltipCaptureElement {
        child: AnyElement,
        captured_active_tooltip: CapturedActiveTooltip,
    }

    impl IntoElement for TooltipCaptureElement {
        type Element = Self;

        fn into_element(self) -> Self::Element {
            self
        }
    }

    impl Element for TooltipCaptureElement {
        type RequestLayoutState = ();
        type PrepaintState = ();

        fn id(&self) -> Option<ElementId> {
            None
        }

        fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
            None
        }

        fn request_layout(
            &mut self,
            _id: Option<&GlobalElementId>,
            _inspector_id: Option<&InspectorElementId>,
            window: &mut Window,
            cx: &mut App,
        ) -> (LayoutId, Self::RequestLayoutState) {
            (self.child.request_layout(window, cx), ())
        }

        fn prepaint(
            &mut self,
            _id: Option<&GlobalElementId>,
            _inspector_id: Option<&InspectorElementId>,
            _bounds: Bounds<Pixels>,
            _request_layout: &mut Self::RequestLayoutState,
            window: &mut Window,
            cx: &mut App,
        ) -> Self::PrepaintState {
            self.child.prepaint(window, cx);
        }

        fn paint(
            &mut self,
            _id: Option<&GlobalElementId>,
            _inspector_id: Option<&InspectorElementId>,
            _bounds: Bounds<Pixels>,
            _request_layout: &mut Self::RequestLayoutState,
            _prepaint: &mut Self::PrepaintState,
            window: &mut Window,
            cx: &mut App,
        ) {
            self.child.paint(window, cx);
            window.with_global_id("target".into(), |global_id, window| {
                window.with_element_state::<InteractiveElementState, _>(
                    global_id,
                    |state, _window| {
                        let state = state.unwrap();
                        *self.captured_active_tooltip.borrow_mut() =
                            state.active_tooltip.as_ref().map(Rc::downgrade);
                        ((), state)
                    },
                )
            });
        }
    }

    struct TooltipOwner {
        captured_active_tooltip: CapturedActiveTooltip,
        show_delay_override: Option<Duration>,
    }

    impl Render for TooltipOwner {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            TooltipCaptureElement {
                child: div()
                    .size_full()
                    .child(
                        div()
                            .id("target")
                            .w(px(50.))
                            .h(px(50.))
                            .tooltip(|_, cx| cx.new(|_| TestTooltipView).into())
                            .when_some(self.show_delay_override, |this, delay| {
                                this.tooltip_show_delay(delay)
                            }),
                    )
                    .into_any_element(),
                captured_active_tooltip: self.captured_active_tooltip.clone(),
            }
        }
    }

    #[test]
    fn scroll_handle_aligns_wide_children_to_left_edge() {
        let handle = ScrollHandle::new();
        {
            let mut state = handle.0.borrow_mut();
            state.bounds = Bounds::new(point(px(0.), px(0.)), size(px(80.), px(20.)));
            state.child_bounds = vec![Bounds::new(point(px(25.), px(0.)), size(px(200.), px(20.)))];
            state.overflow.x = Overflow::Scroll;
            state.active_item = Some(ScrollActiveItem {
                index: 0,
                strategy: ScrollStrategy::default(),
            });
        }

        handle.scroll_to_active_item();

        assert_eq!(handle.offset().x, px(-25.));
    }

    #[test]
    fn scroll_handle_aligns_tall_children_to_top_edge() {
        let handle = ScrollHandle::new();
        {
            let mut state = handle.0.borrow_mut();
            state.bounds = Bounds::new(point(px(0.), px(0.)), size(px(20.), px(80.)));
            state.child_bounds = vec![Bounds::new(point(px(0.), px(25.)), size(px(20.), px(200.)))];
            state.overflow.y = Overflow::Scroll;
            state.active_item = Some(ScrollActiveItem {
                index: 0,
                strategy: ScrollStrategy::default(),
            });
        }

        handle.scroll_to_active_item();

        assert_eq!(handle.offset().y, px(-25.));
    }

    fn setup_tooltip_owner_test(
        show_delay_override: Option<Duration>,
    ) -> (
        TestAppContext,
        crate::AnyWindowHandle,
        CapturedActiveTooltip,
    ) {
        let mut test_app = TestAppContext::single();
        let captured_active_tooltip: CapturedActiveTooltip = Rc::new(RefCell::new(None));
        let window = test_app.add_window({
            let captured_active_tooltip = captured_active_tooltip.clone();
            move |_, _| TooltipOwner {
                captured_active_tooltip,
                show_delay_override,
            }
        });
        let any_window = window.into();

        test_app
            .update_window(any_window, |_, window, cx| {
                window.draw(cx).clear(cx);
            })
            .unwrap();

        test_app
            .update_window(any_window, |_, window, cx| {
                window.dispatch_event(
                    MouseMoveEvent {
                        position: point(px(10.), px(10.)),
                        modifiers: Default::default(),
                        pressed_button: None,
                    }
                    .to_platform_input(),
                    cx,
                );
            })
            .unwrap();

        test_app
            .update_window(any_window, |_, window, cx| {
                window.draw(cx).clear(cx);
            })
            .unwrap();

        (test_app, any_window, captured_active_tooltip)
    }

    #[test]
    fn tooltip_waiting_for_show_is_released_when_its_owner_disappears() {
        let (mut test_app, any_window, captured_active_tooltip) = setup_tooltip_owner_test(None);

        let weak_active_tooltip = captured_active_tooltip.borrow().clone().unwrap();
        let active_tooltip = weak_active_tooltip.upgrade().unwrap();
        assert!(matches!(
            active_tooltip.borrow().as_ref(),
            Some(ActiveTooltip::WaitingForShow { .. })
        ));

        test_app
            .update_window(any_window, |_, window, _| {
                window.remove_window();
            })
            .unwrap();
        test_app.run_until_parked();
        drop(active_tooltip);

        assert!(weak_active_tooltip.upgrade().is_none());
    }

    struct HoverListenerLayoutTestView {
        target_left: Pixels,
        hover_transitions: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for HoverListenerLayoutTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let hover_transitions = self.hover_transitions.clone();
            div().relative().size_full().child(
                div()
                    .id("hover-target")
                    .absolute()
                    .left(self.target_left)
                    .top_0()
                    .size(px(20.))
                    .on_click(|_, _, _| {})
                    .on_hover(move |is_hovered, _, _| {
                        hover_transitions.borrow_mut().push(*is_hovered);
                    }),
            )
        }
    }

    #[rgpui::test]
    fn hover_listeners_update_when_layout_changes_under_stationary_mouse(cx: &mut TestAppContext) {
        let hover_transitions = Rc::new(RefCell::new(Vec::new()));
        let window = cx.add_window({
            let hover_transitions = hover_transitions.clone();
            move |_, _| HoverListenerLayoutTestView {
                target_left: px(40.),
                hover_transitions,
            }
        });
        let any_window = AnyWindowHandle::from(window);

        cx.update_window(any_window, |_, window, cx| {
            window.draw(cx).clear(cx);
            window.simulate_mouse_move(point(px(10.), px(10.)), cx);
        })
        .unwrap();
        assert!(hover_transitions.borrow().is_empty());

        window
            .update(cx, |view, _, cx| {
                view.target_left = px(0.);
                cx.notify();
            })
            .unwrap();
        cx.update_window(any_window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        assert_eq!(*hover_transitions.borrow(), [true]);

        window
            .update(cx, |view, _, cx| {
                view.target_left = px(40.);
                cx.notify();
            })
            .unwrap();
        cx.update_window(any_window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        assert_eq!(*hover_transitions.borrow(), [true, false]);
    }

    #[rgpui::test]
    fn hover_listeners_remain_hovered_during_stationary_mouse_press(cx: &mut TestAppContext) {
        let hover_transitions = Rc::new(RefCell::new(Vec::new()));
        let window = cx.add_window({
            let hover_transitions = hover_transitions.clone();
            move |_, _| HoverListenerLayoutTestView {
                target_left: px(0.),
                hover_transitions,
            }
        });
        let any_window = AnyWindowHandle::from(window);
        let mouse_position = point(px(10.), px(10.));

        cx.update_window(any_window, |_, window, cx| {
            window.draw(cx).clear(cx);
            window.simulate_mouse_move(mouse_position, cx);
        })
        .unwrap();
        assert_eq!(*hover_transitions.borrow(), [true]);

        cx.update_window(any_window, |_, window, cx| {
            window.dispatch_event(
                MouseDownEvent {
                    position: mouse_position,
                    button: MouseButton::Left,
                    modifiers: Default::default(),
                    click_count: 1,
                    first_mouse: false,
                }
                .to_platform_input(),
                cx,
            );
            window.draw(cx).clear(cx);
        })
        .unwrap();
        assert_eq!(*hover_transitions.borrow(), [true]);

        cx.update_window(any_window, |_, window, cx| {
            window.dispatch_event(
                MouseUpEvent {
                    position: mouse_position,
                    button: MouseButton::Left,
                    modifiers: Default::default(),
                    click_count: 1,
                }
                .to_platform_input(),
                cx,
            );
            window.draw(cx).clear(cx);
        })
        .unwrap();
        assert_eq!(*hover_transitions.borrow(), [true]);
    }

    #[test]
    fn tooltip_respects_custom_show_delay() {
        let extra_delay = Duration::from_secs(1);
        let show_delay_override = DEFAULT_TOOLTIP_SHOW_DELAY + extra_delay;
        let (mut test_app, _any_window, captured_active_tooltip) =
            setup_tooltip_owner_test(Some(show_delay_override));

        let weak_active_tooltip = captured_active_tooltip.borrow().clone().unwrap();
        let active_tooltip = weak_active_tooltip.upgrade().unwrap();

        test_app
            .dispatcher
            .advance_clock(DEFAULT_TOOLTIP_SHOW_DELAY);
        test_app.run_until_parked();

        assert!(matches!(
            active_tooltip.borrow().as_ref(),
            Some(ActiveTooltip::WaitingForShow { .. })
        ));

        test_app.dispatcher.advance_clock(extra_delay);
        test_app.run_until_parked();

        assert!(matches!(
            active_tooltip.borrow().as_ref(),
            Some(ActiveTooltip::Visible { .. })
        ));
    }

    #[test]
    fn tooltip_is_released_when_its_owner_disappears() {
        let (mut test_app, any_window, captured_active_tooltip) = setup_tooltip_owner_test(None);

        let weak_active_tooltip = captured_active_tooltip.borrow().clone().unwrap();
        let active_tooltip = weak_active_tooltip.upgrade().unwrap();

        test_app
            .dispatcher
            .advance_clock(DEFAULT_TOOLTIP_SHOW_DELAY);
        test_app.run_until_parked();

        assert!(matches!(
            active_tooltip.borrow().as_ref(),
            Some(ActiveTooltip::Visible { .. })
        ));

        test_app
            .update_window(any_window, |_, window, _| {
                window.remove_window();
            })
            .unwrap();
        test_app.run_until_parked();
        drop(active_tooltip);

        assert!(weak_active_tooltip.upgrade().is_none());
    }

    #[test]
    fn tooltip_hides_after_mouse_leaves_origin() {
        let (mut test_app, any_window, captured_active_tooltip) = setup_tooltip_owner_test(None);

        let weak_active_tooltip = captured_active_tooltip.borrow().clone().unwrap();
        let active_tooltip = weak_active_tooltip.upgrade().unwrap();

        test_app
            .dispatcher
            .advance_clock(DEFAULT_TOOLTIP_SHOW_DELAY);
        test_app.run_until_parked();

        assert!(matches!(
            active_tooltip.borrow().as_ref(),
            Some(ActiveTooltip::Visible { .. })
        ));

        test_app
            .update_window(any_window, |_, window, cx| {
                window.dispatch_event(
                    MouseMoveEvent {
                        position: point(px(75.), px(75.)),
                        modifiers: Default::default(),
                        pressed_button: None,
                    }
                    .to_platform_input(),
                    cx,
                );
            })
            .unwrap();

        assert!(active_tooltip.borrow().is_none());
    }

    #[test]
    fn test_write_a11y_info_string_and_numeric_properties() {
        let mut interactivity = Interactivity::default();
        interactivity.aria.label = Some("Buffer Font Size".into());
        interactivity.aria.value = Some("15".into());
        interactivity.aria.placeholder = Some("Search".into());
        interactivity.aria.numeric_value = Some(15.0);
        interactivity.aria.min_numeric_value = Some(6.0);
        interactivity.aria.max_numeric_value = Some(72.0);
        interactivity.aria.numeric_value_step = Some(1.0);

        let mut node = accesskit::Node::new(accesskit::Role::SpinButton);
        interactivity.write_a11y_info(&mut node);

        assert_eq!(node.label(), Some("Buffer Font Size"));
        assert_eq!(node.value(), Some("15"));
        assert_eq!(node.placeholder(), Some("Search"));
        assert_eq!(node.numeric_value(), Some(15.0));
        assert_eq!(node.min_numeric_value(), Some(6.0));
        assert_eq!(node.max_numeric_value(), Some(72.0));
        assert_eq!(node.numeric_value_step(), Some(1.0));
    }

    /// 两个可聚焦、可点击的元素（"a" 和 "b"），用于测试
    /// Enter/Space 合成点击的按下/释放配对。
    struct KeyboardActivationTest {
        focus_a: FocusHandle,
        focus_b: FocusHandle,
        clicks: Rc<RefCell<Vec<&'static str>>>,
    }

    impl Render for KeyboardActivationTest {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let clicks_a = self.clicks.clone();
            let clicks_b = self.clicks.clone();
            div()
                .size_full()
                .child(
                    div()
                        .id("a")
                        .w(px(50.))
                        .h(px(50.))
                        .track_focus(&self.focus_a)
                        .on_click(move |_, _, _| clicks_a.borrow_mut().push("a")),
                )
                .child(
                    div()
                        .id("b")
                        .w(px(50.))
                        .h(px(50.))
                        .track_focus(&self.focus_b)
                        .on_click(move |_, _, _| clicks_b.borrow_mut().push("b")),
                )
        }
    }

    fn setup_keyboard_activation_test() -> (
        TestAppContext,
        AnyWindowHandle,
        Rc<RefCell<Vec<&'static str>>>,
        FocusHandle,
        FocusHandle,
    ) {
        let mut cx = TestAppContext::single();
        let (focus_a, focus_b) = cx.update(|cx| (cx.focus_handle(), cx.focus_handle()));
        let clicks: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
        let window = cx.add_window({
            let focus_a = focus_a.clone();
            let focus_b = focus_b.clone();
            let clicks = clicks.clone();
            move |_, _| KeyboardActivationTest {
                focus_a,
                focus_b,
                clicks,
            }
        });
        (cx, window.into(), clicks, focus_a, focus_b)
    }

    /// 将焦点移动到 `handle`，刷新副作用，然后绘制，使新聚焦的元素
    /// 为下一个派发事件注册其按键处理器。
    fn focus_and_draw(cx: &mut TestAppContext, window: AnyWindowHandle, handle: &FocusHandle) {
        cx.update_window(window, |_, window, cx| window.focus(handle, cx))
            .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.draw(cx).clear(cx);
        })
        .unwrap();
    }

    fn key_down(cx: &mut TestAppContext, window: AnyWindowHandle, key: &str) {
        let keystroke = Keystroke::parse(key).unwrap();
        cx.update_window(window, |_, window, cx| {
            window.dispatch_event(
                KeyDownEvent {
                    keystroke,
                    is_held: false,
                    prefer_character_input: false,
                }
                .to_platform_input(),
                cx,
            );
        })
        .unwrap();
    }

    fn key_up(cx: &mut TestAppContext, window: AnyWindowHandle, key: &str) {
        let keystroke = Keystroke::parse(key).unwrap();
        cx.update_window(window, |_, window, cx| {
            window.dispatch_event(KeyUpEvent { keystroke }.to_platform_input(), cx);
        })
        .unwrap();
    }

    /// 在同一聚焦元素上按下并释放 Enter 会触发点击。
    #[test]
    fn keyboard_activation_fires_click_on_same_element() {
        let (mut cx, window, clicks, focus_a, _focus_b) = setup_keyboard_activation_test();

        focus_and_draw(&mut cx, window, &focus_a);
        key_down(&mut cx, window, "enter");
        key_up(&mut cx, window, "enter");

        assert_eq!(*clicks.borrow(), vec!["a"]);
    }

    /// 按键按下后，如果按键释放发生在*不同的*元素上（因为焦点在此期间
    /// 发生了移动），则不能将合成点击泄漏到新聚焦的元素上。这是核心回归：
    /// 之前按键释放处理器会在按键释放时聚焦的元素上无条件触发。
    #[test]
    fn keyboard_activation_does_not_leak_across_focus_change() {
        let (mut cx, window, clicks, focus_a, focus_b) = setup_keyboard_activation_test();

        // Enter pressed while "a" is focused...
        focus_and_draw(&mut cx, window, &focus_a);
        key_down(&mut cx, window, "enter");

        // ...focus moves to "b" before the release (as a confirm action would)...
        focus_and_draw(&mut cx, window, &focus_b);
        key_up(&mut cx, window, "enter");

        // ...so neither element is clicked: "a" never saw the up, and "b"
        // never saw the down.
        assert!(clicks.borrow().is_empty(), "clicks: {:?}", clicks.borrow());
    }

    /// 按键按下时标记为待定，但焦点在按键释放前移走，当焦点稍后*返回*
    /// 到同一元素时（如菜单触发器重新打开的情况），不得触发点击。记录的
    /// 焦点代次已不再匹配，因此过期的待定状态会被忽略。
    #[test]
    fn keyboard_activation_does_not_leak_when_focus_returns() {
        let (mut cx, window, clicks, focus_a, focus_b) = setup_keyboard_activation_test();

        // Enter pressed on "a"...
        focus_and_draw(&mut cx, window, &focus_a);
        key_down(&mut cx, window, "enter");

        // ...focus leaves "a" before its keyup (so the pending state is never
        // consumed), then comes back to "a"...
        focus_and_draw(&mut cx, window, &focus_b);
        focus_and_draw(&mut cx, window, &focus_a);
        key_up(&mut cx, window, "enter");

        // ...and the now-stale pending keydown must not fire a click.
        assert!(clicks.borrow().is_empty(), "clicks: {:?}", clicks.borrow());
    }

    /// 在按下期间*释放*的非激活键必须取消待定的激活。对于
    /// escape-down、space-down、escape-up、space-up 序列，space 形成
    /// 干净的按下/释放配对，但中间的 escape-up 意味着这不是简单的
    /// space 激活，因此不会触发点击。
    #[test]
    fn keyboard_activation_cleared_by_intervening_key_release() {
        let (mut cx, window, clicks, focus_a, _focus_b) = setup_keyboard_activation_test();

        focus_and_draw(&mut cx, window, &focus_a);
        key_down(&mut cx, window, "escape");
        key_down(&mut cx, window, "space");
        key_up(&mut cx, window, "escape");
        key_up(&mut cx, window, "space");

        assert!(clicks.borrow().is_empty(), "clicks: {:?}", clicks.borrow());
    }

    /// 该标记是单一的激活标记，不区分使用了哪个激活键，因此
    /// Space 按下配对同一元素上的 Enter 释放仍会触发点击。
    #[test]
    fn keyboard_activation_does_not_distinguish_space_and_enter() {
        let (mut cx, window, clicks, focus_a, _focus_b) = setup_keyboard_activation_test();

        focus_and_draw(&mut cx, window, &focus_a);
        key_down(&mut cx, window, "space");
        key_up(&mut cx, window, "enter");

        assert_eq!(*clicks.borrow(), vec!["a"]);
    }

    /// 在激活键按下和释放之间按下的非激活键会清除待定标记，
    /// 从而阻止点击。
    #[test]
    fn keyboard_activation_cleared_by_intervening_keydown() {
        let (mut cx, window, clicks, focus_a, _focus_b) = setup_keyboard_activation_test();

        focus_and_draw(&mut cx, window, &focus_a);
        key_down(&mut cx, window, "enter");
        key_down(&mut cx, window, "a");
        key_up(&mut cx, window, "enter");

        assert!(clicks.borrow().is_empty(), "clicks: {:?}", clicks.borrow());
    }

    /// 带修饰符的 Enter（如 cmd-enter）不被视为激活键，
    /// 因此既不设置待定标记，也不会在释放时触发点击。
    #[test]
    fn keyboard_activation_ignores_modified_keys() {
        let (mut cx, window, clicks, focus_a, _focus_b) = setup_keyboard_activation_test();

        focus_and_draw(&mut cx, window, &focus_a);
        key_down(&mut cx, window, "cmd-enter");
        key_up(&mut cx, window, "cmd-enter");

        assert!(clicks.borrow().is_empty(), "clicks: {:?}", clicks.borrow());
    }

    /// 两个同级标签页组，每个都是可聚焦的容器，*本身不是*制表位，
    /// 且各持有一个制表位。模拟标题栏和状态栏将其控件作为
    /// ARIA 工具栏暴露的方式。
    struct TabGroupFocus {
        group_a: FocusHandle,
        item_a: FocusHandle,
        group_b: FocusHandle,
        item_b: FocusHandle,
    }

    impl Render for TabGroupFocus {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            fn group(container: &FocusHandle, item: &FocusHandle) -> Div {
                div()
                    .track_focus(container)
                    .tab_group()
                    .child(div().track_focus(item))
            }
            div()
                .child(group(&self.group_a, &self.item_a))
                .child(group(&self.group_b, &self.item_b))
        }
    }

    /// 聚焦标签页组容器并按下 Tab（`focus_next`）必须将焦点移动到
    /// *该容器内部*的第一个制表位，如 [`InteractiveElement::tab_stop`]
    /// 所述。
    #[test]
    fn focus_next_from_tab_group_container_enters_that_group() {
        let mut cx = TestAppContext::single();
        let (group_a, item_a, group_b, item_b) = cx.update(|cx| {
            (
                cx.focus_handle(),
                cx.focus_handle().tab_stop(true),
                cx.focus_handle(),
                cx.focus_handle().tab_stop(true),
            )
        });
        let window: AnyWindowHandle = cx
            .add_window({
                let (group_a, item_a, group_b, item_b) =
                    (group_a, item_a, group_b.clone(), item_b.clone());
                move |_, _| TabGroupFocus {
                    group_a,
                    item_a,
                    group_b,
                    item_b,
                }
            })
            .into();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();

        // Focus the *second* group's container, then advance like Tab would.
        let focused = cx
            .update_window(window, |_, window, cx| {
                window.focus(&group_b, cx);
                window.focus_next(cx);
                window.focused(cx).map(|handle| handle.id)
            })
            .unwrap();

        assert_eq!(focused, Some(item_b.id));
    }
}
