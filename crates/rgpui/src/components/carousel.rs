//! 轮播组件：索引切换 + 自动播放 + 循环 + 指示器 + 滑动手势。
//!
//! 展示型场景（story/引导页）复用。状态与元素分离（同 `SplitPane` 模式）：
//! 父组件 `cx.new(|cx| CarouselState::new())` 持有状态，`render` 里放
//! `Carousel::new(state).child(...)`，只渲染当前页。

use crate::{prelude::FluentBuilder as _, *};
use std::rc::Rc;
use std::time::Duration;

/// 轮播状态实体。
pub struct CarouselState {
    /// 当前页下标。
    index: usize,
    /// 自动播放间隔（`None` 关闭）。
    autoplay: Option<Duration>,
    /// 是否循环（首尾相接）。
    looping: bool,
    /// 按下时的 X 坐标（滑动手势起点）。
    press_x: Option<Pixels>,
    /// 自动播放待推进（后台任务置位，render 里消费并翻页）。
    pending_advance: bool,
    /// 自动播放任务是否已启动（防重复）。
    autoplay_running: bool,
    /// 页切换回调（旧下标，新下标）。
    on_change: Option<Rc<dyn Fn(usize, usize, &mut Window, &mut App)>>,
}

impl CarouselState {
    /// 创建轮播状态（默认循环开、自动播放关）。
    pub fn new() -> Self {
        Self {
            index: 0,
            autoplay: None,
            looping: true,
            press_x: None,
            pending_advance: false,
            autoplay_running: false,
            on_change: None,
        }
    }

    /// 设置自动播放间隔（元素首次渲染时自动启动）。
    pub fn autoplay(mut self, interval: Duration) -> Self {
        self.autoplay = Some(interval);
        self
    }

    /// 设置是否循环。
    pub fn looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    /// 设置页切换回调。
    pub fn on_change<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, usize, &mut Window, &mut App) + 'static,
    {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// 当前页下标。
    pub fn index(&self) -> usize {
        self.index
    }

    /// 下一页（`count` 为总页数）。
    pub fn next(&mut self, count: usize, window: &mut Window, cx: &mut App) {
        if count == 0 {
            return;
        }
        let next = if self.index + 1 >= count {
            if self.looping { 0 } else { count - 1 }
        } else {
            self.index + 1
        };
        self.go_to(next, window, cx);
    }

    /// 上一页（`count` 为总页数）。
    pub fn prev(&mut self, count: usize, window: &mut Window, cx: &mut App) {
        if count == 0 {
            return;
        }
        let prev = if self.index == 0 {
            if self.looping { count - 1 } else { 0 }
        } else {
            self.index - 1
        };
        self.go_to(prev, window, cx);
    }

    /// 跳到指定页（越界钳制；调用方负责 `notify`）。
    pub fn go_to(&mut self, index: usize, window: &mut Window, cx: &mut App) {
        let old = self.index;
        self.index = index;
        if old != index {
            if let Some(ref cb) = self.on_change.clone() {
                cb(old, index, window, cx);
            }
        }
    }

    /// 记录按下位置（滑动手势起点，由元素调用）。
    fn press(&mut self, x: Pixels) {
        self.press_x = Some(x);
    }

    /// 抬起时按水平位移决定翻页（阈值 24px，由元素调用）。
    fn release(&mut self, x: Pixels, count: usize, window: &mut Window, cx: &mut App) {
        if let Some(start) = self.press_x.take() {
            let dx: f32 = (x - start).into();
            if dx <= -24.0 {
                self.next(count, window, cx);
            } else if dx >= 24.0 {
                self.prev(count, window, cx);
            }
        }
    }

    /// 确保自动播放任务启动（元素渲染时调用一次）。
    fn ensure_autoplay(&mut self, count: usize, cx: &mut Context<Self>) {
        let Some(interval) = self.autoplay else {
            return;
        };
        if self.autoplay_running || count == 0 {
            return;
        }
        self.autoplay_running = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(interval).await;
                let alive = this
                    .update(cx, |state, cx| {
                        state.pending_advance = true;
                        cx.notify();
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        })
        .detach();
    }
}

