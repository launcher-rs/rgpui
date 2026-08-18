//! 跑马灯：内容双副本无缝循环平移。

use std::rc::Rc;
use std::time::Duration;

use rgpui::*;

/// 跑马灯移动方向。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MarqueeDirection {
    /// 向左滚动。
    Left,
    /// 向右滚动。
    Right,
}

/// 跑马灯组件：将 `render_content` 渲染两份并循环平移，形成无缝滚动。
#[derive(IntoElement)]
pub struct Marquee {
    id: ElementId,
    base: Div,
    speed: f32,
    direction: MarqueeDirection,
    gap: Pixels,
    content_width: Pixels,
    render_content: Rc<dyn Fn() -> AnyElement>,
}

impl Marquee {
    /// 创建跑马灯，`render_content` 用于渲染内容副本。
    pub fn new(
        id: impl Into<ElementId>,
        render_content: impl Fn() -> AnyElement + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            base: div(),
            speed: 50.0,
            direction: MarqueeDirection::Left,
            gap: px(32.0),
            content_width: px(1000.0),
            render_content: Rc::new(render_content),
        }
    }

    /// 设置滚动速度（像素/秒）。
    pub fn speed(mut self, pixels_per_second: f32) -> Self {
        self.speed = pixels_per_second.max(1.0);
        self
    }

    /// 设置滚动方向。
    pub fn direction(mut self, direction: MarqueeDirection) -> Self {
        self.direction = direction;
        self
    }

    /// 设置两副本之间的间隙。
    pub fn gap(mut self, gap: Pixels) -> Self {
        self.gap = gap;
        self
    }

    /// 设置内容宽度（默认 1000px，应设为内容实际宽度以保证无缝）。
    pub fn content_width(mut self, width: Pixels) -> Self {
        self.content_width = width;
        self
    }
}

impl Styled for Marquee {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for Marquee {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let total_travel = self.content_width + self.gap;
        let duration_ms = (total_travel / px(1.0) / self.speed * 1000.0) as u64;
        let duration = Duration::from_millis(duration_ms.max(100));
        let gap = self.gap;
        let content_width = self.content_width;
        let direction = self.direction;

        let copy_one = (self.render_content)();
        let copy_two = (self.render_content)();

        self.base.overflow_hidden().child(
            div()
                .id(self.id)
                .flex()
                .flex_row()
                .items_center()
                .w(content_width * 2.0 + gap * 2.0)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .flex_shrink_0()
                        .w(content_width)
                        .mr(gap)
                        .child(copy_one),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .flex_shrink_0()
                        .w(content_width)
                        .mr(gap)
                        .child(copy_two),
                )
                .with_animation(
                    "marquee-scroll",
                    Animation::new(duration).repeat().with_easing(linear),
                    move |el, delta| {
                        let offset = total_travel * delta;
                        match direction {
                            MarqueeDirection::Left => el.left(-offset),
                            MarqueeDirection::Right => el.left(offset - total_travel),
                        }
                    },
                ),
        )
    }
}
