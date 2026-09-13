//! 侧边栏导航组件。
//!
//! 应用级纵向导航：固定宽度、图标 + 标签条目、选中高亮、可折叠为图标栏。
//! 会话列表类场景（如 AntDX Conversations）可直接用本组件承载。

use crate::{prelude::FluentBuilder as _, *};
use std::sync::Arc;

/// 侧边栏条目。
#[derive(Clone)]
pub struct SidebarItem {
    /// 条目 ID。
    pub id: SharedString,
    /// 条目标签（折叠时隐藏）。
    pub label: SharedString,
    /// 条目图标。
    pub icon: Option<Icon>,
    /// 角标（如未读数，折叠时隐藏）。
    pub badge: Option<SharedString>,
}

impl SidebarItem {
    /// 创建条目。
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            badge: None,
        }
    }

    /// 设置条目图标。
    pub fn with_icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 设置角标（如未读数）。
    pub fn with_badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }
}

/// 侧边栏分组（一组条目 + 分组头，IDE 侧边栏标配）。
#[derive(Clone)]
pub struct SidebarSection {
    /// 分组标题。
    pub title: SharedString,
    /// 分组内条目。
    pub items: Vec<SidebarItem>,
    /// 是否折叠（状态由父持有，经 `on_toggle_section` 回写）。
    pub collapsed: bool,
}

impl SidebarSection {
    /// 创建分组。
    pub fn new(title: impl Into<SharedString>, items: Vec<SidebarItem>) -> Self {
        Self {
            title: title.into(),
            items,
            collapsed: false,
        }
    }

    /// 设置折叠状态。
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

/// 侧边栏导航组件。
#[derive(IntoElement)]
pub struct Sidebar {
    /// 条目列表。
    items: Vec<SidebarItem>,
    /// 分组列表（渲染在 `items` 之后）。
    sections: Vec<SidebarSection>,
    /// 选中条目 ID。
    selected_id: Option<SharedString>,
    /// 是否允许折叠。
    collapsible: bool,
    /// 是否已折叠（仅图标栏）。
    collapsed: bool,
    /// 选中回调。
    on_select: Option<Arc<dyn Fn(&SharedString, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 折叠切换回调（参数为折叠后的状态，引用传参与 `cx.listener` 兼容）。
    on_toggle_collapsed: Option<Arc<dyn Fn(&bool, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 分组折叠切换回调（分组标题，折叠后的状态）。
    on_toggle_section:
        Option<Arc<dyn Fn(&SharedString, &bool, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl Sidebar {
    /// 创建空侧边栏。
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            sections: Vec::new(),
            selected_id: None,
            collapsible: false,
            collapsed: false,
            on_select: None,
            on_toggle_collapsed: None,
            on_toggle_section: None,
            style: StyleRefinement::default(),
        }
    }

    /// 添加一个条目。
    pub fn item(mut self, item: SidebarItem) -> Self {
        self.items.push(item);
        self
    }

    /// 批量设置条目。
    pub fn items(mut self, items: Vec<SidebarItem>) -> Self {
        self.items = items;
        self
    }

    /// 添加一个分组。
    pub fn section(mut self, section: SidebarSection) -> Self {
        self.sections.push(section);
        self
    }

    /// 批量设置分组。
    pub fn sections(mut self, sections: Vec<SidebarSection>) -> Self {
        self.sections = sections;
        self
    }

    /// 设置选中条目 ID。
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected_id = Some(id.into());
        self
    }

    /// 设置是否允许折叠为图标栏。
    pub fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    /// 设置折叠状态（折叠状态由父持有，切换经 `on_toggle_collapsed` 回写）。
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    /// 设置选中回调。
    pub fn on_select<F>(mut self, f: F) -> Self
    where
        F: Fn(&SharedString, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_select = Some(Arc::new(f));
        self
    }

    /// 设置折叠切换回调。
    pub fn on_toggle_collapsed<F>(mut self, f: F) -> Self
    where
        F: Fn(&bool, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_toggle_collapsed = Some(Arc::new(f));
        self
    }

    /// 设置分组折叠切换回调（分组标题，折叠后的状态）。
    pub fn on_toggle_section<F>(mut self, f: F) -> Self
    where
        F: Fn(&SharedString, &bool, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_toggle_section = Some(Arc::new(f));
        self
    }
}

impl Default for Sidebar {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for Sidebar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Sidebar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let border = theme.tokens.border;
        let muted_foreground = theme.tokens.muted_foreground;
        let popover = theme.tokens.popover;
        let accent = theme.tokens.accent.color;

