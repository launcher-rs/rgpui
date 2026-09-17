//! 无状态页签（值驱动 Tabs）——两三个静态页签零同步代码接入。
//!
//! 与 [`crate::components::status_bar::StatusBar`] 同模式：调用方每帧直接组装
//! `items` + `active` 传入并经 `on_change` 回写，无需持有实体状态。
//! 需要拖拽/滑动指示器等重交互时改用实体状态式的 `TabBar` / `SegmentedNav`。
//!
//! ```rust,ignore
//! use rgpui::tabs::{Tabs, TabsItem};
//!
//! Tabs::new(vec![TabsItem::new("home", "首页"), TabsItem::new("settings", "设置")])
//!     .active(active_id.clone())
//!     .on_change(cx.listener_value(|this, id: SharedString, _, cx| {
//!         this.active_id = Some(id);
//!         cx.notify();
//!     }))
//! ```

use std::sync::Arc;

use crate::prelude::FluentBuilder as _;
use crate::*;

/// 无状态页签的单个条目。
#[derive(Clone)]
pub struct TabsItem {
    /// 条目稳定标识（选中比对与元素 diff 用）。
    pub id: SharedString,
    /// 显示文本。
    pub label: SharedString,
    /// 前置图标（无则不显示）。
    pub icon: Option<IconName>,
    /// 是否禁用（置灰且不可点击）。
    pub disabled: bool,
}

impl TabsItem {
    /// 创建页签条目（id 与显示文本相同）。
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            disabled: false,
        }
    }

    /// 设置前置图标。
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// 设置是否禁用。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// 无状态页签（纯值驱动，直接收条目 + 选中 id + 变更回调）。
#[derive(IntoElement)]
pub struct Tabs {
    items: Vec<TabsItem>,
    active: Option<SharedString>,
    on_change: Option<Arc<dyn Fn(SharedString, &mut Window, &mut App) + Send + Sync + 'static>>,
    style: StyleRefinement,
}

impl Tabs {
    /// 创建无状态页签。
    pub fn new(items: Vec<TabsItem>) -> Self {
        Self {
            items,
            active: None,
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置选中的条目 id（无则都不选中）。
    pub fn active(mut self, id: Option<SharedString>) -> Self {
        self.active = id;
        self
    }

    /// 设置值变更回调（被选中的条目 id，按值传递）。
    pub fn on_change<F>(mut self, f: F) -> Self
    where
        F: Fn(SharedString, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_change = Some(Arc::new(f));
        self
    }
}

impl Styled for Tabs {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Tabs {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;
        let active = self.active;
        let on_change = self.on_change;

        h_flex()
            .items_center()
            .gap(px(2.0))
            .p(px(2.0))
            .rounded(theme.radius)
            .bg(theme.tab_bar_segmented)
            .children(self.items.into_iter().map(|item| {
                let is_active = active.as_ref() == Some(&item.id);
                let id = item.id.clone();
                let on_change = on_change.clone();
                div()
                    .id(format!("tabs-{}", item.id))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(12.0))
                    .py(px(6.0))
                    .rounded(theme.radius * 0.75)
                    .text_sm()
                    .text_color(if is_active {
                        theme.tab_active_foreground
                    } else {
                        theme.tab_foreground
                    })
                    .when(is_active, |this| this.bg(theme.tab_active))
                    .when(!is_active && !item.disabled, |this| {
                        this.hover(|this| this.bg(theme.tab_active.opacity(0.4)))
                    })
                    .when(item.disabled, |this| this.opacity(0.5))
                    .when_some(item.icon, |this, icon| this.child(Icon::new(icon).xsmall()))
                    .child(item.label)
                    .when(!item.disabled, |this| {
                        this.cursor_pointer().on_click(move |_, window, cx| {
                            if let Some(ref cb) = on_change {
                                cb(id.clone(), window, cx);
                            }
                        })
                    })
            }))
            .refine_style(&user_style)
    }
}

#[cfg(test)]
mod tests {
    use super::{Tabs, TabsItem};
    use crate::{Context, IntoElement, Render, SharedString, Window};

    /// 测试宿主视图（值驱动：选中 id 存视图状态）。
    struct Probe {
        active: Option<SharedString>,
    }

    impl Render for Probe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            Tabs::new(vec![
                TabsItem::new("home", "首页"),
                TabsItem::new("settings", "设置").disabled(true),
            ])
            .active(self.active.clone())
        }
    }

    /// 默认无选中，条目 id 透传。
    #[test]
    fn tabs_defaults_to_no_active() {
        let tabs = Tabs::new(vec![TabsItem::new("home", "首页")]);
        assert!(tabs.active.is_none());
        assert!(tabs.on_change.is_none());
        assert_eq!(tabs.items.len(), 1);
        assert_eq!(tabs.items[0].id, SharedString::from("home"));
    }

    /// 静态页签零同步代码可绘制（冒烟测试）。
    #[rgpui::test]
    fn renders_static_tabs_without_panic(cx: &mut crate::TestAppContext) {
        let (_view, cx) = cx.add_window_view(|_, _| Probe {
            active: Some("home".into()),
        });
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    }
}
