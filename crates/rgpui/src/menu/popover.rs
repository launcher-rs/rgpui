//! 弹出层，可由按钮或任何其他元素触发的通用弹出容器。

use crate::{
    Anchor, AnyElement, App, Bounds, Context, Deferred, DismissEvent, Div, ElementId, EventEmitter,
    FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyBinding, MouseButton,
    ParentElement, Pixels, Point, Render, RenderOnce, Stateful, StyleRefinement, Styled,
    Subscription, Window, anchored, deferred, div, prelude::FluentBuilder as _, px,
};
use std::{cell::Cell, rc::Rc};

use crate::{
    ElementExt, Selectable, StyledExt as _, menu::actions::Cancel, menu::global_state::GlobalState,
    v_flex,
};

const CONTEXT: &str = "Popover";
/// 初始化 Popover 的快捷键绑定
pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Cancel, Some(CONTEXT))])
}

/// 可由按钮或任何其他元素触发的 Popover 元素
#[derive(IntoElement)]
pub struct Popover {
    id: ElementId,
    style: StyleRefinement,
    anchor: Anchor,
    default_open: bool,
    open: Option<bool>,
    tracked_focus_handle: Option<FocusHandle>,
    trigger: Option<Box<dyn FnOnce(bool, &Window, &App) -> AnyElement + 'static>>,
    content: Option<
        Rc<
            dyn Fn(&mut PopoverState, &mut Window, &mut Context<PopoverState>) -> AnyElement
                + 'static,
        >,
    >,
    children: Vec<AnyElement>,
    /// 触发器元素的样式
    ///
    /// 用于修复触发器元素样式以支持 w_full 的问题。
    trigger_style: Option<StyleRefinement>,
    mouse_button: MouseButton,
    appearance: bool,
    overlay_closable: bool,
    on_open_change: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,
}