impl Default for CarouselState {
    fn default() -> Self {
        Self::new()
    }
}

/// 轮播元素（只渲染当前页 + 箭头 + 指示器）。
#[derive(IntoElement)]
pub struct Carousel {
    state: Entity<CarouselState>,
    children: Vec<AnyElement>,
    show_arrows: bool,
    show_dots: bool,
    style: StyleRefinement,
}

impl Carousel {
    /// 由状态实体创建。
    pub fn new(state: Entity<CarouselState>) -> Self {
        Self {
            state,
            children: Vec::new(),
            show_arrows: true,
            show_dots: true,
            style: StyleRefinement::default(),
        }
    }

    /// 设置是否显示左右箭头。
    pub fn arrows(mut self, show: bool) -> Self {
        self.show_arrows = show;
        self
    }

    /// 设置是否显示底部指示器。
    pub fn dots(mut self, show: bool) -> Self {
        self.show_dots = show;
        self
    }
}

impl ParentElement for Carousel {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Carousel {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Carousel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let count = self.children.len();
        let (index, show_arrows, show_dots) = self.state.read_with(cx, |state, _| {
            (state.index, self.show_arrows, self.show_dots)
        });
        let index = if count == 0 { 0 } else { index.min(count - 1) };
        let state = self.state.clone();
        let user_style = self.style;

        // 自动播放：首次渲染时启动；后台任务置位后在这里拿 Window 真正翻页。
        self.state.update(cx, |state, cx| {
            state.ensure_autoplay(count, cx);
        });
        if self.state.read(cx).pending_advance {
            self.state.update(cx, |state, cx| {
                state.pending_advance = false;
                state.next(count, window, cx);
                cx.notify();
            });
        }

        let theme = cx.theme();
        let muted_foreground = theme.tokens.muted_foreground;
        let accent = theme.tokens.accent.color;

        let mut root = div()
            .flex()
            .flex_col()
            .w_full()
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, {
                let state = state.clone();
                move |event: &MouseDownEvent, _, cx| {
                    state.update(cx, |state, _| state.press(event.position.x));
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let state = state.clone();
                move |event: &MouseUpEvent, window, cx| {
                    state.update(cx, |state, cx| {
                        state.release(event.position.x, count, window, cx);
                        cx.notify();
                    });
                }
            });

        // 当前页。
        if let Some(child) = self.children.into_iter().nth(index) {
            root = root.child(div().flex_1().child(child));
        }

        // 左右箭头。
        if show_arrows && count > 1 {
            let prev_state = state.clone();
            let next_state = state.clone();
            root = root.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py(px(4.0))
                    .child(
                        Button::new("carousel-prev")
                            .ghost()
                            .small()
                            .icon(IconName::ChevronLeft)
                            .on_click(move |_, window, cx| {
                                prev_state.update(cx, |state, cx| {
                                    state.prev(count, window, cx);
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        Button::new("carousel-next")
                            .ghost()
                            .small()
                            .icon(IconName::ChevronRight)
                            .on_click(move |_, window, cx| {
                                next_state.update(cx, |state, cx| {
                                    state.next(count, window, cx);
                                    cx.notify();
                                });
                            }),
                    ),
            );
        }

        // 底部指示器。
        if show_dots && count > 1 {
            let mut dots = div().flex().items_center().justify_center().gap(px(6.0));
            for dot_ix in 0..count {
                let dot_state = state.clone();
                dots = dots.child(
                    div()
                        .id(ElementId::named_usize("carousel-dot", dot_ix))
                        .w(px(if dot_ix == index { 20.0 } else { 8.0 }))
                        .h(px(8.0))
                        .rounded_full()
                        .bg(if dot_ix == index {
                            accent
                        } else {
                            muted_foreground.color.opacity(0.3)
                        })
                        .cursor_pointer()
                        .on_click(move |_, window, cx| {
                            dot_state.update(cx, |state, cx| {
                                state.go_to(dot_ix, window, cx);
                                cx.notify();
                            });
                        }),
                );
            }
            root = root.child(dots);
        }

        root.map(|mut this| {
            this.style().refine(&user_style);
            this
        })
    }
}
