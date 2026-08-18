//! 视图路由器：带页面切换动画的多屏桌面应用视图栈管理器。

use rgpui::{prelude::FluentBuilder as _, *};
use std::{rc::Rc, time::Duration};

use crate::animation::easing::easings;

/// 页面切换动画类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageTransition {
    /// 向左滑入。
    SlideLeft,
    /// 向右滑入。
    SlideRight,
    /// 向上滑入。
    SlideUp,
    /// 向下滑入。
    SlideDown,
    /// 淡入淡出。
    Fade,
    /// 无动画。
    None,
}

impl PageTransition {
    /// 获取反向动画（用于返回）。
    fn reverse(&self) -> Self {
        match self {
            Self::SlideLeft => Self::SlideRight,
            Self::SlideRight => Self::SlideLeft,
            Self::SlideUp => Self::SlideDown,
            Self::SlideDown => Self::SlideUp,
            Self::Fade => Self::Fade,
            Self::None => Self::None,
        }
    }
}

/// 视图条目：记录 id 与渲染函数。
struct ViewEntry {
    /// 视图标识。
    id: SharedString,
    /// 渲染函数。
    render: Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>,
}

/// 视图路由器状态：管理视图栈与切换动画。
pub struct ViewRouterState {
    /// 视图栈。
    stack: Vec<ViewEntry>,
    /// 上一页渲染函数（切换动画期间使用）。
    previous_render: Option<Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>>,
    /// 默认切换动画。
    transition: PageTransition,
    /// 当前生效的动画。
    active_transition: Option<PageTransition>,
    /// 版本号（用于生成唯一动画 ID）。
    version: usize,
    /// 是否正在切换。
    is_transitioning: bool,
    /// 动画时长。
    duration: Duration,
}

impl ViewRouterState {
    /// 创建状态。
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            previous_render: None,
            transition: PageTransition::SlideLeft,
            active_transition: None,
            version: 0,
            is_transitioning: false,
            duration: Duration::from_millis(300),
        }
    }

    /// 压入新视图并播放切入动画。
    pub fn push(
        &mut self,
        id: impl Into<SharedString>,
        render: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
        cx: &mut Context<Self>,
    ) {
        self.previous_render = self.stack.last().map(|e| e.render.clone());

        self.stack.push(ViewEntry {
            id: id.into(),
            render: Rc::new(render),
        });

        self.begin_transition(self.transition, cx);
    }

    /// 弹出当前视图并播放返回动画。
    pub fn pop(&mut self, cx: &mut Context<Self>) {
        if self.stack.len() <= 1 {
            return;
        }

        self.previous_render = self.stack.last().map(|e| e.render.clone());
        self.stack.pop();
        self.begin_transition(self.transition.reverse(), cx);
    }

    /// 用新视图替换当前视图。
    pub fn replace(
        &mut self,
        id: impl Into<SharedString>,
        render: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
        cx: &mut Context<Self>,
    ) {
        self.previous_render = self.stack.last().map(|e| e.render.clone());

        if !self.stack.is_empty() {
            self.stack.pop();
        }

        self.stack.push(ViewEntry {
            id: id.into(),
            render: Rc::new(render),
        });

        self.begin_transition(self.transition, cx);
    }

    /// 开始一次页面切换动画。
    fn begin_transition(&mut self, transition: PageTransition, cx: &mut Context<Self>) {
        if matches!(transition, PageTransition::None) {
            self.previous_render = None;
            cx.notify();
            return;
        }

        self.active_transition = Some(transition);
        self.version += 1;
        self.is_transitioning = true;
        cx.notify();

        let duration = self.duration;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(duration).await;
            _ = this.update(cx, |this, cx| {
                this.is_transitioning = false;
                this.previous_render = None;
                this.active_transition = None;
                cx.notify();
            });
        })
        .detach();
    }

    /// 获取当前视图 id。
    pub fn current_id(&self) -> Option<SharedString> {
        self.stack.last().map(|e| e.id.clone())
    }

    /// 是否可以返回上一页。
    pub fn can_go_back(&self) -> bool {
        self.stack.len() > 1
    }

    /// 获取视图栈深度。
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// 设置默认切换动画。
    pub fn set_transition(&mut self, transition: PageTransition) {
        self.transition = transition;
    }

    /// 设置动画时长。
    pub fn set_duration(&mut self, duration: Duration) {
        self.duration = duration;
    }
}