impl Popover {
    /// 创建新的 Popover（`view` 模式）
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            anchor: Anchor::TopLeft,
            trigger: None,
            trigger_style: None,
            content: None,
            tracked_focus_handle: None,
            children: vec![],
            mouse_button: MouseButton::Left,
            appearance: true,
            overlay_closable: true,
            default_open: false,
            open: None,
            on_open_change: None,
        }
    }

    /// 设置 Popover 的锚点角，默认为 [`Anchor::TopLeft`]
    ///
    /// 想象 Popover 有一个指针尖端（如同气泡的尾巴）。锚点是尖端相对于触发器
    /// 的位置：`Anchor::TopLeft` 将其放在触发器的左上角，`Anchor::BottomRight`
    /// 放在右下角，以此类推。Popover 然后从该点悬挂下来。
    pub fn anchor(mut self, anchor: impl Into<Anchor>) -> Self {
        self.anchor = anchor.into();
        self
    }

    /// 设置触发 Popover 的鼠标按钮，默认为 `MouseButton::Left`
    pub fn mouse_button(mut self, mouse_button: MouseButton) -> Self {
        self.mouse_button = mouse_button;
        self
    }

    /// 设置 Popover 的触发器元素
    pub fn trigger<T>(mut self, trigger: T) -> Self
    where
        T: Selectable + IntoElement + 'static,
    {
        self.trigger = Some(Box::new(|is_open, _, _| {
            let selected = trigger.is_selected();
            trigger.selected(selected || is_open).into_any_element()
        }));
        self
    }

    /// 设置 Popover 的默认打开状态，默认为 `false`
    ///
    /// 仅用于初始化 Popover 的打开状态。
    ///
    /// 请注意，如果使用 `open` 方法，此值将被忽略。
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// 强制设置 Popover 的打开状态
    ///
    /// 如果设置，Popover 将由该值控制。
    ///
    /// 注意：必须与 `on_open_change` 结合使用来处理状态变化。
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// 添加打开状态变化时的回调
    ///
    /// 第一个 `&bool` 参数是**新的打开状态**。
    ///
    /// 在使用 `open` 方法控制 Popover 状态时很有用。
    pub fn on_open_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(&bool, &mut Window, &mut App) + 'static,
    {
        self.on_open_change = Some(Rc::new(callback));
        self
    }

    /// 设置触发器元素的样式
    pub fn trigger_style(mut self, style: StyleRefinement) -> Self {
        self.trigger_style = Some(style);
        self
    }

    /// 设置点击 Popover 外部是否关闭它，默认为 `true`
    pub fn overlay_closable(mut self, closable: bool) -> Self {
        self.overlay_closable = closable;
        self
    }

    /// 设置 Popover 内容的构建器
    ///
    /// 此回调在每次渲染 Popover 时都会被调用。
    /// 因此，应避免在 content 闭包中创建新的元素或实体。
    pub fn content<F, E>(mut self, content: F) -> Self
    where
        E: IntoElement,
        F: Fn(&mut PopoverState, &mut Window, &mut Context<PopoverState>) -> E + 'static,
    {
        self.content = Some(Rc::new(move |state, window, cx| {
            content(state, window, cx).into_any_element()
        }));
        self
    }

    /// 设置 Popover 是否无样式，默认为 `false`
    ///
    /// 如果无样式：
    ///
    /// - Popover 将没有背景、边框、阴影或内边距。
    /// - 点击 Popover 外部不会关闭它。
    pub fn appearance(mut self, appearance: bool) -> Self {
        self.appearance = appearance;
        self
    }

    /// 绑定打开 Popover 时要获得焦点的焦点句柄
    ///
    /// 如果不设置，将为 Popover 创建一个新的焦点句柄。
    ///
    /// 如果 Popover 打开，焦点将移动到该焦点句柄。
    pub fn track_focus(mut self, handle: &FocusHandle) -> Self {
        self.tracked_focus_handle = Some(handle.clone());
        self
    }

    /// 解析锚点角对应的触发器边界角点
    pub(crate) fn resolved_corner(anchor: Anchor, trigger_bounds: Bounds<Pixels>) -> Point<Pixels> {
        match anchor {
            Anchor::TopLeft => trigger_bounds.origin,
            Anchor::TopCenter => trigger_bounds.top_center(),
            Anchor::TopRight => trigger_bounds.top_right(),
            Anchor::BottomLeft => Point {
                x: trigger_bounds.origin.x,
                y: trigger_bounds.origin.y - trigger_bounds.size.height,
            },
            Anchor::BottomCenter => Point {
                x: trigger_bounds.top_center().x,
                y: trigger_bounds.origin.y - trigger_bounds.size.height,
            },
            Anchor::BottomRight => Point {
                x: trigger_bounds.top_right().x,
                y: trigger_bounds.origin.y - trigger_bounds.size.height,
            },
            // LeftCenter/RightCenter 的回退
            _ => trigger_bounds.origin,
        }
    }
}

impl ParentElement for Popover {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Popover {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// Popover 状态实体
pub struct PopoverState {
    focus_handle: FocusHandle,
    pub(crate) tracked_focus_handle: Option<FocusHandle>,
    previous_focus_handle: Option<FocusHandle>,
    trigger_bounds: Bounds<Pixels>,
    trigger_bounds_captured: bool,
    open: bool,
    on_open_change: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,

    _dismiss_subscription: Option<Subscription>,
}

impl PopoverState {
    /// 使用给定的默认打开状态创建新的 Popover 状态
    pub fn new(default_open: bool, cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            tracked_focus_handle: None,
            previous_focus_handle: None,
            trigger_bounds: Bounds::default(),
            trigger_bounds_captured: false,
            open: default_open,
            on_open_change: None,
            _dismiss_subscription: None,
        }
    }

