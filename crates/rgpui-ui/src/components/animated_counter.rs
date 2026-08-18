//! 数字滚动动画：数值变化时以缓动插值逐帧滚动。

use std::time::Duration;

use crate::animation::*;
use rgpui::*;

/// 数字滚动动画状态：持有展示值与目标值，负责逐帧插值推进。
pub struct AnimatedCounterState {
    display_value: f64,
    target_value: f64,
    version: usize,
    duration: Duration,
}

impl AnimatedCounterState {
    /// 创建状态，默认 300ms 动画时长。
    pub fn new(initial: f64) -> Self {
        Self {
            display_value: initial,
            target_value: initial,
            version: 0,
            duration: NORMAL,
        }
    }

    /// 设置动画时长。
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// 设置目标值并开始滚动动画。
    pub fn set_value(&mut self, value: f64, cx: &mut Context<Self>) {
        if (self.target_value - value).abs() < f64::EPSILON {
            return;
        }

        let from = self.display_value;
        let target = value;
        self.target_value = value;
        self.version += 1;
        let version = self.version;

        let frame_count = (self.duration.as_millis() as u32 / 16).clamp(1, 30);
        let frame_dur = self.duration / frame_count;

        cx.spawn(async move |this, cx| {
            for frame in 1..=frame_count {
                cx.background_executor().timer(frame_dur).await;
                let t = frame as f32 / frame_count as f32;
                let eased = ease_out_cubic(t);
                let interpolated = from + (target - from) * eased as f64;
                let is_last = frame == frame_count;

                _ = this.update(cx, |state, cx| {
                    if state.version != version {
                        return;
                    }
                    state.display_value = if is_last {
                        state.target_value
                    } else {
                        interpolated
                    };
                    cx.notify();
                });
            }
        })
        .detach();

        cx.notify();
    }

    /// 返回当前展示值。
    pub fn display_value(&self) -> f64 {
        self.display_value
    }

    /// 返回目标值。
    pub fn target_value(&self) -> f64 {
        self.target_value
    }
}

/// 数字滚动显示组件。
#[derive(IntoElement)]
pub struct AnimatedCounter {
    base: Div,
    state: Entity<AnimatedCounterState>,
    decimal_places: usize,
    prefix: String,
    suffix: String,
}

impl AnimatedCounter {
    /// 创建数字滚动显示组件。
    pub fn new(_id: impl Into<ElementId>, state: Entity<AnimatedCounterState>) -> Self {
        Self {
            base: div(),
            state,
            decimal_places: 0,
            prefix: String::new(),
            suffix: String::new(),
        }
    }

    /// 设置小数位数。
    pub fn decimal_places(mut self, places: usize) -> Self {
        self.decimal_places = places;
        self
    }

    /// 设置前缀文本。
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// 设置后缀文本。
    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = suffix.into();
        self
    }
}

impl RenderOnce for AnimatedCounter {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let state = self.state.read(cx);
        let value = state.display_value();

        let number_str = if self.decimal_places > 0 {
            format!("{:.prec$}", value, prec = self.decimal_places)
        } else {
            format!("{}", value.round() as i64)
        };

        let formatted = format!("{}{}{}", self.prefix, number_str, self.suffix);

        self.base
            .text_color(theme.tokens.foreground)
            .font_family(theme.mono_font_family.clone())
            .child(formatted)
    }
}

impl Styled for AnimatedCounter {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for AnimatedCounter {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for AnimatedCounter {}

impl ParentElement for AnimatedCounter {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements)
    }
}
