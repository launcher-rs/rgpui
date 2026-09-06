//! 面包屑导航。
//!
//! 路径式层级导航：首页 / 分类 / 当前页。中间项可点击，末项为当前页（高亮）。

use crate::{prelude::*, *};
use std::sync::Arc;

/// 面包屑条目。
#[derive(Clone)]
pub struct BreadcrumbItem {
    /// 标签。
    pub label: SharedString,
    /// 点击回调（无则不可点）。
    pub on_click: Option<Arc<dyn Fn(&mut Window, &mut App) + Send + Sync + 'static>>,
}

impl BreadcrumbItem {
    /// 创建条目。
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            on_click: None,
        }
    }

    /// 设置点击回调（可点的中间项）。
    pub fn on_click<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_click = Some(Arc::new(f));
        self
    }
}

/// 面包屑导航。
#[derive(IntoElement)]
pub struct Breadcrumb {
    /// 条目列表（末项视为当前页）。
    items: Vec<BreadcrumbItem>,
    /// 用户样式。
    style: StyleRefinement,
}

impl Breadcrumb {
    /// 创建面包屑。
    pub fn new(items: Vec<BreadcrumbItem>) -> Self {
        Self {
            items,
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for Breadcrumb {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Breadcrumb {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let accent = theme.tokens.accent.color;
        let muted_foreground = theme.tokens.muted_foreground.color;
        let user_style = self.style;
        let last = self.items.len().saturating_sub(1);

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .children(self.items.into_iter().enumerate().map(|(ix, item)| {
                let is_current = ix == last;
                let label = div()
                    .id(ix)
                    .text_sm()
                    .text_color(if is_current { accent } else { muted_foreground })
                    .child(item.label.clone());
                let label = match item.on_click {
                    Some(cb) if !is_current => label
                        .cursor_pointer()
                        .on_click(move |_, window, cx| cb(window, cx))
                        .into_any_element(),
                    _ => label.into_any_element(),
                };
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(label)
                    .when(ix < last, |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(muted_foreground)
                                .child(IconName::ChevronRight),
                        )
                    })
                    .into_any_element()
            }))
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
    }
}