    /// 检查 Popover 是否打开
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// 如果 Popover 打开则关闭它
    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.toggle_open(window, cx);
        }
    }

    /// 如果 Popover 关闭则打开它
    pub fn show(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            self.toggle_open(window, cx);
        }
    }

    fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.open = open;
        if self.open {
            GlobalState::global_mut(cx).register_deferred_popover(&self.focus_handle);
        } else {
            GlobalState::global_mut(cx).unregister_deferred_popover(&self.focus_handle);
        }
    }

    fn toggle_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let opening = !self.open;
        if opening {
            // 打开前保存聚焦元素，以便关闭时恢复。
            self.previous_focus_handle = window.focused(cx);
        }
        self.set_open(opening, cx);
        if self.open {
            let state = cx.entity();
            let focus_handle = if let Some(tracked_focus_handle) = self.tracked_focus_handle.clone()
            {
                tracked_focus_handle
            } else {
                self.focus_handle.clone()
            };
            focus_handle.focus(window, cx);

            self._dismiss_subscription =
                Some(
                    window.subscribe(&cx.entity(), cx, move |_, _: &DismissEvent, window, cx| {
                        state.update(cx, |state, cx| {
                            state.dismiss(window, cx);
                        });
                        window.refresh();
                    }),
                );
        } else {
            self._dismiss_subscription = None;
            // 恢复焦点到 Popover 打开前聚焦的元素
            if let Some(prev) = self.previous_focus_handle.take() {
                if self.focus_handle.contains_focused(window, cx) {
                    prev.focus(window, cx);
                }
            }
        }

        if let Some(callback) = self.on_open_change.as_ref() {
            callback(&self.open, window, cx);
        }
        cx.notify();
    }

    fn on_action_cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss(window, cx);
    }
}

impl Focusable for PopoverState {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PopoverState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl EventEmitter<DismissEvent> for PopoverState {}

impl Popover {
    /// 渲染 Popover 的延迟层
    pub(crate) fn render_popover<E>(
        anchor: Anchor,
        position: Rc<Cell<Point<Pixels>>>,
        content: E,
        _: &mut Window,
        _: &mut App,
    ) -> Deferred
    where
        E: IntoElement + 'static,
    {
        deferred(
            anchored()
                .snap_to_window_with_margin(px(8.))
                .anchor(anchor)
                .position(position.get())
                .child(div().relative().child(content)),
        )
        .with_priority(1)
    }

    /// 渲染 Popover 内容
    pub(crate) fn render_popover_content(
        anchor: Anchor,
        appearance: bool,
        _: &mut Window,
        cx: &mut App,
    ) -> Stateful<Div> {
        v_flex()
            .id("content")
            .occlude()
            .tab_group()
            .when(appearance, |this| this.popover_style(cx).p_3())
            .map(|this| match anchor {
                Anchor::TopLeft | Anchor::TopCenter | Anchor::TopRight => this.top_1(),
                Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => this.bottom_1(),
                Anchor::LeftCenter | Anchor::RightCenter => this.top_1(), // 居中时的回退
            })
    }
}

impl RenderOnce for Popover {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let force_open = self.open;
        let default_open = self.default_open;
        let tracked_focus_handle = self.tracked_focus_handle.clone();
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| {
            PopoverState::new(default_open, cx)
        });

        state.update(cx, |state, cx| {
            if let Some(tracked_focus_handle) = tracked_focus_handle {
                state.tracked_focus_handle = Some(tracked_focus_handle);
            }
            state.on_open_change = self.on_open_change.clone();
            if let Some(force_open) = force_open {
                state.set_open(force_open, cx);
            }
        });

        let open = state.read(cx).open;
        let focus_handle = state.read(cx).focus_handle.clone();
        let trigger_bounds = state.read(cx).trigger_bounds;
        let trigger_bounds_captured = state.read(cx).trigger_bounds_captured;

        let Some(trigger) = self.trigger else {
            return div().id("empty");
        };

        let parent_view_id = window.current_view();

        // 共享的 Cell，使延迟的 Anchored 元素能够在 prepaint 时读取真实的触发器边界
        // （在触发器的 on_prepaint 已经以正确的边界触发之后）。
        let position = Rc::new(Cell::new(Self::resolved_corner(
            self.anchor,
            trigger_bounds,
        )));

