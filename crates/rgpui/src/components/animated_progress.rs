//! 带动画的进度条：值变化时平滑过渡到目标进度，可叠加流光（shimmer）效果。

use std::time::Duration;

use crate::{prelude::FluentBuilder as _, *};

/// 进度条尺寸。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimatedProgressSize {
    /// 小（4px）。
    Sm,
    /// 中等（8px）。
    #[default]
    Md,
    /// 大（12px）。
    Lg,
}

/// 进度条样式变体。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimatedProgressVariant {
    /// 默认（主题主色）。
    #[default]
    Default,
    /// 成功（绿色）。
    Success,
    /// 警告（琥珀色）。
    Warning,
    /// 危险（主题危险色）。
    Destructive,
}

/// 带动画的进度条组件。
#[derive(IntoElement)]
pub struct AnimatedProgress {
    /// 元素 ID。
    id: ElementId,
    /// 基础容器。
    base: Div,
    /// 当前进度值（0.0~1.0）。
    value: f32,
    /// 样式变体。
    variant: AnimatedProgressVariant,
    /// 尺寸。
    size: AnimatedProgressSize,
    /// 是否启用流光效果。
    shimmer: bool,
    /// 自定义颜色。
    color: Option<Hsla>,
    /// 进度过渡动画时长。
    duration: Duration,
}

impl AnimatedProgress {
    /// 创建带动画的进度条，默认尺寸中等、动画时长 500ms。
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            base: div(),
            value: 0.0,
            variant: AnimatedProgressVariant::Default,
            size: AnimatedProgressSize::Md,
            shimmer: false,
            color: None,
            duration: Duration::from_millis(500),
        }
    }

    /// 设置进度值（自动钳制在 0.0~1.0）。
    pub fn value(mut self, value: f32) -> Self {
        self.value = value.clamp(0.0, 1.0);
        self
    }

    /// 设置样式变体。
    pub fn variant(mut self, variant: AnimatedProgressVariant) -> Self {
        self.variant = variant;
        self
    }

    /// 设置尺寸。
    pub fn size(mut self, size: AnimatedProgressSize) -> Self {
        self.size = size;
        self
    }

    /// 设置是否启用流光效果。
    pub fn shimmer(mut self, shimmer: bool) -> Self {
        self.shimmer = shimmer;
        self
    }

    /// 设置自定义颜色（优先级高于变体色）。
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// 设置进度过渡动画时长。
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }
}

impl Styled for AnimatedProgress {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for AnimatedProgress {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        let height = match self.size {
            AnimatedProgressSize::Sm => px(4.0),
            AnimatedProgressSize::Md => px(8.0),
            AnimatedProgressSize::Lg => px(12.0),
        };

        let bar_color = self.color.unwrap_or_else(|| match self.variant {
            AnimatedProgressVariant::Default => *theme.tokens.primary,
            AnimatedProgressVariant::Success => rgb(0x22c55e).into(),
            AnimatedProgressVariant::Warning => rgb(0xf59e0b).into(),
            AnimatedProgressVariant::Destructive => *theme.tokens.danger,
        });

        let target_value = self.value;
        let duration = self.duration;
        let shimmer_enabled = self.shimmer;
        let value_key = (target_value * 10000.0) as u32;

        self.base.w_full().child(
            div()
                .relative()
                .w_full()
                .h(height)
                .rounded(theme.radius_lg)
                .bg(*theme.tokens.muted)
                .overflow_hidden()
                .child(
                    div()
                        .id(self.id)
                        .absolute()
                        .top_0()
                        .left_0()
                        .h_full()
                        .bg(bar_color)
                        .rounded(theme.radius_lg)
                        .overflow_hidden()
                        .when(shimmer_enabled, |el| {
                            el.child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .bottom_0()
                                    .w(px(120.0))
                                    .bg(crate::linear_gradient(
                                        90.0,
                                        crate::linear_color_stop(crate::transparent_black(), 0.0),
                                        crate::linear_color_stop(hsla(0.0, 0.0, 1.0, 0.2), 1.0),
                                    ))
                                    .with_animation(
                                        "shimmer-sweep",
                                        Animation::new(Duration::from_millis(1500))
                                            .repeat()
                                            .with_easing(crate::linear),
                                        move |el, delta| {
                                            let start = px(-120.0);
                                            let end = px(600.0);
                                            let pos = start + (end - start) * delta;
                                            el.left(pos)
                                        },
                                    ),
                            )
                        })
                        .with_animation(
                            ("progress-fill", value_key),
                            Animation::new(duration).with_easing(ease_out_cubic),
                            move |el, delta| {
                                let width = target_value * delta;
                                el.w(relative(width))
                            },
                        ),
                ),
        )
    }
}
