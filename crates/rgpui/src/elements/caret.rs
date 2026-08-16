use crate::prelude::FluentBuilder;
use crate::{App, ElementSize, Hsla, IntoElement, RenderOnce, Sizable, Styled, Window};
use crate::{Icon, IconName};

/// 下拉选择箭头（caret）元素。
#[derive(IntoElement)]
pub struct Caret {
    /// 尺寸
    size: ElementSize,
    /// 颜色
    color: Option<Hsla>,
}

impl Caret {
    /// 创建尺寸适配触发器的选择箭头。
    pub fn new(size: ElementSize) -> Self {
        Self { size, color: None }
    }

    /// 设置箭头颜色。
    pub fn text_color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl RenderOnce for Caret {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        Icon::new(IconName::ChevronDown)
            .with_size(match self.size {
                ElementSize::XSmall => ElementSize::XSmall,
                ElementSize::Small => ElementSize::Small,
                _ => ElementSize::Medium,
            })
            .when_some(self.color, |this, color| this.text_color(color))
    }
}