        let collapsed = self.collapsible && self.collapsed;
        let selected_id = self.selected_id;
        let on_select = self.on_select;
        let user_style = self.style;

        let mut root = div()
            .flex()
            .flex_col()
            .h_full()
            .w(if collapsed { px(56.0) } else { px(220.0) })
            .bg(popover)
            .border_r(px(1.0))
            .border_color(border)
            .py(px(8.0))
            .gap(px(2.0));

        // 条目行渲染闭包（未分组条目与分组内条目共用）。
        let item_row = |item: &SidebarItem,
                        selected_id: &Option<SharedString>,
                        on_select: &Option<
            Arc<dyn Fn(&SharedString, &mut Window, &mut App) + Send + Sync + 'static>,
        >,
                        collapsed: bool| {
            let is_selected = selected_id.as_ref() == Some(&item.id);
            let id = item.id.clone();
            let label = item.label.clone();
            let icon = item.icon.clone();
            let badge = item.badge.clone();
            let on_select = on_select.clone();
            div()
                .id(id.clone())
                .flex()
                .items_center()
                .gap(px(8.0))
                .px(px(12.0))
                .py(px(8.0))
                .rounded_md()
                .cursor_pointer()
                .when(is_selected, |this| this.bg(accent.opacity(0.15)))
                .when(!is_selected, |this| {
                    this.hover(|this| this.bg(accent.opacity(0.08)))
                })
                .when_some(icon, |this, icon| this.child(icon))
                .when(!collapsed, |this| {
                    this.child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(if is_selected {
                                accent
                            } else {
                                muted_foreground.color
                            })
                            .child(label),
                    )
                })
                .when_some(badge, |this, badge| {
                    this.child(
                        div()
                            .text_xs()
                            .px(px(6.0))
                            .rounded_full()
                            .bg(accent.opacity(0.15))
                            .text_color(accent)
                            .child(badge),
                    )
                })
                .on_click(move |_, window, cx| {
                    if let Some(ref cb) = on_select {
                        cb(&id, window, cx);
                    }
                })
        };

        for item in &self.items {
            root = root.child(item_row(item, &selected_id, &on_select, collapsed));
        }

        for (section_ix, section) in self.sections.iter().enumerate() {
            let title = section.title.clone();
            let section_collapsed = section.collapsed;
            let on_toggle_section = self.on_toggle_section.clone();
            root = root.child(
                div()
                    .id(ElementId::named_usize("sidebar-section", section_ix))
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .px(px(12.0))
                    .py(px(6.0))
                    .cursor_pointer()
                    .child(
                        Icon::new(if section_collapsed {
                            IconName::ChevronRight
                        } else {
                            IconName::ChevronDown
                        })
                        .xsmall()
                        .text_color(cx.theme().muted_foreground),
                    )
                    .when(!collapsed, |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(muted_foreground.color)
                                .child(title.clone()),
                        )
                    })
                    .on_click(move |_, window, cx| {
                        if let Some(ref cb) = on_toggle_section {
                            cb(&title, &!section_collapsed, window, cx);
                        }
                    }),
            );
            if !section_collapsed {
                for item in &section.items {
                    root = root.child(item_row(item, &selected_id, &on_select, collapsed));
                }
            }
        }

        if self.collapsible {
            let on_toggle_collapsed = self.on_toggle_collapsed;
            root = root.child(
                div().flex_1().flex().flex_col().justify_end().child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .p(px(8.0))
                        .child(
                            Button::new("sidebar-toggle")
                                .ghost()
                                .small()
                                .icon(if collapsed {
                                    IconName::ChevronRight
                                } else {
                                    IconName::ChevronLeft
                                })
                                .on_click(move |_, window, cx| {
                                    if let Some(ref cb) = on_toggle_collapsed {
                                        let next = !collapsed;
                                        cb(&next, window, cx);
                                    }
                                }),
                        ),
                ),
            );
        }

        root.map(|mut this| {
            this.style().refine(&user_style);
            this
        })
    }
}
