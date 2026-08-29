use std::{cell::Cell, rc::Rc, time::Duration};

use crate::{
    Anchor, AnyElement, App, Bounds, Context, ElementExt, ElementId, InteractiveElement as _,
    IntoElement, ParentElement, Pixels, Render, RenderOnce, StatefulInteractiveElement,
    StyleRefinement, Styled, StyledExt as _, Task, Window, div, prelude::FluentBuilder as _,
};

use super::Popover;

/// 悬停卡片元素，在鼠标悬停到触发器上时显示内容。
///
/// 与 Popover 类似，但由鼠标悬停而非点击触发，支持配置显示与隐藏延迟。
#[derive(IntoElement)]
pub struct HoverCard {
    id: ElementId,
    style: StyleRefinement,
    anchor: Anchor,
    trigger: Option<Box<dyn FnOnce(&mut Window, &App) -> AnyElement + 'static>>,
    content: Option<
        Rc<
            dyn Fn(&mut HoverCardState, &mut Window, &mut Context<HoverCardState>) -> AnyElement
                + 'static,
        >,
    >,
    children: Vec<AnyElement>,
    open_delay: Duration,
    close_delay: Duration,
    appearance: bool,
    on_open_change: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,
}

impl HoverCard {
    /// 创建一个新的悬停卡片
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            anchor: Anchor::TopCenter,
            trigger: None,
            content: None,
            children: vec![],
            open_delay: Duration::from_secs_f64(0.6),
            close_delay: Duration::from_secs_f64(0.3),
            appearance: true,
            on_open_change: None,
        }
    }

    /// 设置悬停卡片的锚点角，默认是 [`Anchor::TopCenter`]
    pub fn anchor(mut self, anchor: impl Into<Anchor>) -> Self {
        self.anchor = anchor.into();
        self
    }

    /// 设置悬停卡片的触发器元素
    pub fn trigger<T>(mut self, trigger: T) -> Self
    where
        T: IntoElement + 'static,
    {
        self.trigger = Some(Box::new(|_, _| trigger.into_any_element()));
        self
    }

    /// 设置悬停卡片的内容构建器
    ///
    /// 构建器函数接收 HoverCardState、Window 和 Context 作为参数
    pub fn content<F, E>(mut self, content: F) -> Self
    where
        F: Fn(&mut HoverCardState, &mut Window, &mut Context<HoverCardState>) -> E + 'static,
        E: IntoElement + 'static,
    {
        self.content = Some(Rc::new(move |state, window, cx| {
            content(state, window, cx).into_any_element()
        }));
        self
    }

    /// 设置显示悬停卡片的延迟毫秒数，默认是 600ms
    pub fn open_delay(mut self, duration: Duration) -> Self {
        self.open_delay = duration;
        self
    }

    /// 设置隐藏悬停卡片的延迟毫秒数，默认是 300ms
    pub fn close_delay(mut self, duration: Duration) -> Self {
        self.close_delay = duration;
        self
    }

    /// 设置是否应用默认外观样式，默认是 `true`
    pub fn appearance(mut self, appearance: bool) -> Self {
        self.appearance = appearance;
        self
    }

    /// 设置打开状态改变时调用的回调
    pub fn on_open_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(&bool, &mut Window, &mut App) + 'static,
    {
        self.on_open_change = Some(Rc::new(callback));
        self
    }
}

impl Styled for HoverCard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for HoverCard {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// HoverCard 组件的状态管理
pub struct HoverCardState {
    open: bool,
    trigger_bounds: Bounds<Pixels>,
    open_delay: Duration,
    close_delay: Duration,

    // 定时器管理
    open_task: Option<Task<()>>,
    close_task: Option<Task<()>>,
    epoch: usize, // 用于取消过期的定时器

    // 悬停状态跟踪
    is_hovering_trigger: bool,
    is_hovering_content: bool,

    // 回调
    on_open_change: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,
}

impl HoverCardState {
    fn new(open_delay: Duration, close_delay: Duration) -> Self {
        Self {
            open: false,
            trigger_bounds: Bounds::default(),
            open_delay,
            close_delay,
            open_task: None,
            close_task: None,
            epoch: 0,
            is_hovering_trigger: false,
            is_hovering_content: false,
            on_open_change: None,
        }
    }

