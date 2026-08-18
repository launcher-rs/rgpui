//! 微光扫过效果：一条半透明白色光带周期性从左到右掠过内容。

use std::time::Duration;

use rgpui::*;

/// 微光扫过组件，通常覆盖在卡片、按钮等内容的容器上。
#[derive(IntoElement)]
pub struct Shimmer {
    base: Div,
    duration: Duration,
}

impl Shimmer {
    /// 创建微光组件，默认 1.5 秒扫过一圈。
    pub fn new() -> Self {
        Self {
            base: div(),
            duration: Duration::from_millis(1500),
        }
    }

    /// 设置扫过一圈的时长。
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }
}

impl Default for Shimmer {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Shimmer {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let duration = self.duration;

        self.base.relative().overflow_hidden().child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .w(px(200.0))
                .bg(linear_gradient(
                    90.0,
                    linear_color_stop(transparent_black(), 0.0),
                    linear_color_stop(hsla(0.0, 0.0, 1.0, 0.15), 1.0),
                ))
                .with_animation(
                    "shimmer-sweep",
                    Animation::new(duration).repeat().with_easing(linear),
                    move |this, delta| {
                        let start = px(-200.0);
                        let end = px(600.0);
                        let current = start + (end - start) * delta;
                        this.left(current)
                    },
                ),
        )
    }
}

impl Styled for Shimmer {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for Shimmer {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for Shimmer {}

impl ParentElement for Shimmer {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements)
    }
}