/// 视图路由器组件。
#[derive(IntoElement)]
pub struct ViewRouter {
    /// 元素 ID。
    id: ElementId,
    /// 绑定状态实体。
    state: Entity<ViewRouterState>,
    /// 动画覆盖设置。
    transition_override: Option<PageTransition>,
    /// 用户样式。
    style: StyleRefinement,
}

impl ViewRouter {
    /// 创建视图路由器。
    pub fn new(id: impl Into<ElementId>, state: Entity<ViewRouterState>) -> Self {
        Self {
            id: id.into(),
            state,
            transition_override: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置切换动画。
    pub fn transition(mut self, transition: PageTransition) -> Self {
        self.transition_override = Some(transition);
        self
    }
}

impl Styled for ViewRouter {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ViewRouter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let user_style = self.style;

        let (version, duration, transition, current_render, previous_render) = {
            let state = self.state.read(cx);
            let trans = self
                .transition_override
                .or(state.active_transition)
                .unwrap_or(PageTransition::Fade);
            let prev = if state.is_transitioning {
                state.previous_render.clone()
            } else {
                None
            };
            (
                state.version,
                state.duration,
                trans,
                state.stack.last().map(|e| e.render.clone()),
                prev,
            )
        };

        let current_content = current_render.map(|f| f(window, cx));
        let previous_content = previous_render.map(|f| f(window, cx));

        let id = self.id;

        let mut container = div().size_full().overflow_hidden().relative();

        if let Some(old) = previous_content {
            let exit_id = ElementId::Name(format!("{}-exit-{}", id, version).into());
            let enter_id = ElementId::Name(format!("{}-enter-{}", id, version).into());

            container = container.child(render_exit(old, exit_id, transition, duration));

            container = container.when_some(current_content, |this, new| {
                this.child(render_enter(new, enter_id, transition, duration))
            });
        } else {
            container = container.when_some(current_content, |this, content| {
                this.child(div().size_full().child(content))
            });
        }

        container.map(|this| {
            let mut d = this;
            d.style().refine(&user_style);
            d
        })
    }
}

/// 渲染退场视图（旧页）。
fn render_exit(
    content: AnyElement,
    anim_id: ElementId,
    transition: PageTransition,
    duration: Duration,
) -> impl IntoElement {
    div()
        .absolute()
        .inset_0()
        .size_full()
        .child(content)
        .with_animation(anim_id, anim(duration), move |el, delta| {
            apply_exit(el, delta, transition)
        })
}

/// 渲染进场视图（新页）。
fn render_enter(
    content: AnyElement,
    anim_id: ElementId,
    transition: PageTransition,
    duration: Duration,
) -> impl IntoElement {
    div()
        .size_full()
        .child(content)
        .with_animation(anim_id, anim(duration), move |el, delta| {
            apply_enter(el, delta, transition)
        })
}

/// 创建缓出动画。
fn anim(duration: Duration) -> Animation {
    Animation::new(duration).with_easing(easings::ease_out_cubic)
}

/// 应用退场动画样式。
fn apply_exit(el: Div, delta: f32, transition: PageTransition) -> Div {
    let progress = delta;
    match transition {
        PageTransition::SlideLeft => el.left(px(-300.0 * progress)).opacity(1.0 - progress),
        PageTransition::SlideRight => el.left(px(300.0 * progress)).opacity(1.0 - progress),
        PageTransition::SlideUp => el.top(px(-300.0 * progress)).opacity(1.0 - progress),
        PageTransition::SlideDown => el.top(px(300.0 * progress)).opacity(1.0 - progress),
        PageTransition::Fade => el.opacity(1.0 - progress),
        PageTransition::None => el,
    }
}

/// 应用进场动画样式。
fn apply_enter(el: Div, delta: f32, transition: PageTransition) -> Div {
    let progress = delta;
    match transition {
        PageTransition::SlideLeft => el.left(px(300.0 * (1.0 - progress))).opacity(progress),
        PageTransition::SlideRight => el.left(px(-300.0 * (1.0 - progress))).opacity(progress),
        PageTransition::SlideUp => el.top(px(300.0 * (1.0 - progress))).opacity(progress),
        PageTransition::SlideDown => el.top(px(-300.0 * (1.0 - progress))).opacity(progress),
        PageTransition::Fade => el.opacity(progress),
        PageTransition::None => el,
    }
}