        let el = div()
            .id(self.id)
            .child((trigger)(open, window, cx))
            .on_mouse_down(self.mouse_button, {
                let state = state.clone();
                move |_, window, cx| {
                    cx.stop_propagation();
                    state.update(cx, |state, cx| {
                        // 强制将 open 设置为 false 以正确切换。
                        // 因为如果鼠标按下外部，会先切换 open。
                        state.set_open(open, cx);
                        state.toggle_open(window, cx);
                    });
                    cx.notify(parent_view_id);
                }
            })
            .on_prepaint({
                let state = state.clone();
                let position = position.clone();
                let anchor = self.anchor;
                move |bounds, window, cx| {
                    position.set(Self::resolved_corner(anchor, bounds));
                    let first_capture = state.update(cx, |state, _| {
                        let first = !state.trigger_bounds_captured;
                        state.trigger_bounds = bounds;
                        state.trigger_bounds_captured = true;
                        first
                    });
                    // 在首次捕获边界时，请求新帧以便 Popover 在正确位置渲染
                    // （在当前绘制周期之外）。
                    if first_capture {
                        window.request_animation_frame();
                    }
                }
            });

        if !open || !trigger_bounds_captured {
            return el;
        }

        let popover_content =
            Self::render_popover_content(self.anchor, self.appearance, window, cx)
                .track_focus(&focus_handle)
                .key_context(CONTEXT)
                .on_action(window.listener_for(&state, PopoverState::on_action_cancel))
                .when_some(self.content, |this, content| {
                    this.child(state.update(cx, |state, cx| (content)(state, window, cx)))
                })
                .children(self.children)
                .when(self.overlay_closable, |this| {
                    this.on_mouse_down_out({
                        let state = state.clone();
                        move |_, window, cx| {
                            state.update(cx, |state, cx| {
                                state.dismiss(window, cx);
                            });
                            cx.notify(parent_view_id);
                        }
                    })
                })
                .refine_style(&self.style);

        el.child(Self::render_popover(
            self.anchor,
            position,
            popover_content,
            window,
            cx,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MouseButton;

    #[test]
    fn test_popover_builder_chaining() {
        let popover = Popover::new("test")
            .anchor(Anchor::BottomCenter)
            .mouse_button(MouseButton::Right)
            .default_open(true)
            .appearance(false)
            .overlay_closable(false);

        assert_eq!(popover.anchor, Anchor::BottomCenter);
        assert_eq!(popover.mouse_button, MouseButton::Right);
        assert!(popover.default_open);
        assert!(!popover.appearance);
        assert!(!popover.overlay_closable);
    }

    #[test]
    fn test_resolved_corner_top_positions() {
        use crate::px;

        let bounds = Bounds {
            origin: Point {
                x: px(100.),
                y: px(100.),
            },
            size: crate::Size {
                width: px(200.),
                height: px(50.),
            },
        };

        let pos = Popover::resolved_corner(Anchor::TopLeft, bounds);
        assert_eq!(pos.x, px(100.));
        assert_eq!(pos.y, px(100.));

        let pos = Popover::resolved_corner(Anchor::TopCenter, bounds);
        assert_eq!(pos.x, px(200.));
        assert_eq!(pos.y, px(100.));

        let pos = Popover::resolved_corner(Anchor::TopRight, bounds);
        assert_eq!(pos.x, px(300.));
        assert_eq!(pos.y, px(100.));

        let pos = Popover::resolved_corner(Anchor::BottomLeft, bounds);
        assert_eq!(pos.x, px(100.));
        assert_eq!(pos.y, px(50.));

        let pos = Popover::resolved_corner(Anchor::BottomCenter, bounds);
        assert_eq!(pos.x, px(200.));
        assert_eq!(pos.y, px(50.));

        let pos = Popover::resolved_corner(Anchor::BottomRight, bounds);
        assert_eq!(pos.x, px(300.));
        assert_eq!(pos.y, px(50.));
    }
}
