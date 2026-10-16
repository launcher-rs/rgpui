//! 分组框组件：把相关内容聚在一起，可选标题与页脚。
//!
//! 主题令牌：`tokens.group_box`（填充背景）、`group_box_foreground`（内容文字）、
//! `muted_foreground`（标题/页脚文字）与 `border`（描边）。

use crate::{prelude::FluentBuilder as _, *};

/// 分组框变体。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum GroupBoxVariant {
    /// 无背景、无边框，仅作布局分组（默认）。
    #[default]
    Normal,
    /// 填充背景。
    Fill,
    /// 描边。
    Outline,
}

/// 为分组框元素设置变体的 trait。
pub trait GroupBoxVariants: Sized {
    /// 设置分组框变体。
    fn with_variant(self, variant: GroupBoxVariant) -> Self;

    /// 使用 [`GroupBoxVariant::Normal`]。
    fn normal(self) -> Self {
        self.with_variant(GroupBoxVariant::Normal)
    }

    /// 使用 [`GroupBoxVariant::Fill`]。
    fn fill(self) -> Self {
        self.with_variant(GroupBoxVariant::Fill)
    }

    /// 使用 [`GroupBoxVariant::Outline`]。
    fn outline(self) -> Self {
        self.with_variant(GroupBoxVariant::Outline)
    }
}

/// 分组框：带可选标题与页脚的相关内容容器。
#[derive(IntoElement)]
pub struct GroupBox {
    /// 元素 ID，默认 `group-box`。
    id: Option<ElementId>,
    /// 变体。
    variant: GroupBoxVariant,
    /// 用户样式。
    style: StyleRefinement,
    /// 标题样式覆盖。
    title_style: StyleRefinement,
    /// 内容样式覆盖。
    content_style: StyleRefinement,
    /// 标题。
    title: Option<AnyElement>,
    /// 内容子元素。
    children: Vec<AnyElement>,
    /// 页脚说明。
    footer: Option<AnyElement>,
}

impl GroupBox {
    /// 创建分组框。
    pub fn new() -> Self {
        Self {
            id: None,
            variant: GroupBoxVariant::default(),
            style: StyleRefinement::default(),
            title_style: StyleRefinement::default(),
            content_style: StyleRefinement::default(),
            title: None,
            children: Vec::new(),
            footer: None,
        }
    }

    /// 设置元素 ID。
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// 设置标题。
    pub fn title(mut self, title: impl IntoElement) -> Self {
        self.title = Some(title.into_any_element());
        self
    }

    /// 覆盖标题样式。
    pub fn title_style(mut self, style: StyleRefinement) -> Self {
        self.title_style = style;
        self
    }

    /// 覆盖内容样式。
    pub fn content_style(mut self, style: StyleRefinement) -> Self {
        self.content_style = style;
        self
    }

    /// 设置分组表面下方的辅助说明（小号弱化文字，与标题左对齐）。
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }
}

impl Default for GroupBox {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for GroupBox {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for GroupBox {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl GroupBoxVariants for GroupBox {
    fn with_variant(mut self, variant: GroupBoxVariant) -> Self {
        self.variant = variant;
        self
    }
}

impl RenderOnce for GroupBox {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (background, border_color, has_paddings) = match self.variant {
            GroupBoxVariant::Normal => (None, None, false),
            GroupBoxVariant::Fill => (Some(theme.tokens.group_box.background), None, true),
            GroupBoxVariant::Outline => (None, Some(theme.border), true),
        };
        let radius = theme.radius;
        let title_color = theme.muted_foreground;
        let content_color = theme.group_box_foreground;

        let user_style = self.style;
        let title_style = self.title_style;
        let content_style = self.content_style;

        v_flex()
            .id(self.id.unwrap_or_else(|| "group-box".into()))
            .w_full()
            // 有填充/描边时表面已提供内边距，标题与表面间距可收紧。
            .gap(if has_paddings { px(12.0) } else { px(16.0) })
            .refine_style(&user_style)
            .when_some(self.title, |this, title| {
                this.child(
                    div()
                        .text_color(title_color)
                        .line_height(relative(1.25))
                        .refine_style(&title_style)
                        .child(title),
                )
            })
            .child(
                // 页脚放在表面槽位内，其 8px 间距独立于标题与表面之间的根间距。
                v_flex()
                    .gap(px(8.0))
                    .child(
                        v_flex()
                            .when_some(background, |this, background| this.bg(background))
                            .when_some(border_color, |this, border_color| {
                                this.border_1().border_color(border_color)
                            })
                            .text_color(content_color)
                            .when(has_paddings, |this| this.p(px(16.0)))
                            .gap(px(16.0))
                            .rounded(radius)
                            .refine_style(&content_style)
                            .children(self.children),
                    )
                    .when_some(self.footer, |this, footer| {
                        this.child(div().text_sm().text_color(title_color).child(footer))
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    // 显式导入，避免 `use super::*` 把 `crate::test` 宏带进来遮蔽内置 `#[test]`。
    use super::{GroupBox, GroupBoxVariant, GroupBoxVariants as _};

    /// 变体设置：`fill`/`outline`/`normal` 便捷方法正确写入变体字段。
    #[test]
    fn test_group_box_variant_builder() {
        assert_eq!(GroupBox::new().fill().variant, GroupBoxVariant::Fill);
        assert_eq!(GroupBox::new().outline().variant, GroupBoxVariant::Outline);
        assert_eq!(
            GroupBox::new().fill().normal().variant,
            GroupBoxVariant::Normal
        );
        assert_eq!(
            GroupBox::new()
                .with_variant(GroupBoxVariant::Outline)
                .variant,
            GroupBoxVariant::Outline
        );
    }

    /// 构建器：标题/页脚/ID/样式覆盖均写入对应字段。
    #[test]
    fn test_group_box_builder_sets_fields() {
        let group = GroupBox::new()
            .id("my-group")
            .title("标题")
            .footer("说明")
            .fill();

        assert!(group.id.is_some());
        assert!(group.title.is_some());
        assert!(group.footer.is_some());
        assert_eq!(group.variant, GroupBoxVariant::Fill);
    }
}
