use crate::prelude::FluentBuilder as _;
use crate::{
    ActiveTheme, App, Axis, Div, Hsla, IntoElement, ParentElement, PathBuilder, RenderOnce,
    SharedString, StyleRefinement, Styled, StyledExt as _, Window, canvas, div, point, px,
};

/// 分隔线的样式。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SeparatorStyle {
    /// 实线
    #[default]
    Solid,
    /// 虚线
    Dashed,
}

/// 垂直或水平方向的分隔线（Separator）。
#[derive(IntoElement)]
pub struct Separator {
    /// 基础 Div 元素
    base: Div,
    /// 样式精炼
    style: StyleRefinement,
    /// 标签文本
    label: Option<SharedString>,
    /// 方向
    axis: Axis,
    /// 线条颜色
    color: Option<Hsla>,
    /// 线条样式
    line_style: SeparatorStyle,
}

impl Separator {
    /// 创建垂直分隔线。
    pub fn vertical() -> Self {
        Self {
            base: div().h_full(),
            axis: Axis::Vertical,
            label: None,
            color: None,
            style: StyleRefinement::default(),
            line_style: SeparatorStyle::Solid,
        }
    }

    /// 创建水平分隔线。
    pub fn horizontal() -> Self {
        Self {
            base: div(),
            axis: Axis::Horizontal,
            label: None,
            color: None,
            style: StyleRefinement::default(),
            line_style: SeparatorStyle::Solid,
        }
    }

    /// 创建垂直虚线分隔线。
    pub fn vertical_dashed() -> Self {
        Self::vertical().dashed()
    }

    /// 创建水平虚线分隔线。
    pub fn horizontal_dashed() -> Self {
        Self::horizontal().dashed()
    }

    /// 设置分隔线的标签。
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 设置分隔线的颜色。
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// 将分隔线样式设置为虚线。
    pub fn dashed(mut self) -> Self {
        self.line_style = SeparatorStyle::Dashed;
        self
    }

    fn render_base(axis: Axis) -> Div {
        div().absolute().map(|this| match axis {
            Axis::Vertical => this.w(px(1.)).h_full(),
            Axis::Horizontal => this.h(px(1.)).w_full(),
        })
    }

    fn render_solid(axis: Axis, color: Hsla) -> impl IntoElement {
        Self::render_base(axis).bg(color)
    }

    fn render_dashed(axis: Axis, color: Hsla) -> impl IntoElement {
        Self::render_base(axis).child(
            canvas(
                move |_, _, _| {},
                move |bounds, _, window, _| {
                    let mut builder = PathBuilder::stroke(px(1.)).dash_array(&[px(4.), px(2.)]);
                    let (start, end) = match axis {
                        Axis::Horizontal => {
                            let x = bounds.origin.x;
                            let y = bounds.origin.y + px(0.5);
                            (point(x, y), point(x + bounds.size.width, y))
                        }
                        Axis::Vertical => {
                            let x = bounds.origin.x + px(0.5);
                            let y = bounds.origin.y;
                            (point(x, y), point(x, y + bounds.size.height))
                        }
                    };
                    builder.move_to(start);
                    builder.line_to(end);
                    if let Ok(line) = builder.build() {
                        window.paint_path(line, color);
                    }
                },
            )
            .size_full(),
        )
    }
}

impl Styled for Separator {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Separator {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = self.color.unwrap_or(cx.theme().border);
        let axis = self.axis;
        let line_style = self.line_style;

        self.base
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .refine_style(&self.style)
            .child(match line_style {
                SeparatorStyle::Solid => Self::render_solid(axis, color).into_any_element(),
                SeparatorStyle::Dashed => Self::render_dashed(axis, color).into_any_element(),
            })
            .when_some(self.label, |this, label| {
                this.child(
                    div()
                        .px_2()
                        .py_1()
                        .mx_auto()
                        .text_xs()
                        .bg(cx.theme().tokens.background)
                        .text_color(cx.theme().muted_foreground)
                        .child(label),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 Separator 方向构造
    #[test]
    fn test_separator_axis() {
        let v = Separator::vertical();
        assert_eq!(v.axis, Axis::Vertical);
        let h = Separator::horizontal();
        assert_eq!(h.axis, Axis::Horizontal);
    }

    /// 测试 Separator 虚线样式
    #[test]
    fn test_separator_dashed() {
        let s = Separator::horizontal_dashed();
        assert_eq!(s.line_style, SeparatorStyle::Dashed);
        assert_eq!(s.axis, Axis::Horizontal);
    }

    /// 测试 Separator 标签与颜色
    #[test]
    fn test_separator_label_color() {
        let s = Separator::vertical().label("分隔").color(crate::red_500());
        assert_eq!(s.label, Some("分隔".into()));
        assert!(s.color.is_some());
    }
}
