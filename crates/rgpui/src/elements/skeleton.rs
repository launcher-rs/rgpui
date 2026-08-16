use std::time::Duration;

use crate::{
    ActiveTheme, Animation, AnimationExt as _, IntoElement, RenderOnce, StyleRefinement, Styled,
    bounce, div, ease_in_out,
};
use crate::StyledExt as _;

/// 骨架屏（Skeleton）加载占位元素。
#[derive(IntoElement)]
pub struct Skeleton {
    /// 样式精炼
    style: StyleRefinement,
    /// 是否使用次要颜色
    secondary: bool,
}

impl Skeleton {
    /// 创建新的 Skeleton 元素。
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            secondary: false,
        }
    }

    /// 设置使用次要颜色。
    pub fn secondary(mut self) -> Self {
        self.secondary = true;
        self
    }
}

impl Default for Skeleton {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for Skeleton {
    fn style(&mut self) -> &mut crate::StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Skeleton {
    fn render(self, _: &mut crate::Window, cx: &mut crate::App) -> impl IntoElement {
        div()
            .w_full()
            .h_4()
            .bg(if self.secondary {
                cx.theme().skeleton.opacity(0.5).into()
            } else {
                cx.theme().skeleton
            })
            .refine_style(&self.style)
            .with_animation(
                "skeleton",
                Animation::new(Duration::from_secs(2))
                    .repeat()
                    .with_easing(bounce(ease_in_out)),
                move |this, delta| {
                    let v = 1.0 - delta * 0.5;
                    this.opacity(v)
                },
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 Skeleton 基本构造
    #[test]
    fn test_skeleton_build() {
        let s = Skeleton::new();
        assert!(!s.secondary);
        let s2 = Skeleton::new().secondary();
        assert!(s2.secondary);
    }
}