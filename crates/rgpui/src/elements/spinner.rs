use std::time::Duration;

use crate::prelude::FluentBuilder as _;
use crate::{
    Animation, AnimationExt as _, App, ElementSize, Hsla, Icon, IconName, IntoElement,
    ParentElement as _, RenderOnce, Sizable, Styled as _, Transformation, Window, div, ease_in_out,
    percentage,
};

/// 循环加载的旋转指示器（Spinner）。
#[derive(IntoElement)]
pub struct Spinner {
    /// 尺寸
    size: ElementSize,
    /// 图标
    icon: Icon,
    /// 旋转速度
    speed: Duration,
    /// 缓动函数
    easing: Box<dyn Fn(f32) -> f32>,
    /// 图标颜色
    color: Option<Hsla>,
}

impl Spinner {
    /// 创建新的加载指示器。
    pub fn new() -> Self {
        Self {
            size: ElementSize::Medium,
            speed: Duration::from_secs_f64(0.8),
            easing: Box::new(ease_in_out),
            icon: Icon::new(IconName::Loader),
            color: None,
        }
    }

    /// 为加载指示器设置指定图标。
    ///
    /// 默认是 [`IconName::Loader`]。
    ///
    /// 请确保所用图标适合用作加载指示器。
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = icon.into();
        self
    }

    /// 设置图标颜色。
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// 设置缓动函数。
    pub fn ease(mut self, easing: impl Fn(f32) -> f32 + 'static) -> Self {
        self.easing = Box::new(easing);
        self
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl Sizable for Spinner {
    fn with_size(mut self, size: impl Into<ElementSize>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for Spinner {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .child(
                self.icon
                    .with_size(self.size)
                    .when_some(self.color, |this, color| this.text_color(color))
                    .with_animation(
                        "circle",
                        Animation::new(self.speed).repeat().with_easing(self.easing),
                        |this, delta| this.transform(Transformation::rotate(percentage(delta))),
                    ),
            )
            .into_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ElementSize;

    /// 测试 Spinner 基本构造
    #[test]
    fn test_spinner_build() {
        let s = Spinner::new()
            .with_size(ElementSize::Small)
            .color(crate::red_500());
        assert_eq!(s.size, ElementSize::Small);
        assert!(s.color.is_some());
    }

    /// 测试 Spinner 默认值
    #[test]
    fn test_spinner_default() {
        let s = Spinner::new();
        assert_eq!(s.size, ElementSize::Medium);
        assert_eq!(s.speed, Duration::from_secs_f64(0.8));
    }
}