    /// 检查悬停卡片是否打开
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// 在配置的延迟后调度打开悬停卡片
    fn schedule_open(&mut self, cx: &mut Context<Self>) {
        self.cancel_tasks();
        let epoch = self.next_epoch();
        let delay = self.open_delay;

        self.open_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;

            let _ = this.update(cx, |state, cx| {
                if state.epoch == epoch {
                    state.set_open(true, cx);
                }
            });
        }));
    }

    /// 在配置的延迟后调度关闭悬停卡片
    fn schedule_close(&mut self, cx: &mut Context<Self>) {
        self.cancel_tasks();
        let epoch = self.next_epoch();
        let delay = self.close_delay;

        self.close_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;

            let _ = this.update(cx, |state, cx| {
                if state.epoch == epoch && !state.is_hovering_trigger && !state.is_hovering_content
                {
                    state.set_open(false, cx);
                }
            });
        }));
    }

    fn cancel_tasks(&mut self) {
        self.epoch += 1; // 使所有挂起的定时器失效
        self.open_task = None;
        self.close_task = None;
    }

    fn next_epoch(&mut self) -> usize {
        self.epoch += 1;
        self.epoch
    }

    fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.open == open {
            return;
        }

        self.open = open;
        cx.notify();
    }

    /// 处理触发器元素上的悬停状态变化
    fn on_trigger_hover(&mut self, hovering: bool, cx: &mut Context<Self>) {
        self.is_hovering_trigger = hovering;

        if hovering {
            self.schedule_open(cx);
        } else {
            // 只有在未悬停内容时才关闭
            if !self.is_hovering_content {
                self.schedule_close(cx);
            }
        }
    }

    /// 处理内容元素上的悬停状态变化
    fn on_content_hover(&mut self, hovered: bool, cx: &mut Context<Self>) {
        self.is_hovering_content = hovered;

        if hovered {
            self.cancel_tasks();
        } else {
            // 只有在未悬停触发器时才关闭
            if !self.is_hovering_trigger {
                self.schedule_close(cx);
            }
        }
    }
}

impl Render for HoverCardState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div() // 空渲染
    }
}

impl RenderOnce for HoverCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| {
            HoverCardState::new(self.open_delay, self.close_delay)
        });

        // 更新状态并跟踪受控模式是否改变了打开状态
        let prev_open = state.read(cx).open;
        state.update(cx, |state, _| {
            state.open_delay = self.open_delay;
            state.close_delay = self.close_delay;
            state.on_open_change = self.on_open_change.clone();
        });

        let open = state.read(cx).open;
        let trigger_bounds = state.read(cx).trigger_bounds;

        // 如果受控模式下状态发生变化则触发回调
        if prev_open != open {
            if let Some(ref callback) = self.on_open_change {
                callback(&open, window, cx);
            }
        }

        let Some(trigger) = self.trigger else {
            return div().id("empty");
        };

        let anchor = self.anchor;
        let position = Rc::new(Cell::new(Popover::resolved_corner(anchor, trigger_bounds)));

        let root = div().id(self.id).child(
            div()
                .id("trigger")
                .child((trigger)(window, cx))
                .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
                    state.on_trigger_hover(*hovered, cx);
                }))
                .on_prepaint({
                    let state = state.clone();
                    let position = position.clone();
                    move |bounds, _, cx| {
                        position.set(Popover::resolved_corner(anchor, bounds));
                        state.update(cx, |state, _| {
                            state.trigger_bounds = bounds;
                        });
                    }
                }),
        );

        if !open {
            return root;
        }

        let popover_content =
            Popover::render_popover_content(self.anchor, self.appearance, window, cx)
                .overflow_hidden()
                .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
                    state.on_content_hover(*hovered, cx);
                }))
                .when_some(self.content, |this, content| {
                    this.child(state.update(cx, |state, cx| (content)(state, window, cx)))
                })
                .children(self.children)
                .refine_style(&self.style);

        root.child(Popover::render_popover(
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
    use std::time::Duration;

    use crate::{ParentElement as _, TestAppContext, div, rgpui};

    use super::{HoverCard, HoverCardState};

    #[rgpui::test]
    fn test_hover_card_state_initial_closed(cx: &mut TestAppContext) {
        cx.update(|_cx| {
            let state = HoverCardState::new(Duration::from_millis(100), Duration::from_millis(50));
            assert!(!state.is_open());
        });
    }

    #[rgpui::test]
    fn test_hover_card_builder(cx: &mut TestAppContext) {
        cx.update(|_cx| {
            let card = HoverCard::new("hover")
                .trigger(div().child("trigger"))
                .content(|_, _, _| div().child("content"))
                .open_delay(Duration::from_millis(200))
                .close_delay(Duration::from_millis(100));
            let _ = card;
        });
    }
}
