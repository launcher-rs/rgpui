//! 带动画过渡的内容切换容器：新旧内容淡入淡出或滑动切换。

use std::time::Duration;

use crate::*;

/// 切换动画类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimatedSwitchTransition {
    /// 淡入淡出。
    #[default]
    Fade,
    /// 向左滑动。
    SlideLeft,
    /// 向右滑动。
    SlideRight,
    /// 向上滑动。
    SlideUp,
    /// 向下滑动。
    SlideDown,
}

/// 带动画过渡的内容切换容器。
///
/// 通过 `child(key, content)` 注册多个内容，`active(key)` 指定当前显示项。
/// 提供 `previous(key, content)` 时，新旧内容会同时做进出场动画。
#[derive(IntoElement)]
pub struct AnimatedSwitch {
    id: ElementId,
    active: usize,
    children: Vec<(usize, AnyElement)>,
    previous: Option<(usize, AnyElement)>,
    transition: AnimatedSwitchTransition,
    duration: Duration,
}

impl AnimatedSwitch {
    /// 创建切换容器。
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            active: 0,
            children: Vec::new(),
            previous: None,
            transition: AnimatedSwitchTransition::default(),
            duration: Duration::from_millis(300),
        }
    }

    /// 设置当前激活内容的关键字。
    pub fn active(mut self, key: usize) -> Self {
        self.active = key;
        self
    }

    /// 设置切换动画类型。
    pub fn transition(mut self, transition: AnimatedSwitchTransition) -> Self {
        self.transition = transition;
        self
    }

    /// 设置动画时长。
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// 注册一个内容项。
    pub fn child(mut self, key: usize, content: impl IntoElement) -> Self {
        self.children.push((key, content.into_any_element()));
        self
    }

    /// 注册上一项内容（用于出场动画）。
    pub fn previous(mut self, key: usize, content: impl IntoElement) -> Self {
        self.previous = Some((key, content.into_any_element()));
        self
    }
}

impl RenderOnce for AnimatedSwitch {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let active_child = self
            .children
            .into_iter()
            .find(|(key, _)| *key == self.active);

        let has_previous = self.previous.is_some();
        let transition = self.transition;
        let duration = self.duration;
        let id = self.id;

        let mut container = div().relative().size_full().overflow_hidden();

        if let Some((prev_key, prev_content)) = self.previous {
            let exit_id = ElementId::Name(format!("{}-exit-{}", id, prev_key).into());

            container = container.child(
                div()
                    .absolute()
                    .inset_0()
                    .child(prev_content)
                    .with_animation(
                        exit_id,
                        Animation::new(duration).with_easing(ease_in_cubic),
                        move |el, delta| apply_exit_transform(el, delta, transition),
                    ),
            );
        }

        if let Some((active_key, active_content)) = active_child {
            if has_previous {
                let enter_id = ElementId::Name(format!("{}-enter-{}", id, active_key).into());

                container =
                    container.child(div().size_full().child(active_content).with_animation(
                        enter_id,
                        Animation::new(duration).with_easing(ease_out_cubic),
                        move |el, delta| apply_enter_transform(el, delta, transition),
                    ));
            } else {
                container = container.child(div().size_full().child(active_content));
            }
        }

        container
    }
}

/// 出场变换：淡出并按方向滑出。
fn apply_exit_transform(el: Div, delta: f32, transition: AnimatedSwitchTransition) -> Div {
    let slide_distance = 100.0;
    match transition {
        AnimatedSwitchTransition::Fade => el.opacity(1.0 - delta),
        AnimatedSwitchTransition::SlideLeft => {
            el.opacity(1.0 - delta).left(px(-slide_distance * delta))
        }
        AnimatedSwitchTransition::SlideRight => {
            el.opacity(1.0 - delta).left(px(slide_distance * delta))
        }
        AnimatedSwitchTransition::SlideUp => {
            el.opacity(1.0 - delta).top(px(-slide_distance * delta))
        }
        AnimatedSwitchTransition::SlideDown => {
            el.opacity(1.0 - delta).top(px(slide_distance * delta))
        }
    }
}

/// 入场变换：淡入并从反方向滑入。
fn apply_enter_transform(el: Div, delta: f32, transition: AnimatedSwitchTransition) -> Div {
    let slide_distance = 100.0;
    let inverse = 1.0 - delta;
    match transition {
        AnimatedSwitchTransition::Fade => el.opacity(delta),
        AnimatedSwitchTransition::SlideLeft => el.opacity(delta).left(px(slide_distance * inverse)),
        AnimatedSwitchTransition::SlideRight => {
            el.opacity(delta).left(px(-slide_distance * inverse))
        }
        AnimatedSwitchTransition::SlideUp => el.opacity(delta).top(px(slide_distance * inverse)),
        AnimatedSwitchTransition::SlideDown => el.opacity(delta).top(px(-slide_distance * inverse)),
    }
}
