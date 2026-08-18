//! 呼吸灯指示器：中心实心圆点 + 向外扩散并淡出的脉动环。

use std::time::Duration;

use crate::*;

/// 呼吸灯指示器组件。
#[derive(IntoElement)]
pub struct PulseIndicator {
    base: Stateful<Div>,
    color: Hsla,
    dot_size: Pixels,
    speed: Duration,
}

impl PulseIndicator {
    /// 创建呼吸灯，默认绿色、8px 圆点、2 秒一圈。
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            base: div().id(id.into()),
            color: hsla(142.0 / 360.0, 0.71, 0.45, 1.0),
            dot_size: px(8.0),
            speed: Duration::from_secs(2),
        }
    }

    /// 设置圆点颜色。
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// 设置圆点尺寸。
    pub fn size(mut self, size: Pixels) -> Self {
        self.dot_size = size;
        self
    }

    /// 设置一圈动画时长。
    pub fn speed(mut self, speed: Duration) -> Self {
        self.speed = speed;
        self
    }
}

impl RenderOnce for PulseIndicator {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let color = self.color;
        let dot = self.dot_size;
        let ring_max = dot * 2.5;
        let speed = self.speed;

        self.base
            .flex()
            .items_center()
            .justify_center()
            .size(ring_max)
            .child(
                div()
                    .absolute()
                    .rounded_full()
                    .bg(color.opacity(0.6))
                    .size(dot)
                    .with_animation(
                        "pulse-ring",
                        Animation::new(speed).repeat().with_easing(ease_out_cubic),
                        move |this, delta| {
                            let current_size = dot + (ring_max - dot) * delta;
                            let current_opacity = 0.6 * (1.0 - delta);
                            this.size(current_size).opacity(current_opacity)
                        },
                    ),
            )
            .child(div().absolute().rounded_full().bg(color).size(dot))
    }
}

impl Styled for PulseIndicator {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for PulseIndicator {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for PulseIndicator {}

impl ParentElement for PulseIndicator {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements)
    }
}
