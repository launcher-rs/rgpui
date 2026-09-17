//! 状态栏组件 —— 应用底部状态信息条（左右两组条目）。
//!
//! 纯值驱动：调用方每帧直接组装条目传入，无需持有实体状态，
//! 也不需要手工做内容比对（条目以稳定 id 参与元素 diff）。
//!
//! ```rust,ignore
//! use rgpui::components::status_bar::{StatusBar, StatusBarItem};
//!
//! StatusBar::new()
//!     .left(vec![StatusBarItem::new("main").id("branch")])
//!     .right(vec![StatusBarItem::new("Ln 42, Col 15").muted(true)])
//! ```

use std::sync::Arc;

use crate::*;

/// 状态栏单个条目（纯文本 / 图标文本 / 可点击按钮）。
#[derive(Clone)]
pub struct StatusBarItem {
    /// 条目稳定标识（默认取显示文本；文本会随状态变化时应显式指定）。
    pub id: SharedString,
    /// 显示文本。
    pub label: SharedString,
    /// 前置图标（无则不显示）。
    pub icon: Option<IconName>,
    /// 悬停提示（无则不显示）。
    pub tooltip: Option<SharedString>,
    /// 点击回调（无则为纯文本，不响应悬停）。
    pub on_click: Option<Arc<dyn Fn(&ClickEvent, &mut Window, &mut App) + Send + Sync>>,
    /// 是否使用次前景色（纯文本类条目）。
    pub muted: bool,
    /// 是否处于激活态（开关类条目高亮）。
    pub active: bool,
    /// 是否禁用（置灰且不可点击）。
    pub disabled: bool,
}

impl StatusBarItem {
    /// 创建状态栏条目（id 默认为显示文本）。
    pub fn new(label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            id: label.clone(),
            label,
            icon: None,
            tooltip: None,
            on_click: None,
            muted: false,
            active: false,
            disabled: false,
        }
    }

    /// 设置条目稳定标识（显示文本随状态变化时必须指定，否则元素状态会被复用错位）。
    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.id = id.into();
        self
    }

    /// 设置前置图标。
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// 设置悬停提示。
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// 设置点击回调（设置后条目呈现按钮样式，`disabled` 时不生效）。
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_click = Some(Arc::new(handler));
        self
    }

    /// 设置是否使用次前景色。
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    /// 设置是否为激活态（开关类条目高亮）。
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// 设置是否禁用（置灰且不可点击）。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// 状态栏组件（纯值驱动，直接收左右条目）。
#[derive(IntoElement)]
pub struct StatusBar {
    left_items: Vec<StatusBarItem>,
    right_items: Vec<StatusBarItem>,
    style: StyleRefinement,
}

impl StatusBar {
    /// 创建新的状态栏。
    pub fn new() -> Self {
        Self {
            left_items: Vec::new(),
            right_items: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// 设置左侧条目。
    pub fn left(mut self, items: Vec<StatusBarItem>) -> Self {
        self.left_items = items;
        self
    }

    /// 设置右侧条目。
    pub fn right(mut self, items: Vec<StatusBarItem>) -> Self {
        self.right_items = items;
        self
    }
}

impl Styled for StatusBar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for StatusBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;

        h_flex()
            .w_full()
            .h(px(28.))
            .items_center()
            .justify_between()
            .px_2()
            .bg(theme.status_bar)
            .border_t_1()
            .border_color(theme.status_bar_border)
            .child(
                h_flex().items_center().gap_3().children(
                    self.left_items
                        .iter()
                        .map(|item| render_status_bar_item("left", item, cx)),
                ),
            )
            .child(
                h_flex().items_center().gap_3().children(
                    self.right_items
                        .iter()
                        .map(|item| render_status_bar_item("right", item, cx)),
                ),
            )
            .refine_style(&user_style)
    }
}

/// 渲染单个状态栏条目（可点击条目呈按钮样式，否则为纯文本）。
fn render_status_bar_item(side: &'static str, item: &StatusBarItem, cx: &mut App) -> AnyElement {
    let theme = cx.theme();
    let clickable = item.on_click.is_some() && !item.disabled;
    let mut element = div()
        .id(format!("status-bar-{side}-{}", item.id))
        .h_full()
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .rounded(theme.radius * 0.5)
        .text_xs()
        .text_color(if item.muted || item.disabled {
            theme.muted_foreground
        } else {
            theme.foreground
        });

    if item.disabled {
        element = element.opacity(0.5);
    } else if item.active {
        element = element.bg(theme.accent.opacity(0.15));
    }

    if let Some(icon) = item.icon.clone() {
        element = element.child(Icon::new(icon).small());
    }
    element = element.child(item.label.clone());

    if clickable {
        let on_click = item.on_click.clone().expect("clickable implies handler");
        element = element
            .cursor_pointer()
            .hover(|this| this.bg(theme.secondary_hover))
            .on_click(move |event, window, cx| on_click(event, window, cx));
    }

    if let Some(tooltip) = item.tooltip.clone() {
        element =
            element.tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx));
    }

    element.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{StatusBar, StatusBarItem};
    use crate::{Context, IconName, Render, SharedString, Window};

    /// 测试宿主视图。
    struct Probe {
        left: Vec<StatusBarItem>,
        right: Vec<StatusBarItem>,
    }

    impl Render for Probe {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> impl crate::IntoElement {
            StatusBar::new()
                .left(self.left.clone())
                .right(self.right.clone())
        }
    }

    /// 空状态栏左右均为空。
    #[test]
    fn status_bar_defaults_to_empty() {
        let bar = StatusBar::new();
        assert!(bar.left_items.is_empty());
        assert!(bar.right_items.is_empty());
    }

    /// 条目默认 id 取显示文本，显式 id 覆盖它。
    #[test]
    fn item_id_defaults_to_label() {
        assert_eq!(StatusBarItem::new("42").id, SharedString::from("42"));
        assert_eq!(
            StatusBarItem::new("42").id("cursor").id,
            SharedString::from("cursor")
        );
    }

    /// 各形态条目可绘制（冒烟测试）。
    #[rgpui::test]
    fn renders_items_without_panic(cx: &mut crate::TestAppContext) {
        let left = vec![
            StatusBarItem::new("main")
                .id("branch")
                .icon(IconName::Check)
                .on_click(|_, _, _| {}),
            StatusBarItem::new("rendered")
                .id("mode")
                .active(true)
                .on_click(|_, _, _| {}),
        ];
        let right = vec![
            StatusBarItem::new("Ln 1, Col 1").id("cursor"),
            StatusBarItem::new("3 words")
                .id("words")
                .muted(true)
                .tooltip("word count"),
            StatusBarItem::new("off").id("off").disabled(true),
        ];
        let (_view, cx) = cx.add_window_view(|_, _| Probe { left, right });
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    }
}
